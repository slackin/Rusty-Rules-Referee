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

// ---------------------------------------------------------------------------
// Player Groups — shared cross-server permission records
// ---------------------------------------------------------------------------

/// A named collection of player permission records that can be assigned to
/// multiple game servers. Servers inherit the union of all assigned groups'
/// records plus their own local `clients.group_bits` entries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerGroup {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A single player entry inside a `PlayerGroup`. `client_guid` links to
/// `clients.guid` for name/alias lookups; `group_bits` uses the same bitmask
/// encoding as `clients.group_bits`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerGroupMember {
    pub id: i64,
    pub player_group_id: i64,
    pub client_guid: String,
    /// Display name resolved from the `clients` table (not stored).
    #[serde(default)]
    pub client_name: Option<String>,
    pub group_bits: u64,
    pub note: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Effective player permission row returned by `GET /servers/:id/users`.
/// Merges group-sourced entries and local server entries. `source` identifies
/// where the permission comes from so the UI can show which group set it.
#[derive(Debug, Clone, Serialize)]
pub struct EffectiveUser {
    pub client_guid: String,
    pub client_name: Option<String>,
    pub group_bits: u64,
    /// "Local" or the player group name (e.g. "ATL Admins").
    pub source: String,
    /// FK to `player_groups.id` when `source != "Local"`.
    pub player_group_id: Option<i64>,
}

/// A known player as listed on a server's Users tab. Unlike [`EffectiveUser`]
/// (permissions only), this includes *every* client that has connected to the
/// server, plus members of any player group assigned to it.
///
/// Identity is anchored on GUID in storage (always present), but `auth` — the
/// in-game FrozenSand account — is the strongest identity signal and is shown
/// as the primary identifier when available. Ban-evasion matching prefers auth,
/// then GUID, IP, and alias.
#[derive(Debug, Clone, Serialize)]
pub struct KnownUser {
    pub client_id: i64,
    pub client_guid: String,
    /// FrozenSand auth account — primary identity signal when present.
    pub auth: Option<String>,
    pub client_name: Option<String>,
    pub ip: Option<String>,
    pub last_seen: Option<DateTime<Utc>>,
    /// Effective permission level on this server (max of local + group bits).
    pub group_bits: u64,
    /// Where the permission/listing comes from: "Local", a group name, or
    /// "Seen" (connected but no special permissions).
    pub source: String,
    /// FK to `player_groups.id` when the permission comes from a group.
    pub player_group_id: Option<i64>,
    /// True when this client currently has an active Ban/TempBan.
    pub banned: bool,
    /// Ban-evasion signals matched against *other* banned accounts. Any of
    /// "ip", "guid", "auth", "alias". Empty when no match.
    pub evasion: Vec<String>,
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
    /// Whether auto-update is enabled on this server bot. Master-controlled
    /// toggle; pushed to the client via heartbeat response so the client
    /// can start or pause its update loop without a restart.
    pub update_enabled: bool,
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
    /// Whether auto-update is enabled on this hub. Master-controlled
    /// toggle; pushed to the hub via heartbeat response.
    pub update_enabled: bool,
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

/// A public-submitted bug/feature report queued for admin triage.
#[derive(Debug, Clone, Serialize)]
pub struct BugReport {
    pub id: i64,
    pub title: String,
    pub description: String,
    pub steps: String,
    /// low | normal | high | critical
    pub severity: String,
    pub reporter_email: Option<String>,
    /// new | triaged | approved | in_progress | completed | failed | rejected
    pub status: String,
    pub ip_address: Option<String>,
    pub admin_notes: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// An AI fix job: one attempt to resolve a bug report via the Copilot CLI.
#[derive(Debug, Clone, Serialize)]
pub struct BugJob {
    pub id: i64,
    pub bug_report_id: i64,
    /// Copilot model id used for this run.
    pub model: String,
    /// queued | running | testing | deploying | success | failed | cancelled
    pub status: String,
    pub branch_name: String,
    pub git_commit: Option<String>,
    /// Streamed agent + build output.
    pub log: String,
    pub error: Option<String>,
    /// admin_users.id that approved this job.
    pub created_by: Option<i64>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
