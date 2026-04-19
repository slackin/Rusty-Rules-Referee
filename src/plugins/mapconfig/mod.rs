use async_trait::async_trait;
use tracing::info;

use crate::core::context::BotContext;
use crate::core::MapConfig;
use crate::events::{Event, EventData};
use crate::plugins::{Plugin, PluginInfo};

/// Per-map configuration plugin — applies map-specific server settings on map change.
///
/// Settings are stored in the database (map_configs table) and managed via the web UI.
/// On map change, the plugin reads the config for the new map and issues RCON commands
/// to set the appropriate cvars.
pub struct MapconfigPlugin {
    enabled: bool,
}

impl MapconfigPlugin {
    pub fn new() -> Self {
        Self {
            enabled: true,
        }
    }

    /// Build a list of RCON commands from a MapConfig.
    fn build_commands(config: &MapConfig) -> Vec<String> {
        let mut cmds = Vec::new();

        if !config.gametype.is_empty() {
            cmds.push(format!("g_gametype {}", config.gametype));
        }
        if let Some(v) = config.capturelimit {
            cmds.push(format!("capturelimit {}", v));
        }
        if let Some(v) = config.timelimit {
            cmds.push(format!("timelimit {}", v));
        }
        if let Some(v) = config.fraglimit {
            cmds.push(format!("fraglimit {}", v));
        }
        if !config.g_gear.is_empty() {
            cmds.push(format!("g_gear {}", config.g_gear));
        }
        if let Some(v) = config.g_gravity {
            cmds.push(format!("g_gravity {}", v));
        }
        if let Some(v) = config.g_friendlyfire {
            cmds.push(format!("g_friendlyfire {}", v));
        }
        if let Some(v) = config.g_followstrict {
            cmds.push(format!("g_followstrict {}", v));
        }
        if let Some(v) = config.g_waverespawns {
            cmds.push(format!("g_waverespawns {}", v));
        }
        if let Some(v) = config.g_bombdefusetime {
            cmds.push(format!("g_bombdefusetime {}", v));
        }
        if let Some(v) = config.g_bombexplodetime {
            cmds.push(format!("g_bombexplodetime {}", v));
        }
        if let Some(v) = config.g_swaproles {
            cmds.push(format!("g_swaproles {}", v));
        }
        if let Some(v) = config.g_maxrounds {
            cmds.push(format!("g_maxrounds {}", v));
        }
        if let Some(v) = config.g_matchmode {
            cmds.push(format!("g_matchmode {}", v));
        }
        if let Some(v) = config.g_respawndelay {
            cmds.push(format!("g_respawndelay {}", v));
        }
        if config.bot > 0 {
            cmds.push(format!("bot_minplayers {}", config.bot));
        }

        // Custom RCON commands (one per line)
        for line in config.custom_commands.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                cmds.push(trimmed.to_string());
            }
        }

        cmds
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
            description: "Applies per-map server settings on map change",
            requires_config: false,
            requires_plugins: &[],
            requires_parsers: &[],
            requires_storage: &[],
            load_after: &[],
        }
    }

    async fn on_load_config(&mut self, _settings: Option<&toml::Table>) -> anyhow::Result<()> {
        Ok(())
    }

    async fn on_startup(&mut self) -> anyhow::Result<()> {
        info!("Mapconfig plugin started (database-backed)");
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

                match ctx.storage.get_map_config(&map_name).await {
                    Ok(Some(config)) => {
                        let commands = Self::build_commands(&config);
                        info!(map = %map_name, commands = commands.len(), "Applying per-map configuration");

                        if !config.startmessage.is_empty() {
                            let _ = ctx.say(&config.startmessage).await;
                        }

                        for cmd in &commands {
                            match ctx.write(cmd).await {
                                Ok(_) => {
                                    info!(map = %map_name, cmd = %cmd, "Executed map config command");
                                }
                                Err(e) => {
                                    info!(map = %map_name, cmd = %cmd, error = %e, "Failed to execute map config command");
                                }
                            }
                        }
                    }
                    Ok(None) => {
                        info!(map = %map_name, "No per-map configuration found");
                    }
                    Err(e) => {
                        info!(map = %map_name, error = %e, "Failed to load map configuration");
                    }
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
