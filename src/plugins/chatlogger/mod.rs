use async_trait::async_trait;
use chrono::Utc;
use std::path::PathBuf;
use tokio::sync::RwLock;
use tracing::info;

use crate::core::context::BotContext;
use crate::core::ChatMessage;
use crate::events::{Event, EventData};
use crate::plugins::{Plugin, PluginInfo};

/// The ChatLogger plugin — logs all chat messages to a file.
/// Messages are rotated daily with date-stamped filenames.
pub struct ChatLogPlugin {
    enabled: bool,
    log_dir: PathBuf,
    current_date: RwLock<String>,
}

impl ChatLogPlugin {
    pub fn new() -> Self {
        Self {
            enabled: true,
            log_dir: PathBuf::from("chat_logs"),
            current_date: RwLock::new(String::new()),
        }
    }

    pub fn with_log_dir(mut self, dir: PathBuf) -> Self {
        self.log_dir = dir;
        self
    }

    async fn append_log(&self, client_id: i64, channel: &str, message: &str) {
        let today = Utc::now().format("%Y-%m-%d").to_string();

        // Ensure log directory exists
        if let Err(e) = tokio::fs::create_dir_all(&self.log_dir).await {
            tracing::error!(error = %e, "Failed to create chat log directory");
            return;
        }

        let filename = self.log_dir.join(format!("chat_{}.log", today));
        let timestamp = Utc::now().format("%H:%M:%S").to_string();
        let line = format!("[{}] [{}] client#{}: {}\n", timestamp, channel, client_id, message);

        match tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&filename)
            .await
        {
            Ok(file) => {
                use tokio::io::AsyncWriteExt;
                let mut file = file;
                if let Err(e) = file.write_all(line.as_bytes()).await {
                    tracing::error!(error = %e, "Failed to write chat log");
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to open chat log file");
            }
        }
    }
}

impl Default for ChatLogPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Plugin for ChatLogPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "chatlogger",
            description: "Logs all chat messages to daily rotating files",
            requires_config: false,
            requires_plugins: &[],
            requires_parsers: &[],
            requires_storage: &[],
            load_after: &[],
        }
    }

    async fn on_load_config(&mut self, settings: Option<&toml::Table>) -> anyhow::Result<()> {
        if let Some(s) = settings {
            if let Some(v) = s.get("log_dir").and_then(|v| v.as_str()) {
                self.log_dir = std::path::PathBuf::from(v);
            }
        }
        Ok(())
    }

    async fn on_startup(&mut self) -> anyhow::Result<()> {
        *self.current_date.write().await = Utc::now().format("%Y-%m-%d").to_string();
        info!(log_dir = %self.log_dir.display(), "ChatLogger plugin started");
        Ok(())
    }

    async fn on_event(&self, event: &Event, ctx: &BotContext) -> anyhow::Result<()> {
        let Some(client_id) = event.client_id else {
            return Ok(());
        };

        if let EventData::Text(ref text) = event.data {
            let channel = ctx
                .event_registry
                .get_key(event.event_type)
                .unwrap_or("UNKNOWN");

            let channel_label = match channel {
                "EVT_CLIENT_SAY" => "SAY",
                "EVT_CLIENT_TEAM_SAY" => "TEAM",
                "EVT_CLIENT_PRIVATE_SAY" => "PM",
                _ => channel,
            };

            // File-based logging (existing behavior)
            self.append_log(client_id, channel_label, text).await;

            // DB-based logging for the web dashboard
            // client_id from the event is a game slot number, resolve to DB ID
            let (db_id, client_name) = match ctx.clients.get_by_cid(&client_id.to_string()).await {
                Some(c) => (c.id, c.name.clone()),
                None => (client_id, String::new()),
            };
            let msg = ChatMessage {
                id: 0,
                client_id: db_id,
                client_name,
                channel: channel_label.to_string(),
                message: text.clone(),
                time_add: Utc::now(),
            };
            if let Err(e) = ctx.storage.save_chat_message(&msg).await {
                tracing::warn!(error = %e, "Failed to persist chat message to DB");
            }
        }

        Ok(())
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn on_enable(&mut self) {
        self.enabled = true;
    }

    fn on_disable(&mut self) {
        self.enabled = false;
    }

    fn subscribed_events(&self) -> Option<Vec<String>> {
        Some(vec![
            "EVT_CLIENT_SAY".to_string(),
            "EVT_CLIENT_TEAM_SAY".to_string(),
            "EVT_CLIENT_PRIVATE_SAY".to_string(),
        ])
    }
}
