use async_trait::async_trait;
use regex::Regex;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::core::context::BotContext;
use crate::events::{Event, EventData};
use crate::plugins::{Plugin, PluginInfo};

const DEFAULT_CHECK_INTERVAL: u64 = 30;
const DEFAULT_MAX_NAME_CHANGES: u32 = 5;
const DEFAULT_NAME_CHANGE_WINDOW: i64 = 300; // 5 minutes
const DEFAULT_KICK_REASON: &str = "Name violation";

/// The NameChecker plugin — monitors player names for violations.
///
/// Features:
/// - Kicks players with duplicate names (same as another connected player)
/// - Kicks players with forbidden name patterns (configurable regex list)
/// - Limits rapid name changes per time window
/// - Checks on connect and on name change events
pub struct NameCheckerPlugin {
    enabled: bool,
    /// Forbidden name patterns (regex).
    forbidden_patterns: Vec<Regex>,
    /// Maximum name changes allowed per window.
    max_name_changes: u32,
    /// Time window (seconds) for name change tracking.
    name_change_window: i64,
    /// Per-client name change tracking: client_id -> Vec<timestamp>
    name_changes: RwLock<HashMap<i64, Vec<i64>>>,
    /// Whether to check for duplicate names.
    check_duplicates: bool,
}

impl NameCheckerPlugin {
    pub fn new() -> Self {
        let mut forbidden = Vec::new();
        // Default forbidden patterns: all-spaces, "player", impersonation patterns
        if let Ok(re) = Regex::new(r"(?i)^\s*$") {
            forbidden.push(re);
        }
        if let Ok(re) = Regex::new(r"(?i)^player$") {
            forbidden.push(re);
        }
        if let Ok(re) = Regex::new(r"(?i)^unnamed\s*player$") {
            forbidden.push(re);
        }
        if let Ok(re) = Regex::new(r"(?i)^newplayer$") {
            forbidden.push(re);
        }

        Self {
            enabled: true,
            forbidden_patterns: forbidden,
            max_name_changes: DEFAULT_MAX_NAME_CHANGES,
            name_change_window: DEFAULT_NAME_CHANGE_WINDOW,
            name_changes: RwLock::new(HashMap::new()),
            check_duplicates: true,
        }
    }

    /// Strip UrT color codes from a name for comparison.
    fn strip_colors(name: &str) -> String {
        let re = Regex::new(r"\^[0-9a-zA-Z]").unwrap();
        re.replace_all(name, "").to_string()
    }

    /// Check if a name matches any forbidden pattern.
    fn is_forbidden(&self, name: &str) -> bool {
        let clean = Self::strip_colors(name);
        self.forbidden_patterns.iter().any(|re| re.is_match(&clean))
    }

    /// Check for duplicate names among connected players.
    async fn has_duplicate(&self, client_id: i64, name: &str, ctx: &BotContext) -> bool {
        if !self.check_duplicates {
            return false;
        }
        let clean = Self::strip_colors(name).to_lowercase();
        let all = ctx.clients.get_all().await;
        for c in &all {
            if c.id != client_id {
                let other_clean = Self::strip_colors(&c.name).to_lowercase();
                if other_clean == clean {
                    return true;
                }
            }
        }
        false
    }

    /// Track a name change and return true if limit exceeded.
    async fn track_name_change(&self, client_id: i64) -> bool {
        let now = chrono::Utc::now().timestamp();
        let mut changes = self.name_changes.write().await;
        let entry = changes.entry(client_id).or_default();

        // Remove old entries outside the window
        entry.retain(|&t| now - t < self.name_change_window);
        entry.push(now);

        entry.len() as u32 > self.max_name_changes
    }

    /// Perform all name checks for a client. Returns a kick reason if violation found.
    async fn check_name(&self, client_id: i64, name: &str, ctx: &BotContext) -> Option<String> {
        if self.is_forbidden(name) {
            return Some("Forbidden player name".to_string());
        }
        if self.has_duplicate(client_id, name, ctx).await {
            return Some("Duplicate player name".to_string());
        }
        None
    }
}

