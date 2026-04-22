use async_trait::async_trait;
use chrono::Utc;
use regex::Regex;
use tracing::{info, warn};

use crate::core::context::BotContext;
use crate::core::{Penalty, PenaltyType};
use crate::events::{Event, EventData};
use crate::plugins::{Plugin, PluginInfo};

/// What to do when a player with a banned name is detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CensorAction {
    Kick,
    Ban,
}

impl CensorAction {
    fn from_str(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "ban" => Self::Ban,
            _ => Self::Kick,
        }
    }
}

/// Urban Terror specific censoring — kicks/bans players with offensive names or clan tags.
pub struct CensorurtPlugin {
    enabled: bool,
    /// Regex patterns matching banned names/clan tags.
    banned_names: Vec<Regex>,
    /// Whether to kick or ban when a banned name is detected.
    action: CensorAction,
}

impl CensorurtPlugin {
    pub fn new() -> Self {
        let patterns = [
            r"(?i)\bn[i1]gg",
            r"(?i)f[a@]gg",
            r"(?i)\badolf\b",
            r"(?i)\bnazi\b",
            r"(?i)\bhitler\b",
        ];
        let banned_names = patterns
            .iter()
            .filter_map(|p| Regex::new(p).ok())
            .collect();

        Self {
            enabled: true,
            banned_names,
            action: CensorAction::Kick,
        }
    }

    /// Add a banned name pattern.
    pub fn add_banned_name(&mut self, pattern: &str) -> anyhow::Result<()> {
        let re = Regex::new(pattern)?;
        self.banned_names.push(re);
        Ok(())
    }

    fn is_name_banned(&self, name: &str) -> bool {
        self.matching_pattern(name).is_some()
    }

    /// Return the first banned regex pattern (as a string) that matches the
    /// given name after stripping color codes, or `None` if the name is clean.
    fn matching_pattern(&self, name: &str) -> Option<String> {
        let stripped = strip_color_codes(name);
        self.banned_names
            .iter()
            .find(|re| re.is_match(&stripped))
            .map(|re| re.as_str().to_string())
    }

    /// Apply the configured action (kick or ban) and record a Penalty row
    /// so the enforcement shows up in the penalties / audit views.
    async fn enforce(
        &self,
        ctx: &BotContext,
        cid: &str,
        client_db_id: i64,
        name: &str,
        pattern: &str,
    ) -> anyhow::Result<()> {
        let reason = format!("Offensive player name (matched {})", pattern);
        let short_reason = "Offensive player name";

        match self.action {
            CensorAction::Kick => ctx.kick(cid, short_reason).await?,
            CensorAction::Ban => ctx.ban(cid, short_reason).await?,
        }

        if client_db_id > 0 {
            let penalty_type = match self.action {
                CensorAction::Kick => PenaltyType::Kick,
                CensorAction::Ban => PenaltyType::Ban,
            };
            let now = Utc::now();
            let penalty = Penalty {
                id: 0,
                penalty_type,
                client_id: client_db_id,
                admin_id: None,
                duration: None,
                reason,
                keyword: "censorurt".to_string(),
                inactive: false,
                time_add: now,
                time_edit: now,
                time_expire: None,
                server_id: None,
            };
            if let Err(e) = ctx.storage.save_penalty(&penalty).await {
                warn!(
                    error = %e,
                    client = client_db_id,
                    name = %name,
                    "Failed to record censorurt penalty"
                );
            }
        }
        Ok(())
    }
}
fn strip_color_codes(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '^' {
            // Skip the next character (color code)
            chars.next();
        } else {
            result.push(c);
        }
    }
    result
}

impl Default for CensorurtPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Plugin for CensorurtPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "censorurt",
            description: "Urban Terror specific name/clan tag censoring",
            requires_config: true,
            requires_plugins: &[],
            requires_parsers: &[],
            requires_storage: &[],
            load_after: &["admin"],
        }
    }

    async fn on_load_config(&mut self, settings: Option<&toml::Table>) -> anyhow::Result<()> {
        if let Some(s) = settings {
            if let Some(arr) = s.get("banned_names").and_then(|v| v.as_array()) {
                let mut compiled = Vec::new();
                for entry in arr.iter().filter_map(|v| v.as_str()) {
                    let trimmed = entry.trim();
                    if trimmed.is_empty() {
                        warn!("censorurt: skipping empty banned_names entry");
                        continue;
                    }
                    match Regex::new(&format!("(?i){}", trimmed)) {
                        Ok(re) => compiled.push(re),
                        Err(e) => warn!(
                            pattern = %trimmed,
                            error = %e,
                            "censorurt: skipping invalid banned_names regex"
                        ),
                    }
                }
                self.banned_names = compiled;
            }
            if let Some(v) = s.get("action").and_then(|v| v.as_str()) {
                self.action = CensorAction::from_str(v);
            }
        }
        Ok(())
    }

    async fn on_startup(&mut self) -> anyhow::Result<()> {
        info!(
            banned_patterns = self.banned_names.len(),
            action = ?self.action,
            "CensorUrt plugin started"
        );
        Ok(())
    }

    async fn on_event(&self, event: &Event, ctx: &BotContext) -> anyhow::Result<()> {
        let Some(client_id) = event.client_id else {
            return Ok(());
        };
        let cid_str = client_id.to_string();

        if let Some(event_key) = ctx.event_registry.get_key(event.event_type) {
            match event_key {
                "EVT_CLIENT_AUTH" => {
                    if let Some(client) = ctx.clients.get_by_cid(&cid_str).await {
                        if let Some(pattern) = self.matching_pattern(&client.name) {
                            info!(
                                client = client_id,
                                name = %client.name,
                                pattern = %pattern,
                                action = ?self.action,
                                "Banned name detected on auth"
                            );
                            self.enforce(ctx, &cid_str, client.id, &client.name, &pattern).await?;
                        }
                    }
                }

                "EVT_CLIENT_NAME_CHANGE" => {
                    if let EventData::Text(ref new_name) = event.data {
                        if let Some(pattern) = self.matching_pattern(new_name) {
                            info!(
                                client = client_id,
                                name = %new_name,
                                pattern = %pattern,
                                action = ?self.action,
                                "Banned name detected on name change"
                            );
                            let db_id = ctx.clients.get_by_cid(&cid_str).await.map(|c| c.id).unwrap_or(0);
                            self.enforce(ctx, &cid_str, db_id, new_name, &pattern).await?;
                        }
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
            "EVT_CLIENT_AUTH".to_string(),
            "EVT_CLIENT_NAME_CHANGE".to_string(),
        ])
    }
}
