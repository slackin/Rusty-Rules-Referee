//! Client-side sync manager for communicating with the master server.
//!
//! Runs as a background tokio task alongside the main bot loop.
//! Handles: registration, WebSocket connection, event forwarding,
//! periodic sync, heartbeats, and offline queue draining.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, watch, RwLock};
use tracing::{debug, error, info, warn};

use crate::config::ClientSection;
use crate::core::context::BotContext;
use crate::events::Event;
use crate::storage::Storage;
use crate::sync::handlers::{self, SharedInstallState};
use crate::sync::protocol::*;
use crate::sync::queue::SyncQueue;
use crate::sync::tls;

/// Connection state visible to the rest of the bot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
}

/// The client sync manager.
pub struct ClientSyncManager {
    config: ClientSection,
    config_path: String,
    /// Release channel this client follows for updates (forwarded to force-update handler).
    update_channel: String,
    storage: Arc<dyn Storage>,
    queue: SyncQueue,
    server_id: Arc<RwLock<Option<i64>>>,
    state: Arc<RwLock<ConnectionState>>,
    local_config_version: i64,
    /// Channel to receive events from the main bot loop for forwarding.
    event_rx: mpsc::Receiver<Event>,
    /// Channel to receive commands from the master.
    command_tx: mpsc::Sender<SyncMessage>,
    /// Notify the main loop that the config file has been updated on disk.
    config_updated_tx: watch::Sender<bool>,
}

/// Handle returned to the main loop for interacting with the sync manager.
pub struct SyncHandle {
    /// Send events to the sync manager for forwarding to master.
    pub event_tx: mpsc::Sender<Event>,
    /// Receive commands from the master (kick, ban, config updates, etc.).
    pub command_rx: mpsc::Receiver<SyncMessage>,
    /// Current connection state.
    pub state: Arc<RwLock<ConnectionState>>,
    /// Server ID assigned by master.
    pub server_id: Arc<RwLock<Option<i64>>>,
    /// Watch channel that fires when the sync manager writes a config update to disk.
    pub config_updated: watch::Receiver<bool>,
}

impl ClientSyncManager {
    /// Create a new sync manager and return the handle for the main loop.
    pub fn new(
        config: ClientSection,
        storage: Arc<dyn Storage>,
        config_path: String,
        update_channel: String,
    ) -> (Self, SyncHandle) {
        let (event_tx, event_rx) = mpsc::channel::<Event>(1024);
        let (command_tx, command_rx) = mpsc::channel::<SyncMessage>(256);
        let (config_updated_tx, config_updated_rx) = watch::channel(false);
        let state = Arc::new(RwLock::new(ConnectionState::Disconnected));
        let server_id = Arc::new(RwLock::new(None));
        let queue = SyncQueue::new(storage.clone(), None);

        let manager = Self {
            config,
            config_path,
            update_channel,
            storage,
            queue,
            server_id: server_id.clone(),
            state: state.clone(),
            local_config_version: 0,
            event_rx,
            command_tx,
            config_updated_tx,
        };

        let handle = SyncHandle {
            event_tx,
            command_rx,
            state,
            server_id,
            config_updated: config_updated_rx,
        };

        (manager, handle)
    }

