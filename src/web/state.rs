use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, oneshot, RwLock};

use crate::config::RefereeConfig;
use crate::core::context::BotContext;
use crate::events::Event;
use crate::storage::Storage;
use crate::sync::master::{ClientVersionInfo, ConnectedClient, ConnectedHub, HubActionLog};
use crate::sync::protocol::{ClientRequest, ClientResponse, GameServerWizardParams, HubAction, HubResponse};

/// Shared state for all web handlers.
#[derive(Clone)]
pub struct AppState {
    /// Bot context — present in standalone/client modes, absent in master mode.
    pub ctx: Option<Arc<BotContext>>,
    pub config: RefereeConfig,
    pub config_path: String,
    pub jwt_secret: String,
    pub event_tx: broadcast::Sender<Event>,
    pub storage: Arc<dyn Storage>,
    /// Connected game-server bots — only populated in master mode.
    pub connected_clients: Option<Arc<RwLock<HashMap<i64, ConnectedClient>>>>,
    /// Pending request/response correlations (master mode only).
    pub pending_responses: Option<Arc<RwLock<HashMap<String, oneshot::Sender<ClientResponse>>>>>,
    /// Pending requests queued for client bots (master mode only).
    pub pending_client_requests: Option<Arc<RwLock<HashMap<i64, Vec<(String, ClientRequest)>>>>>,
    /// Last-reported client versions keyed by server_id (master mode only).
    pub client_versions: Option<Arc<RwLock<HashMap<i64, ClientVersionInfo>>>>,
    /// Connected hubs (master mode only).
    pub connected_hubs: Option<Arc<RwLock<HashMap<i64, ConnectedHub>>>>,
    /// Pending hub actions queued by master, polled by hubs (master mode only).
    pub pending_hub_actions: Option<Arc<RwLock<HashMap<i64, Vec<(String, HubAction)>>>>>,
    /// Pending hub action responses (master mode only).
    pub pending_hub_responses: Option<Arc<RwLock<HashMap<String, oneshot::Sender<HubResponse>>>>>,
    /// Last-reported hub versions keyed by hub_id (master mode only).
    pub hub_versions: Option<Arc<RwLock<HashMap<i64, ClientVersionInfo>>>>,
    /// In-memory per-action progress log (master mode only).
    pub hub_action_logs: Option<Arc<RwLock<HashMap<String, HubActionLog>>>>,
    /// Wizard params most recently submitted to `/wizard/install` per server,
    /// stashed so the install_status auto-persist can reconstruct the
    /// `ServerConfigPayload` even when the client bot is still on an older
    /// build that doesn't populate the extended `InstallComplete` fields
    /// (master mode only).
    pub pending_wizard_params: Option<Arc<RwLock<HashMap<i64, GameServerWizardParams>>>>,
}

impl AppState {
    /// Returns true if running in master mode.
    pub fn is_master(&self) -> bool {
        self.config.master.is_some()
    }

    /// Get the BotContext, or return 503 if in master mode (no local game server).
    pub fn require_ctx(&self) -> Result<&Arc<BotContext>, axum::http::StatusCode> {
        self.ctx.as_ref().ok_or(axum::http::StatusCode::SERVICE_UNAVAILABLE)
    }
}
