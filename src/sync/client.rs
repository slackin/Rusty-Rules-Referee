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
    storage: Arc<dyn Storage>,
    queue: SyncQueue,
    server_id: Arc<RwLock<Option<i64>>>,
    state: Arc<RwLock<ConnectionState>>,
    /// Channel to receive events from the main bot loop for forwarding.
    event_rx: mpsc::Receiver<Event>,
    /// Channel to receive commands from the master.
    command_tx: mpsc::Sender<SyncMessage>,
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
}

impl ClientSyncManager {
    /// Create a new sync manager and return the handle for the main loop.
    pub fn new(
        config: ClientSection,
        storage: Arc<dyn Storage>,
    ) -> (Self, SyncHandle) {
        let (event_tx, event_rx) = mpsc::channel::<Event>(1024);
        let (command_tx, command_rx) = mpsc::channel::<SyncMessage>(256);
        let state = Arc::new(RwLock::new(ConnectionState::Disconnected));
        let server_id = Arc::new(RwLock::new(None));
        let queue = SyncQueue::new(storage.clone(), None);

        let manager = Self {
            config,
            storage,
            queue,
            server_id: server_id.clone(),
            state: state.clone(),
            event_rx,
            command_tx,
        };

        let handle = SyncHandle {
            event_tx,
            command_rx,
            state,
            server_id,
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
                    info!(server_id = response.server_id, "Registered with master");
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
}
