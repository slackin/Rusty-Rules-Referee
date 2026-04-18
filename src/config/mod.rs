use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// Top-level Rusty Rules Referee configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct RefereeConfig {
    pub referee: RefereeSection,
    pub server: ServerSection,
    #[serde(default)]
    pub plugins: Vec<PluginConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RefereeSection {
    pub bot_name: String,
    #[serde(default = "default_bot_prefix")]
    pub bot_prefix: String,
    pub database: String,
    pub logfile: String,
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

fn default_bot_prefix() -> String {
    "^2RRR:^3".to_string()
}

fn default_log_level() -> String {
    "info".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerSection {
    pub public_ip: String,
    pub port: u16,
    #[serde(default)]
    pub rcon_ip: Option<String>,
    #[serde(default)]
    pub rcon_port: Option<u16>,
    pub rcon_password: String,
    #[serde(default)]
    pub game_log: Option<String>,
    #[serde(default = "default_delay")]
    pub delay: f64,
}

fn default_delay() -> f64 {
    0.33
}

#[derive(Debug, Clone, Deserialize)]
pub struct PluginConfig {
    pub name: String,
    #[serde(default)]
    pub config_file: Option<String>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

impl RefereeConfig {
    /// Load configuration from a TOML file.
    pub fn from_file(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: RefereeConfig = toml::from_str(&content)?;
        Ok(config)
    }

    /// Get the effective RCON IP (falls back to public_ip).
    pub fn rcon_ip(&self) -> &str {
        self.server
            .rcon_ip
            .as_deref()
            .unwrap_or(&self.server.public_ip)
    }

    /// Get the effective RCON port (falls back to game port).
    pub fn rcon_port(&self) -> u16 {
        self.server.rcon_port.unwrap_or(self.server.port)
    }
}

/// Default messages used when a game-specific message isn't configured.
pub fn default_messages() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert(
        "kicked_by".to_string(),
        "$clientname^7 was kicked by $adminname^7 $reason".to_string(),
    );
    m.insert(
        "kicked".to_string(),
        "$clientname^7 was kicked $reason".to_string(),
    );
    m.insert(
        "banned_by".to_string(),
        "$clientname^7 was banned by $adminname^7 $reason".to_string(),
    );
    m.insert(
        "banned".to_string(),
        "$clientname^7 was banned $reason".to_string(),
    );
    m.insert(
        "temp_banned_by".to_string(),
        "$clientname^7 was temp banned by $adminname^7 for $banduration^7 $reason".to_string(),
    );
    m.insert(
        "temp_banned".to_string(),
        "$clientname^7 was temp banned for $banduration^7 $reason".to_string(),
    );
    m.insert(
        "unbanned_by".to_string(),
        "$clientname^7 was un-banned by $adminname^7 $reason".to_string(),
    );
    m.insert(
        "unbanned".to_string(),
        "$clientname^7 was un-banned $reason".to_string(),
    );
    m
}
