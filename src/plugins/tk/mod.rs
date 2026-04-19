use async_trait::async_trait;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::core::context::BotContext;
use crate::events::{Event, EventData};
use crate::plugins::{Plugin, PluginInfo};

#[derive(Debug, Default)]
struct TkRecord {
    team_kills: u32,
    team_damage: f32,
}

/// The TK (Team Kill) plugin — monitors and penalizes team killing.
pub struct TkPlugin {
    enabled: bool,
    max_team_kills: u32,
    max_team_damage: f32,
    /// Per-client TK tracking. Interior mutability via RwLock.
    records: RwLock<HashMap<i64, TkRecord>>,
}

impl TkPlugin {
    pub fn new() -> Self {
        Self {
            enabled: true,
            max_team_kills: 5,
            max_team_damage: 300.0,
            records: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for TkPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Plugin for TkPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "tk",
            description: "Monitors and penalizes team killing / team damage",
            requires_config: true,
            requires_plugins: &["admin"],
            requires_parsers: &[],
            requires_storage: &[],
            load_after: &["admin"],
        }
    }

    async fn on_load_config(&mut self, settings: Option<&toml::Table>) -> anyhow::Result<()> {
        if let Some(s) = settings {
            if let Some(v) = s.get("max_team_kills").and_then(|v| v.as_integer()) {
                self.max_team_kills = v as u32;
            }
            if let Some(v) = s.get("max_team_damage").and_then(|v| v.as_float()) {
                self.max_team_damage = v as f32;
            }
        }
        Ok(())
    }

    async fn on_startup(&mut self) -> anyhow::Result<()> {
        info!(
            max_tk = self.max_team_kills,
            max_td = self.max_team_damage,
            "TK plugin started"
        );
        Ok(())
    }

    async fn on_event(&self, event: &Event, ctx: &BotContext) -> anyhow::Result<()> {
        // Check if this is a round start event — reset records
        if let Some(key) = ctx.event_registry.get_key(event.event_type) {
            if key == "EVT_GAME_ROUND_START" {
                let mut records = self.records.write().await;
                records.clear();
                info!("TK records cleared for new round");
                return Ok(());
            }
        }

        if let EventData::Kill { damage, .. } = &event.data {
                let Some(attacker_id) = event.client_id else {
                    return Ok(());
                };

                let mut records = self.records.write().await;
                let record = records.entry(attacker_id).or_default();

                // Determine if this is a kill or damage event
                if let Some(key) = ctx.event_registry.get_key(event.event_type) {
                    if key == "EVT_CLIENT_KILL_TEAM" {
                        record.team_kills += 1;
                        record.team_damage += damage;
                        info!(
                            attacker = attacker_id,
                            tk_count = record.team_kills,
                            "Team kill recorded"
                        );

                        if record.team_kills >= self.max_team_kills {
                            warn!(attacker = attacker_id, kills = record.team_kills, "TK limit exceeded");
                            drop(records);
                            ctx.message(
                                &attacker_id.to_string(),
                                &format!("^1You have been kicked for {} team kills", self.max_team_kills),
                            ).await?;
                            ctx.kick(&attacker_id.to_string(), "Too many team kills").await?;
                        }
                    } else if key == "EVT_CLIENT_DAMAGE_TEAM" {
                        record.team_damage += damage;

                        if record.team_damage >= self.max_team_damage {
                            warn!(attacker = attacker_id, damage = record.team_damage, "Team damage limit exceeded");
                            drop(records);
                            ctx.message(
                                &attacker_id.to_string(),
                                "^1You have been kicked for excessive team damage",
                            ).await?;
                            ctx.kick(&attacker_id.to_string(), "Excessive team damage").await?;
                        }
                    }
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
            "EVT_CLIENT_KILL_TEAM".to_string(),
            "EVT_CLIENT_DAMAGE_TEAM".to_string(),
            "EVT_GAME_ROUND_START".to_string(),
        ])
    }
}
