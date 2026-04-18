use async_trait::async_trait;
use tracing::info;

use crate::core::context::BotContext;
use crate::events::{Event, EventData};
use crate::plugins::{Plugin, PluginInfo};

/// XLRstats — Extended Live Rankings and Statistics.
///
/// Tracks kills, deaths, assists, headshots, team kills, etc. per player
/// and computes ELO-style skill ratings.
///
/// Requires additional database tables (see migration below).
pub struct XlrstatsPlugin {
    enabled: bool,
    /// Kill bonus for skill calculation.
    kill_bonus: f64,
    /// Assist bonus for skill calculation.
    assist_bonus: f64,
    /// Minimum kills before displaying stats.
    min_kills: u32,
}

impl XlrstatsPlugin {
    pub fn new() -> Self {
        Self {
            enabled: true,
            kill_bonus: 1.2,
            assist_bonus: 0.5,
            min_kills: 50,
        }
    }

    /// SQL to create XLRstats tables. Should be run as a migration.
    pub fn migration_sql() -> &'static str {
        r#"
CREATE TABLE IF NOT EXISTS xlr_playerstats (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    client_id INTEGER NOT NULL UNIQUE,
    kills INTEGER NOT NULL DEFAULT 0,
    deaths INTEGER NOT NULL DEFAULT 0,
    teamkills INTEGER NOT NULL DEFAULT 0,
    teamdeaths INTEGER NOT NULL DEFAULT 0,
    suicides INTEGER NOT NULL DEFAULT 0,
    ratio REAL NOT NULL DEFAULT 0.0,
    skill REAL NOT NULL DEFAULT 1000.0,
    assists INTEGER NOT NULL DEFAULT 0,
    assistskill REAL NOT NULL DEFAULT 0.0,
    curstreak INTEGER NOT NULL DEFAULT 0,
    winstreak INTEGER NOT NULL DEFAULT 0,
    losestreak INTEGER NOT NULL DEFAULT 0,
    rounds INTEGER NOT NULL DEFAULT 0,
    smallestratio REAL NOT NULL DEFAULT 0.0,
    biggestratio REAL NOT NULL DEFAULT 0.0,
    smalleststreak INTEGER NOT NULL DEFAULT 0,
    biggeststreak INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (client_id) REFERENCES clients(id)
);

CREATE INDEX IF NOT EXISTS idx_xlr_playerstats_client_id ON xlr_playerstats(client_id);
CREATE INDEX IF NOT EXISTS idx_xlr_playerstats_skill ON xlr_playerstats(skill);

CREATE TABLE IF NOT EXISTS xlr_weaponstats (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    client_id INTEGER NOT NULL,
    name VARCHAR(64) NOT NULL,
    kills INTEGER NOT NULL DEFAULT 0,
    deaths INTEGER NOT NULL DEFAULT 0,
    teamkills INTEGER NOT NULL DEFAULT 0,
    teamdeaths INTEGER NOT NULL DEFAULT 0,
    suicides INTEGER NOT NULL DEFAULT 0,
    headshots INTEGER NOT NULL DEFAULT 0,
    UNIQUE(client_id, name),
    FOREIGN KEY (client_id) REFERENCES clients(id)
);

CREATE TABLE IF NOT EXISTS xlr_weaponusage (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name VARCHAR(64) NOT NULL UNIQUE,
    kills INTEGER NOT NULL DEFAULT 0,
    deaths INTEGER NOT NULL DEFAULT 0,
    teamkills INTEGER NOT NULL DEFAULT 0,
    teamdeaths INTEGER NOT NULL DEFAULT 0,
    suicides INTEGER NOT NULL DEFAULT 0,
    headshots INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS xlr_bodyparts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    client_id INTEGER NOT NULL,
    name VARCHAR(64) NOT NULL,
    kills INTEGER NOT NULL DEFAULT 0,
    deaths INTEGER NOT NULL DEFAULT 0,
    teamkills INTEGER NOT NULL DEFAULT 0,
    teamdeaths INTEGER NOT NULL DEFAULT 0,
    suicides INTEGER NOT NULL DEFAULT 0,
    UNIQUE(client_id, name),
    FOREIGN KEY (client_id) REFERENCES clients(id)
);

CREATE TABLE IF NOT EXISTS xlr_opponents (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    client_id INTEGER NOT NULL,
    target_id INTEGER NOT NULL,
    kills INTEGER NOT NULL DEFAULT 0,
    deaths INTEGER NOT NULL DEFAULT 0,
    retals INTEGER NOT NULL DEFAULT 0,
    UNIQUE(client_id, target_id),
    FOREIGN KEY (client_id) REFERENCES clients(id),
    FOREIGN KEY (target_id) REFERENCES clients(id)
);

CREATE TABLE IF NOT EXISTS xlr_mapstats (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name VARCHAR(64) NOT NULL UNIQUE,
    kills INTEGER NOT NULL DEFAULT 0,
    suicides INTEGER NOT NULL DEFAULT 0,
    teamkills INTEGER NOT NULL DEFAULT 0,
    rounds INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS xlr_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    client_id INTEGER NOT NULL,
    kills INTEGER NOT NULL DEFAULT 0,
    deaths INTEGER NOT NULL DEFAULT 0,
    skill REAL NOT NULL DEFAULT 0.0,
    time_add DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (client_id) REFERENCES clients(id)
);

CREATE INDEX IF NOT EXISTS idx_xlr_history_client_id ON xlr_history(client_id);
        "#
    }

    /// Calculate new skill rating after a kill event.
    fn calculate_skill(killer_skill: f64, victim_skill: f64, kill_bonus: f64) -> f64 {
        let expected = 1.0 / (1.0 + 10.0_f64.powf((victim_skill - killer_skill) / 400.0));
        let k_factor = if killer_skill < 1100.0 { 16.0 } else { 10.0 };
        killer_skill + k_factor * (1.0 - expected) * kill_bonus
    }

    fn calculate_skill_loss(loser_skill: f64, winner_skill: f64) -> f64 {
        let expected = 1.0 / (1.0 + 10.0_f64.powf((winner_skill - loser_skill) / 400.0));
        let k_factor = if loser_skill < 1100.0 { 16.0 } else { 10.0 };
        let new_skill = loser_skill + k_factor * (0.0 - expected);
        if new_skill < 0.0 { 0.0 } else { new_skill }
    }
}

