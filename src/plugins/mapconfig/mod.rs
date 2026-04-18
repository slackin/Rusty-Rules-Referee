use async_trait::async_trait;
use std::collections::HashMap;
use tracing::info;

use crate::core::context::BotContext;
use crate::events::{Event, EventData};
use crate::plugins::{Plugin, PluginInfo};

/// Per-map configuration plugin — executes map-specific RCON commands on map change.
pub struct MapconfigPlugin {
    enabled: bool,
    /// Map name -> list of RCON commands to execute.
    map_configs: HashMap<String, Vec<String>>,
}

impl MapconfigPlugin {
    pub fn new() -> Self {
        Self {
            enabled: true,
            map_configs: HashMap::new(),
        }
    }

    /// Add a map configuration with a list of RCON commands.
    pub fn add_map_config(&mut self, map_name: &str, commands: Vec<String>) {
        self.map_configs.insert(map_name.to_string(), commands);
    }
}

impl Default for MapconfigPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Plugin for MapconfigPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "mapconfig",
            description: "Executes map-specific RCON commands on map change",
            requires_config: true,
            requires_plugins: &[],
            requires_parsers: &[],
            requires_storage: &[],
            load_after: &[],
        }
    }

    async fn on_startup(&mut self) -> anyhow::Result<()> {
        info!(
            maps = self.map_configs.len(),
            "Mapconfig plugin started"
        );
        Ok(())
    }

    async fn on_event(&self, event: &Event, ctx: &BotContext) -> anyhow::Result<()> {
        let Some(event_key) = ctx.event_registry.get_key(event.event_type) else {
            return Ok(());
        };

        match event_key {
            "EVT_GAME_MAP_CHANGE" => {
                let map_name = match &event.data {
                    EventData::MapChange { new, .. } => new.clone(),
                    EventData::Text(text) => text.clone(),
                    _ => return Ok(()),
                };

                if let Some(commands) = self.map_configs.get(&map_name) {
                    info!(map = %map_name, commands = commands.len(), "Applying map configuration");
                    for cmd in commands {
                        match ctx.write(cmd).await {
                            Ok(_) => {
                                info!(map = %map_name, cmd = %cmd, "Executed map config command");
                            }
                            Err(e) => {
                                info!(map = %map_name, cmd = %cmd, error = %e, "Failed to execute map config command");
                            }
                        }
                    }
                } else {
                    info!(map = %map_name, "No map configuration found");
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
        Some(vec!["EVT_GAME_MAP_CHANGE".to_string()])
    }
}
