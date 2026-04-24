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
    /// Host orchestrator: pairs with a master, reports host telemetry, and
    /// installs / starts / stops / uninstalls R3 client bots and game
    /// servers on this physical host.
    Hub,
}

impl fmt::Display for RunMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunMode::Standalone => write!(f, "standalone"),
            RunMode::Master => write!(f, "master"),
            RunMode::Client => write!(f, "client"),
            RunMode::Hub => write!(f, "hub"),
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
            "hub" => Ok(RunMode::Hub),
            other => Err(format!(
                "unknown run mode '{}': expected standalone, master, client, or hub",
                other
            )),
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
    pub hub: Option<HubSection>,
    #[serde(default)]
    pub map_repo: MapRepoSection,
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
    /// Optional absolute path to the game server's primary `server.cfg`
    /// file (as selected during setup). When set, admin tooling (e.g.
    /// the server.cfg editor) will prefer this path over auto-discovery
    /// from the `game_log` home directory.
    #[serde(default)]
    pub server_cfg_path: Option<String>,
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
    2727
}

/// Auto-update configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UpdateSection {
    /// Whether auto-update checking is enabled. Defaults to `true` so that
    /// fleet-managed hubs and client bots stay current on the configured
    /// channel; operators who want to pin a build can set this to `false`.
    #[serde(default = "default_update_enabled")]
    pub enabled: bool,
    /// URL of the update server (serves `<channel>/latest.json`).
    #[serde(default = "default_update_url")]
    pub url: String,
    /// Release channel to follow: `production`, `beta`, `alpha`, or `dev`.
    #[serde(default = "default_update_channel")]
    pub channel: String,
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
            enabled: default_update_enabled(),
            url: default_update_url(),
            channel: default_update_channel(),
            check_interval: default_update_interval(),
            auto_restart: default_auto_restart(),
        }
    }
}

fn default_update_enabled() -> bool {
    true
}

fn default_update_url() -> String {
    "https://r3.pugbot.net/api/updates".to_string()
}

fn default_update_channel() -> String {
    "beta".to_string()
}

/// Valid release channels, in order of stability (most stable first).
pub const VALID_UPDATE_CHANNELS: &[&str] = &["production", "beta", "alpha", "dev"];

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

/// Hub configuration (used when running in hub mode).
///
/// A hub pairs with a master like a client does, but instead of running a
/// game-server bot itself it manages a fleet of `r3-client@<slug>.service`
/// systemd units on the local host (each one a full R3 client bot with its
/// own config, certs, and database).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HubSection {
    /// URL of the master server, e.g. "https://master.example.com:9443".
    pub master_url: String,
    /// Human-readable name for this hub (typically the hostname).
    pub hub_name: String,
    /// Path to the hub TLS certificate (PEM).
    pub tls_cert: String,
    /// Path to the hub TLS private key (PEM).
    pub tls_key: String,
    /// Path to the CA certificate for verifying the master (PEM).
    pub ca_cert: String,
    /// Directory under which per-client install dirs are created.
    /// Each managed client lives at `<clients_root>/<slug>/` with its own
    /// `r3.toml`, `certs/`, and `r3.db`.
    #[serde(default = "default_clients_root")]
    pub clients_root: String,
    /// Default base directory for new Urban Terror installs requested via
    /// the master UI. The hub will create per-instance subdirectories
    /// underneath this path (e.g. `<urt_install_root>/<slug>/`).
    #[serde(default = "default_urt_install_root")]
    pub urt_install_root: String,
    /// Absolute path to the `rusty-rules-referee` binary the hub uses
    /// when launching managed clients. Defaults to the hub's own
    /// executable path (resolved at runtime when empty).
    #[serde(default)]
    pub r3_binary_path: Option<String>,
    /// systemd template unit name used for managed clients. Instances
    /// are addressed as `<systemd_unit_template>@<slug>.service` after
    /// stripping a trailing `@.service` from this value.
    #[serde(default = "default_hub_systemd_template")]
    pub systemd_unit_template: String,
    /// How often (seconds) to send a heartbeat (with metrics) to master.
    #[serde(default = "default_hub_heartbeat_interval")]
    pub heartbeat_interval: u64,
    /// How often (seconds) to refresh static host info (CPU model,
    /// detected UrT installs, etc.). Cheap deltas only; metrics ride on
    /// every heartbeat.
    #[serde(default = "default_hub_host_refresh_interval")]
    pub host_refresh_interval: u64,
}

fn default_clients_root() -> String {
    "clients".to_string()
}

fn default_urt_install_root() -> String {
    "urbanterror".to_string()
}

fn default_hub_systemd_template() -> String {
    "r3-client@.service".to_string()
}

fn default_hub_heartbeat_interval() -> u64 {
    30
}

fn default_hub_host_refresh_interval() -> u64 {
    300
}

