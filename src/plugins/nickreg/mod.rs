use async_trait::async_trait;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::info;

use crate::core::context::BotContext;
use crate::events::Event;
use crate::plugins::{Plugin, PluginInfo};

/// Nickname registration protection — tracks registered nicknames tied to
/// client database IDs and kicks impostors who use a registered nick.
pub struct NickregPlugin {
    enabled: bool,
    /// Maps lowercase registered nickname to the owning client database ID.
    registered_nicks: RwLock<HashMap<String, i64>>,
    /// Number of seconds to wait before kicking (gives time for a warning).
    warn_before_kick: bool,
}

impl NickregPlugin {
    pub fn new() -> Self {
        Self {
            enabled: true,
            registered_nicks: RwLock::new(HashMap::new()),
            warn_before_kick: true,
        }
    }

    /// Register a nickname for a client database ID.
    pub async fn register_nick(&self, nick: &str, client_db_id: i64) {
        self.registered_nicks
            .write()
            .await
            .insert(strip_color_codes(nick).to_lowercase(), client_db_id);
    }

    /// Unregister a nickname.
    pub async fn unregister_nick(&self, nick: &str) -> bool {
        self.registered_nicks
            .write()
            .await
            .remove(&strip_color_codes(nick).to_lowercase())
            .is_some()
    }

    /// Check if a nickname is registered and, if so, whether the given client owns it.
    async fn check_nick(&self, name: &str, client_db_id: i64) -> NickStatus {
        let stripped = strip_color_codes(name).to_lowercase();
        let nicks = self.registered_nicks.read().await;
        match nicks.get(&stripped) {
            Some(&owner_id) if owner_id == client_db_id => NickStatus::Owner,
            Some(_) => NickStatus::Impostor,
            None => NickStatus::Unregistered,
        }
    }
}

enum NickStatus {
    Owner,
    Impostor,
    Unregistered,
}

/// Strip Quake 3 / Urban Terror color codes.
fn strip_color_codes(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '^' {
            chars.next();
        } else {
            result.push(c);
        }
    }
    result
}

impl Default for NickregPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Plugin for NickregPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "nickreg",
            description: "Nickname registration and protection",
            requires_config: true,
            requires_plugins: &[],
            requires_parsers: &[],
            requires_storage: &[],
            load_after: &["admin"],
        }
    }

    async fn on_load_config(&mut self, settings: Option<&toml::Table>) -> anyhow::Result<()> {
        if let Some(s) = settings {
            if let Some(v) = s.get("warn_before_kick").and_then(|v| v.as_bool()) {
                self.warn_before_kick = v;
            }
        }
        Ok(())
    }

    async fn on_startup(&mut self) -> anyhow::Result<()> {
        info!("NickReg plugin started");
        Ok(())
    }

    async fn on_event(&self, event: &Event, ctx: &BotContext) -> anyhow::Result<()> {
        let Some(client_id) = event.client_id else {
            return Ok(());
        };

        if let Some(event_key) = ctx.event_registry.get_key(event.event_type) {
            match event_key {
                "EVT_CLIENT_AUTH" => {
                    let cid_str = client_id.to_string();
                    let Some(client) = ctx.clients.get_by_cid(&cid_str).await else {
                        return Ok(());
                    };

                    match self.check_nick(&client.name, client.id).await {
                        NickStatus::Impostor => {
                            info!(
                                client = client_id,
                                name = %client.name,
                                db_id = client.id,
                                "Impostor using registered nickname"
                            );
                            if self.warn_before_kick {
                                ctx.message(
                                    &cid_str,
                                    "^1WARNING: ^7This nickname is registered. Change your name or you will be kicked.",
                                )
                                .await?;
                            }
                            ctx.kick(&cid_str, "Using a registered nickname").await?;
                        }
                        NickStatus::Owner => {
                            info!(
                                client = client_id,
                                name = %client.name,
                                "Registered nickname owner authenticated"
                            );
                        }
                        NickStatus::Unregistered => {}
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
        Some(vec!["EVT_CLIENT_AUTH".to_string()])
    }
}
