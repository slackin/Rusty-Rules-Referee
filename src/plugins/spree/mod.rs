use async_trait::async_trait;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::info;

use crate::core::context::BotContext;
use crate::events::Event;
use crate::plugins::{Plugin, PluginInfo};

/// Tracks and announces killing sprees and ending sprees.
pub struct SpreePlugin {
    enabled: bool,
    /// Minimum kills for a spree announcement.
    min_spree: u32,
    /// Messages for spree milestones.
    spree_messages: HashMap<u32, String>,
    /// Track current spree count per client.
    spree_counts: RwLock<HashMap<i64, u32>>,
}

impl SpreePlugin {
    pub fn new() -> Self {
        let mut messages = HashMap::new();
        messages.insert(5, "^2{name} ^7is on a ^3KILLING SPREE ^7(5 kills)!".to_string());
        messages.insert(10, "^2{name} ^7is ^1UNSTOPPABLE ^7(10 kills)!".to_string());
        messages.insert(15, "^2{name} ^7is ^1GODLIKE ^7(15 kills)!".to_string());
        messages.insert(20, "^2{name} ^7is ^1LEGENDARY ^7(20 kills)!".to_string());

        Self {
            enabled: true,
            min_spree: 5,
            spree_messages: messages,
            spree_counts: RwLock::new(HashMap::new()),
        }
    }

    async fn get_spree(&self, client_id: i64) -> u32 {
        *self.spree_counts.read().await.get(&client_id).unwrap_or(&0)
    }
}

impl Default for SpreePlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Plugin for SpreePlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "spree",
            description: "Announces killing sprees",
            requires_config: true,
            requires_plugins: &[],
            requires_parsers: &[],
            requires_storage: &[],
            load_after: &[],
        }
    }

    async fn on_load_config(&mut self, settings: Option<&toml::Table>) -> anyhow::Result<()> {
        if let Some(s) = settings {
            if let Some(v) = s.get("min_spree").and_then(|v| v.as_integer()) {
                self.min_spree = v as u32;
            }
            if let Some(t) = s.get("spree_messages").and_then(|v| v.as_table()) {
                self.spree_messages.clear();
                for (key, val) in t {
                    if let (Ok(k), Some(v)) = (key.parse::<u32>(), val.as_str()) {
                        self.spree_messages.insert(k, v.to_string());
                    }
                }
            }
        }
        Ok(())
    }

    async fn on_startup(&mut self) -> anyhow::Result<()> {
        info!(min_spree = self.min_spree, "Spree plugin started");
        Ok(())
    }

    async fn on_event(&self, event: &Event, ctx: &BotContext) -> anyhow::Result<()> {
        if let Some(event_key) = ctx.event_registry.get_key(event.event_type) {
            match event_key {
                "EVT_CLIENT_KILL" => {
                    if let Some(killer_id) = event.client_id {
                        if let Some(victim_id) = event.target_id {
                            if killer_id == victim_id {
                                return Ok(());
                            }

                            // Increment killer spree
                            let count = {
                                let mut counts = self.spree_counts.write().await;
                                let entry = counts.entry(killer_id).or_insert(0);
                                *entry += 1;
                                *entry
                            };

                            // Check for spree milestones
                            if let Some(msg_template) = self.spree_messages.get(&count) {
                                let name = ctx.clients.get_by_cid(&killer_id.to_string()).await
                                    .map(|c| c.name.clone())
                                    .unwrap_or_else(|| format!("Player#{}", killer_id));
                                let msg = msg_template.replace("{name}", &name);
                                ctx.say(&msg).await?;
                            }

                            // End victim spree
                            let victim_spree = {
                                let mut counts = self.spree_counts.write().await;
                                counts.remove(&victim_id).unwrap_or(0)
                            };

                            if victim_spree >= self.min_spree {
                                let victim_name = ctx.clients.get_by_cid(&victim_id.to_string()).await
                                    .map(|c| c.name.clone())
                                    .unwrap_or_else(|| format!("Player#{}", victim_id));
                                let killer_name = ctx.clients.get_by_cid(&killer_id.to_string()).await
                                    .map(|c| c.name.clone())
                                    .unwrap_or_else(|| format!("Player#{}", killer_id));
                                ctx.say(&format!(
                                    "^2{} ^7ended ^2{}^7's killing spree ({} kills)!",
                                    killer_name, victim_name, victim_spree
                                )).await?;
                            }
                        }
                    }
                }

                "EVT_GAME_ROUND_START" => {
                    self.spree_counts.write().await.clear();
                }

                "EVT_CLIENT_DISCONNECT" => {
                    if let Some(cid) = event.client_id {
                        self.spree_counts.write().await.remove(&cid);
                    }
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
            "EVT_CLIENT_DISCONNECT".to_string(),
        ])
    }
}
