use async_trait::async_trait;
use tracing::info;

use crate::core::context::BotContext;
use crate::events::Event;
use crate::plugins::{Plugin, PluginInfo};

/// The Welcome plugin — greets players when they connect/authenticate.
/// First-time visitors get a welcome message, returning players get a "welcome back".
pub struct WelcomePlugin {
    enabled: bool,
    new_player_message: String,
    returning_player_message: String,
}

impl WelcomePlugin {
    pub fn new() -> Self {
        Self {
            enabled: true,
            new_player_message: "^7Welcome to the server, ^2$name^7! Type ^3!help^7 for commands.".to_string(),
            returning_player_message: "^7Welcome back, ^2$name^7! You were last seen ^3$last_visit^7.".to_string(),
        }
    }

    fn format_message(template: &str, name: &str, last_visit: Option<&str>) -> String {
        template
            .replace("$name", name)
            .replace("$last_visit", last_visit.unwrap_or("never"))
    }
}

impl Default for WelcomePlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Plugin for WelcomePlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "welcome",
            description: "Greets new and returning players",
            requires_config: false,
            requires_plugins: &[],
            requires_parsers: &[],
            requires_storage: &[],
            load_after: &["admin"],
        }
    }

    async fn on_startup(&mut self) -> anyhow::Result<()> {
        info!("Welcome plugin started");
        Ok(())
    }

    async fn on_event(&self, event: &Event, ctx: &BotContext) -> anyhow::Result<()> {
        let Some(client_id) = event.client_id else {
            return Ok(());
        };

        // Only handle auth events (client is authenticated and in the Clients manager)
        if let Some(key) = ctx.event_registry.get_key(event.event_type) {
            if key != "EVT_CLIENT_AUTH" {
                return Ok(());
            }
        }

        let cid_str = client_id.to_string();

        // Look up client from the connected-clients manager
        if let Some(client) = ctx.clients.get_by_cid(&cid_str).await {
            if client.last_visit.is_some() {
                // Returning player
                let last_visit = client
                    .last_visit
                    .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string());
                let msg = Self::format_message(
                    &self.returning_player_message,
                    &client.name,
                    last_visit.as_deref(),
                );
                ctx.message(&cid_str, &msg).await?;
                info!(client = client_id, name = %client.name, "Welcome back message sent");
            } else {
                // New player
                let msg = Self::format_message(&self.new_player_message, &client.name, None);
                ctx.message(&cid_str, &msg).await?;
                info!(client = client_id, name = %client.name, "Welcome message sent to new player");
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
        ])
    }
}
