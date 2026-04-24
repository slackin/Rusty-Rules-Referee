//! Hub mode — manages a fleet of `r3-client@<slug>.service` systemd units
//! on a single physical host. Pairs with the master like a regular client
//! but reports host telemetry and acts on `HubAction` commands the master
//! queues for it.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::config::RefereeConfig;
use crate::sync::protocol::{
    HostInfoPayload, HubHeartbeatRequest, HubHeartbeatResponse, HubRegisterRequest,
    HubRegisterResponse, HubResponse, PendingHubActionItem,
};
use crate::sync::tls;

pub mod actions;
pub mod client_manager;
pub mod game_server_manager;
pub mod host_info;

/// Entry point for `--mode hub`.
pub async fn run_hub(config: RefereeConfig, _config_path: String) -> anyhow::Result<()> {
    info!("Starting in HUB mode");

    let hub_cfg = config
        .hub
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("[hub] section missing in config"))?;

    let tls_config = tls::build_client_tls_config(
        Path::new(&hub_cfg.tls_cert),
        Path::new(&hub_cfg.tls_key),
        Path::new(&hub_cfg.ca_cert),
    )?;

    let http_client = reqwest::Client::builder()
        .use_preconfigured_tls(tls_config.as_ref().clone())
        .timeout(Duration::from_secs(30))
        .build()?;

    let base_url = hub_cfg.master_url.trim_end_matches('/').to_string();

    // Compute cert fingerprint for registration.
    let certs = tls::load_certs(Path::new(&hub_cfg.tls_cert))?;
    let fingerprint = certs
        .first()
        .map(tls::cert_fingerprint)
        .ok_or_else(|| anyhow::anyhow!("hub TLS cert is empty"))?;

    // Static host info, refreshed on a slow timer.
    let host_state = Arc::new(RwLock::new(host_info::collect_host_info()));

    // Long-lived hub_id once registered.
    let hub_id_state: Arc<RwLock<Option<i64>>> = Arc::new(RwLock::new(None));

    let version = env!("CARGO_PKG_VERSION").to_string();
    let build_hash = env!("BUILD_HASH").to_string();

    // Release channel the master wants us to follow; seeded from local config
    // and updated on every heartbeat.
    let update_channel: Arc<RwLock<String>> =
        Arc::new(RwLock::new(config.update.channel.clone()));

    // Kick off a periodic auto-update checker if enabled. The channel is
    // read from the shared `update_channel` state so live changes from the
    // master take effect on the next tick without a restart.
    if config.update.enabled {
        let update_cfg = config.update.clone();
        let channel_watch = update_channel.clone();
        tokio::spawn(async move {
            crate::update::run_update_loop_with_overrides(
                update_cfg,
                env!("BUILD_HASH"),
                Some(channel_watch),
                None,
            )
            .await;
        });
    }

    // Heartbeat / register loop with reconnection on failure.
    loop {
        // Register if we don't have a hub_id yet (or want to refresh on reconnect).
        let hub_id = match register_with_master(
            &http_client,
            &base_url,
            hub_cfg,
            &fingerprint,
            &version,
            &build_hash,
            host_state.clone(),
        )
        .await
        {
            Ok(id) => {
                *hub_id_state.write().await = Some(id);
                id
            }
            Err(e) => {
                warn!(error = %e, "Hub register failed, retrying in 10s");
                tokio::time::sleep(Duration::from_secs(10)).await;
                continue;
            }
        };

        info!(hub_id, "Hub registered with master");

        // Spawn host-info refresher (slow timer).
        let host_state_refresh = host_state.clone();
        let refresh_secs = hub_cfg.host_refresh_interval.max(60);
        let refresher = tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(refresh_secs));
            tick.tick().await; // skip immediate
            loop {
                tick.tick().await;
                let info = host_info::collect_host_info();
                *host_state_refresh.write().await = info;
            }
        });

        // Heartbeat loop.
        let heartbeat_interval = hub_cfg.heartbeat_interval.max(5);
        let mut hb_timer = tokio::time::interval(Duration::from_secs(heartbeat_interval));
        let mut last_pushed_host_info: Option<HostInfoPayload> = None;

        let disconnected = loop {
            hb_timer.tick().await;

            // Snapshot host info; only resend if it changed.
            let host_snapshot = host_state.read().await.clone();
            let host_info_to_send = match &last_pushed_host_info {
                Some(prev) if same_host_info(prev, &host_snapshot) => None,
                _ => Some(host_snapshot.clone()),
            };

            let metrics = host_info::sample_metrics();
            let clients = client_manager::list_client_statuses(hub_cfg).await;

            let req = HubHeartbeatRequest {
                hub_id,
                host_info: host_info_to_send.clone(),
                metrics,
                clients,
                version: version.clone(),
                build_hash: build_hash.clone(),
            };

            match http_client
                .post(format!("{}/internal/hub/heartbeat", base_url))
                .json(&req)
                .send()
                .await
            {
                Ok(resp) => match resp.json::<HubHeartbeatResponse>().await {
                    Ok(body) => {
                        if host_info_to_send.is_some() {
                            last_pushed_host_info = Some(host_snapshot);
                        }
                        // Adopt any channel change pushed by the master.
                        if let Some(remote_channel) = body.update_channel.as_ref() {
                            let current = update_channel.read().await.clone();
                            if &current != remote_channel {
                                info!(
                                    from = %current,
                                    to = %remote_channel,
                                    "Adopting release channel from master"
                                );
                                *update_channel.write().await = remote_channel.clone();
                            }
                        }
                        // Dispatch any queued actions.
                        for item in body.pending_actions {
                            dispatch_action(&http_client, &base_url, hub_id, hub_cfg, item).await;
                        }
                    }
                    Err(e) => {
                        warn!(error = %e, "Bad heartbeat response from master");
                        break true;
                    }
                },
                Err(e) => {
                    warn!(error = %e, "Heartbeat request failed");
                    break true;
                }
            }
        };

        refresher.abort();
        if disconnected {
            warn!("Hub disconnected from master, will re-register in 10s");
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    }
}

