use async_trait::async_trait;
use std::collections::HashMap;
use tracing::info;

use crate::core::context::BotContext;
use crate::events::{Event, EventData};
use crate::plugins::{Plugin, PluginInfo};

/// Custom text commands — maps command names to response messages.
/// For example, `!rules_ctf` could respond with the CTF-specific rules text.
pub struct CustomcommandsPlugin {
    enabled: bool,
    /// Maps command name (without prefix) to response text.
    commands: HashMap<String, String>,
}

impl CustomcommandsPlugin {
    pub fn new() -> Self {
        let mut commands = HashMap::new();
        commands.insert(
            "rules".to_string(),
            "^7Server rules: No cheating, no racism, respect admins.".to_string(),
        );
        commands.insert(
            "discord".to_string(),
            "^7Join our Discord: discord.gg/example".to_string(),
        );

        Self {
            enabled: true,
            commands,
        }
    }

    /// Register a custom command with the given response text.
    pub fn add_command(&mut self, name: &str, response: &str) {
        self.commands
            .insert(name.to_lowercase(), response.to_string());
    }

    /// Remove a custom command.
    pub fn remove_command(&mut self, name: &str) -> bool {
        self.commands.remove(&name.to_lowercase()).is_some()
    }

    /// Check if a chat message matches a custom command, return the response.
    fn match_command(&self, text: &str) -> Option<&str> {
        let trimmed = text.trim();
        if !trimmed.starts_with('!') {
            return None;
        }
        // Extract command name (first word after '!')
        let cmd = trimmed[1..]
            .split_whitespace()
            .next()?
            .to_lowercase();
        self.commands.get(&cmd).map(|s| s.as_str())
    }
}

impl Default for CustomcommandsPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Plugin for CustomcommandsPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "customcommands",
            description: "User-defined text commands",
            requires_config: true,
            requires_plugins: &[],
            requires_parsers: &[],
            requires_storage: &[],
            load_after: &["admin"],
        }
    }

    async fn on_load_config(&mut self, settings: Option<&toml::Table>) -> anyhow::Result<()> {
        if let Some(s) = settings {
            if let Some(t) = s.get("commands").and_then(|v| v.as_table()) {
                self.commands.clear();
                for (key, val) in t {
                    if let Some(v) = val.as_str() {
                        self.commands.insert(key.clone(), v.to_string());
                    }
                }
            }
        }
        Ok(())
    }

    async fn on_startup(&mut self) -> anyhow::Result<()> {
        info!(
            commands = self.commands.len(),
            "CustomCommands plugin started"
        );
        Ok(())
    }

    async fn on_event(&self, event: &Event, ctx: &BotContext) -> anyhow::Result<()> {
        if let Some(event_key) = ctx.event_registry.get_key(event.event_type) {
            match event_key {
                "EVT_CLIENT_SAY" => {
                    if let EventData::Text(ref text) = event.data {
                        if let Some(response) = self.match_command(text) {
                            if let Some(client_id) = event.client_id {
                                let cid_str = client_id.to_string();
                                ctx.message(&cid_str, response).await?;
                            }
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
        Some(vec!["EVT_CLIENT_SAY".to_string()])
    }
}
