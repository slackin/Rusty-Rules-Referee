//! Master server internal API for game server bot clients.
//!
//! This runs on a separate mTLS-protected port (default 9443) and provides:
//! - Registration for new game server bots
//! - Event batch ingestion
//! - Penalty sync (bidirectional)
//! - Config distribution
//! - Heartbeat / health monitoring
//! - WebSocket for real-time bidirectional communication

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{State, WebSocketUpgrade, ws},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post, put},
};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, oneshot, RwLock};
use tracing::{debug, error, info, warn};

use crate::config::MasterSection;
use crate::core::GameServer;
use crate::storage::Storage;
use crate::sync::ca;
use crate::sync::protocol::*;
use crate::sync::tls;

/// Shared state for the master internal API.
#[derive(Clone)]
pub struct MasterState {
    pub storage: Arc<dyn Storage>,
    /// Connected client bots by server_id, with their WS sender.
    pub connected_clients: Arc<RwLock<HashMap<i64, ConnectedClient>>>,
    /// Broadcast channel for forwarding events to the web UI.
    pub event_tx: broadcast::Sender<EventPayload>,
    /// Pending request/response correlations: request_id → oneshot sender.
    pub pending_responses: Arc<RwLock<HashMap<String, oneshot::Sender<ClientResponse>>>>,
    /// Pending requests queued for client bots to pick up via polling.
    /// Key: server_id → Vec of (request_id, request).
    pub pending_client_requests: Arc<RwLock<HashMap<i64, Vec<(String, ClientRequest)>>>>,
    /// Last-known version info reported by each client via heartbeat.
    /// Key: server_id → (build_hash, version, last_reported_at)
    pub client_versions:
        Arc<RwLock<HashMap<i64, ClientVersionInfo>>>,
    /// Connected hubs by hub_id.
    pub connected_hubs: Arc<RwLock<HashMap<i64, ConnectedHub>>>,
    /// Pending hub actions queued by the master, polled by hubs.
    /// Key: hub_id → Vec of (action_id, HubAction).
    pub pending_hub_actions: Arc<RwLock<HashMap<i64, Vec<(String, HubAction)>>>>,
    /// Pending hub action responses awaiting the hub's reply.
    pub pending_hub_responses: Arc<RwLock<HashMap<String, oneshot::Sender<HubResponse>>>>,
    /// Last-known hub version info, refreshed on every hub heartbeat.
    pub hub_versions: Arc<RwLock<HashMap<i64, ClientVersionInfo>>>,
    /// In-memory log of recent hub actions (progress events + final
    /// response). Keyed by `action_id`. Entries older than ~10 min are
    /// GC'd by a background task. Used to power the UI install-progress
    /// view via `GET /api/v1/hubs/:id/actions/:action_id`.
    pub hub_action_logs: Arc<RwLock<HashMap<String, HubActionLog>>>,
    /// Path to the master's `r3.toml` (needed for cert minting on behalf of hubs).
    pub config_path: String,
    /// Master config snapshot (cloned at startup; used for sync URL/CA paths).
    pub master_config: MasterSection,
    /// Public IP of the master (used to build the master sync URL handed back to hubs).
    pub public_ip: String,
}

/// Client-reported build/version info, refreshed on every heartbeat.
#[derive(Debug, Clone)]
pub struct ClientVersionInfo {
    pub build_hash: Option<String>,
    pub version: Option<String>,
    pub reported_at: chrono::DateTime<chrono::Utc>,
}

/// In-memory record of an enqueued hub action: the progress events
/// pushed by the hub during execution and the final response (if the
/// hub has completed the action). Created when the action is enqueued.
#[derive(Debug, Clone)]
pub struct HubActionLog {
    pub hub_id: i64,
    pub action_kind: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub events: Vec<HubProgressEvent>,
    pub result: Option<HubResponse>,
}

/// Represents a connected client bot.
pub struct ConnectedClient {
    pub server_id: i64,
    pub server_name: String,
    pub tx: tokio::sync::mpsc::Sender<SyncMessage>,
}

/// Represents a connected hub orchestrator.
pub struct ConnectedHub {
    pub hub_id: i64,
    pub hub_name: String,
    pub last_seen: chrono::DateTime<chrono::Utc>,
}