async fn register_with_master(
    http: &reqwest::Client,
    base_url: &str,
    hub_cfg: &crate::config::HubSection,
    fingerprint: &str,
    version: &str,
    build_hash: &str,
    host_state: Arc<RwLock<HostInfoPayload>>,
) -> anyhow::Result<i64> {
    let host_info = host_state.read().await.clone();
    let req = HubRegisterRequest {
        hub_name: hub_cfg.hub_name.clone(),
        address: host_info
            .public_ip
            .clone()
            .or_else(|| host_info.external_ip.clone())
            .unwrap_or_default(),
        cert_fingerprint: fingerprint.to_string(),
        version: version.to_string(),
        build_hash: build_hash.to_string(),
        host_info,
    };

    let resp = http
        .post(format!("{}/internal/hub/register", base_url))
        .json(&req)
        .send()
        .await?;
    if !resp.status().is_success() {
        anyhow::bail!("master returned {}", resp.status());
    }
    let body: HubRegisterResponse = resp.json().await?;
    Ok(body.hub_id)
}

async fn dispatch_action(
    http: &reqwest::Client,
    base_url: &str,
    hub_id: i64,
    hub_cfg: &crate::config::HubSection,
    item: PendingHubActionItem,
) {
    let action_id = item.action_id.clone();
    debug!(?item.action, action_id = %action_id, "Dispatching hub action");

    let response = match actions::execute(http, base_url, hub_id, hub_cfg, &action_id, item.action).await {
        Ok((message, data)) => HubResponse {
            action_id: action_id.clone(),
            ok: true,
            message,
            data,
        },
        Err(e) => HubResponse {
            action_id: action_id.clone(),
            ok: false,
            message: format!("{}", e),
            data: None,
        },
    };

    if let Err(e) = http
        .post(format!("{}/internal/hub/responses", base_url))
        .json(&response)
        .send()
        .await
    {
        error!(action_id = %action_id, error = %e, "Failed to POST hub action response");
    }
}

fn same_host_info(a: &HostInfoPayload, b: &HostInfoPayload) -> bool {
    a.hostname == b.hostname
        && a.os == b.os
        && a.kernel == b.kernel
        && a.cpu_model == b.cpu_model
        && a.cpu_cores == b.cpu_cores
        && a.total_ram_bytes == b.total_ram_bytes
        && a.disk_total_bytes == b.disk_total_bytes
        && a.public_ip == b.public_ip
        && a.external_ip == b.external_ip
        && a.urt_installs_json == b.urt_installs_json
}
