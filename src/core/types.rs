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
