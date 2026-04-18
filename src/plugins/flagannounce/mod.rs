use async_trait::async_trait;
use tracing::info;

use crate::core::context::BotContext;
use crate::events::Event;
use crate::plugins::{Plugin, PluginInfo};

/// Announces flag captures, drops, pickups, and returns with big text messages.
pub struct FlagannouncePlugin {
    enabled: bool,
}

impl FlagannouncePlugin {
    pub fn new() -> Self {
        Self { enabled: true }
    }
}

impl Default for FlagannouncePlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Plugin for FlagannouncePlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "flagannounce",
            description: "Announces flag events with big text",
            requires_config: false,
            requires_plugins: &[],
            requires_parsers: &[],
            requires_storage: &[],
            load_after: &[],
        }
    }

    async fn on_startup(&mut self) -> anyhow::Result<()> {
        info!("Flagannounce plugin started");
        Ok(())
    }

    async fn on_event(&self, event: &Event, ctx: &BotContext) -> anyhow::Result<()> {
        let Some(client_id) = event.client_id else {
            return Ok(());
        };

        let Some(event_key) = ctx.event_registry.get_key(event.event_type) else {
            return Ok(());
        };

        let cid_str = client_id.to_string();
        let player_name = ctx
            .clients
            .get_by_cid(&cid_str)
            .await
            .map(|c| c.name.clone())
            .unwrap_or_else(|| format!("Player#{}", client_id));

        match event_key {
            "EVT_CLIENT_FLAG_PICKUP" => {
                ctx.bigtext(&format!("^2{} ^7picked up the flag!", player_name))
                    .await?;
            }
            "EVT_CLIENT_FLAG_DROPPED" => {
                ctx.bigtext(&format!("^2{} ^7dropped the flag!", player_name))
                    .await?;
            }
            "EVT_CLIENT_FLAG_CAPTURED" => {
                ctx.bigtext(&format!("^2{} ^3CAPTURED ^7the flag!", player_name))
                    .await?;
            }
            "EVT_CLIENT_FLAG_RETURNED" => {
                ctx.bigtext(&format!("^2{} ^7returned the flag!", player_name))
                    .await?;
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
            "EVT_CLIENT_FLAG_PICKUP".to_string(),
            "EVT_CLIENT_FLAG_DROPPED".to_string(),
            "EVT_CLIENT_FLAG_CAPTURED".to_string(),
            "EVT_CLIENT_FLAG_RETURNED".to_string(),
        ])
    }
}
