use async_trait::async_trait;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::core::context::BotContext;
use crate::events::{Event, EventData};
use crate::plugins::{Plugin, PluginInfo};

const DEFAULT_WARN_HS_RATIO: f64 = 0.70; // 70% headshot ratio triggers warning
const DEFAULT_BAN_HS_RATIO: f64 = 0.85; // 85% headshot ratio triggers auto-tempban
const DEFAULT_MIN_KILLS: u32 = 15; // Minimum kills before checking ratio
const DEFAULT_BAN_DURATION: u32 = 60; // 60 minute tempban
const DEFAULT_ANNOUNCE_INTERVAL: u32 = 10; // Announce headshot streaks every N headshots

/// Per-player headshot tracking data.
#[derive(Debug, Clone, Default)]
struct PlayerStats {
    kills: u32,
    headshots: u32,
    headshot_streak: u32,
    best_streak: u32,
    warned: bool,
}

impl PlayerStats {
    fn hs_ratio(&self) -> f64 {
        if self.kills == 0 {
            return 0.0;
        }
        self.headshots as f64 / self.kills as f64
    }
}

/// The HeadshotCounter plugin — tracks headshots and detects aimbot behavior.
///
/// Features:
/// - Counts headshots per player per map
/// - Announces headshot streaks to the server
/// - Warns players with suspicious headshot ratios
/// - Auto-tempbans players exceeding aimbot threshold
/// - Resets on map change
/// - Players can check their stats with !hs
pub struct HeadshotCounterPlugin {
    enabled: bool,
    /// Headshot ratio threshold for warning.
    warn_ratio: f64,
    /// Headshot ratio threshold for auto-tempban.
    ban_ratio: f64,
    /// Minimum kills before ratio checks apply.
    min_kills: u32,
    /// Tempban duration (minutes).
    ban_duration: u32,
    /// Announce headshot streaks every N headshots.
    announce_interval: u32,
    /// Per-client stats: client_id -> PlayerStats
    stats: RwLock<HashMap<i64, PlayerStats>>,
}

impl HeadshotCounterPlugin {
    pub fn new() -> Self {
        Self {
            enabled: true,
            warn_ratio: DEFAULT_WARN_HS_RATIO,
            ban_ratio: DEFAULT_BAN_HS_RATIO,
            min_kills: DEFAULT_MIN_KILLS,
            ban_duration: DEFAULT_BAN_DURATION,
            announce_interval: DEFAULT_ANNOUNCE_INTERVAL,
            stats: RwLock::new(HashMap::new()),
        }
    }

    fn is_headshot(hit_location: &str) -> bool {
        let loc = hit_location.to_uppercase();
        loc == "HEAD" || loc == "HELMET" || loc == "1"
    }

    async fn handle_kill(&self, event: &Event, ctx: &BotContext) -> anyhow::Result<()> {
        let client_id = match event.client_id {
            Some(id) => id,
            None => return Ok(()),
        };

        let is_hs = match &event.data {
            EventData::Kill { hit_location, .. } => Self::is_headshot(hit_location),
            _ => return Ok(()),
        };

        let mut stats_map = self.stats.write().await;
        let stats = stats_map.entry(client_id).or_default();
        stats.kills += 1;

        if is_hs {
            stats.headshots += 1;
            stats.headshot_streak += 1;
            if stats.headshot_streak > stats.best_streak {
                stats.best_streak = stats.headshot_streak;
            }

            // Announce streaks
            if stats.headshot_streak > 0 && stats.headshot_streak % self.announce_interval == 0 {
                let client = ctx.clients.get_by_id(client_id).await;
                if let Some(c) = client {
                    ctx.say(&format!(
                        "^2{} ^7is on a ^1{} ^7headshot streak! ({}/{} = {:.0}% HS)",
                        c.name, stats.headshot_streak, stats.headshots, stats.kills,
                        stats.hs_ratio() * 100.0
                    )).await?;
                }
            }
        } else {
            stats.headshot_streak = 0;
        }

        // Check for suspicious ratio after minimum kills
        if stats.kills >= self.min_kills {
            let ratio = stats.hs_ratio();
            let client = ctx.clients.get_by_id(client_id).await;

            if ratio >= self.ban_ratio {
                if let Some(c) = client {
                    if let Some(ref cid) = c.cid {
                        warn!(
                            player = %c.name, ratio = %format!("{:.0}%", ratio * 100.0),
                            kills = stats.kills, headshots = stats.headshots,
                            "HeadshotCounter auto-tempbanning for suspected aimbot"
                        );
                        ctx.say(&format!(
                            "^1ALERT: ^2{} ^7has been auto-banned — {:.0}% headshot ratio ({}/{} kills)",
                            c.name, ratio * 100.0, stats.headshots, stats.kills
                        )).await?;
                        ctx.temp_ban(
                            cid,
                            &format!("Suspected aimbot ({:.0}% headshot ratio)", ratio * 100.0),
                            self.ban_duration,
                        ).await?;
                        stats_map.remove(&client_id);
                    }
                }
            } else if ratio >= self.warn_ratio && !stats.warned {
                stats.warned = true;
                if let Some(c) = client {
                    if let Some(ref cid) = c.cid {
                        warn!(
                            player = %c.name, ratio = %format!("{:.0}%", ratio * 100.0),
                            "HeadshotCounter warning player for high headshot ratio"
                        );
                        ctx.message(
                            cid,
                            &format!(
                                "^3WARNING: ^7Your headshot ratio ({:.0}%) is being monitored",
                                ratio * 100.0
                            ),
                        ).await?;
                    }
                }
            }
        }

        Ok(())
    }

