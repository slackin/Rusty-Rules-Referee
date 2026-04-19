use async_trait::async_trait;
use tracing::info;

use crate::core::context::BotContext;
use crate::events::Event;
use crate::plugins::{Plugin, PluginInfo};

/// Makes room for admins by kicking the lowest-level non-admin player
/// when the server is full and an admin-level player joins.
pub struct MakeroomPlugin {
    enabled: bool,
    /// Minimum admin level required to trigger a room-making kick.
    min_admin_level: u32,
    /// Maximum number of player slots on the server.
    max_players: usize,
}

impl MakeroomPlugin {
    pub fn new() -> Self {
        Self {
            enabled: true,
            min_admin_level: 20,
            max_players: 32,
        }
    }
}

impl Default for MakeroomPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Plugin for MakeroomPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "makeroom",
            description: "Kicks lowest-level player to make room for admins",
            requires_config: true,
            requires_plugins: &[],
            requires_parsers: &[],
            requires_storage: &[],
            load_after: &["admin"],
        }
    }

    async fn on_load_config(&mut self, settings: Option<&toml::Table>) -> anyhow::Result<()> {
        if let Some(s) = settings {
            if let Some(v) = s.get("min_admin_level").and_then(|v| v.as_integer()) {
                self.min_admin_level = v as u32;
            }
            if let Some(v) = s.get("max_players").and_then(|v| v.as_integer()) {
                self.max_players = v as usize;
            }
        }
        Ok(())
    }

    async fn on_startup(&mut self) -> anyhow::Result<()> {
        info!(
            min_admin_level = self.min_admin_level,
            max_players = self.max_players,
            "MakeRoom plugin started"
        );
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

                    // Check if the joining client has admin level
                    let joining_level = ctx
                        .clients
                        .get_by_cid(&cid_str)
                        .await
                        .map(|c| c.max_level())
                        .unwrap_or(0);

                    if joining_level < self.min_admin_level {
                        return Ok(());
                    }

                    // Check if server is full
                    let all_clients = ctx.clients.get_all().await;
                    if all_clients.len() < self.max_players {
                        return Ok(());
                    }

                    // Find the lowest-level non-admin player to kick
                    let kick_target = all_clients
                        .iter()
                        .filter(|c| {
                            c.max_level() < self.min_admin_level
                                && c.cid.as_deref() != Some(&cid_str)
                        })
                        .min_by_key(|c| c.max_level());

                    if let Some(target) = kick_target {
                        if let Some(ref target_cid) = target.cid {
                            info!(
                                admin = client_id,
                                kicked = %target.name,
                                kicked_level = target.max_level(),
                                "Making room for admin by kicking lowest-level player"
                            );
                            ctx.message(
                                target_cid,
                                "^1Server is full. Making room for an admin.",
                            )
                            .await?;
                            ctx.kick(target_cid, "Making room for admin").await?;
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
        Some(vec!["EVT_CLIENT_AUTH".to_string()])
    }
}
