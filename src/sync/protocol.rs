//! Shared protocol types for master/client communication.
//!
//! These types are used in both REST and WebSocket messages between
//! the master server and game server bot clients.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Sent by a client bot when it first connects to the master.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRequest {
    /// Human-readable server name.
    pub server_name: String,
    /// Public IP of the game server.
    pub address: String,
    /// Game server port.
    pub port: u16,
    /// SHA-256 fingerprint of the client TLS certificate.
    pub cert_fingerprint: String,
}

/// Returned by the master after a successful registration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterResponse {
    /// Assigned server ID in the master database.
    pub server_id: i64,
    /// Current config version on the master.
    pub config_version: i64,
}

// ---------------------------------------------------------------------------
// Heartbeat
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatRequest {
    pub server_id: i64,
    pub status: String,
    pub current_map: Option<String>,
    pub player_count: u32,
    pub max_clients: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatResponse {
    pub ok: bool,
    /// If the master has newer config, include the version so client can pull.
    pub config_version: i64,
    /// Global bans that the client should enforce (since last heartbeat).
    pub pending_global_bans: Vec<PenaltySync>,
}

// ---------------------------------------------------------------------------
// Event sync
// ---------------------------------------------------------------------------

/// A batch of events sent from client to master.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventBatch {
    pub server_id: i64,
    pub events: Vec<EventPayload>,
}

/// A single event in the batch, serialized for transport.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventPayload {
    pub event_type: String,
    pub timestamp: DateTime<Utc>,
    pub client_id: Option<i64>,
    pub target_id: Option<i64>,
    pub data: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Penalty sync
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PenaltySync {
    /// Penalty ID on the originating server.
    pub origin_id: i64,
    pub penalty_type: String,
    pub client_guid: String,
    pub client_name: String,
    pub admin_name: Option<String>,
    pub reason: String,
    pub duration: Option<i64>,
    pub scope: PenaltyScope,
    pub server_id: i64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PenaltyScope {
    Local,
    Global,
}

// ---------------------------------------------------------------------------
// Player sync
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerSync {
    pub guid: String,
    pub name: String,
    pub ip: Option<String>,
    pub group_bits: u64,
    pub aliases: Vec<String>,
}

/// Batch of player data pushed from client to master.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerSyncBatch {
    pub server_id: i64,
    pub players: Vec<PlayerSync>,
}

// ---------------------------------------------------------------------------
// Config sync
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSync {
    pub server_id: i64,
    pub config_json: String,
    pub config_version: i64,
}

/// Game server configuration payload pushed from master to client.
/// This is what gets serialized into the `config_json` DB column.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfigPayload {
    pub address: String,
    pub port: u16,
    pub rcon_password: String,
    #[serde(default)]
    pub game_log: Option<String>,
}

// ---------------------------------------------------------------------------
// WebSocket messages (bidirectional)
// ---------------------------------------------------------------------------

/// Messages sent over the internal WebSocket between master and client.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum SyncMessage {
    // -- Client → Master --
    /// Real-time event stream.
    Event(EventPayload),
    /// Batch of events (queued offline).
    EventBatch(EventBatch),
    /// Penalty notification.
    Penalty(PenaltySync),
    /// Player data sync.
    PlayerSync(PlayerSyncBatch),
    /// Heartbeat ping.
    Heartbeat(HeartbeatRequest),

    // -- Master → Client --
    /// Execute a command on the game server.
    Command(RemoteCommand),
    /// Configuration update pushed from master.
    ConfigUpdate(ConfigSync),
    /// Global penalty to enforce.
    GlobalPenalty(PenaltySync),
    /// Heartbeat acknowledgement.
    HeartbeatAck(HeartbeatResponse),
    /// Error/status message.
    Status(StatusMessage),
}

/// A command sent from master to client for execution via RCON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteCommand {
    /// Unique ID to correlate responses.
    pub command_id: String,
    pub action: RemoteAction,
}

/// Possible remote actions that the master can request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", content = "params")]
pub enum RemoteAction {
    /// Execute raw RCON command.
    Rcon { command: String },
    /// Kick a player by slot ID.
    Kick { cid: String, reason: String },
    /// Ban a player by slot ID.
    Ban { cid: String, reason: String },
    /// Temporary ban.
    TempBan { cid: String, reason: String, duration_minutes: i64 },
    /// Unban by client database ID.
    Unban { client_id: i64 },
    /// Send a public message.
    Say { message: String },
    /// Send a private message.
    Message { cid: String, message: String },
}

/// Result of a remote command execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    pub command_id: String,
    pub success: bool,
    pub output: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusMessage {
    pub code: u16,
    pub message: String,
}