impl Default for NameCheckerPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Plugin for NameCheckerPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "namechecker",
            description: "Monitors and enforces player name policies (duplicates, forbidden names, name change limits)",
            requires_config: false,
            requires_plugins: &[],
            requires_parsers: &["iourt43"],
            requires_storage: &[],
            load_after: &["admin"],
        }
    }
    async fn on_load_config(&mut self, settings: Option<&toml::Table>) -> anyhow::Result<()> {
        if let Some(s) = settings {
            if let Some(v) = s.get("max_name_changes").and_then(|v| v.as_integer()) {
                self.max_name_changes = v as u32;
            }
            if let Some(v) = s.get("name_change_window").and_then(|v| v.as_integer()) {
                self.name_change_window = v;
            }
            if let Some(v) = s.get("check_duplicates").and_then(|v| v.as_bool()) {
                self.check_duplicates = v;
            }
            if let Some(arr) = s.get("forbidden_patterns").and_then(|v| v.as_array()) {
                self.forbidden_patterns = arr.iter()
                    .filter_map(|v| v.as_str())
                    .filter_map(|s| regex::Regex::new(&format!("(?i){}", s)).ok())
                    .collect();
            }
        }
        Ok(())
    }
    async fn on_startup(&mut self) -> anyhow::Result<()> {
        info!("NameChecker plugin started — checking for forbidden names, duplicates, and name spam");
        Ok(())
    }

    async fn on_event(&self, event: &Event, ctx: &BotContext) -> anyhow::Result<()> {
        let event_key = ctx.event_registry.get_key(event.event_type).unwrap_or("");

        match event_key {
            "EVT_CLIENT_AUTH" => {
                // Check name on connect
                let client_id = match event.client_id {
                    Some(id) => id,
                    None => return Ok(()),
                };
                let client = match ctx.clients.get_by_id(client_id).await {
                    Some(c) => c,
                    None => return Ok(()),
                };
                if let Some(reason) = self.check_name(client_id, &client.name, ctx).await {
                    if let Some(ref cid) = client.cid {
                        warn!(player = %client.name, reason = %reason, "NameChecker kicking player");
                        ctx.message(cid, &format!("^1{}", reason)).await?;
                        ctx.kick(cid, &reason).await?;
                    }
                }
            }
            "EVT_CLIENT_NAME_CHANGE" => {
                let client_id = match event.client_id {
                    Some(id) => id,
                    None => return Ok(()),
                };
                let client = match ctx.clients.get_by_id(client_id).await {
                    Some(c) => c,
                    None => return Ok(()),
                };

                // Check new name
                let new_name = match &event.data {
                    EventData::Text(t) => t.clone(),
                    _ => client.name.clone(),
                };

                if let Some(reason) = self.check_name(client_id, &new_name, ctx).await {
                    if let Some(ref cid) = client.cid {
                        warn!(player = %new_name, reason = %reason, "NameChecker kicking player");
                        ctx.message(cid, &format!("^1{}", reason)).await?;
                        ctx.kick(cid, &reason).await?;
                    }
                    return Ok(());
                }

                // Check name change frequency
                if self.track_name_change(client_id).await {
                    if let Some(ref cid) = client.cid {
                        warn!(player = %client.name, "NameChecker kicking player for name change spam");
                        ctx.message(cid, "^1Too many name changes").await?;
                        ctx.kick(cid, "Too many name changes").await?;
                    }
                }
            }
            "EVT_CLIENT_DISCONNECT" => {
                // Clean up tracking data
                if let Some(client_id) = event.client_id {
                    self.name_changes.write().await.remove(&client_id);
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
            "EVT_CLIENT_AUTH".to_string(),
            "EVT_CLIENT_NAME_CHANGE".to_string(),
            "EVT_CLIENT_DISCONNECT".to_string(),
        ])
    }
}
