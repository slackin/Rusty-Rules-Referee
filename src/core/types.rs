use chrono::{DateTime, Utc};
use serde::Serialize;

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
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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
#[derive(Debug, Clone, Serialize)]
pub struct MapConfig {
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
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
