use async_trait::async_trait;
use regex::Regex;
use tracing::info;

use crate::core::context::BotContext;
use crate::events::{Event, EventData};
use crate::plugins::{Plugin, PluginInfo};

/// Urban Terror specific censoring — bans players with offensive names or clan tags.
pub struct CensorurtPlugin {
    enabled: bool,
    /// Regex patterns matching banned names/clan tags.
    banned_names: Vec<Regex>,
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
                self.banned_names = arr.iter()
                    .filter_map(|v| v.as_str())
                    .filter_map(|s| regex::Regex::new(&format!("(?i){}", s)).ok())
                    .collect();
            }
        }
        Ok(())
    }

    async fn on_startup(&mut self) -> anyhow::Result<()> {
        info!(
            banned_patterns = self.banned_names.len(),
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
                                "Banned name detected on auth"
                            );
                            ctx.kick(&cid_str, "Offensive player name").await?;
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
                                "Banned name detected on name change"
                            );
                            ctx.kick(&cid_str, "Offensive player name").await?;
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
