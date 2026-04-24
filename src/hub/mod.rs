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

    // Auto-update check interval (seconds). Seeded from local config and
    // replaced with whatever the master sends on every heartbeat so admins
    // can retune it without restarting the hub.
    let update_interval: Arc<RwLock<u64>> =
        Arc::new(RwLock::new(config.update.check_interval));

    // Kick off a periodic auto-update checker if enabled. The channel is
    // read from the shared `update_channel` state so live changes from the
    // master take effect on the next tick without a restart.
    // Always run the auto-update loop in hub mode. The hub is the control
    // plane for many sub-clients that share its binary via the
    // /usr/local/bin/rusty-rules-referee symlink, so keeping the hub up to
    // date also keeps the managed clients up to date (after a restart). We
    // still respect `config.update.enabled=false` for operators who want to
    // pin a specific build — but the installed default is now `true`.
    if config.update.enabled {
        let update_cfg = config.update.clone();
        let channel_watch = update_channel.clone();
        let interval_watch = update_interval.clone();
        let clients_root = hub_cfg.clients_root.clone();
        tokio::spawn(async move {
            // After the new hub binary has been laid down (and just before
            // we exec() into it), bounce every managed sub-client so they
            // drop the old in-memory binary and start running the new one.
            // The sub-clients' ExecStart points at
            // /usr/local/bin/rusty-rules-referee which is a symlink to the
            // hub's binary — updated by apply_update() a moment ago.
            let pre_restart = move || {
                restart_managed_sub_clients(&clients_root);
            };
            crate::update::run_update_loop_full(
                update_cfg,
                env!("BUILD_HASH"),
                Some(channel_watch),
                Some(interval_watch),
                Some(pre_restart),
            )
            .await;
        });
    } else {
        tracing::warn!(
            "Hub auto-update is disabled in r3.toml ([update].enabled = false). \
             Set it to true so the hub and its managed sub-clients stay on the selected channel."
        );
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
                        // Adopt any update-interval change pushed by the master.
                        if let Some(remote_interval) = body.update_interval {
                            let current = *update_interval.read().await;
                            if current != remote_interval {
                                info!(
                                    from = current,
                                    to = remote_interval,
                                    "Adopting update check interval from master"
                                );
                                *update_interval.write().await = remote_interval;
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

/// Restart every `r3-client@<slug>.service` unit managed by this hub.
///
/// Called right before the hub itself exec()s into the freshly-downloaded
/// binary. The sub-clients share the hub's binary via the
/// `/usr/local/bin/rusty-rules-referee` symlink — the file on disk has
/// already been replaced by `apply_update()`, but the running sub-client
/// processes still hold the old one mapped in memory. `systemctl restart`
/// stops them and starts fresh ones that exec the new symlink target.
///
/// Best-effort: logs and continues on errors. We don't block hub restart
/// on sub-client bounces because the hub coming up cleanly is more
/// important than any individual sub-client.
fn restart_managed_sub_clients(clients_root: &str) {
    let dir = match std::fs::read_dir(clients_root) {
        Ok(d) => d,
        Err(e) => {
            warn!(error = %e, path = %clients_root,
                "Could not list clients_root to restart sub-clients after hub update");
            return;
        }
    };
    let mut slugs: Vec<String> = Vec::new();
    for entry in dir.flatten() {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        if let Some(name) = entry.file_name().to_str() {
            slugs.push(name.to_string());
        }
    }
    if slugs.is_empty() {
        info!("Hub update: no managed sub-clients found — nothing to restart");
        return;
    }
    info!(count = slugs.len(), "Hub update: restarting managed sub-clients");
    for slug in slugs {
        let unit = format!("r3-client@{}.service", slug);
        // sudo -n because we run unprivileged — the r3-<user>-hub sudoers
        // drop-in granted NOPASSWD systemctl for r3-client@*.service.
        let out = std::process::Command::new("sudo")
            .args(["-n", "systemctl", "restart", &unit])
            .output();
        match out {
            Ok(o) if o.status.success() => {
                info!(%unit, "Sub-client restarted after hub update");
            }
            Ok(o) => {
                warn!(
                    %unit,
                    stderr = %String::from_utf8_lossy(&o.stderr).trim(),
                    "Sub-client restart failed (continuing)"
                );
            }
            Err(e) => {
                warn!(%unit, error = %e, "Could not spawn sudo systemctl restart");
            }
        }
    }
}
