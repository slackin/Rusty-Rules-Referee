//! Per-server installed-map scanner.
//!
//! Mirrors [`crate::maprepo`] but queries each connected game server for its
//! installed `.bsp` list via RCON `fdir *.bsp` (through the existing
//! `ClientRequest::ListMaps` sync plumbing in master mode, or a direct
//! handler call in standalone mode). Results are cached per-server in
//! `server_maps`; run outcomes land in `server_map_scans`.
//!
//! Triggers:
//! - Periodic scheduler (see [`spawn_scheduler`]) every N hours.
//! - On server registration / reconnect (see [`scan_on_connect`]).
//! - Manual refresh endpoint from the UI.
//! - Post-import best-effort refresh.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use tokio::sync::{oneshot, RwLock};
use tracing::{debug, info, warn};

use crate::core::context::BotContext;
use crate::core::ServerMap;
use crate::storage::Storage;
use crate::sync::protocol::{ClientRequest, ClientResponse};

/// Minimum interval between scans for the same server. Prevents reconnect
/// storms from firing a scan per heartbeat.
const MIN_SCAN_INTERVAL_SECS: i64 = 60;

/// Timeout for the `ListMaps` RCON round-trip (master mode) or direct
/// handler call (standalone).
const SCAN_TIMEOUT_SECS: u64 = 20;

/// Master-mode sync plumbing handles, required for enqueuing `ListMaps` to
/// a remote client and awaiting the correlated response.
pub type PendingResponses =
    Arc<RwLock<HashMap<String, oneshot::Sender<ClientResponse>>>>;
pub type PendingClientRequests =
    Arc<RwLock<HashMap<i64, Vec<(String, ClientRequest)>>>>;

/// Entry point for master mode: scan a single server by asking the connected
/// client for its installed maps over the sync channel, then persist.
pub async fn scan_remote_server(
    storage: Arc<dyn Storage>,
    pending_responses: PendingResponses,
    pending_client_requests: PendingClientRequests,
    server_id: i64,
) -> Result<u64, String> {
    let started = Utc::now();
    let result = crate::sync::master::send_request_to_server(
        &pending_responses,
        &pending_client_requests,
        server_id,
        ClientRequest::ListMaps,
        Duration::from_secs(SCAN_TIMEOUT_SECS),
    )
    .await;

    let maps_result = match result {
        Ok(ClientResponse::MapList { maps }) => Ok(maps),
        Ok(ClientResponse::Error { message }) => Err(message),
        Ok(other) => Err(format!("Unexpected response: {:?}", other)),
        Err(e) => Err(e),
    };

    finalize_scan(storage, server_id, maps_result, started).await
}

/// Entry point for standalone mode: scan the local bot's game server
/// directly via its `BotContext`.
pub async fn scan_local_server(
    storage: Arc<dyn Storage>,
    ctx: &BotContext,
    server_id: i64,
) -> Result<u64, String> {
    let started = Utc::now();
    let resp = crate::sync::handlers::handle_list_maps(Some(ctx)).await;
    let maps_result = match resp {
        ClientResponse::MapList { maps } => Ok(maps),
        ClientResponse::Error { message } => Err(message),
        other => Err(format!("Unexpected response: {:?}", other)),
    };
    finalize_scan(storage, server_id, maps_result, started).await
}

async fn finalize_scan(
    storage: Arc<dyn Storage>,
    server_id: i64,
    maps_result: Result<Vec<String>, String>,
    started: DateTime<Utc>,
) -> Result<u64, String> {
    match maps_result {
        Ok(names) => {
            let scan_maps: Vec<ServerMap> = names
                .into_iter()
                .map(|n| ServerMap {
                    map_name: n,
                    pk3_filename: None,
                    first_seen_at: started,
                    last_seen_at: started,
                    pending_restart: false,
                })
                .collect();
            let count = match storage
                .replace_server_maps(server_id, &scan_maps, started)
                .await
            {
                Ok(c) => c,
                Err(e) => {
                    let msg = format!("storage: {}", e);
                    let _ = storage
                        .record_server_map_scan(
                            server_id,
                            false,
                            Some(&msg),
                            0,
                            started,
                        )
                        .await;
                    return Err(msg);
                }
            };
            let _ = storage
                .record_server_map_scan(
                    server_id,
                    true,
                    None,
                    count as i64,
                    started,
                )
                .await;
            info!(server_id, count, "map-scan: refreshed installed maps");
            Ok(count)
        }
        Err(msg) => {
            warn!(server_id, error = %msg, "map-scan: refresh failed");
            let _ = storage
                .record_server_map_scan(
                    server_id,
                    false,
                    Some(&msg),
                    0,
                    started,
                )
                .await;
            Err(msg)
        }
    }
}

