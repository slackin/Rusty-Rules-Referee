use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

use crate::config::RefereeConfig;
use crate::core::context::BotContext;
use crate::events::Event;
use crate::storage::Storage;
use crate::sync::master::ConnectedClient;

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
