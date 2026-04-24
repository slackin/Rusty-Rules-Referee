use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// User group (permission level).
#[derive(Debug, Clone, Serialize)]
pub struct Group {
    pub id: u64,
    pub name: String,
    pub keyword: String,
    pub level: u32,
    pub time_add: DateTime<Utc>,
    pub time_edit: DateTime<Utc>,
}

/// Types of penalty that can be applied to a client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PenaltyType {
    Warning,
    Notice,
    Kick,
    Ban,
    TempBan,
    Mute,
}

/// A penalty record (ban, kick, warning, etc.).
#[derive(Debug, Clone, Serialize)]
pub struct Penalty {
    pub id: i64,
    pub penalty_type: PenaltyType,
    pub client_id: i64,
    pub admin_id: Option<i64>,
    pub duration: Option<i64>,
    pub reason: String,
    pub keyword: String,
    pub inactive: bool,
    pub time_add: DateTime<Utc>,
    pub time_edit: DateTime<Utc>,
    pub time_expire: Option<DateTime<Utc>>,
    /// Originating server (NULL for global/legacy rows).
    #[serde(default)]
    pub server_id: Option<i64>,
}

/// An alias record — tracks a name that a client has used.
#[derive(Debug, Clone, Serialize)]
pub struct Alias {
    pub id: i64,
    pub client_id: i64,
    pub alias: String,
    pub num_used: u32,
    pub time_add: DateTime<Utc>,
    pub time_edit: DateTime<Utc>,
}

