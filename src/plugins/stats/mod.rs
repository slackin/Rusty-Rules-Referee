use async_trait::async_trait;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::info;

use crate::core::context::BotContext;
use crate::events::{Event, EventData};
use crate::plugins::{Plugin, PluginInfo};

/// Per-player statistics.
#[derive(Debug, Clone, Default)]
struct PlayerStats {
    kills: u64,
    deaths: u64,
    team_kills: u64,
    damage_dealt: f64,
    damage_received: f64,
}

impl PlayerStats {
    fn kd_ratio(&self) -> f64 {
        if self.deaths == 0 {
            self.kills as f64
        } else {
            self.kills as f64 / self.deaths as f64
        }
    }
}

/// The Stats plugin — tracks player kill/death statistics.
/// Provides !stats and !topstats commands.
pub struct StatsPlugin {
    enabled: bool,
    stats: RwLock<HashMap<i64, PlayerStats>>,
}

impl StatsPlugin {
    pub fn new() -> Self {
        Self {
            enabled: true,
            stats: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for StatsPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Plugin for StatsPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "stats",
            description: "Tracks player kill/death statistics with !stats and !topstats commands",
            requires_config: false,
            requires_plugins: &[],
            requires_parsers: &[],
            requires_storage: &[],
            load_after: &["admin"],
        }
    }

    async fn on_startup(&mut self) -> anyhow::Result<()> {
        info!("Stats plugin started");
        Ok(())
    }

    async fn on_event(&self, event: &Event, ctx: &BotContext) -> anyhow::Result<()> {
        let event_key = ctx.event_registry.get_key(event.event_type).unwrap_or("");

        match event_key {
            "EVT_CLIENT_KILL" | "EVT_CLIENT_KILL_TEAM" => {
                if let EventData::Kill { damage, .. } = &event.data {
                    if let Some(attacker_id) = event.client_id {
                        let mut stats = self.stats.write().await;
                        let attacker = stats.entry(attacker_id).or_default();
                        attacker.kills += 1;
                        attacker.damage_dealt += *damage as f64;

                        if event_key == "EVT_CLIENT_KILL_TEAM" {
                            attacker.team_kills += 1;
                        }
                    }

                    if let Some(victim_id) = event.target_id {
                        let mut stats = self.stats.write().await;
                        let victim = stats.entry(victim_id).or_default();
                        victim.deaths += 1;
                        victim.damage_received += *damage as f64;
                    }
                }
            }
            "EVT_CLIENT_SAY" | "EVT_CLIENT_TEAM_SAY" => {
                if let EventData::Text(ref text) = event.data {
                    let Some(client_id) = event.client_id else {
                        return Ok(());
                    };

                    if text.starts_with("!stats") {
                        let stats = self.stats.read().await;
                        if let Some(ps) = stats.get(&client_id) {
                            ctx.message(
                                &client_id.to_string(),
                                &format!(
                                    "^3Stats: ^7Kills: {} Deaths: {} K/D: {:.2} TK: {}",
                                    ps.kills, ps.deaths, ps.kd_ratio(), ps.team_kills
                                ),
                            )
                            .await?;
                        } else {
                            ctx.message(&client_id.to_string(), "^7No stats recorded yet.")
                                .await?;
                        }
                    } else if text.starts_with("!topstats") {
                        let stats = self.stats.read().await;
                        let mut sorted: Vec<_> = stats.iter().collect();
                        sorted.sort_by(|a, b| b.1.kills.cmp(&a.1.kills));

                        let top: Vec<String> = sorted
                            .iter()
                            .take(5)
                            .enumerate()
                            .map(|(i, (id, ps))| {
                                format!(
                                    "^3{}. ^7#{} - K:{} D:{} K/D:{:.2}",
                                    i + 1,
                                    id,
                                    ps.kills,
                                    ps.deaths,
                                    ps.kd_ratio()
                                )
                            })
                            .collect();

                        if top.is_empty() {
                            ctx.message(&client_id.to_string(), "^7No stats recorded yet.")
                                .await?;
                        } else {
                            ctx.say(&format!("^3Top Players: {}", top.join(" | ")))
                                .await?;
                        }
                    }
                }
            }
            _ => {}
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
            "EVT_CLIENT_KILL_TEAM".to_string(),
            "EVT_CLIENT_SAY".to_string(),
            "EVT_CLIENT_TEAM_SAY".to_string(),
        ])
    }
}