/// Should we skip a scan for this server because we just ran one?
pub async fn should_skip(storage: &Arc<dyn Storage>, server_id: i64) -> bool {
    match storage.get_server_map_scan(server_id).await {
        Ok(Some(status)) => match status.last_scan_at {
            Some(at) if (Utc::now() - at).num_seconds() < MIN_SCAN_INTERVAL_SECS => true,
            _ => false,
        },
        _ => false,
    }
}

/// Fire a best-effort scan on server connect/reconnect (master mode).
/// Spawns a detached task so the registration handler isn't blocked. Skips
/// if a recent scan succeeded.
pub fn scan_on_connect(
    storage: Arc<dyn Storage>,
    pending_responses: PendingResponses,
    pending_client_requests: PendingClientRequests,
    server_id: i64,
) {
    tokio::spawn(async move {
        // Give the client a moment to finish registering / attach ctx so
        // the RCON path is available.
        tokio::time::sleep(Duration::from_secs(5)).await;
        if should_skip(&storage, server_id).await {
            debug!(server_id, "map-scan: skipping on-connect scan (recent scan present)");
            return;
        }
        if let Err(e) = scan_remote_server(
            storage,
            pending_responses,
            pending_client_requests,
            server_id,
        )
        .await
        {
            debug!(server_id, error = %e, "map-scan: on-connect scan failed (will retry on schedule)");
        }
    });
}

/// Spawn the background scheduler for master mode. Iterates all online
/// servers every `interval_hours` and runs a scan for each. `0` hours
/// disables scheduled scans entirely (on-connect and manual refresh still
/// work).
pub fn spawn_master_scheduler(
    storage: Arc<dyn Storage>,
    pending_responses: PendingResponses,
    pending_client_requests: PendingClientRequests,
    interval_hours: u32,
) {
    if interval_hours == 0 {
        info!("map-scan: periodic refresher disabled (scan_interval_hours=0)");
        return;
    }
    tokio::spawn(async move {
        // Small initial delay so boot-time work isn't blocked.
        tokio::time::sleep(Duration::from_secs(60)).await;
        loop {
            let servers = match storage.get_servers().await {
                Ok(s) => s,
                Err(e) => {
                    warn!(error = %e, "map-scan: failed to list servers");
                    Vec::new()
                }
            };
            for server in servers {
                if server.status != "online" {
                    continue;
                }
                if should_skip(&storage, server.id).await {
                    continue;
                }
                let _ = scan_remote_server(
                    storage.clone(),
                    pending_responses.clone(),
                    pending_client_requests.clone(),
                    server.id,
                )
                .await;
            }
            tokio::time::sleep(Duration::from_secs(
                interval_hours as u64 * 3600,
            ))
            .await;
        }
    });
}

/// Spawn the background scheduler for standalone mode. Every `interval_hours`,
/// scan the local server (server_id = 0 by convention for standalone — we
/// use the single row the master would otherwise reference).
pub fn spawn_standalone_scheduler(
    storage: Arc<dyn Storage>,
    ctx: Arc<BotContext>,
    server_id: i64,
    interval_hours: u32,
) {
    if interval_hours == 0 {
        info!("map-scan: periodic refresher disabled (scan_interval_hours=0)");
        return;
    }
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(30)).await;
        loop {
            let _ = scan_local_server(storage.clone(), &ctx, server_id).await;
            tokio::time::sleep(Duration::from_secs(
                interval_hours as u64 * 3600,
            ))
            .await;
        }
    });
}