/// An admin user for the web UI.
#[derive(Debug, Clone, Serialize)]
pub struct AdminUser {
    pub id: i64,
    pub username: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub role: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// An audit log entry.
#[derive(Debug, Clone, Serialize)]
pub struct AuditEntry {
    pub id: i64,
    pub admin_user_id: Option<i64>,
    pub action: String,
    pub detail: String,
    pub ip_address: Option<String>,
    pub created_at: DateTime<Utc>,
    /// Originating server (NULL for global/legacy rows).
    #[serde(default)]
    pub server_id: Option<i64>,
}

/// A persisted chat message.
#[derive(Debug, Clone, Serialize)]
pub struct ChatMessage {
    pub id: i64,
    pub client_id: i64,
    pub client_name: String,
    pub channel: String,
    pub message: String,
    pub time_add: DateTime<Utc>,
    /// Originating server (NULL for global/legacy rows).
    #[serde(default)]
    pub server_id: Option<i64>,
}

/// A persisted vote history entry.
#[derive(Debug, Clone, Serialize)]
pub struct VoteRecord {
    pub id: i64,
    pub client_id: i64,
    pub client_name: String,
    pub vote_type: String,
    pub vote_data: String,
    pub time_add: DateTime<Utc>,
}

/// A personal admin note (dashboard scratchpad).
#[derive(Debug, Clone, Serialize)]
pub struct AdminNote {
    pub id: i64,
    pub admin_user_id: i64,
    pub content: String,
    pub updated_at: DateTime<Utc>,
}

/// Dashboard summary statistics.
#[derive(Debug, Clone, Serialize)]
pub struct DashboardSummary {
    pub total_clients: u64,
    pub total_warnings: u64,
    pub total_tempbans: u64,
    pub total_bans: u64,
}

/// A registered game server (used in master/client mode).
#[derive(Debug, Clone, Serialize)]
pub struct GameServer {
    pub id: i64,
    pub name: String,
    pub address: String,
    pub port: u16,
    pub status: String,
    pub current_map: Option<String>,
    pub player_count: u32,
    pub max_clients: u32,
    pub last_seen: Option<DateTime<Utc>>,
    pub config_json: Option<String>,
    pub config_version: i64,
    pub cert_fingerprint: Option<String>,
    /// Release channel this server's bot follows for updates
    /// (one of `production`, `beta`, `alpha`, `dev`). Master-controlled.
    pub update_channel: String,
    /// Auto-update check interval in seconds. Master-controlled; pushed to
    /// the client via heartbeat response.
    pub update_interval: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// FK to `hubs.id` when this client lives on a hub-managed host.
    /// `None` for standalone-installed clients.
    #[serde(default)]
    pub hub_id: Option<i64>,
    /// Stable systemd-instance slug used by the owning hub
    /// (`r3-client@<slug>.service`). `None` for non-hub clients.
    #[serde(default)]
    pub slug: Option<String>,
}

/// A host orchestrator (hub) registered with the master. Hubs install,
/// start, stop and uninstall R3 client bots on their physical host.
#[derive(Debug, Clone, Serialize)]
pub struct Hub {
    pub id: i64,
    pub name: String,
    /// Network address the hub last contacted the master from.
    pub address: String,
    pub status: String,
    pub last_seen: Option<DateTime<Utc>>,
    pub cert_fingerprint: Option<String>,
    pub hub_version: Option<String>,
    pub build_hash: Option<String>,
    /// Release channel this hub pulls R3 updates from (production|beta|alpha|dev).
    pub update_channel: String,
    /// Auto-update check interval in seconds. Master-controlled; pushed
    /// back to the hub in every heartbeat response so changes take effect
    /// without a restart.
    pub update_interval: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Static-ish host information reported by a hub on register / heartbeat.
#[derive(Debug, Clone, Serialize)]
pub struct HubHostInfo {
    pub hub_id: i64,
    pub hostname: String,
    pub os: String,
    pub kernel: String,
    pub cpu_model: String,
    pub cpu_cores: u32,
    pub total_ram_bytes: u64,
    pub disk_total_bytes: u64,
    pub public_ip: Option<String>,
    pub external_ip: Option<String>,
    /// JSON-encoded list of detected UrT installs on the host.
    pub urt_installs_json: String,
    pub updated_at: DateTime<Utc>,
}

/// Periodic point-in-time host metric sample.
#[derive(Debug, Clone, Serialize)]
pub struct HubMetricSample {
    pub hub_id: i64,
    pub ts: DateTime<Utc>,
    pub cpu_pct: f32,
    pub mem_pct: f32,
    pub disk_pct: f32,
    pub load1: f32,
    pub load5: f32,
    pub load15: f32,
    pub uptime_s: u64,
}

/// An entry in the offline sync queue (used by client bots).
#[derive(Debug, Clone, Serialize)]
pub struct SyncQueueEntry {
    pub id: i64,
    pub entity_type: String,
    pub entity_id: Option<i64>,
    pub action: String,
    pub payload: String,
    pub server_id: Option<i64>,
    pub retry_count: i32,
    pub created_at: DateTime<Utc>,
    pub synced_at: Option<DateTime<Utc>>,
}

/// Per-map server configuration applied on map change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapConfig {
    #[serde(default)]
    pub id: i64,
    pub map_name: String,
    pub gametype: String,
    pub capturelimit: Option<i32>,
    pub timelimit: Option<i32>,
    pub fraglimit: Option<i32>,
    pub g_gear: String,
    pub g_gravity: Option<i32>,
    pub g_friendlyfire: Option<i32>,
    pub g_followstrict: Option<i32>,
    pub g_waverespawns: Option<i32>,
    pub g_bombdefusetime: Option<i32>,
    pub g_bombexplodetime: Option<i32>,
    pub g_swaproles: Option<i32>,
    pub g_maxrounds: Option<i32>,
    pub g_matchmode: Option<i32>,
    pub g_respawndelay: Option<i32>,
    pub startmessage: String,
    pub skiprandom: i32,
    pub bot: i32,
    pub custom_commands: String,
    /// CSV of gametype ids the map supports. Empty = all allowed.
    #[serde(default)]
    pub supported_gametypes: String,
    /// Gametype to switch to if current `g_gametype` is not in
    /// `supported_gametypes`. Falls back to `gametype` when None.
    #[serde(default)]
    pub default_gametype: Option<String>,
    /// g_suddendeath cvar (0/1). Separate from friendly-fire.
    #[serde(default)]
    pub g_suddendeath: Option<i32>,
    /// g_teamdamage cvar (0/1). Distinct from `g_friendlyfire`.
    #[serde(default)]
    pub g_teamdamage: Option<i32>,
    /// 'user' | 'auto' | 'default_seed' — used by UI to flag unedited rows.
    #[serde(default = "default_source_user")]
    pub source: String,
    #[serde(default = "default_now")]
    pub created_at: DateTime<Utc>,
    #[serde(default = "default_now")]
    pub updated_at: DateTime<Utc>,
}

fn default_now() -> DateTime<Utc> {
    Utc::now()
}

fn default_source_user() -> String {
    "user".to_string()
}

/// Global (master-only) template for map configuration. Mirrors
/// `MapConfig` minus `id`/`server_id`. `map_name` is the primary key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapConfigDefault {
    pub map_name: String,
    #[serde(default)]
    pub gametype: String,
    #[serde(default)]
    pub supported_gametypes: String,
    #[serde(default)]
    pub default_gametype: Option<String>,
    pub capturelimit: Option<i32>,
    pub timelimit: Option<i32>,
    pub fraglimit: Option<i32>,
    #[serde(default)]
    pub g_gear: String,
    pub g_gravity: Option<i32>,
    pub g_friendlyfire: Option<i32>,
    pub g_teamdamage: Option<i32>,
    pub g_suddendeath: Option<i32>,
    pub g_followstrict: Option<i32>,
    pub g_waverespawns: Option<i32>,
    pub g_bombdefusetime: Option<i32>,
    pub g_bombexplodetime: Option<i32>,
    pub g_swaproles: Option<i32>,
    pub g_maxrounds: Option<i32>,
    pub g_matchmode: Option<i32>,
    pub g_respawndelay: Option<i32>,
    #[serde(default)]
    pub startmessage: String,
    #[serde(default)]
    pub skiprandom: i32,
    #[serde(default)]
    pub bot: i32,
    #[serde(default)]
    pub custom_commands: String,
    #[serde(default = "default_now")]
    pub created_at: DateTime<Utc>,
    #[serde(default = "default_now")]
    pub updated_at: DateTime<Utc>,
}

