use async_trait::async_trait;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::info;

use crate::core::context::BotContext;
use crate::events::Event;
use crate::plugins::{Plugin, PluginInfo};

/// Announces the first kill of each round.
pub struct FirstkillPlugin {
    enabled: bool,
    first_kill_announced: AtomicBool,
}

impl FirstkillPlugin {
    pub fn new() -> Self {
        Self {
            enabled: true,
            first_kill_announced: AtomicBool::new(false),
        }
    }
}

impl Default for FirstkillPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Plugin for FirstkillPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "firstkill",
            description: "Announces the first kill of each round",
            requires_config: false,
            requires_plugins: &[],
            requires_parsers: &[],
            requires_storage: &[],
            load_after: &[],
        }
    }

    async fn on_startup(&mut self) -> anyhow::Result<()> {
        info!("Firstkill plugin started");
        Ok(())
    }

    async fn on_event(&self, event: &Event, ctx: &BotContext) -> anyhow::Result<()> {
        if let Some(event_key) = ctx.event_registry.get_key(event.event_type) {
            match event_key {
                "EVT_CLIENT_KILL" => {
                    if self.first_kill_announced.compare_exchange(
                        false, true, Ordering::SeqCst, Ordering::SeqCst,
                    ).is_ok() {
                        if let Some(killer_id) = event.client_id {
                            if let Some(victim_id) = event.target_id {
                                if killer_id == victim_id {
                                    return Ok(());
                                }
                                let killer_name = ctx.clients.get_by_cid(&killer_id.to_string()).await
                                    .map(|c| c.name.clone())
                                    .unwrap_or_else(|| "Someone".to_string());
                                let victim_name = ctx.clients.get_by_cid(&victim_id.to_string()).await
                                    .map(|c| c.name.clone())
                                    .unwrap_or_else(|| "Someone".to_string());
                                ctx.say(&format!(
                                    "^3FIRST KILL: ^2{} ^7killed ^2{}",
                                    killer_name, victim_name
                                )).await?;
                            }
                        }
                    }
                }

                "EVT_GAME_ROUND_START" => {
                    self.first_kill_announced.store(false, Ordering::SeqCst);
                }

                _ => {}
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
            "EVT_CLIENT_KILL".to_string(),
            "EVT_GAME_ROUND_START".to_string(),
        ])
    }
}
