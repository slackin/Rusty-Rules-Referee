use async_trait::async_trait;
use std::collections::HashMap;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::core::context::BotContext;
use crate::events::{Event, EventData};
use crate::plugins::{Plugin, PluginInfo};

struct MessageHistory {
    timestamps: Vec<Instant>,
    last_message: Option<String>,
    repeat_count: u32,
}

/// The SpamControl plugin — prevents players from flooding chat.
pub struct SpamControlPlugin {
    enabled: bool,
    max_messages: u32,
    time_window_secs: u64,
    max_repeats: u32,
    /// Per-client tracking (keyed by client CID). Interior mutability via RwLock.
    history: RwLock<HashMap<i64, MessageHistory>>,
}

impl SpamControlPlugin {
    pub fn new() -> Self {
        Self {
            enabled: true,
            max_messages: 5,
            time_window_secs: 10,
            max_repeats: 3,
            history: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for SpamControlPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Plugin for SpamControlPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "spamcontrol",
            description: "Prevents chat flooding and message repetition",
            requires_config: true,
            requires_plugins: &["admin"],
            requires_parsers: &[],
            requires_storage: &[],
            load_after: &["admin"],
        }
    }

    async fn on_startup(&mut self) -> anyhow::Result<()> {
        info!(
            max_messages = self.max_messages,
            window_secs = self.time_window_secs,
            "SpamControl plugin started"
        );
        Ok(())
    }

    async fn on_event(&self, event: &Event, ctx: &BotContext) -> anyhow::Result<()> {
        let Some(client_id) = event.client_id else {
            return Ok(());
        };

        if let EventData::Text(ref text) = event.data {
            let now = Instant::now();
            let mut history = self.history.write().await;
            let entry = history.entry(client_id).or_insert_with(|| MessageHistory {
                timestamps: Vec::new(),
                last_message: None,
                repeat_count: 0,
            });

            // Prune old timestamps outside the window
            let cutoff = now - std::time::Duration::from_secs(self.time_window_secs);
            entry.timestamps.retain(|t| *t > cutoff);
            entry.timestamps.push(now);

            // Check flood (too many messages in time window)
            if entry.timestamps.len() > self.max_messages as usize {
                warn!(client = client_id, count = entry.timestamps.len(), "Spam detected: flooding");
                drop(history); // release lock before async call
                ctx.message(&client_id.to_string(), "^1Spam warning: slow down!").await?;
                return Ok(());
            }

            // Check repeats
            if entry.last_message.as_deref() == Some(text) {
                entry.repeat_count += 1;
                if entry.repeat_count >= self.max_repeats {
                    warn!(client = client_id, repeats = entry.repeat_count, "Spam detected: repeated message");
                    entry.repeat_count = 0;
                    drop(history);
                    ctx.message(&client_id.to_string(), "^1Spam warning: stop repeating!").await?;
                    return Ok(());
                }
            } else {
                entry.last_message = Some(text.clone());
                entry.repeat_count = 1;
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
        ])
    }
}