/// A single `.pk3` map file cached from an external map repository.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapRepoEntry {
    /// Exact filename including `.pk3` extension. Primary key.
    pub filename: String,
    /// File size in bytes, if reported by the index.
    #[serde(default)]
    pub size: Option<i64>,
    /// Last-modified timestamp string as reported by the index (free-form,
    /// e.g. `2024-05-01 12:30`). Parseable formats vary across mirrors.
    #[serde(default)]
    pub mtime: Option<String>,
    /// Absolute URL to download the `.pk3` from.
    pub source_url: String,
    /// When this entry was last observed on one of the configured sources.
    pub last_seen_at: DateTime<Utc>,
}

/// A single installed map reported by a game server's engine via
/// `fdir *.bsp`. Cached per-server and refreshed on a schedule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerMap {
    /// Map name without the `.bsp` extension (e.g. `ut4_turnpike`).
    pub map_name: String,
    /// Best-effort `.pk3` filename if known (filled in at import time; left
    /// unset by engine-only scans, since `fdir *.bsp` doesn't reveal which
    /// pk3 provided a given `.bsp`).
    #[serde(default)]
    pub pk3_filename: Option<String>,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    /// True when a `.pk3` was imported but the game engine has not yet
    /// re-scanned its filesystem (UrT caches the filesystem at startup and
    /// only rediscovers new pk3 files on `fs_restart` or map change).
    #[serde(default)]
    pub pending_restart: bool,
}

/// Status of the most recent installed-maps scan for a single server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerMapScanStatus {
    pub last_scan_at: Option<DateTime<Utc>>,
    pub last_scan_ok: bool,
    pub last_scan_error: Option<String>,
    pub map_count: i64,
}
