use async_trait::async_trait;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::info;

use crate::core::context::BotContext;
use crate::events::{Event, EventData};
use crate::plugins::{Plugin, PluginInfo};

const LEVEL_MOD: u32 = 20;

/// Requires high-level admins to authenticate with a password before their
/// admin privileges are active.
pub struct LoginPlugin {
    enabled: bool,
    /// Minimum level that must login before using commands.
    min_level: u32,
    /// Per-client login status: client_id -> logged_in.
    logged_in: RwLock<HashMap<i64, bool>>,
}

impl LoginPlugin {
    pub fn new() -> Self {
        Self {
            enabled: true,
            min_level: LEVEL_MOD,
            logged_in: RwLock::new(HashMap::new()),
        }
    }

    pub async fn is_logged_in(&self, client_id: i64) -> bool {
        *self.logged_in.read().await.get(&client_id).unwrap_or(&false)
    }
}

impl Default for LoginPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Plugin for LoginPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "login",
            description: "Requires high-level admins to authenticate before using commands",
            requires_config: true,
            requires_plugins: &["admin"],
            requires_parsers: &[],
            requires_storage: &[],
            load_after: &["admin"],
        }
    }

    async fn on_load_config(&mut self, settings: Option<&toml::Table>) -> anyhow::Result<()> {
        if let Some(s) = settings {
            if let Some(v) = s.get("min_level").and_then(|v| v.as_integer()) {
                self.min_level = v as u32;
            }
        }
        Ok(())
    }

    async fn on_startup(&mut self) -> anyhow::Result<()> {
        info!(min_level = self.min_level, "Login plugin started");
        Ok(())
    }

    async fn on_event(&self, event: &Event, ctx: &BotContext) -> anyhow::Result<()> {
        if let Some(event_key) = ctx.event_registry.get_key(event.event_type) {
            match event_key {
                "EVT_CLIENT_SAY" | "EVT_CLIENT_TEAM_SAY" => {
                    if let EventData::Text(ref text) = event.data {
                        if let Some(cmd) = text.strip_prefix('!') {
                            let parts: Vec<&str> = cmd.splitn(2, ' ').collect();
                            let command = parts[0].to_lowercase();
                            let args = parts.get(1).unwrap_or(&"").trim();

                            if command == "login" {
                                if let Some(client_id) = event.client_id {
                                    let cid_str = client_id.to_string();
                                    if let Some(client) = ctx.clients.get_by_cid(&cid_str).await {
                                        if client.max_level() < self.min_level {
                                            ctx.message(&cid_str, "^7You don't need to login").await?;
                                        } else if args.is_empty() {
                                            ctx.message(&cid_str, "Usage: !login <password>").await?;
                                        } else {
                                            // Check password against client's stored password
                                            let stored_pw = client.get_var("login", "password").map(|v| v.as_str().to_string());
                                            if let Some(pw) = stored_pw {
                                                if pw == args {
                                                    self.logged_in.write().await.insert(client_id, true);
                                                    ctx.message(&cid_str, "^2Login successful").await?;
                                                } else {
                                                    ctx.message(&cid_str, "^1Invalid password").await?;
                                                }
                                            } else {
                                                ctx.message(&cid_str, "^1No password set. Use !setpassword first").await?;
                                            }
                                        }
                                    }
                                }
                            } else if command == "setpassword" {
                                if let Some(client_id) = event.client_id {
                                    let cid_str = client_id.to_string();
                                    if args.is_empty() {
                                        ctx.message(&cid_str, "Usage: !setpassword <password>").await?;
                                    } else if args.len() < 4 {
                                        ctx.message(&cid_str, "^1Password must be at least 4 characters").await?;
                                    } else {
                                        ctx.clients.update(&cid_str, |c| {
                                            c.set_var("login", "password", serde_json::Value::String(args.to_string()));
                                        }).await;
                                        ctx.message(&cid_str, "^2Password set successfully").await?;
                                    }
                                }
                            }
                        }
                    }
                }

                "EVT_CLIENT_DISCONNECT" => {
                    if let Some(cid) = event.client_id {
                        self.logged_in.write().await.remove(&cid);
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
            "EVT_CLIENT_DISCONNECT".to_string(),
        ])
    }
}