    /// Run the sync manager. This method runs indefinitely.
    pub async fn run(mut self) -> anyhow::Result<()> {
        info!(master = %self.config.master_url, "Starting client sync manager");

        // Build TLS config
        let tls_config = tls::build_client_tls_config(
            Path::new(&self.config.tls_cert),
            Path::new(&self.config.tls_key),
            Path::new(&self.config.ca_cert),
        )?;

        // Build HTTP client with mTLS
        let http_client = reqwest::Client::builder()
            .use_preconfigured_tls(tls_config.as_ref().clone())
            .timeout(Duration::from_secs(30))
            .build()?;

        let base_url = self.config.master_url.trim_end_matches('/').to_string();

        // Compute cert fingerprint for registration
        let certs = tls::load_certs(Path::new(&self.config.tls_cert))?;
        let fingerprint = if let Some(cert) = certs.first() {
            tls::cert_fingerprint(cert)
        } else {
            anyhow::bail!("No certificate found for fingerprint computation");
        };

        // Main connection loop with reconnection
        loop {
            *self.state.write().await = ConnectionState::Connecting;

            // Step 1: Register with master
            match self.register(&http_client, &base_url, &fingerprint).await {
                Ok(response) => {
                    *self.server_id.write().await = Some(response.server_id);
                    self.local_config_version = response.config_version;
                    info!(server_id = response.server_id, config_version = response.config_version, "Registered with master");
                }
                Err(e) => {
                    warn!(error = %e, "Failed to register with master, will retry");
                    *self.state.write().await = ConnectionState::Disconnected;
                    tokio::time::sleep(Duration::from_secs(10)).await;
                    continue;
                }
            }

            let server_id = self.server_id.read().await.unwrap();

            // Step 2: Drain any queued items
            match self.queue.drain(100, |entry| {
                let client = http_client.clone();
                let url = format!("{}/internal/events", base_url);
                async move {
                    // Send queued item as an event batch
                    let payload: EventPayload = serde_json::from_str(&entry.payload)?;
                    let batch = EventBatch {
                        server_id: entry.server_id.unwrap_or(0),
                        events: vec![payload],
                    };
                    client.post(&url).json(&batch).send().await?;
                    Ok(())
                }
            }).await {
                Ok(count) => {
                    if count > 0 {
                        info!(count, "Drained queued sync items");
                    }
                }
                Err(e) => {
                    warn!(error = %e, "Failed to drain queue");
                }
            }

            *self.state.write().await = ConnectionState::Connected;
            info!("Connected to master, starting event forwarding");

            // Step 3: Run the main event forwarding + heartbeat loop
            let heartbeat_interval = Duration::from_secs(self.config.heartbeat_interval);
            let mut heartbeat_timer = tokio::time::interval(heartbeat_interval);
            let sync_interval = Duration::from_secs(self.config.sync_interval);
            let mut sync_timer = tokio::time::interval(sync_interval);
            let mut request_poll_timer = tokio::time::interval(Duration::from_secs(2));
            let install_state: SharedInstallState = handlers::new_install_state();

            let mut disconnected = false;

            while !disconnected {
                tokio::select! {
                    // Forward events from the bot to master
                    event = self.event_rx.recv() => {
                        match event {
                            Some(event) => {
                                let payload = EventPayload {
                                    event_type: format!("{}", event.event_type),
                                    timestamp: chrono::Utc::now(),
                                    client_id: event.client_id.map(|id| id as i64),
                                    target_id: event.target_id.map(|id| id as i64),
                                    data: serde_json::to_value(&event.data).unwrap_or_default(),
                                };

                                let batch = EventBatch {
                                    server_id,
                                    events: vec![payload.clone()],
                                };

                                match http_client
                                    .post(format!("{}/internal/events", base_url))
                                    .json(&batch)
                                    .send()
                                    .await
                                {
                                    Ok(_) => {}
                                    Err(e) => {
                                        warn!(error = %e, "Failed to send event to master, queueing");
                                        let _ = self.queue.enqueue(
                                            "event", None, "create", &payload
                                        ).await;
                                        disconnected = true;
                                    }
                                }
                            }
                            None => {
                                info!("Event channel closed, shutting down sync manager");
                                return Ok(());
                            }
                        }
                    }

                    // Periodic heartbeat
                    _ = heartbeat_timer.tick() => {
                        let hb = HeartbeatRequest {
                            server_id,
                            status: "online".to_string(),
                            current_map: None,  // TODO: get from game state
                            player_count: 0,    // TODO: get from clients manager
                            max_clients: 0,     // TODO: get from game state
                            build_hash: Some(env!("BUILD_HASH").to_string()),
                            version: Some(env!("CARGO_PKG_VERSION").to_string()),
                        };

                        match http_client
                            .post(format!("{}/internal/heartbeat", base_url))
                            .json(&hb)
                            .send()
                            .await
                        {
                            Ok(resp) => {
                                if let Ok(hb_resp) = resp.json::<HeartbeatResponse>().await {
                                    // Handle any pending global bans
                                    for ban in &hb_resp.pending_global_bans {
                                        let _ = self.command_tx.send(
                                            SyncMessage::GlobalPenalty(ban.clone())
                                        ).await;
                                    }

                                    // Check for config version mismatch
                                    if hb_resp.config_version > self.local_config_version {
                                        info!(
                                            local = self.local_config_version,
                                            remote = hb_resp.config_version,
                                            "Config version mismatch detected, pulling update"
                                        );
                                        match self.pull_and_apply_config(&http_client, &base_url, server_id).await {
                                            Ok(()) => {
                                                self.local_config_version = hb_resp.config_version;
                                                info!(version = hb_resp.config_version, "Config updated from master");
                                            }
                                            Err(e) => {
                                                warn!(error = %e, "Failed to pull config update from master");
                                            }
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                warn!(error = %e, "Heartbeat failed");
                                disconnected = true;
                            }
                        }
                    }

                    // Periodic sync (prune old queue entries)
                    _ = sync_timer.tick() => {
                        let _ = self.queue.prune(7).await;
                    }

                    // Poll master for pending requests (config scan, install, etc.)
                    _ = request_poll_timer.tick() => {
                        match http_client
                            .get(format!("{}/internal/requests/{}", base_url, server_id))
                            .send()
                            .await
                        {
                            Ok(resp) => {
                                if let Ok(poll_resp) = resp.json::<PendingRequestsResponse>().await {
                                    for item in poll_resp.requests {
                                        let response = match item.request {
                                            ClientRequest::ScanConfigFiles => {
                                                handlers::handle_scan_config_files().await
                                            }
                                            ClientRequest::ParseConfigFile { path } => {
                                                handlers::handle_parse_config_file(&path).await
                                            }
                                            ClientRequest::BrowseFiles { path } => {
                                                handlers::handle_browse_files(&path).await
                                            }
                                            ClientRequest::InstallGameServer { install_path } => {
                                                handlers::start_install_game_server(
                                                    install_path, install_state.clone(),
                                                );
                                                ClientResponse::InstallStarted
                                            }
                                            ClientRequest::InstallStatus => {
                                                handlers::handle_install_status(&install_state).await
                                            }
                                            ClientRequest::GetVersion => {
                                                handlers::handle_get_version().await
                                            }
                                            ClientRequest::ForceUpdate { update_url } => {
                                                match update_url {
                                                    Some(url) if !url.is_empty() => {
                                                        // Master supplies the URL; client always uses its own configured channel.
                                                        handlers::handle_force_update(url, self.update_channel.clone()).await
                                                    }
                                                    _ => ClientResponse::Error {
                                                        message: "Master did not supply an update URL".to_string(),
                                                    },
                                                }
                                            }
                                            ClientRequest::CheckGameLog { path } => {
                                                handlers::handle_check_game_log(&path).await
                                            }
                                        };

                                        let submission = ClientResponseSubmission {
                                            request_id: item.request_id,
                                            response,
                                        };

                                        if let Err(e) = http_client
                                            .post(format!("{}/internal/responses", base_url))
                                            .json(&submission)
                                            .send()
                                            .await
                                        {
                                            warn!(error = %e, "Failed to send request response to master");
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                debug!(error = %e, "Failed to poll for requests");
                            }
                        }
                    }
                }
            }

            // Disconnected — switch to offline mode
            *self.state.write().await = ConnectionState::Disconnected;
            warn!("Lost connection to master, entering offline mode");
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }

    async fn register(
        &self,
        client: &reqwest::Client,
        base_url: &str,
        fingerprint: &str,
    ) -> anyhow::Result<RegisterResponse> {
        let req = RegisterRequest {
            server_name: self.config.server_name.clone(),
            address: String::new(), // TODO: get from server config
            port: 0,                // TODO: get from server config
            cert_fingerprint: fingerprint.to_string(),
        };

        let resp = client
            .post(format!("{}/internal/register", base_url))
            .json(&req)
            .send()
            .await?;

        if !resp.status().is_success() {
            anyhow::bail!("Registration failed with status {}", resp.status());
        }

        let response = resp.json::<RegisterResponse>().await?;
        Ok(response)
    }

    /// Pull the latest config from master and apply it to the local TOML file.
    async fn pull_and_apply_config(
        &self,
        client: &reqwest::Client,
        base_url: &str,
        server_id: i64,
    ) -> anyhow::Result<()> {
        let resp = client
            .get(format!("{}/internal/config/{}", base_url, server_id))
            .send()
            .await?;

        if !resp.status().is_success() {
            anyhow::bail!("Config pull failed with status {}", resp.status());
        }

        let config_sync = resp.json::<ConfigSync>().await?;
        if config_sync.config_json.is_empty() {
            debug!("Config JSON is empty, nothing to apply");
            return Ok(());
        }

        let server_config: ServerConfigPayload =
            serde_json::from_str(&config_sync.config_json)?;

        // Read the current TOML config, update [server] section, and write back
        let config_path = &self.config_path;
        let content = std::fs::read_to_string(config_path)?;
        let mut doc: toml::Value = toml::from_str(&content)?;

        if let Some(server) = doc.get_mut("server") {
            if let Some(table) = server.as_table_mut() {
                table.insert(
                    "public_ip".to_string(),
                    toml::Value::String(server_config.address),
                );
                table.insert(
                    "port".to_string(),
                    toml::Value::Integer(server_config.port as i64),
                );
                table.insert(
                    "rcon_password".to_string(),
                    toml::Value::String(server_config.rcon_password),
                );
                if let Some(log) = server_config.game_log {
                    table.insert(
                        "game_log".to_string(),
                        toml::Value::String(log),
                    );
                } else {
                    table.remove("game_log");
                }
            }
        }

        let output = toml::to_string_pretty(&doc)?;
        std::fs::write(config_path, &output)?;
        info!(path = %config_path, "Config file updated on disk");

        // Signal the main loop that config has been updated
        let _ = self.config_updated_tx.send(true);

        Ok(())
    }
}
