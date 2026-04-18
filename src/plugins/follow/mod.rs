use async_trait::async_trait;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::info;

use crate::core::context::BotContext;
use crate::events::Event;
use crate::plugins::{Plugin, PluginInfo};

/// Admin "follow" command — track a player's activity and receive private notifications.
/// Admins use !follow <player> to start and !unfollow <player> to stop tracking.
pub struct FollowPlugin {
    enabled: bool,
    /// Maps target_id -> list of admin client_ids following that target.
    follows: RwLock<HashMap<i64, Vec<i64>>>,
}

impl FollowPlugin {
    pub fn new() -> Self {
        Self {
            enabled: true,
            follows: RwLock::new(HashMap::new()),
        }
    }

    /// Notify all admins following the given target.
    async fn notify_followers(
        &self,
        ctx: &BotContext,
        target_id: i64,
        message: &str,
    ) -> anyhow::Result<()> {
        let follows = self.follows.read().await;
        if let Some(admin_ids) = follows.get(&target_id) {
            for &admin_id in admin_ids {
                ctx.message(&admin_id.to_string(), message).await?;
            }
        }
        Ok(())
    }

    /// Add an admin as a follower of the target.
    async fn add_follow(&self, target_id: i64, admin_id: i64) {
        let mut follows = self.follows.write().await;
        let entry = follows.entry(target_id).or_default();
        if !entry.contains(&admin_id) {
            entry.push(admin_id);
        }
    }

    /// Remove an admin from following the target.
    async fn remove_follow(&self, target_id: i64, admin_id: i64) -> bool {
        let mut follows = self.follows.write().await;
        if let Some(entry) = follows.get_mut(&target_id) {
            if let Some(pos) = entry.iter().position(|&id| id == admin_id) {
                entry.remove(pos);
                if entry.is_empty() {
                    follows.remove(&target_id);
                }
                return true;
            }
        }
        false
    }
}

impl Default for FollowPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Plugin for FollowPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "follow",
            description: "Admin command to follow a player and get notifications about their activity",
            requires_config: false,
            requires_plugins: &["admin"],
            requires_parsers: &[],
            requires_storage: &[],
            load_after: &["admin"],
        }
    }

    async fn on_startup(&mut self) -> anyhow::Result<()> {
        info!("Follow plugin started");
        Ok(())
    }

    async fn on_event(&self, event: &Event, ctx: &BotContext) -> anyhow::Result<()> {
        let Some(event_key) = ctx.event_registry.get_key(event.event_type) else {
            return Ok(());
        };

        match event_key {
            "EVT_CLIENT_SAY" => {
                let Some(client_id) = event.client_id else {
                    return Ok(());
                };

                // Handle !follow and !unfollow commands
                if let crate::events::EventData::Text(ref text) = event.data {
                    if text.starts_with("!follow ") {
                        let target_name = text.trim_start_matches("!follow ").trim();
                        let matches = ctx.clients.find_by_name(target_name).await;
                        if let Some(target) = matches.into_iter().next() {
                            self.add_follow(target.id, client_id).await;
                            ctx.message(
                                &client_id.to_string(),
                                &format!("^7Now following ^2{}", target.name),
                            )
                            .await?;
                            info!(admin = client_id, target = target.id, name = %target.name, "Admin started following player");
                        } else {
                            ctx.message(
                                &client_id.to_string(),
                                &format!("^7No player found matching ^1{}", target_name),
                            )
                            .await?;
                        }
                        return Ok(());
                    } else if text.starts_with("!unfollow ") {
                        let target_name = text.trim_start_matches("!unfollow ").trim();
                        let matches = ctx.clients.find_by_name(target_name).await;
                        if let Some(target) = matches.into_iter().next() {
                            if self.remove_follow(target.id, client_id).await {
                                ctx.message(
                                    &client_id.to_string(),
                                    &format!("^7Stopped following ^2{}", target.name),
                                )
                                .await?;
                            } else {
                                ctx.message(
                                    &client_id.to_string(),
                                    "^7You are not following that player.",
                                )
                                .await?;
                            }
                        } else {
                            ctx.message(
                                &client_id.to_string(),
                                &format!("^7No player found matching ^1{}", target_name),
                            )
                            .await?;
                        }
                        return Ok(());
                    }

                    // Notify followers about chat
                    let name = ctx
                        .clients
                        .get_by_cid(&client_id.to_string())
                        .await
                        .map(|c| c.name.clone())
                        .unwrap_or_else(|| format!("Player#{}", client_id));
                    self.notify_followers(
                        ctx,
                        client_id,
                        &format!("^7[follow] ^2{} ^7said: {}", name, text),
                    )
                    .await?;
                }
            }

            "EVT_CLIENT_KILL" => {
                if let Some(killer_id) = event.client_id {
                    if let Some(victim_id) = event.target_id {
                        let killer_name = ctx
                            .clients
                            .get_by_cid(&killer_id.to_string())
                            .await
                            .map(|c| c.name.clone())
                            .unwrap_or_else(|| format!("Player#{}", killer_id));
                        let victim_name = ctx
                            .clients
                            .get_by_cid(&victim_id.to_string())
                            .await
                            .map(|c| c.name.clone())
                            .unwrap_or_else(|| format!("Player#{}", victim_id));

                        self.notify_followers(
                            ctx,
                            killer_id,
                            &format!("^7[follow] ^2{} ^7killed ^2{}", killer_name, victim_name),
                        )
                        .await?;

                        self.notify_followers(
                            ctx,
                            victim_id,
                            &format!(
                                "^7[follow] ^2{} ^7was killed by ^2{}",
                                victim_name, killer_name
                            ),
                        )
                        .await?;
                    }
                }
            }

            "EVT_CLIENT_TEAM_CHANGE" => {
                if let Some(client_id) = event.client_id {
                    let name = ctx
                        .clients
                        .get_by_cid(&client_id.to_string())
                        .await
                        .map(|c| c.name.clone())
                        .unwrap_or_else(|| format!("Player#{}", client_id));
                    let team_info = if let crate::events::EventData::Text(ref text) = event.data {
                        text.clone()
                    } else {
                        "unknown".to_string()
                    };
                    self.notify_followers(
                        ctx,
                        client_id,
                        &format!("^7[follow] ^2{} ^7changed team to ^3{}", name, team_info),
                    )
                    .await?;
                }
            }

            "EVT_CLIENT_DISCONNECT" => {
                // Clean up follows for disconnecting client
                if let Some(client_id) = event.client_id {
                    let name = ctx
                        .clients
                        .get_by_cid(&client_id.to_string())
                        .await
                        .map(|c| c.name.clone())
                        .unwrap_or_else(|| format!("Player#{}", client_id));
                    self.notify_followers(
                        ctx,
                        client_id,
                        &format!("^7[follow] ^2{} ^7disconnected", name),
                    )
                    .await?;
                    self.follows.write().await.remove(&client_id);
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
            "EVT_CLIENT_SAY".to_string(),
            "EVT_CLIENT_KILL".to_string(),
            "EVT_CLIENT_TEAM_CHANGE".to_string(),
            "EVT_CLIENT_DISCONNECT".to_string(),
        ])
    }
}