/// External `.pk3` map repository configuration.
///
/// The master periodically scrapes each listed HTML autoindex, persists a
/// searchable cache of `.pk3` filenames in the `map_repo_entries` table, and
/// lets admins one-click import selected maps onto a game server.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MapRepoSection {
    /// Whether the map repo browser (and background refresher) is enabled.
    #[serde(default = "default_map_repo_enabled")]
    pub enabled: bool,
    /// Ordered list of autoindex URLs to scrape. Each URL must serve an
    /// Apache/nginx-style directory listing of `.pk3` files (trailing slash
    /// required). Filename conflicts across sources are resolved by "last
    /// seen wins" on refresh.
    #[serde(default = "default_map_repo_sources")]
    pub sources: Vec<String>,
    /// Background refresh interval in hours. Set to 0 to disable the timer
    /// (manual `POST /api/v1/map-repo/refresh` still works).
    #[serde(default = "default_map_repo_refresh_hours")]
    pub refresh_interval_hours: u32,
    /// How often the master should ask each connected game server for its
    /// list of installed `.bsp` maps (via `fdir *.bsp` over RCON) and
    /// refresh the per-server cache in the `server_maps` table. Set to 0
    /// to disable periodic scans (on-connect and manual refresh still
    /// work).
    #[serde(default = "default_map_scan_hours")]
    pub scan_interval_hours: u32,
    /// Optional explicit target directory for imported `.pk3` files. When
    /// unset (the default), the bot auto-discovers a writable directory by
    /// trying in order: the parent of `server.game_log`, then `fs_homepath`
    /// and `fs_basepath` reported by the game server. Set this only if the
    /// auto-discovered location is not writable by the bot process (e.g.
    /// when the bot runs under systemd with `ProtectHome=yes` or a
    /// read-only mount). The directory must already exist.
    #[serde(default)]
    pub download_dir: Option<String>,
}

impl Default for MapRepoSection {
    fn default() -> Self {
        Self {
            enabled: default_map_repo_enabled(),
            sources: default_map_repo_sources(),
            refresh_interval_hours: default_map_repo_refresh_hours(),
            scan_interval_hours: default_map_scan_hours(),
            download_dir: None,
        }
    }
}

fn default_map_repo_enabled() -> bool {
    true
}

fn default_map_scan_hours() -> u32 {
    24
}

fn default_map_repo_sources() -> Vec<String> {
    vec![
        "https://maps.pugbot.net/q3ut4/".to_string(),
        "https://urt.li/q3ut4/".to_string(),
    ]
}

fn default_map_repo_refresh_hours() -> u32 {
    24
}

impl RefereeConfig {
    /// Load configuration from a TOML file.
    pub fn from_file(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: RefereeConfig = toml::from_str(&content)?;
        if !VALID_UPDATE_CHANNELS.contains(&config.update.channel.as_str()) {
            anyhow::bail!(
                "Invalid [update] channel '{}' — expected one of: {}",
                config.update.channel,
                VALID_UPDATE_CHANNELS.join(", ")
            );
        }
        Ok(config)
    }

    /// Fleet-managed bots (hubs and master-paired clients) are expected to
    /// auto-update on their channel. Early installer templates shipped with
    /// `[update].enabled = false`, so legacy installs silently never auto-
    /// update. On startup we detect that stale default and rewrite the
    /// config file in place so the running process and any future restart
    /// both honour the new behaviour. Returns `true` when the file was
    /// rewritten.
    pub fn migrate_update_enabled_default(path: &Path) -> anyhow::Result<bool> {
        let content = std::fs::read_to_string(path)?;
        let mut doc: toml::Value = toml::from_str(&content)?;
        let Some(table) = doc.as_table_mut() else {
            return Ok(false);
        };
        let update_tbl = table
            .entry("update".to_string())
            .or_insert_with(|| toml::Value::Table(toml::value::Table::new()));
        let Some(update_map) = update_tbl.as_table_mut() else {
            return Ok(false);
        };
        let currently_false = matches!(
            update_map.get("enabled"),
            Some(toml::Value::Boolean(false))
        );
        if !currently_false {
            return Ok(false);
        }
        update_map.insert("enabled".to_string(), toml::Value::Boolean(true));
        let output = toml::to_string_pretty(&doc)?;
        std::fs::write(path, &output)?;
        Ok(true)
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
            RunMode::Hub => {
                let h = self.hub.as_ref().ok_or_else(|| {
                    anyhow::anyhow!(
                        "Hub mode requires a [hub] config section with master_url and TLS paths"
                    )
                })?;
                if h.master_url.is_empty() {
                    anyhow::bail!("Hub mode requires master_url in [hub]");
                }
                if h.tls_cert.is_empty() || h.tls_key.is_empty() || h.ca_cert.is_empty() {
                    anyhow::bail!("Hub mode requires tls_cert, tls_key, and ca_cert paths in [hub]");
                }
                if h.hub_name.is_empty() {
                    anyhow::bail!("Hub mode requires hub_name in [hub]");
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
