use std::sync::Arc;
use tokio::sync::broadcast;

use crate::config::RefereeConfig;
use crate::core::context::BotContext;
use crate::events::Event;
use crate::storage::Storage;

/// Shared state for all web handlers.
#[derive(Clone)]
pub struct AppState {
    pub ctx: Arc<BotContext>,
    pub config: RefereeConfig,
    pub config_path: String,
    pub jwt_secret: String,
    pub event_tx: broadcast::Sender<Event>,
    pub storage: Arc<dyn Storage>,
}