    async fn handle_command(&self, event: &Event, ctx: &BotContext) -> anyhow::Result<()> {
        let client_id = match event.client_id {
            Some(id) => id,
            None => return Ok(()),
        };

        let text = match &event.data {
            EventData::Text(t) => t.as_str(),
            _ => return Ok(()),
        };

        if !text.starts_with("!hs") && !text.starts_with("!headshots") {
            return Ok(());
        }

        let client = ctx.clients.get_by_id(client_id).await;
        let cid = match client.as_ref().and_then(|c| c.cid.as_ref()) {
            Some(c) => c.clone(),
            None => return Ok(()),
        };

        let args = text.splitn(2, ' ').nth(1).unwrap_or("").trim();

        // Check if looking up another player
        let target_id = if args.is_empty() {
            client_id
        } else {
            let matches = ctx.clients.find_by_name(args).await;
            if matches.len() == 1 {
                matches[0].id
            } else {
                ctx.message(&cid, &format!("No single player found matching '{}'", args)).await?;
                return Ok(());
            }
        };

        let stats_map = self.stats.read().await;
        if let Some(stats) = stats_map.get(&target_id) {
            let target_name = ctx.clients.get_by_id(target_id).await
                .map(|c| c.name.clone())
                .unwrap_or_else(|| "?".to_string());
            ctx.message(
                &cid,
                &format!(
                    "^7{}: ^2{} ^7kills, ^1{} ^7headshots ({:.0}%), best streak: ^3{}",
                    target_name, stats.kills, stats.headshots,
                    stats.hs_ratio() * 100.0, stats.best_streak
                ),
            ).await?;
        } else {
            ctx.message(&cid, "^7No headshot data recorded yet").await?;
        }

        Ok(())
    }
}

impl Default for HeadshotCounterPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Plugin for HeadshotCounterPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "headshotcounter",
            description: "Tracks headshots per player, announces streaks, and auto-bans suspected aimbots",
            requires_config: false,
            requires_plugins: &[],
            requires_parsers: &["iourt43"],
            requires_storage: &[],
            load_after: &["admin"],
        }
    }
    async fn on_load_config(&mut self, settings: Option<&toml::Table>) -> anyhow::Result<()> {
        if let Some(s) = settings {
            if let Some(v) = s.get("warn_ratio").and_then(|v| v.as_float()) {
                self.warn_ratio = v;
            }
            if let Some(v) = s.get("ban_ratio").and_then(|v| v.as_float()) {
                self.ban_ratio = v;
            }
            if let Some(v) = s.get("min_kills").and_then(|v| v.as_integer()) {
                self.min_kills = v as u32;
            }
            if let Some(v) = s.get("ban_duration").and_then(|v| v.as_integer()) {
                self.ban_duration = v as u32;
            }
            if let Some(v) = s.get("announce_interval").and_then(|v| v.as_integer()) {
                self.announce_interval = v as u32;
            }
        }
        Ok(())
    }
    async fn on_startup(&mut self) -> anyhow::Result<()> {
        info!(
            "HeadshotCounter plugin started — warn_ratio={:.0}%, ban_ratio={:.0}%, min_kills={}",
            self.warn_ratio * 100.0, self.ban_ratio * 100.0, self.min_kills
        );
        Ok(())
    }

    async fn on_event(&self, event: &Event, ctx: &BotContext) -> anyhow::Result<()> {
        let event_key = ctx.event_registry.get_key(event.event_type).unwrap_or("");

        match event_key {
            "EVT_CLIENT_KILL" => {
                self.handle_kill(event, ctx).await?;
            }
            "EVT_CLIENT_SAY" | "EVT_CLIENT_TEAM_SAY" => {
                self.handle_command(event, ctx).await?;
            }
            "EVT_GAME_MAP_CHANGE" | "EVT_GAME_EXIT" => {
                // Reset all stats on map change
                self.stats.write().await.clear();
            }
            "EVT_CLIENT_DISCONNECT" => {
                if let Some(client_id) = event.client_id {
                    self.stats.write().await.remove(&client_id);
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
            "EVT_CLIENT_SAY".to_string(),
            "EVT_CLIENT_TEAM_SAY".to_string(),
            "EVT_GAME_MAP_CHANGE".to_string(),
            "EVT_GAME_EXIT".to_string(),
            "EVT_CLIENT_DISCONNECT".to_string(),
        ])
    }
}
