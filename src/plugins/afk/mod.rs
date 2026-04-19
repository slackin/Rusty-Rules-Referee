use async_trait::async_trait;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::info;

use crate::core::context::BotContext;
use crate::events::Event;
use crate::plugins::{Plugin, PluginInfo};

/// Tracks player activity and kicks AFK players after a configurable threshold.
pub struct AfkPlugin {
    enabled: bool,
    /// Seconds before a player is considered AFK.
    afk_threshold_secs: u64,
    /// Minimum player count before AFK kicks are active.
    min_players: u32,
    /// Seconds between each AFK check.
    check_interval_secs: u64,
    /// Last activity timestamp per client_id.
    last_activity: RwLock<HashMap<i64, i64>>,
    /// Whether to move to spectator first instead of kicking.
    move_to_spec: bool,
    /// Message shown to AFK players.
    afk_message: String,
}

impl AfkPlugin {
    pub fn new() -> Self {
        Self {
            enabled: true,
            afk_threshold_secs: 300,
            min_players: 4,
            check_interval_secs: 60,
            last_activity: RwLock::new(HashMap::new()),
            move_to_spec: true,
            afk_message: "^7AFK: You have been inactive too long".to_string(),
        }
    }

    async fn mark_active(&self, client_id: i64) {
        let now = chrono::Utc::now().timestamp();
        self.last_activity.write().await.insert(client_id, now);
    }

    async fn check_afk(&self, ctx: &BotContext) -> anyhow::Result<()> {
        let count = ctx.clients.count().await;
        if count < self.min_players as usize {
            return Ok(());
        }

        let now = chrono::Utc::now().timestamp();
        let threshold = self.afk_threshold_secs as i64;

        let activity = self.last_activity.read().await;
        let all = ctx.clients.get_all().await;

        for client in &all {
            let last = activity.get(&client.id).copied().unwrap_or(now);
            if now - last > threshold {
                if let Some(ref cid) = client.cid {
                    if self.move_to_spec {
                        ctx.write(&format!("forceteam {} spectator", cid)).await?;
                        ctx.message(cid, &self.afk_message).await?;
                    } else {
                        ctx.kick(cid, "AFK").await?;
                    }
                }
            }
        }

        Ok(())
    }
}

impl Default for AfkPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Plugin for AfkPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "afk",
            description: "Detects and handles AFK players",
            requires_config: true,
            requires_plugins: &[],
            requires_parsers: &[],
            requires_storage: &[],
            load_after: &[],
        }
    }

    async fn on_load_config(&mut self, settings: Option<&toml::Table>) -> anyhow::Result<()> {
        if let Some(s) = settings {
            if let Some(v) = s.get("afk_threshold_secs").and_then(|v| v.as_integer()) {
                self.afk_threshold_secs = v as u64;
            }
            if let Some(v) = s.get("min_players").and_then(|v| v.as_integer()) {
                self.min_players = v as u32;
            }
            if let Some(v) = s.get("check_interval_secs").and_then(|v| v.as_integer()) {
                self.check_interval_secs = v as u64;
            }
            if let Some(v) = s.get("move_to_spec").and_then(|v| v.as_bool()) {
                self.move_to_spec = v;
            }
            if let Some(v) = s.get("afk_message").and_then(|v| v.as_str()) {
                self.afk_message = v.to_string();
            }
        }
        Ok(())
    }

    async fn on_startup(&mut self) -> anyhow::Result<()> {
        info!(
            threshold = self.afk_threshold_secs,
            min_players = self.min_players,
            "AFK plugin started"
        );
        Ok(())
    }

    async fn on_event(&self, event: &Event, ctx: &BotContext) -> anyhow::Result<()> {
        // Mark activity for any client event
        if let Some(client_id) = event.client_id {
            self.mark_active(client_id).await;
        }

        // On round start, run AFK check
        if let Some(event_key) = ctx.event_registry.get_key(event.event_type) {
            match event_key {
                "EVT_GAME_ROUND_START" => {
                    self.check_afk(ctx).await?;
                }
                "EVT_CLIENT_DISCONNECT" => {
                    if let Some(cid) = event.client_id {
                        self.last_activity.write().await.remove(&cid);
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
            "EVT_CLIENT_SAY".to_string(),
            "EVT_CLIENT_TEAM_SAY".to_string(),
            "EVT_CLIENT_KILL".to_string(),
            "EVT_CLIENT_DAMAGE".to_string(),
            "EVT_CLIENT_DISCONNECT".to_string(),
            "EVT_GAME_ROUND_START".to_string(),
        ])
    }
}