impl Default for XlrstatsPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Plugin for XlrstatsPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "xlrstats",
            description: "Extended Live Rankings and Statistics (ELO-based skill tracking)",
            requires_config: true,
            requires_plugins: &[],
            requires_parsers: &[],
            requires_storage: &["xlr_playerstats"],
            load_after: &[],
        }
    }

    async fn on_startup(&mut self) -> anyhow::Result<()> {
        info!(
            kill_bonus = self.kill_bonus,
            min_kills = self.min_kills,
            "XLRstats plugin started"
        );
        Ok(())
    }

    async fn on_event(&self, event: &Event, ctx: &BotContext) -> anyhow::Result<()> {
        if let Some(event_key) = ctx.event_registry.get_key(event.event_type) {
            match event_key {
                "EVT_CLIENT_KILL" => {
                    // In a full implementation, we'd update xlr_playerstats,
                    // xlr_weaponstats, xlr_opponents, etc.
                    // For now, we log the event and update in-memory stats.
                    if let Some(killer_id) = event.client_id {
                        if let Some(victim_id) = event.target_id {
                            if killer_id != victim_id {
                                // Get weapon from event data
                                let weapon = match &event.data {
                                    EventData::Text(t) => t.clone(),
                                    _ => "unknown".to_string(),
                                };

                                // TODO: Full DB stat updates
                                // For now just track in plugin variables
                                if let Some(cid) = ctx.clients.get_by_cid(&killer_id.to_string()).await {
                                    ctx.clients.update(
                                        cid.cid.as_deref().unwrap_or("0"),
                                        |c| {
                                            let kills: u64 = c.get_var("xlrstats", "kills")
                                                .map(|v| v.as_i64() as u64)
                                                .unwrap_or(0);
                                            c.set_var("xlrstats", "kills", serde_json::json!(kills + 1));
                                        },
                                    ).await;
                                }
                            }
                        }
                    }
                }

                "EVT_CLIENT_SAY" | "EVT_CLIENT_TEAM_SAY" => {
                    if let EventData::Text(ref text) = event.data {
                        if let Some(cmd) = text.strip_prefix('!') {
                            let command = cmd.split_whitespace().next().unwrap_or("").to_lowercase();
                            if command == "xlrstats" || command == "xlr" {
                                if let Some(client_id) = event.client_id {
                                    let cid_str = client_id.to_string();
                                    if let Some(client) = ctx.clients.get_by_cid(&cid_str).await {
                                        let kills: i64 = client.get_var("xlrstats", "kills")
                                            .map(|v| v.as_i64())
                                            .unwrap_or(0);
                                        let deaths: i64 = client.get_var("xlrstats", "deaths")
                                            .map(|v| v.as_i64())
                                            .unwrap_or(0);
                                        let skill: f64 = client.get_var("xlrstats", "skill")
                                            .and_then(|v| v.value.as_f64())
                                            .unwrap_or(1000.0);
                                        let ratio = if deaths > 0 {
                                            kills as f64 / deaths as f64
                                        } else {
                                            kills as f64
                                        };
                                        ctx.message(
                                            &cid_str,
                                            &format!(
                                                "^3XLR Stats ^7for ^2{}: ^7Kills:^2{} ^7Deaths:^2{} ^7Ratio:^2{:.2} ^7Skill:^2{:.0}",
                                                client.name, kills, deaths, ratio, skill
                                            ),
                                        ).await?;
                                    }
                                }
                            } else if command == "xlrtopstats" || command == "topstats" {
                                if let Some(client_id) = event.client_id {
                                    let cid_str = client_id.to_string();
                                    ctx.message(&cid_str, "^3Top stats: ^7(Coming soon — requires DB queries)").await?;
                                }
                            }
                        }
                    }
                }

                "EVT_GAME_ROUND_START" => {
                    // Could snapshot stats to xlr_history here
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
            "EVT_CLIENT_SAY".to_string(),
            "EVT_CLIENT_TEAM_SAY".to_string(),
            "EVT_GAME_ROUND_START".to_string(),
        ])
    }
}
