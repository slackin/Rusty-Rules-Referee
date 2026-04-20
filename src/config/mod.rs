use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::path::Path;

/// Run mode for the R3 binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RunMode {
    /// Single-server, self-contained mode (current behavior).
    #[default]
    Standalone,
    /// Central server: hosts database, web UI, manages game server bots.
    Master,
    /// Game server bot: runs plugins locally, syncs with a master server.
    Client,
}

impl fmt::Display for RunMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunMode::Standalone => write!(f, "standalone"),
            RunMode::Master => write!(f, "master"),
            RunMode::Client => write!(f, "client"),
        }
    }
}

impl std::str::FromStr for RunMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "standalone" => Ok(RunMode::Standalone),
            "master" => Ok(RunMode::Master),
            "client" => Ok(RunMode::Client),
            other => Err(format!("unknown run mode '{}': expected standalone, master, or client", other)),
        }
    }
}

/// Top-level Rusty Rules Referee configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RefereeConfig {
    pub referee: RefereeSection,
    pub server: ServerSection,
    #[serde(default)]
    pub web: WebSection,
    #[serde(default)]
    pub update: UpdateSection,
    #[serde(default)]
    pub master: Option<MasterSection>,
    #[serde(default)]
    pub client: Option<ClientSection>,
    #[serde(default)]
    pub plugins: Vec<PluginConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Deserialize, Serialize)]
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

impl ServerSection {
    /// Returns `true` if the game server connection is actually configured
    /// (i.e. not just placeholder values from a client-mode install).
    pub fn is_configured(&self) -> bool {
        !self.public_ip.is_empty()
            && self.public_ip != "0.0.0.0"
            && self.port > 0
            && !self.rcon_password.is_empty()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PluginConfig {
    pub name: String,
    #[serde(default)]
    pub config_file: Option<String>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub settings: Option<toml::Table>,
}

fn default_enabled() -> bool {
    true
}

/// Web admin UI configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebSection {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_bind_address")]
    pub bind_address: String,
    #[serde(default = "default_web_port")]
    pub port: u16,
    #[serde(default)]
    pub jwt_secret: Option<String>,
}

impl Default for WebSection {
    fn default() -> Self {
        Self {
            enabled: false,
            bind_address: default_bind_address(),
            port: default_web_port(),
            jwt_secret: None,
        }
    }
}

fn default_bind_address() -> String {
    "0.0.0.0".to_string()
}

fn default_web_port() -> u16 {
    8080
}

/// Auto-update configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UpdateSection {
    /// Whether auto-update checking is enabled.
    #[serde(default)]
    pub enabled: bool,
    /// URL of the update server (serves latest.json).
    #[serde(default = "default_update_url")]
    pub url: String,
    /// How often (seconds) to check for updates.
    #[serde(default = "default_update_interval")]
    pub check_interval: u64,
    /// Whether to automatically restart after applying an update.
    #[serde(default = "default_auto_restart")]
    pub auto_restart: bool,
}

impl Default for UpdateSection {
    fn default() -> Self {
        Self {
            enabled: false,
            url: default_update_url(),
            check_interval: default_update_interval(),
            auto_restart: default_auto_restart(),
        }
    }
}

fn default_update_url() -> String {
    "https://r3.pugbot.net/api/updates".to_string()
}

fn default_update_interval() -> u64 {
    3600
}

fn default_auto_restart() -> bool {
    true
}

/// Master server configuration (used when running in master mode).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MasterSection {
    /// Address to bind the internal sync API on.
    #[serde(default = "default_bind_address")]
    pub bind_address: String,
    /// Port for the internal mTLS sync API.
    #[serde(default = "default_master_port")]
    pub port: u16,
    /// Path to the server TLS certificate (PEM).
    pub tls_cert: String,
    /// Path to the server TLS private key (PEM).
    pub tls_key: String,
    /// Path to the CA certificate for verifying client certs (PEM).
    pub ca_cert: String,
    /// Path to the CA private key (used to sign client certs during pairing).
    #[serde(default)]
    pub ca_key: String,
    /// Whether the quick-connect pairing endpoint is currently enabled.
    #[serde(default)]
    pub quick_connect_enabled: bool,
    /// The active quick-connect token (set via web UI).
    #[serde(default)]
    pub quick_connect_token: Option<String>,
    /// Expiry timestamp for the quick-connect token (ISO 8601).
    #[serde(default)]
    pub quick_connect_expiry: Option<String>,
}

fn default_master_port() -> u16 {
    9443
}

/// Client bot configuration (used when running in client mode).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClientSection {
    /// URL of the master server, e.g. "https://master.example.com:9443".
    pub master_url: String,
    /// Human-readable name for this game server.
    pub server_name: String,
    /// Path to the client TLS certificate (PEM).
    pub tls_cert: String,
    /// Path to the client TLS private key (PEM).
    pub tls_key: String,
    /// Path to the CA certificate for verifying the master (PEM).
    pub ca_cert: String,
    /// How often (seconds) to batch-sync data with master.
    #[serde(default = "default_sync_interval")]
    pub sync_interval: u64,
    /// How often (seconds) to send heartbeat to master.
    #[serde(default = "default_heartbeat_interval")]
    pub heartbeat_interval: u64,
}

fn default_sync_interval() -> u64 {
    30
}

fn default_heartbeat_interval() -> u64 {
    15
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

    /// Validate the config for the given run mode.
    pub fn validate_for_mode(&self, mode: RunMode) -> anyhow::Result<()> {
        match mode {
            RunMode::Standalone => {
                // Standalone requires server + database — already enforced by TOML parsing
                Ok(())
            }
            RunMode::Master => {
                if self.master.is_none() {
                    anyhow::bail!("Master mode requires a [master] config section");
                }
                // TLS cert/key/ca paths can be empty — they'll be auto-generated on first startup
                Ok(())
            }
            RunMode::Client => {
                if self.client.is_none() {
                    anyhow::bail!("Client mode requires a [client] config section with master_url and TLS paths");
                }
                let c = self.client.as_ref().unwrap();
                if c.master_url.is_empty() {
                    anyhow::bail!("Client mode requires master_url in [client]");
                }
                if c.tls_cert.is_empty() || c.tls_key.is_empty() || c.ca_cert.is_empty() {
                    anyhow::bail!("Client mode requires tls_cert, tls_key, and ca_cert paths in [client]");
                }
                if c.server_name.is_empty() {
                    anyhow::bail!("Client mode requires server_name in [client]");
                }
                Ok(())
            }
        }
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