/// Build the internal API router.
pub fn build_internal_router(state: MasterState) -> Router {
    Router::new()
        .route("/internal/register", post(handle_register))
        .route("/internal/heartbeat", post(handle_heartbeat))
        .route("/internal/events", post(handle_events))
        .route("/internal/penalties", post(handle_penalty_sync))
        .route("/internal/players", post(handle_player_sync))
        .route("/internal/config/:server_id", get(handle_get_config))
        .route("/internal/config/:server_id", put(handle_put_config))
        .route("/internal/bans", get(handle_get_global_bans))
        .route("/internal/ws", get(handle_ws))
        .route("/internal/requests/:server_id", get(handle_poll_requests))
        .route("/internal/responses", post(handle_client_response))
        // Hub orchestration
        .route("/internal/hub/register", post(handle_hub_register))
        .route("/internal/hub/heartbeat", post(handle_hub_heartbeat))
        .route("/internal/hub/actions/:hub_id", get(handle_poll_hub_actions))
        .route("/internal/hub/responses", post(handle_hub_response))
        .route("/internal/hub/progress", post(handle_hub_progress))
        .route("/internal/hub/mint-client-cert", post(handle_mint_client_cert))
        .with_state(state)
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn handle_register(
    State(state): State<MasterState>,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<RegisterResponse>, StatusCode> {
    info!(
        name = %req.server_name,
        address = %req.address,
        port = req.port,
        "Client bot registering"
    );

    // Check if already registered by cert fingerprint
    let existing = state
        .storage
        .get_server_by_fingerprint(&req.cert_fingerprint)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to look up server by fingerprint");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let server_id = if let Some(existing) = existing {
        // Update existing registration.
        // NOTE: client-mode bots register with empty address / port 0 because the
        // game server config is managed by the master and pushed down. Only
        // overwrite address/port when the client reports real values, otherwise
        // a reconnect would wipe a config saved via the master UI.
        let mut server = existing;
        server.name = req.server_name;
        if !req.address.is_empty() && req.address != "0.0.0.0" {
            server.address = req.address;
        }
        if req.port != 0 {
            server.port = req.port;
        }
        server.status = "online".to_string();
        server.last_seen = Some(chrono::Utc::now());
        state.storage.save_server(&server).await.map_err(|e| {
            error!(error = %e, "Failed to update server");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        server.id
    } else {
        // New registration
        let server = GameServer {
            id: 0,
            name: req.server_name,
            address: req.address,
            port: req.port,
            status: "online".to_string(),
            current_map: None,
            player_count: 0,
            max_clients: 0,
            last_seen: Some(chrono::Utc::now()),
            config_json: None,
            config_version: 0,
            cert_fingerprint: Some(req.cert_fingerprint),
            update_channel: "beta".to_string(),
            update_interval: 3600,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            hub_id: None,
            slug: None,
        };
        state.storage.save_server(&server).await.map_err(|e| {
            error!(error = %e, "Failed to save new server");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
    };

    let config_version = state
        .storage
        .get_server(server_id)
        .await
        .map(|s| s.config_version)
        .unwrap_or(0);

    info!(server_id, "Client bot registered");

    // Kick off an installed-map scan for this server in the background.
    // Rate-limited inside `mapscan::scan_on_connect` so reconnect storms
    // don't hammer RCON.
    crate::mapscan::scan_on_connect(
        state.storage.clone(),
        state.pending_responses.clone(),
        state.pending_client_requests.clone(),
        server_id,
    );

    Ok(Json(RegisterResponse {
        server_id,
        config_version,
    }))
}

async fn handle_heartbeat(
    State(state): State<MasterState>,
    Json(req): Json<HeartbeatRequest>,
) -> Result<Json<HeartbeatResponse>, StatusCode> {
    state
        .storage
        .update_server_status(
            req.server_id,
            &req.status,
            req.current_map.as_deref(),
            req.player_count,
            req.max_clients,
        )
        .await
        .map_err(|e| {
            error!(error = %e, server_id = req.server_id, "Failed to update server status");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Record client-reported version info if present in this heartbeat.
    if req.build_hash.is_some() || req.version.is_some() {
        state.client_versions.write().await.insert(
            req.server_id,
            ClientVersionInfo {
                build_hash: req.build_hash.clone(),
                version: req.version.clone(),
                reported_at: chrono::Utc::now(),
            },
        );
    }

    let server_row = state.storage.get_server(req.server_id).await.ok();
    let config_version = server_row.as_ref().map(|s| s.config_version).unwrap_or(0);
    let update_channel = server_row.as_ref().map(|s| s.update_channel.clone());
    let update_interval = server_row.as_ref().map(|s| s.update_interval);

    Ok(Json(HeartbeatResponse {
        ok: true,
        config_version,
        pending_global_bans: Vec::new(), // TODO: track pending bans since last heartbeat
        update_channel,
        update_interval,
    }))
}

async fn handle_events(
    State(state): State<MasterState>,
    Json(batch): Json<EventBatch>,
) -> Result<StatusCode, StatusCode> {
    info!(
        server_id = batch.server_id,
        count = batch.events.len(),
        "Received event batch"
    );

    // Forward events to the web UI broadcast channel
    for event in &batch.events {
        let _ = state.event_tx.send(event.clone());
    }

    // Persist chat-bearing events as ChatMessage rows so master has a
    // per-server chat history. The `event_type` string is the numeric EventId
    // of the client — we can't reliably decode it here without a shared
    // registry, so we detect chat by the payload shape: `data` is a JSON
    // object like `{"Text":"..."}` that originated from EventData::Text.
    for event in &batch.events {
        if let Some(text) = chat_text_from_event(&event.data) {
            if let Some(cid) = event.client_id {
                let msg = crate::core::ChatMessage {
                    id: 0,
                    client_id: cid,
                    client_name: chat_client_name_from_event(&event.data).unwrap_or_default(),
                    channel: "all".to_string(),
                    message: text,
                    time_add: event.timestamp,
                    server_id: Some(batch.server_id),
                };
                if let Err(e) = state.storage.save_chat_message(&msg).await {
                    debug!(error = %e, "Failed to persist chat event (non-fatal)");
                }
            }
        }
    }

    Ok(StatusCode::OK)
}

/// Extract a chat-message text from a serialized EventData, if possible.
/// EventData::Text(String) serializes as `{"Text":"..."}`; any event whose
/// payload has a top-level string field named `message` or `text` is also
/// treated as a chat-bearing event.
fn chat_text_from_event(data: &serde_json::Value) -> Option<String> {
    if let Some(s) = data.get("Text").and_then(|v| v.as_str()) {
        return Some(s.to_string());
    }
    if let Some(s) = data.get("message").and_then(|v| v.as_str()) {
        return Some(s.to_string());
    }
    if let Some(s) = data.get("text").and_then(|v| v.as_str()) {
        return Some(s.to_string());
    }
    None
}

fn chat_client_name_from_event(data: &serde_json::Value) -> Option<String> {
    data.get("name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

async fn handle_penalty_sync(
    State(state): State<MasterState>,
    Json(penalty): Json<PenaltySync>,
) -> Result<StatusCode, StatusCode> {
    info!(
        server_id = penalty.server_id,
        penalty_type = %penalty.penalty_type,
        client = %penalty.client_name,
        scope = ?penalty.scope,
        "Received penalty sync"
    );

    // If global, broadcast to all OTHER connected clients
    if penalty.scope == PenaltyScope::Global {
        let clients = state.connected_clients.read().await;
        for (sid, client) in clients.iter() {
            if *sid != penalty.server_id {
                let msg = SyncMessage::GlobalPenalty(penalty.clone());
                if let Err(e) = client.tx.send(msg).await {
                    warn!(
                        server_id = sid,
                        error = %e,
                        "Failed to forward global penalty to client"
                    );
                }
            }
        }
    }

    // Persist to master DB: upsert the client by GUID, then save the penalty
    // with server_id so the UI can scope it per-server.
    if let Err(e) = persist_penalty(&state, &penalty).await {
        warn!(error = %e, "Failed to persist penalty on master");
    }

    Ok(StatusCode::OK)
}

/// Look up (or create a shell record for) the client identified by guid,
/// then insert a penalty row attributed to that client with server_id set.
async fn persist_penalty(
    state: &MasterState,
    penalty: &PenaltySync,
) -> anyhow::Result<()> {
    let storage = &state.storage;
    let client_id = ensure_client(storage, &penalty.client_guid, &penalty.client_name).await?;

    let ptype = match penalty.penalty_type.as_str() {
        "Warning" => crate::core::PenaltyType::Warning,
        "Notice" => crate::core::PenaltyType::Notice,
        "Kick" => crate::core::PenaltyType::Kick,
        "Ban" => crate::core::PenaltyType::Ban,
        "TempBan" => crate::core::PenaltyType::TempBan,
        "Mute" => crate::core::PenaltyType::Mute,
        _ => crate::core::PenaltyType::Warning,
    };
    let time_expire = penalty
        .duration
        .map(|d| penalty.timestamp + chrono::Duration::seconds(d));

    let row = crate::core::Penalty {
        id: 0,
        penalty_type: ptype,
        client_id,
        admin_id: None,
        duration: penalty.duration,
        reason: penalty.reason.clone(),
        keyword: String::new(),
        inactive: false,
        time_add: penalty.timestamp,
        time_edit: penalty.timestamp,
        time_expire,
        server_id: Some(penalty.server_id),
    };
    let _ = storage.save_penalty(&row).await?;
    Ok(())
}

async fn ensure_client(
    storage: &std::sync::Arc<dyn crate::storage::Storage>,
    guid: &str,
    name: &str,
) -> anyhow::Result<i64> {
    match storage.get_client_by_guid(guid).await {
        Ok(c) => Ok(c.id),
        Err(_) => {
            let mut c = crate::core::Client::new(guid, name);
            c.id = 0;
            let id = storage.save_client(&c).await?;
            Ok(id)
        }
    }
}

async fn handle_player_sync(
    State(state): State<MasterState>,
    Json(batch): Json<PlayerSyncBatch>,
) -> Result<StatusCode, StatusCode> {
    info!(
        server_id = batch.server_id,
        count = batch.players.len(),
        "Received player sync"
    );

    // Upsert each player so the master DB has a canonical client row per GUID.
    for p in &batch.players {
        match state.storage.get_client_by_guid(&p.guid).await {
            Ok(mut existing) => {
                existing.name = p.name.clone();
                if let Some(ip_str) = &p.ip {
                    if let Ok(ip) = ip_str.parse() {
                        existing.ip = Some(ip);
                    }
                }
                existing.group_bits = p.group_bits;
                existing.time_edit = chrono::Utc::now();
                if let Err(e) = state.storage.save_client(&existing).await {
                    debug!(guid = %p.guid, error = %e, "Failed to update client on master");
                }
            }
            Err(_) => {
                let mut c = crate::core::Client::new(&p.guid, &p.name);
                if let Some(ip_str) = &p.ip {
                    if let Ok(ip) = ip_str.parse() {
                        c.ip = Some(ip);
                    }
                }
                c.group_bits = p.group_bits;
                if let Err(e) = state.storage.save_client(&c).await {
                    debug!(guid = %p.guid, error = %e, "Failed to create client on master");
                }
            }
        }
    }

    Ok(StatusCode::OK)
}

async fn handle_get_config(
    State(state): State<MasterState>,
    axum::extract::Path(server_id): axum::extract::Path<i64>,
) -> Result<Json<ConfigSync>, StatusCode> {
    let server = state
        .storage
        .get_server(server_id)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(Json(ConfigSync {
        server_id,
        config_json: server.config_json.unwrap_or_default(),
        config_version: server.config_version,
    }))
}

async fn handle_put_config(
    State(state): State<MasterState>,
    axum::extract::Path(server_id): axum::extract::Path<i64>,
    Json(config): Json<ConfigSync>,
) -> Result<StatusCode, StatusCode> {
    let mut server = state
        .storage
        .get_server(server_id)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    server.config_json = Some(config.config_json.clone());
    server.config_version = config.config_version;
    state.storage.save_server(&server).await.map_err(|e| {
        error!(error = %e, "Failed to save server config");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Push to connected client if online
    let clients = state.connected_clients.read().await;
    if let Some(client) = clients.get(&server_id) {
        let msg = SyncMessage::ConfigUpdate(config);
        let _ = client.tx.send(msg).await;
    }

    Ok(StatusCode::OK)
}

async fn handle_get_global_bans(
    State(_state): State<MasterState>,
) -> Result<Json<Vec<PenaltySync>>, StatusCode> {
    // TODO: query global bans from database
    Ok(Json(Vec::new()))
}

async fn handle_ws(
    State(state): State<MasterState>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws_connection(socket, state))
}

async fn handle_ws_connection(mut socket: ws::WebSocket, state: MasterState) {
    info!("Internal WebSocket connection established");

    // The client should send a Heartbeat as the first message to identify itself
    let server_id = match socket.recv().await {
        Some(Ok(ws::Message::Text(text))) => {
            match serde_json::from_str::<SyncMessage>(&text) {
                Ok(SyncMessage::Heartbeat(hb)) => {
                    info!(server_id = hb.server_id, "Client identified via WS");
                    hb.server_id
                }
                _ => {
                    warn!("First WS message was not a Heartbeat, closing");
                    return;
                }
            }
        }
        _ => {
            warn!("Failed to receive identification message on WS");
            return;
        }
    };

    // Create a channel for sending messages to this client
    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel::<SyncMessage>(256);

    // Register this client
    {
        let server_name = state
            .storage
            .get_server(server_id)
            .await
            .map(|s| s.name)
            .unwrap_or_else(|_| format!("server-{}", server_id));

        state.connected_clients.write().await.insert(
            server_id,
            ConnectedClient {
                server_id,
                server_name,
                tx: cmd_tx,
            },
        );
    }

    // Use the socket directly — axum's WebSocket has recv()/send() methods
    loop {
        tokio::select! {
            // Messages from the client bot
            msg = socket.recv() => {
                match msg {
                    Some(Ok(ws::Message::Text(text))) => {
                        match serde_json::from_str::<SyncMessage>(&text) {
                            Ok(SyncMessage::Event(event)) => {
                                let _ = state.event_tx.send(event.clone());
                                if let Some(text) = chat_text_from_event(&event.data) {
                                    if let Some(cid) = event.client_id {
                                        let msg = crate::core::ChatMessage {
                                            id: 0,
                                            client_id: cid,
                                            client_name: chat_client_name_from_event(&event.data).unwrap_or_default(),
                                            channel: "all".to_string(),
                                            message: text,
                                            time_add: event.timestamp,
                                            server_id: Some(server_id),
                                        };
                                        let _ = state.storage.save_chat_message(&msg).await;
                                    }
                                }
                            }
                            Ok(SyncMessage::EventBatch(batch)) => {
                                for event in batch.events {
                                    if let Some(text) = chat_text_from_event(&event.data) {
                                        if let Some(cid) = event.client_id {
                                            let msg = crate::core::ChatMessage {
                                                id: 0,
                                                client_id: cid,
                                                client_name: chat_client_name_from_event(&event.data).unwrap_or_default(),
                                                channel: "all".to_string(),
                                                message: text,
                                                time_add: event.timestamp,
                                                server_id: Some(server_id),
                                            };
                                            let _ = state.storage.save_chat_message(&msg).await;
                                        }
                                    }
                                    let _ = state.event_tx.send(event);
                                }
                            }
                            Ok(SyncMessage::Penalty(penalty)) => {
                                if penalty.scope == PenaltyScope::Global {
                                    let clients = state.connected_clients.read().await;
                                    for (sid, client) in clients.iter() {
                                        if *sid != server_id {
                                            let _ = client.tx.send(
                                                SyncMessage::GlobalPenalty(penalty.clone())
                                            ).await;
                                        }
                                    }
                                }
                                if let Err(e) = persist_penalty(&state, &penalty).await {
                                    warn!(error = %e, "Failed to persist penalty on master (WS)");
                                }
                            }
                            Ok(SyncMessage::Heartbeat(hb)) => {
                                let _ = state.storage.update_server_status(
                                    hb.server_id,
                                    &hb.status,
                                    hb.current_map.as_deref(),
                                    hb.player_count,
                                    hb.max_clients,
                                ).await;
                            }
                            Ok(SyncMessage::Response { request_id, response }) => {
                                let mut pending = state.pending_responses.write().await;
                                if let Some(tx) = pending.remove(&request_id) {
                                    let _ = tx.send(response);
                                } else {
                                    warn!(request_id, "Received response for unknown request");
                                }
                            }
                            Ok(other) => {
                                warn!(?other, "Unhandled WS message from client");
                            }
                            Err(e) => {
                                warn!(error = %e, "Failed to parse WS message from client");
                            }
                        }
                    }
                    Some(Ok(ws::Message::Close(_))) | None => {
                        info!(server_id, "Client WS disconnected");
                        break;
                    }
                    _ => {}
                }
            }
            // Commands from master to send to this client
            cmd = cmd_rx.recv() => {
                if let Some(msg) = cmd {
                    let text = serde_json::to_string(&msg).unwrap_or_default();
                    if socket.send(ws::Message::Text(text.into())).await.is_err() {
                        warn!(server_id, "Failed to send WS message to client");
                        break;
                    }
                }
            }
        }
    }

    // Cleanup
    state.connected_clients.write().await.remove(&server_id);
    let _ = state.storage.update_server_status(server_id, "offline", None, 0, 0).await;
    info!(server_id, "Client bot disconnected, marked offline");
}

/// Start the master internal API server with mTLS.
/// Returns the shared `connected_clients` map so the web API can also use it.
pub async fn start_master_api(
    config: &MasterSection,
    storage: Arc<dyn Storage>,
    event_tx: broadcast::Sender<EventPayload>,
    connected_clients: Arc<RwLock<HashMap<i64, ConnectedClient>>>,
    pending_responses: Arc<RwLock<HashMap<String, oneshot::Sender<ClientResponse>>>>,
    pending_client_requests: Arc<RwLock<HashMap<i64, Vec<(String, ClientRequest)>>>>,
    client_versions: Arc<RwLock<HashMap<i64, ClientVersionInfo>>>,
    connected_hubs: Arc<RwLock<HashMap<i64, ConnectedHub>>>,
    pending_hub_actions: Arc<RwLock<HashMap<i64, Vec<(String, HubAction)>>>>,
    pending_hub_responses: Arc<RwLock<HashMap<String, oneshot::Sender<HubResponse>>>>,
    hub_versions: Arc<RwLock<HashMap<i64, ClientVersionInfo>>>,
    hub_action_logs: Arc<RwLock<HashMap<String, HubActionLog>>>,
    config_path: String,
    public_ip: String,
) -> anyhow::Result<()> {
    let tls_acceptor = tls::build_master_tls_acceptor(
        Path::new(&config.tls_cert),
        Path::new(&config.tls_key),
        Path::new(&config.ca_cert),
    )?;

    let state = MasterState {
        storage,
        connected_clients,
        event_tx,
        pending_responses,
        pending_client_requests,
        client_versions,
        connected_hubs,
        pending_hub_actions,
        pending_hub_responses,
        hub_versions,
        hub_action_logs: hub_action_logs.clone(),
        config_path,
        master_config: config.clone(),
        public_ip,
    };

    let app = build_internal_router(state);
    let addr: SocketAddr = format!("{}:{}", config.bind_address, config.port).parse()?;

    info!(addr = %addr, "Master internal API starting (mTLS)");

    // Background GC: drop hub action logs older than 10 min to keep the
    // in-memory map bounded.
    let gc_logs = hub_action_logs;
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            tick.tick().await;
            let cutoff = chrono::Utc::now() - chrono::Duration::minutes(10);
            let mut logs = gc_logs.write().await;
            logs.retain(|_, log| log.created_at > cutoff);
        }
    });

    let listener = crate::bind_reuse(&addr.to_string())?;

    loop {
        let (stream, peer_addr) = listener.accept().await?;
        let acceptor = tls_acceptor.clone();
        let app = app.clone();

        tokio::spawn(async move {
            match acceptor.accept(stream).await {
                Ok(tls_stream) => {
                    let io = hyper_util::rt::TokioIo::new(tls_stream);
                    let tower_service = app;

                    let hyper_service = hyper::service::service_fn(
                        move |request: hyper::Request<hyper::body::Incoming>| {
                            let svc = tower_service.clone();
                            async move {
                                use tower::ServiceExt;
                                svc.oneshot(request.map(axum::body::Body::new)).await
                            }
                        },
                    );

                    let builder = hyper_util::server::conn::auto::Builder::new(
                        hyper_util::rt::TokioExecutor::new(),
                    );
                    // Use serve_connection_with_upgrades for WebSocket support
                    if let Err(e) = builder
                        .serve_connection_with_upgrades(io, hyper_service)
                        .await
                    {
                        error!(peer = %peer_addr, error = %e, "Connection error");
                    }
                }
                Err(e) => {
                    warn!(peer = %peer_addr, error = %e, "TLS handshake failed");
                }
            }
        });
    }
}

// ---------------------------------------------------------------------------
// Internal REST endpoints for client request/response polling
// ---------------------------------------------------------------------------

/// GET /internal/requests/:server_id — client polls for pending requests.
async fn handle_poll_requests(
    State(state): State<MasterState>,
    axum::extract::Path(server_id): axum::extract::Path<i64>,
) -> Json<PendingRequestsResponse> {
    let mut pending = state.pending_client_requests.write().await;
    let items = pending.remove(&server_id).unwrap_or_default();

    let requests: Vec<PendingRequestItem> = items
        .into_iter()
        .map(|(request_id, request)| PendingRequestItem { request_id, request })
        .collect();

    Json(PendingRequestsResponse { requests })
}

/// POST /internal/responses — client sends back a response to a request.
async fn handle_client_response(
    State(state): State<MasterState>,
    Json(body): Json<ClientResponseSubmission>,
) -> StatusCode {
    let mut pending = state.pending_responses.write().await;
    if let Some(tx) = pending.remove(&body.request_id) {
        let _ = tx.send(body.response);
        StatusCode::OK
    } else {
        warn!(request_id = %body.request_id, "Response for unknown or expired request");
        StatusCode::NOT_FOUND
    }
}

// ---------------------------------------------------------------------------
// Public helper: send a request to a connected client and await the response
// ---------------------------------------------------------------------------

/// Queue a request for a client bot and wait for its response.
///
/// The request is placed in `pending_client_requests` for the given server_id.
/// The client picks it up during its next poll cycle (every ~2s). A oneshot
/// channel correlates the eventual response. Times out after `timeout`.
pub async fn send_request_to_server(
    pending_responses: &Arc<RwLock<HashMap<String, oneshot::Sender<ClientResponse>>>>,
    pending_client_requests: &Arc<RwLock<HashMap<i64, Vec<(String, ClientRequest)>>>>,
    server_id: i64,
    request: ClientRequest,
    timeout: std::time::Duration,
) -> Result<ClientResponse, String> {
    let request_id = uuid::Uuid::new_v4().to_string();

    // Create oneshot for the response
    let (resp_tx, resp_rx) = oneshot::channel();

    // Store the oneshot sender
    pending_responses.write().await.insert(request_id.clone(), resp_tx);

    // Queue the request for the client to pick up
    {
        let mut pending = pending_client_requests.write().await;
        pending
            .entry(server_id)
            .or_insert_with(Vec::new)
            .push((request_id.clone(), request));
    }

    // Await the response with timeout
    match tokio::time::timeout(timeout, resp_rx).await {
        Ok(Ok(response)) => Ok(response),
        Ok(Err(_)) => {
            pending_responses.write().await.remove(&request_id);
            Err("Client disconnected before responding".to_string())
        }
        Err(_) => {
            pending_responses.write().await.remove(&request_id);
            // Also clean up the queued request if it hasn't been picked up
            {
                let mut pending = pending_client_requests.write().await;
                if let Some(reqs) = pending.get_mut(&server_id) {
                    reqs.retain(|(id, _)| id != &request_id);
                }
            }
            Err("Request timed out waiting for client response".to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// Hub orchestration handlers
// ---------------------------------------------------------------------------

/// POST /internal/hub/register — hub announces itself after pairing.
async fn handle_hub_register(
    State(state): State<MasterState>,
    Json(req): Json<HubRegisterRequest>,
) -> Result<Json<HubRegisterResponse>, StatusCode> {
    info!(name = %req.hub_name, address = %req.address, "Hub registering");

    let existing = state
        .storage
        .get_hub_by_fingerprint(&req.cert_fingerprint)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to look up hub by fingerprint");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let hub_id = if let Some(mut hub) = existing {
        hub.name = req.hub_name.clone();
        if !req.address.is_empty() && req.address != "0.0.0.0" {
            hub.address = req.address;
        }
        hub.status = "online".to_string();
        hub.last_seen = Some(chrono::Utc::now());
        hub.hub_version = Some(req.version.clone());
        hub.build_hash = Some(req.build_hash.clone());
        state.storage.save_hub(&hub).await.map_err(|e| {
            error!(error = %e, "Failed to update hub");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        hub.id
    } else {
        let hub = crate::core::Hub {
            id: 0,
            name: req.hub_name.clone(),
            address: req.address,
            status: "online".to_string(),
            last_seen: Some(chrono::Utc::now()),
            cert_fingerprint: Some(req.cert_fingerprint),
            hub_version: Some(req.version.clone()),
            build_hash: Some(req.build_hash.clone()),
            update_channel: "beta".to_string(),
            update_interval: 3600,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        state.storage.save_hub(&hub).await.map_err(|e| {
            error!(error = %e, "Failed to save new hub");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
    };

    // Persist initial host info
    let info_row = crate::core::HubHostInfo {
        hub_id,
        hostname: req.host_info.hostname,
        os: req.host_info.os,
        kernel: req.host_info.kernel,
        cpu_model: req.host_info.cpu_model,
        cpu_cores: req.host_info.cpu_cores,
        total_ram_bytes: req.host_info.total_ram_bytes,
        disk_total_bytes: req.host_info.disk_total_bytes,
        public_ip: req.host_info.public_ip,
        external_ip: req.host_info.external_ip,
        urt_installs_json: req.host_info.urt_installs_json.unwrap_or_default(),
        updated_at: chrono::Utc::now(),
    };
    if let Err(e) = state.storage.upsert_host_info(&info_row).await {
        warn!(hub_id, error = %e, "Failed to persist hub host info on register");
    }

    state.connected_hubs.write().await.insert(
        hub_id,
        ConnectedHub {
            hub_id,
            hub_name: req.hub_name,
            last_seen: chrono::Utc::now(),
        },
    );
    state.hub_versions.write().await.insert(
        hub_id,
        ClientVersionInfo {
            build_hash: Some(req.build_hash),
            version: Some(req.version),
            reported_at: chrono::Utc::now(),
        },
    );

    info!(hub_id, "Hub registered");
    Ok(Json(HubRegisterResponse { hub_id }))
}

/// POST /internal/hub/heartbeat — hub keepalive + telemetry + action poll.
async fn handle_hub_heartbeat(
    State(state): State<MasterState>,
    Json(req): Json<HubHeartbeatRequest>,
) -> Result<Json<HubHeartbeatResponse>, StatusCode> {
    let hub = match state.storage.get_hub(req.hub_id).await {
        Ok(h) => h,
        Err(_) => return Err(StatusCode::NOT_FOUND),
    };

    // Update last_seen + version
    let mut updated = hub;
    updated.status = "online".to_string();
    updated.last_seen = Some(chrono::Utc::now());
    updated.hub_version = Some(req.version.clone());
    updated.build_hash = Some(req.build_hash.clone());
    let _ = state.storage.save_hub(&updated).await;

    state.hub_versions.write().await.insert(
        req.hub_id,
        ClientVersionInfo {
            build_hash: Some(req.build_hash.clone()),
            version: Some(req.version.clone()),
            reported_at: chrono::Utc::now(),
        },
    );
    state.connected_hubs.write().await.insert(
        req.hub_id,
        ConnectedHub {
            hub_id: req.hub_id,
            hub_name: updated.name.clone(),
            last_seen: chrono::Utc::now(),
        },
    );

    if let Some(info) = req.host_info {
        let row = crate::core::HubHostInfo {
            hub_id: req.hub_id,
            hostname: info.hostname,
            os: info.os,
            kernel: info.kernel,
            cpu_model: info.cpu_model,
            cpu_cores: info.cpu_cores,
            total_ram_bytes: info.total_ram_bytes,
            disk_total_bytes: info.disk_total_bytes,
            public_ip: info.public_ip,
            external_ip: info.external_ip,
            urt_installs_json: info.urt_installs_json.unwrap_or_default(),
            updated_at: chrono::Utc::now(),
        };
        if let Err(e) = state.storage.upsert_host_info(&row).await {
            warn!(hub_id = req.hub_id, error = %e, "Failed to upsert hub host info");
        }
    }

    let metric = crate::core::HubMetricSample {
        hub_id: req.hub_id,
        ts: chrono::Utc::now(),
        cpu_pct: req.metrics.cpu_pct,
        mem_pct: req.metrics.mem_pct,
        disk_pct: req.metrics.disk_pct,
        load1: req.metrics.load1,
        load5: req.metrics.load5,
        load15: req.metrics.load15,
        uptime_s: req.metrics.uptime_s,
    };
    if let Err(e) = state.storage.record_host_metric(&metric).await {
        debug!(hub_id = req.hub_id, error = %e, "Failed to record hub metric (non-fatal)");
    }

    // Drain any pending hub actions
    let pending_actions = {
        let mut pending = state.pending_hub_actions.write().await;
        pending
            .remove(&req.hub_id)
            .unwrap_or_default()
            .into_iter()
            .map(|(action_id, action)| PendingHubActionItem { action_id, action })
            .collect::<Vec<_>>()
    };

    Ok(Json(HubHeartbeatResponse {
        ok: true,
        pending_actions,
        update_channel: Some(updated.update_channel.clone()),
        update_interval: Some(updated.update_interval),
    }))
}

/// GET /internal/hub/actions/:hub_id — alternate poll endpoint (non-heartbeat).
async fn handle_poll_hub_actions(
    State(state): State<MasterState>,
    axum::extract::Path(hub_id): axum::extract::Path<i64>,
) -> Json<Vec<PendingHubActionItem>> {
    let mut pending = state.pending_hub_actions.write().await;
    let items = pending.remove(&hub_id).unwrap_or_default();
    let out = items
        .into_iter()
        .map(|(action_id, action)| PendingHubActionItem { action_id, action })
        .collect();
    Json(out)
}

/// POST /internal/hub/responses — hub returns the result of a queued action.
async fn handle_hub_response(
    State(state): State<MasterState>,
    Json(body): Json<HubResponse>,
) -> StatusCode {
    // Stash the final result in the in-memory log first so UI pollers
    // that already gave up on the oneshot can still retrieve it.
    {
        let mut logs = state.hub_action_logs.write().await;
        if let Some(log) = logs.get_mut(&body.action_id) {
            log.result = Some(body.clone());
        }
    }

    let mut pending = state.pending_hub_responses.write().await;
    if let Some(tx) = pending.remove(&body.action_id) {
        let _ = tx.send(body);
        StatusCode::OK
    } else {
        // Not an error: the UI may have switched to progress-polling mode
        // and stopped awaiting a oneshot reply. The log entry above is
        // the source of truth.
        debug!(action_id = %body.action_id, "Hub response recorded (no pending oneshot)");
        StatusCode::OK
    }
}

/// POST /internal/hub/progress — hub reports an intermediate progress
/// event for an in-flight action. Appended to the in-memory action log.
async fn handle_hub_progress(
    State(state): State<MasterState>,
    Json(event): Json<HubProgressEvent>,
) -> StatusCode {
    let mut logs = state.hub_action_logs.write().await;
    if let Some(log) = logs.get_mut(&event.action_id) {
        log.events.push(event);
        StatusCode::OK
    } else {
        warn!(action_id = %event.action_id, "Progress event for unknown action");
        StatusCode::NOT_FOUND
    }
}

/// POST /internal/hub/mint-client-cert — hub asks the master to mint a fresh
/// client certificate + server_id for a sub-client it is provisioning.
async fn handle_mint_client_cert(
    State(state): State<MasterState>,
    Json(req): Json<MintClientCertRequest>,
) -> Result<Json<MintClientCertResponse>, (StatusCode, String)> {
    // Sanity check: hub must be known.
    let hub = state
        .storage
        .get_hub(req.hub_id)
        .await
        .map_err(|_| (StatusCode::NOT_FOUND, "Unknown hub_id".to_string()))?;

    let ca_cert_pem = std::fs::read_to_string(&state.master_config.ca_cert)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("read CA cert: {}", e)))?;
    let ca_key_pem = std::fs::read_to_string(&state.master_config.ca_key)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("read CA key: {}", e)))?;

    let cert_cn = format!("hub:{}:{}", req.hub_id, req.slug);
    let client_certs = ca::generate_client_cert(&ca_cert_pem, &ca_key_pem, &cert_cn, None)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("mint cert: {}", e)))?;

    let fingerprint = {
        use sha2::{Digest, Sha256};
        let mut reader = std::io::BufReader::new(client_certs.cert_pem.as_bytes());
        let der_certs: Vec<_> = rustls_pemfile::certs(&mut reader)
            .collect::<Result<Vec<_>, _>>()
            .unwrap_or_default();
        if let Some(cert_der) = der_certs.first() {
            let digest = Sha256::digest(cert_der.as_ref());
            digest
                .iter()
                .enumerate()
                .map(|(i, b)| if i > 0 { format!(":{:02X}", b) } else { format!("{:02X}", b) })
                .collect::<String>()
        } else {
            String::new()
        }
    };

    // If the hub didn't supply a public address for this game server
    // (admin left it blank in the wizard → "auto-detect"), fall back to
    // the hub's own address so the UI doesn't immediately flag the new
    // server as unconfigured.
    let effective_address = if req.address.is_empty() || req.address == "0.0.0.0" {
        if hub.address.is_empty() {
            warn!(hub_id = req.hub_id, slug = %req.slug,
                "mint-client-cert: no address supplied and hub has no known address");
        }
        hub.address.clone()
    } else {
        req.address
    };

    // If the hub supplied RCON + log/cfg paths from the fresh install,
    // seed config_json so the server is considered fully configured
    // immediately — no second wizard required on the master UI.
    let (config_json, config_version) = if req.port != 0
        && !effective_address.is_empty()
        && effective_address != "0.0.0.0"
        && req.rcon_password.as_deref().map(|s| !s.is_empty()).unwrap_or(false)
    {
        let payload = crate::sync::protocol::ServerConfigPayload {
            address: effective_address.clone(),
            port: req.port,
            rcon_password: req.rcon_password.clone().unwrap_or_default(),
            game_log: req.game_log.clone(),
            server_cfg_path: req.server_cfg_path.clone(),
            rcon_ip: None,
            rcon_port: None,
            delay: None,
            bot: None,
            plugins: None,
        };
        match serde_json::to_string(&payload) {
            Ok(s) => (Some(s), 1),
            Err(e) => {
                warn!(error = %e, "Failed to serialize initial config_json");
                (None, 1)
            }
        }
    } else {
        (None, 1)
    };

    let server = GameServer {
        id: 0,
        name: req.server_name,
        address: effective_address,
        port: req.port,
        status: "offline".to_string(),
        current_map: None,
        player_count: 0,
        max_clients: 0,
        last_seen: None,
        config_json,
        config_version,
        cert_fingerprint: Some(fingerprint),
        update_channel: "beta".to_string(),
        update_interval: 3600,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        hub_id: Some(req.hub_id),
        slug: Some(req.slug),
    };
    let server_id = state
        .storage
        .save_server(&server)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("save server: {}", e)))?;

    let master_sync_url = format!("https://{}:{}", state.public_ip, state.master_config.port);

    Ok(Json(MintClientCertResponse {
        server_id,
        ca_cert: ca_cert_pem,
        client_cert: client_certs.cert_pem,
        client_key: client_certs.key_pem,
        master_sync_url,
    }))
}

/// Queue a `HubAction` for a hub and await its response.
pub async fn send_action_to_hub(
    pending_hub_responses: &Arc<RwLock<HashMap<String, oneshot::Sender<HubResponse>>>>,
    pending_hub_actions: &Arc<RwLock<HashMap<i64, Vec<(String, HubAction)>>>>,
    hub_id: i64,
    action: HubAction,
    timeout: std::time::Duration,
) -> Result<HubResponse, String> {
    let action_id = uuid::Uuid::new_v4().to_string();
    let (resp_tx, resp_rx) = oneshot::channel();

    pending_hub_responses
        .write()
        .await
        .insert(action_id.clone(), resp_tx);

    {
        let mut pending = pending_hub_actions.write().await;
        pending
            .entry(hub_id)
            .or_insert_with(Vec::new)
            .push((action_id.clone(), action));
    }

    match tokio::time::timeout(timeout, resp_rx).await {
        Ok(Ok(response)) => Ok(response),
        Ok(Err(_)) => {
            pending_hub_responses.write().await.remove(&action_id);
            Err("Hub disconnected before responding".to_string())
        }
        Err(_) => {
            pending_hub_responses.write().await.remove(&action_id);
            {
                let mut pending = pending_hub_actions.write().await;
                if let Some(reqs) = pending.get_mut(&hub_id) {
                    reqs.retain(|(id, _)| id != &action_id);
                }
            }
            Err("Hub action timed out".to_string())
        }
    }
}

/// Enqueue a `HubAction` without awaiting a response. Returns the
/// generated `action_id` immediately so the caller (typically the web
/// API) can return `202 Accepted` and let the UI poll for progress and
/// the final result via `hub_action_logs`.
///
/// Creates an empty `HubActionLog` entry keyed by `action_id` so the
/// progress endpoint and final response handler have a place to write
/// to even if the hub reports back before the UI polls.
pub async fn enqueue_hub_action(
    pending_hub_actions: &Arc<RwLock<HashMap<i64, Vec<(String, HubAction)>>>>,
    hub_action_logs: &Arc<RwLock<HashMap<String, HubActionLog>>>,
    hub_id: i64,
    action: HubAction,
) -> String {
    let action_id = uuid::Uuid::new_v4().to_string();
    let action_kind = action_kind_label(&action);

    {
        let mut logs = hub_action_logs.write().await;
        logs.insert(
            action_id.clone(),
            HubActionLog {
                hub_id,
                action_kind,
                created_at: chrono::Utc::now(),
                events: Vec::new(),
                result: None,
            },
        );
    }

    {
        let mut pending = pending_hub_actions.write().await;
        pending
            .entry(hub_id)
            .or_insert_with(Vec::new)
            .push((action_id.clone(), action));
    }

    action_id
}

/// Short machine-readable label for a `HubAction` variant, used for
/// populating `HubActionLog.action_kind`.
fn action_kind_label(action: &HubAction) -> String {
    match action {
        HubAction::InstallClient { .. } => "install_client",
        HubAction::UninstallClient { .. } => "uninstall_client",
        HubAction::StartClient { .. } => "start_client",
        HubAction::StopClient { .. } => "stop_client",
        HubAction::RestartClient { .. } => "restart_client",
        HubAction::InstallGameServer { .. } => "install_game_server",
        HubAction::RemoveGameServer { .. } => "remove_game_server",
        HubAction::UpdateClient { .. } => "update_client",
        HubAction::GetClientLogs { .. } => "get_client_logs",
        HubAction::GetHubVersion => "get_hub_version",
        HubAction::Restart => "restart_hub",
        HubAction::ForceUpdate { .. } => "force_update",
        HubAction::SelfUninstall { .. } => "self_uninstall",
    }
    .to_string()
}
