use async_trait::async_trait;
use std::collections::HashSet;
use tracing::info;

use crate::core::context::BotContext;
use crate::events::{Event, EventData};
use crate::plugins::{Plugin, PluginInfo};

/// Controls which callvotes are allowed and handles vote spam protection.
pub struct CallvotePlugin {
    enabled: bool,
    /// Minimum level to call votes.
    min_level: u32,
    /// Vote types that are blocked (e.g., "kick", "map", "nextmap").
    blocked_votes: HashSet<String>,
    /// Maximum votes per player per round.
    max_votes_per_round: u32,
}

impl CallvotePlugin {
    pub fn new() -> Self {
        Self {
            enabled: true,
            min_level: 0,
            blocked_votes: HashSet::new(),
            max_votes_per_round: 3,
        }
    }
}

impl Default for CallvotePlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Plugin for CallvotePlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "callvote",
            description: "Controls and filters in-game callvotes",
            requires_config: true,
            requires_plugins: &[],
            requires_parsers: &["iourt43"],
            requires_storage: &[],
            load_after: &[],
        }
    }

    async fn on_startup(&mut self) -> anyhow::Result<()> {
        info!(blocked = ?self.blocked_votes, "Callvote plugin started");
        Ok(())
    }

    async fn on_event(&self, event: &Event, ctx: &BotContext) -> anyhow::Result<()> {
        if let Some(event_key) = ctx.event_registry.get_key(event.event_type) {
            if event_key == "EVT_CLIENT_CALLVOTE" {
                if let EventData::Text(ref vote_text) = event.data {
                    // Parse the vote type from the text (e.g., "kick 3" -> "kick")
                    let vote_type = vote_text.split_whitespace().next().unwrap_or("").to_lowercase();

                    // Check blocked votes
                    if self.blocked_votes.contains(&vote_type) {
                        if let Some(client_id) = event.client_id {
                            let cid_str = client_id.to_string();
                            ctx.message(&cid_str, &format!("^1Callvote '{}' is not allowed", vote_type)).await?;
                            // Cancel the vote
                            ctx.write("veto").await?;
                        }
                        return Ok(());
                    }

                    // Check minimum level
                    if self.min_level > 0 {
                        if let Some(client_id) = event.client_id {
                            if let Some(client) = ctx.clients.get_by_cid(&client_id.to_string()).await {
                                if client.max_level() < self.min_level {
                                    ctx.message(
                                        &client_id.to_string(),
                                        "^1You do not have permission to call votes",
                                    ).await?;
                                    ctx.write("veto").await?;
                                }
                            }
                        }
                    }
                }
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
        Some(vec!["EVT_CLIENT_CALLVOTE".to_string()])
    }
}
