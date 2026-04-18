use async_trait::async_trait;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use tracing::info;

use crate::core::context::BotContext;
use crate::events::Event;
use crate::plugins::{Plugin, PluginInfo};

/// The Advertise plugin — rotates through a list of messages shown to all players.
pub struct AdvPlugin {
    enabled: AtomicBool,
    messages: Vec<String>,
    interval_secs: u64,
    current_index: AtomicUsize,
}

impl AdvPlugin {
    pub fn new() -> Self {
        Self {
            enabled: AtomicBool::new(true),
            messages: vec![
                "^2Welcome to the server!".to_string(),
                "^3Visit our website for more info".to_string(),
                "^7Type ^2!help ^7for a list of commands".to_string(),
                "^7Type ^2!register ^7to save your stats".to_string(),
            ],
            interval_secs: 120,
            current_index: AtomicUsize::new(0),
        }
    }
}

impl Default for AdvPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Plugin for AdvPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "adv",
            description: "Rotating server advertisement messages",
            requires_config: true,
            requires_plugins: &[],
            requires_parsers: &[],
            requires_storage: &[],
            load_after: &[],
        }
    }

    async fn on_startup(&mut self) -> anyhow::Result<()> {
        info!(
            count = self.messages.len(),
            interval = self.interval_secs,
            "Adv plugin started"
        );
        Ok(())
    }

    async fn on_event(&self, event: &Event, ctx: &BotContext) -> anyhow::Result<()> {
        // The adv plugin uses a timer-based approach.
        // In this implementation, we hook into game round starts to cycle messages.
        if let Some(event_key) = ctx.event_registry.get_key(event.event_type) {
            if event_key == "EVT_GAME_ROUND_START" {
                if !self.messages.is_empty() && self.is_enabled() {
                    let idx = self.current_index.fetch_add(1, Ordering::Relaxed) % self.messages.len();
                    ctx.say(&self.messages[idx]).await?;
                }
            }
        }
        Ok(())
    }

    fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    fn on_enable(&mut self) {
        self.enabled.store(true, Ordering::Relaxed);
    }

    fn on_disable(&mut self) {
        self.enabled.store(false, Ordering::Relaxed);
    }

    fn subscribed_events(&self) -> Option<Vec<String>> {
        Some(vec!["EVT_GAME_ROUND_START".to_string()])
    }
}
