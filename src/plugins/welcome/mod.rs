use async_trait::async_trait;
use tracing::info;

use crate::core::context::BotContext;
use crate::events::{Event, EventData};
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

    async fn on_load_config(&mut self, settings: Option<&toml::Table>) -> anyhow::Result<()> {
        if let Some(s) = settings {
            if let Some(v) = s.get("new_player_message").and_then(|v| v.as_str()) {
                self.new_player_message = v.to_string();
            }
            if let Some(v) = s.get("returning_player_message").and_then(|v| v.as_str()) {
                self.returning_player_message = v.to_string();
            }
        }
        Ok(())
    }

    async fn on_startup(&mut self) -> anyhow::Result<()> {
        info!("Welcome plugin started");
        Ok(())
    }

    async fn on_event(&self, event: &Event, ctx: &BotContext) -> anyhow::Result<()> {
        let Some(client_id) = event.client_id else {
            return Ok(());
        };

        let Some(key) = ctx.event_registry.get_key(event.event_type) else {
            return Ok(());
        };

        // Handle !greeting / !setgreeting chat commands
        if key == "EVT_CLIENT_SAY" || key == "EVT_CLIENT_TEAM_SAY" {
            if let EventData::Text(ref text) = event.data {
                if let Some(cmd) = text.strip_prefix('!') {
                    let parts: Vec<&str> = cmd.splitn(2, char::is_whitespace).collect();
                    let command = parts[0].to_lowercase();
                    let args = parts.get(1).map(|s| s.trim()).unwrap_or("");
                    let cid_str = client_id.to_string();

                    match command.as_str() {
                        "greeting" => {
                            // Show your current greeting
                            if let Some(client) = ctx.clients.get_by_cid(&cid_str).await {
                                let greeting = client.get_var("welcome", "greeting")
                                    .map(|v| v.value.as_str().unwrap_or("").to_string())
                                    .unwrap_or_default();
                                if greeting.is_empty() {
                                    ctx.message(&cid_str, "^7You have no custom greeting set. Use ^3!setgreeting <message> ^7to set one.").await?;
                                } else {
                                    ctx.message(&cid_str, &format!("^7Your greeting: ^3{}", greeting)).await?;
                                }
                            }
                        }
                        "setgreeting" => {
                            if args.is_empty() {
                                ctx.message(&cid_str, "Usage: !setgreeting <message> — Use $name for your name. Use 'none' to clear.").await?;
                            } else if args.eq_ignore_ascii_case("none") || args.eq_ignore_ascii_case("clear") {
                                ctx.clients.update(&cid_str, |c| {
                                    c.set_var("welcome", "greeting", serde_json::json!(""));
                                }).await;
                                ctx.message(&cid_str, "^7Custom greeting cleared").await?;
                            } else {
                                let greeting = args.to_string();
                                ctx.clients.update(&cid_str, |c| {
                                    c.set_var("welcome", "greeting", serde_json::json!(greeting));
                                }).await;
                                ctx.message(&cid_str, &format!("^7Greeting set to: ^3{}", args)).await?;
                            }
                        }
                        _ => {}
                    }
                }
            }
            return Ok(());
        }

        if key != "EVT_CLIENT_AUTH" {
            return Ok(());
        }

        let cid_str = client_id.to_string();

        // Look up client from the connected-clients manager
        if let Some(client) = ctx.clients.get_by_cid(&cid_str).await {
            // Check for a custom greeting first
            let custom_greeting = client.get_var("welcome", "greeting")
                .and_then(|v| v.value.as_str().map(|s| s.to_string()));

            if let Some(greeting) = custom_greeting {
                if !greeting.is_empty() {
                    let msg = greeting.replace("$name", &client.name);
                    ctx.say(&msg).await?;
                    info!(client = client_id, name = %client.name, "Custom greeting displayed");
                    return Ok(());
                }
            }

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
            "EVT_CLIENT_SAY".to_string(),
            "EVT_CLIENT_TEAM_SAY".to_string(),
        ])
    }
}
