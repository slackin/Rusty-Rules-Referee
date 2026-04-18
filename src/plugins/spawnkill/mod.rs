use async_trait::async_trait;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::info;

use crate::core::context::BotContext;
use crate::events::Event;
use crate::plugins::{Plugin, PluginInfo};

/// Detects and punishes spawn killing.
pub struct SpawnkillPlugin {
    enabled: bool,
    /// Seconds after spawn during which kills count as spawn kills.
    grace_period_secs: u64,
    /// Maximum spawn kills before action is taken.
    max_spawnkills: u32,
    /// Action to take: "warn", "kick", "tempban".
    action: String,
    /// Tempban duration in minutes (if action is tempban).
    tempban_duration: u32,
    /// Track spawn times: client_id -> last_spawn_timestamp.
    spawn_times: RwLock<HashMap<i64, i64>>,
    /// Track spawn kill counts: killer_id -> count this round.
    spawnkill_counts: RwLock<HashMap<i64, u32>>,
}

impl SpawnkillPlugin {
    pub fn new() -> Self {
        Self {
            enabled: true,
            grace_period_secs: 3,
            max_spawnkills: 3,
            action: "warn".to_string(),
            tempban_duration: 5,
            spawn_times: RwLock::new(HashMap::new()),
            spawnkill_counts: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for SpawnkillPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Plugin for SpawnkillPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "spawnkill",
            description: "Detects and punishes spawn killing",
            requires_config: true,
            requires_plugins: &[],
            requires_parsers: &["iourt43"],
            requires_storage: &[],
            load_after: &[],
        }
    }

    async fn on_startup(&mut self) -> anyhow::Result<()> {
        info!(
            grace_period = self.grace_period_secs,
            max = self.max_spawnkills,
            action = %self.action,
            "Spawnkill plugin started"
        );
        Ok(())
    }

    async fn on_event(&self, event: &Event, ctx: &BotContext) -> anyhow::Result<()> {
        if let Some(event_key) = ctx.event_registry.get_key(event.event_type) {
            match event_key {
                "EVT_CLIENT_SPAWN" => {
                    if let Some(client_id) = event.client_id {
                        let now = chrono::Utc::now().timestamp();
                        self.spawn_times.write().await.insert(client_id, now);
                    }
                }

                "EVT_CLIENT_KILL" => {
                    if let Some(killer_id) = event.client_id {
                        if let Some(victim_id) = event.target_id {
                            if killer_id == victim_id {
                                return Ok(());
                            }

                            let now = chrono::Utc::now().timestamp();
                            let spawn_map = self.spawn_times.read().await;

                            if let Some(&spawn_time) = spawn_map.get(&victim_id) {
                                if now - spawn_time <= self.grace_period_secs as i64 {
                                    drop(spawn_map);

                                    let mut counts = self.spawnkill_counts.write().await;
                                    let count = counts.entry(killer_id).or_insert(0);
                                    *count += 1;
                                    let current = *count;
                                    drop(counts);

                                    let killer_cid = killer_id.to_string();

                                    if current >= self.max_spawnkills {
                                        match self.action.as_str() {
                                            "kick" => {
                                                ctx.kick(&killer_cid, "Too many spawn kills").await?;
                                                ctx.say(&format!("^2{} ^7was kicked for spawn killing", killer_cid)).await?;
                                            }
                                            "tempban" => {
                                                ctx.kick(&killer_cid, "Spawn killing").await?;
                                                // Save penalty through admin plugin ideally, but for now just kick
                                            }
                                            _ => {
                                                ctx.message(&killer_cid, "^1WARNING: ^7Stop spawn killing!").await?;
                                            }
                                        }
                                    } else {
                                        ctx.message(&killer_cid, "^3Please avoid spawn killing").await?;
                                    }
                                }
                            }
                        }
                    }
                }

                "EVT_GAME_ROUND_START" => {
                    // Reset counts each round
                    self.spawnkill_counts.write().await.clear();
                    self.spawn_times.write().await.clear();
                }

                "EVT_CLIENT_DISCONNECT" => {
                    if let Some(cid) = event.client_id {
                        self.spawn_times.write().await.remove(&cid);
                        self.spawnkill_counts.write().await.remove(&cid);
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
            "EVT_CLIENT_SPAWN".to_string(),
            "EVT_CLIENT_KILL".to_string(),
            "EVT_GAME_ROUND_START".to_string(),
            "EVT_CLIENT_DISCONNECT".to_string(),
        ])
    }
}
