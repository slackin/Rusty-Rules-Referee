use async_trait::async_trait;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::core::context::BotContext;
use crate::core::Team;
use crate::events::Event;
use crate::plugins::{Plugin, PluginInfo};

const DEFAULT_MAX_SPEC_TIME: i64 = 300; // 5 minutes
const DEFAULT_MIN_PLAYERS: usize = 8; // Only enforce when server is busy
const DEFAULT_WARN_INTERVAL: i64 = 60; // Warn every 60 seconds before kicking
const LEVEL_IMMUNE: u32 = 20; // Mods and above are immune

/// The SpecChecker plugin — kicks idle spectators when the server is busy.
///
/// Features:
/// - Tracks how long players have been spectating
/// - Warns players before kicking
/// - Only enforces when player count exceeds a threshold
/// - Admins/mods above a configurable level are immune
/// - Resets timer when a player joins a team
pub struct SpecCheckerPlugin {
    enabled: bool,
    /// Maximum time (seconds) a player can spectate before being kicked.
    max_spec_time: i64,
    /// Minimum connected players before enforcement kicks in.
    min_players: usize,
    /// How often to warn spectators (seconds).
    warn_interval: i64,
    /// Level at which players are immune to spec kicks.
    immune_level: u32,
    /// Per-client spectate tracking: client_id -> (spec_start_timestamp, last_warned_timestamp)
    spec_tracking: RwLock<HashMap<i64, (i64, i64)>>,
}

impl SpecCheckerPlugin {
    pub fn new() -> Self {
        Self {
            enabled: true,
            max_spec_time: DEFAULT_MAX_SPEC_TIME,
            min_players: DEFAULT_MIN_PLAYERS,
            warn_interval: DEFAULT_WARN_INTERVAL,
            immune_level: LEVEL_IMMUNE,
            spec_tracking: RwLock::new(HashMap::new()),
        }
    }

    /// Check all spectators and warn/kick as needed.
    /// Called on round events and team changes.
    async fn check_spectators(&self, ctx: &BotContext) -> anyhow::Result<()> {
        let player_count = ctx.clients.count().await;
        if player_count < self.min_players {
            return Ok(());
        }

        let now = chrono::Utc::now().timestamp();
        let all_clients = ctx.clients.get_all().await;
        let mut tracking = self.spec_tracking.write().await;

        for client in &all_clients {
            if client.team != Team::Spectator {
                // Not spectating — remove from tracking
                tracking.remove(&client.id);
                continue;
            }

            // Check immunity
            if client.max_level() >= self.immune_level {
                continue;
            }

            let cid = match &client.cid {
                Some(c) => c.clone(),
                None => continue,
            };

            let entry = tracking.entry(client.id).or_insert((now, 0));
            let spec_duration = now - entry.0;

            if spec_duration >= self.max_spec_time {
                // Kick
                warn!(player = %client.name, duration = spec_duration, "SpecChecker kicking idle spectator");
                ctx.message(&cid, "^1You have been kicked for spectating too long on a full server").await?;
                ctx.kick(&cid, "Idle spectator (server full)").await?;
                tracking.remove(&client.id);
            } else if spec_duration > self.warn_interval && now - entry.1 >= self.warn_interval {
                // Warn
                let remaining = self.max_spec_time - spec_duration;
                ctx.message(
                    &cid,
                    &format!(
                        "^3WARNING: ^7You will be kicked for spectating in ^1{} ^7seconds. Join a team!",
                        remaining
                    ),
                ).await?;
                entry.1 = now;
            }
        }

        Ok(())
    }
}

impl Default for SpecCheckerPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Plugin for SpecCheckerPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "specchecker",
            description: "Kicks idle spectators when the server is busy to free slots for active players",
            requires_config: false,
            requires_plugins: &[],
            requires_parsers: &["iourt43"],
            requires_storage: &[],
            load_after: &["admin"],
        }
    }
    async fn on_load_config(&mut self, settings: Option<&toml::Table>) -> anyhow::Result<()> {
        if let Some(s) = settings {
            if let Some(v) = s.get("max_spec_time").and_then(|v| v.as_integer()) {
                self.max_spec_time = v;
            }
            if let Some(v) = s.get("min_players").and_then(|v| v.as_integer()) {
                self.min_players = v as usize;
            }
            if let Some(v) = s.get("warn_interval").and_then(|v| v.as_integer()) {
                self.warn_interval = v;
            }
            if let Some(v) = s.get("immune_level").and_then(|v| v.as_integer()) {
                self.immune_level = v as u32;
            }
        }
        Ok(())
    }
    async fn on_startup(&mut self) -> anyhow::Result<()> {
        info!(
            "SpecChecker plugin started — max_spec_time={}s, min_players={}, immune_level={}",
            self.max_spec_time, self.min_players, self.immune_level
        );
        Ok(())
    }

    async fn on_event(&self, event: &Event, ctx: &BotContext) -> anyhow::Result<()> {
        let event_key = ctx.event_registry.get_key(event.event_type).unwrap_or("");

        match event_key {
            // When a player joins a team, reset their spec timer
            "EVT_CLIENT_TEAM_CHANGE" | "EVT_CLIENT_TEAM_CHANGE2" | "EVT_CLIENT_JOIN" => {
                if let Some(client_id) = event.client_id {
                    let client = ctx.clients.get_by_id(client_id).await;
                    if let Some(c) = client {
                        if c.team == Team::Spectator {
                            // Player moved to spec — start tracking
                            let now = chrono::Utc::now().timestamp();
                            self.spec_tracking.write().await.insert(client_id, (now, 0));
                        } else {
                            // Player joined a team — stop tracking
                            self.spec_tracking.write().await.remove(&client_id);
                        }
                    }
                }
                // Run check on all spectators
                self.check_spectators(ctx).await?;
            }
            // Periodic checks on round events
            "EVT_GAME_ROUND_START" | "EVT_GAME_ROUND_END" => {
                self.check_spectators(ctx).await?;
            }
            // Clean up on disconnect
            "EVT_CLIENT_DISCONNECT" => {
                if let Some(client_id) = event.client_id {
                    self.spec_tracking.write().await.remove(&client_id);
                }
            }
            // Also check on new connections (server getting fuller)
            "EVT_CLIENT_AUTH" => {
                self.check_spectators(ctx).await?;
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
            "EVT_CLIENT_TEAM_CHANGE".to_string(),
            "EVT_CLIENT_TEAM_CHANGE2".to_string(),
            "EVT_CLIENT_JOIN".to_string(),
            "EVT_CLIENT_AUTH".to_string(),
            "EVT_CLIENT_DISCONNECT".to_string(),
            "EVT_GAME_ROUND_START".to_string(),
            "EVT_GAME_ROUND_END".to_string(),
        ])
    }
}
