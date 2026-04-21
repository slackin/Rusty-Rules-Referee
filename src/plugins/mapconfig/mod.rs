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

    /// Build a list of RCON commands from a MapConfig. `current_gametype`
    /// is the server's current `g_gametype` as a string, used to enforce
    /// `supported_gametypes` (if the current isn't allowed, we switch to
    /// `default_gametype` instead of the stored `gametype`).
    fn build_commands(config: &MapConfig, current_gametype: Option<&str>) -> Vec<String> {
        let mut cmds = Vec::new();

        // Resolve the gametype to apply.
        let target_gt = Self::resolve_gametype(config, current_gametype);
        if !target_gt.is_empty() {
            cmds.push(format!("g_gametype {}", target_gt));
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
        if let Some(v) = config.g_teamdamage {
            cmds.push(format!("g_teamdamage {}", v));
        }
        if let Some(v) = config.g_suddendeath {
            cmds.push(format!("g_suddendeath {}", v));
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

    /// Pick the gametype to apply based on `supported_gametypes` and the
    /// server's current `g_gametype`. Rules:
    ///   * If `supported_gametypes` is empty, apply `gametype` as-is.
    ///   * If the current gametype is already in the supported set, leave
    ///     it alone (return empty string so no `g_gametype` cvar is sent).
    ///   * Otherwise apply `default_gametype` if set, else `gametype`.
    fn resolve_gametype(config: &MapConfig, current: Option<&str>) -> String {
        if config.supported_gametypes.trim().is_empty() {
            return config.gametype.clone();
        }
        let supported: Vec<&str> = config
            .supported_gametypes
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        if let Some(cur) = current.map(|s| s.trim()) {
            if supported.iter().any(|s| *s == cur) {
                return String::new();
            }
        }
        config
            .default_gametype
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| config.gametype.clone())
    }

    /// Apply a `MapConfig` to the live server via RCON. Public so the
    /// sync-layer "apply now" handler can reuse the logic.
    pub async fn apply_config(ctx: &BotContext, config: &MapConfig) {
        let current_gt = ctx.get_cvar("g_gametype").await.ok();
        let commands = Self::build_commands(config, current_gt.as_deref());
        info!(
            map = %config.map_name,
            commands = commands.len(),
            "Applying per-map configuration"
        );

        if !config.startmessage.is_empty() {
            let _ = ctx.say(&config.startmessage).await;
        }

        for cmd in &commands {
            match ctx.write(cmd).await {
                Ok(_) => {
                    info!(map = %config.map_name, cmd = %cmd, "Executed map config command");
                }
                Err(e) => {
                    info!(map = %config.map_name, cmd = %cmd, error = %e, "Failed to execute map config command");
                }
            }
        }
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

                // Auto-create a default config if absent so every map has one.
                match ctx.storage.ensure_map_config(&map_name).await {
                    Ok(config) => {
                        Self::apply_config(ctx, &config).await;
                    }
                    Err(e) => {
                        info!(map = %map_name, error = %e, "Failed to load or create map configuration");
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
