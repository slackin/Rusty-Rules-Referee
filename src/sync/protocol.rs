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
    /// Client's current build hash (set by newer clients; absent on older builds).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_hash: Option<String>,
    /// Client's current semantic version string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatResponse {
    pub ok: bool,
    /// If the master has newer config, include the version so client can pull.
    pub config_version: i64,
    /// Global bans that the client should enforce (since last heartbeat).
    pub pending_global_bans: Vec<PenaltySync>,
    /// Master-controlled update channel for this server. When present and
    /// different from the client's current channel, the client updates its
    /// local config and uses this channel for subsequent update checks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update_channel: Option<String>,
    /// Master-controlled auto-update check interval in seconds. When present
    /// and different from the client's current interval, the client updates
    /// its local config and uses this interval for subsequent update checks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update_interval: Option<u64>,
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
///
/// The core fields (address/port/rcon_password/game_log) describe how the
/// bot talks to its game server. Optional `bot` and `plugins` fields carry
/// the full bot-level settings the master has authority over, giving the
/// master full per-server control (see docs/plan). Both are optional for
/// backward compatibility — older clients parsing a new payload just ignore
/// them, and older payloads without these fields still deserialize cleanly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfigPayload {
    pub address: String,
    pub port: u16,
    pub rcon_password: String,
    #[serde(default)]
    pub game_log: Option<String>,
    /// Optional absolute path to the game server's primary `server.cfg`
    /// file, used by the server.cfg editor on the client.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_cfg_path: Option<String>,
    /// Optional RCON IP override (if RCON is reachable on a different IP).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rcon_ip: Option<String>,
    /// Optional RCON port override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rcon_port: Option<u16>,
    /// Log-tail polling delay in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delay: Option<f64>,
    /// Bot-level settings (name, prefix, log level). `None` leaves the
    /// client's current `[referee]` section untouched.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bot: Option<BotSettingsPayload>,
    /// Full plugin list with enabled/settings. `None` leaves the client's
    /// current `[[plugins]]` array untouched; `Some(vec)` overwrites it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugins: Option<Vec<PluginConfigPayload>>,
}

/// Bot-level settings carried on `ServerConfigPayload`. Matches the
/// `[referee]` section of the TOML config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotSettingsPayload {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bot_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bot_prefix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_level: Option<String>,
}

/// One entry in the per-server plugin list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfigPayload {
    pub name: String,
    #[serde(default = "crate::sync::protocol::default_plugin_enabled")]
    pub enabled: bool,
    /// Free-form settings table (matches `[plugins.settings]` in TOML).
    #[serde(default)]
    pub settings: serde_json::Value,
}

pub(crate) fn default_plugin_enabled() -> bool {
    true
}

// ---------------------------------------------------------------------------
// Client request/response (master → client → master)
// ---------------------------------------------------------------------------

/// Requests that the master can send to a client bot for execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "request_type", content = "params")]
pub enum ClientRequest {
    /// Scan known game directories for .cfg files.
    ScanConfigFiles,
    /// Read and parse a specific server.cfg file.
    ParseConfigFile { path: String },
    /// Browse a directory on the client filesystem.
    BrowseFiles { path: String },
    /// Download and install a fresh UrT 4.3 dedicated server.
    InstallGameServer { install_path: String },
    /// Poll install progress.
    InstallStatus,
    /// Query the client's current version/build.
    GetVersion,
    /// Force the client to check for and apply an update immediately.
    /// If `update_url` is provided, it overrides the client's configured URL.
    /// If `channel` is provided, it overrides the client's configured channel.
    ForceUpdate {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        update_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        channel: Option<String>,
    },
    /// Validate a game-log path on the client filesystem.
    CheckGameLog { path: String },
    /// Restart the client bot process. The current process will exit and
    /// re-exec itself (assumes a process supervisor or re-exec logic keeps it
    /// running, same as after an update).
    Restart,

    // --- Live server control (per-server parity with standalone UI) ---
    /// Get the full live status (game state + scoreboard) from the client's
    /// RCON/state.
    GetLiveStatus,
    /// Get the connected-player scoreboard from the client's in-memory state.
    GetPlayers,
    /// List all maps available on the game server.
    ListMaps,
    /// Change the current map via RCON.
    ChangeMap { map: String },
    /// Mute a player via RCON.
    MutePlayer { cid: String },
    /// Unmute a player via RCON.
    UnmutePlayer { cid: String },
    /// Read the current mapcycle file contents.
    GetMapcycle,
    /// Overwrite the mapcycle file with the given ordered list of maps.
    SetMapcycle { maps: Vec<String> },
    /// Read the current server.cfg contents.
    GetServerCfg,
    /// Write new contents to a config file on the client filesystem.
    SaveConfigFile { path: String, contents: String },
    /// List the per-map config entries stored on the client.
    ListMapConfigs,
    /// Save (create or update) a per-map config entry on the client.
    /// The `config` payload is a JSON object matching the `MapConfig` struct
    /// (id is optional for creation).
    SaveMapConfig { config: serde_json::Value },
    /// Delete a per-map config entry by id on the client.
    DeleteMapConfig { id: i64 },
    /// Ensure a `map_configs` row exists for `map_name` on the client,
    /// creating one from master-seeded defaults / built-ins if absent.
    /// Returns the (possibly newly-created) `MapConfig`.
    EnsureMapConfig { map_name: String },
    /// Apply an existing `map_configs` row to the live server immediately
    /// (re-issues all RCON commands without waiting for a map change).
    ApplyMapConfig { map_name: String },
    /// Reset a per-map config row back to its default / built-in values
    /// (effectively deletes the existing row then re-ensures).
    ResetMapConfig { map_name: String },
    /// Download a `.pk3` map file from the master-approved URL and place it
    /// into the game server's `q3ut4/` directory. Filename is validated to
    /// prevent path traversal and the URL host must be on the allowlist.
    DownloadMapPk3 {
        url: String,
        filename: String,
        #[serde(default)]
        allowed_hosts: Vec<String>,
    },

    /// Read a single server cvar via RCON. Returns the trimmed current
    /// value in the `Ok { data: { value, raw } }` response.
    GetCvar { name: String },
    /// Set a single server cvar via RCON. `value` is forwarded verbatim;
    /// callers are responsible for quoting / validation.
    SetCvar { name: String, value: String },

    // --- Game server install wizard ---
    /// Return the local UrT install-state marker + suggested defaults
    /// (free install dir, free UDP port, server name).
    SuggestInstallDefaults,
    /// Probe a list of ports for availability. Combines passive `ss`
    /// parsing with an active bind probe for each requested port.
    DetectPorts {
        ports: Vec<u16>,
        #[serde(default = "default_port_kind")]
        kind: PortKind,
    },
    /// Run the full install wizard end-to-end: download files (if missing),
    /// generate server.cfg, and optionally register a systemd unit.
    InstallGameServerWizard { params: GameServerWizardParams },
    /// Start/stop/restart/status of the managed `urt@<slug>.service`.
    GameServerService { action: ServiceAction },
}

/// UDP or TCP, for port-availability probes.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PortKind {
    Udp,
    Tcp,
}

pub(crate) fn default_port_kind() -> PortKind {
    PortKind::Udp
}

/// Wizard-collected parameters used to generate a complete `server.cfg`
/// and (optionally) register a systemd service for the game server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameServerWizardParams {
    /// Absolute path where UrT 4.3 files live (or will be placed if missing).
    pub install_path: String,
    /// sv_hostname — colored UrT string allowed.
    pub hostname: String,
    /// Public IP used for RCON and server list registration.
    pub public_ip: String,
    /// UDP port (net_port) the game server should bind.
    pub port: u16,
    /// RCON password (required, non-empty).
    pub rcon_password: String,
    /// Game mode label — one of: FFA, LMS, TDM, TS, FTL, CAH, CTF, BOMB, JUMP, FREEZE, GUNGAME.
    pub game_mode: String,
    /// sv_maxclients — total slot count.
    pub max_clients: u16,
    /// Optional admin (referee) password, distinct from rconPassword.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub admin_password: Option<String>,
    /// If true, install `/etc/systemd/system/urt@.service.d/<slug>.conf`
    /// and enable the instance. Requires the one-time scaffolding laid
    /// down by the shell installer.
    #[serde(default)]
    pub register_systemd: bool,
    /// Optional slug override; defaults to the client's own service slug.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    /// If true, (re-)download the UrT tarball even if files are already present.
    #[serde(default)]
    pub force_download: bool,
}

/// systemd action for the managed `urt@<slug>.service`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ServiceAction {
    Start,
    Stop,
    Restart,
    Status,
    Enable,
    Disable,
}

/// Responses from a client bot back to the master.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "response_type", content = "data")]
pub enum ClientResponse {
    /// List of config files found on the client filesystem.
    ConfigFiles { files: Vec<ConfigFileEntry> },
    /// Directory listing for the file browser.
    DirectoryListing {
        path: String,
        entries: Vec<DirEntry>,
    },
    /// Parsed config from a server.cfg file.
    ParsedConfig {
        settings: ServerConfigPayload,
        checks: Vec<ConfigCheck>,
        all_settings: Vec<CfgSetting>,
        raw: String,
    },
    /// Game server installation started.
    InstallStarted,
    /// Game server installation progress.
    InstallProgress {
        stage: String,
        percent: u8,
        error: Option<String>,
    },
    /// Game server installation completed.
    InstallComplete {
        install_path: String,
        game_log: Option<String>,
    },
    /// Current client version info.
    Version {
        version: String,
        build_hash: String,
        git_commit: String,
        build_timestamp: String,
        platform: String,
    },
    /// A force-update operation was accepted and is now running asynchronously.
    UpdateTriggered {
        current_build: String,
        target_build: String,
        target_version: String,
        download_size: u64,
    },
    /// Force-update found no newer build available.
    AlreadyUpToDate {
        current_build: String,
    },
    /// Restart request accepted; the client will exit/re-exec shortly.
    Restarting {
        current_build: String,
    },
    /// Result of a game-log path check.
    GameLogCheck {
        path: String,
        /// Resolved absolute path (canonicalized) if the file exists.
        resolved_path: Option<String>,
        /// True when the file exists, is a regular file, and is readable.
        ok: bool,
        exists: bool,
        is_file: bool,
        readable: bool,
        /// File size in bytes, if known.
        size: Option<u64>,
        /// Seconds since the file was last modified, if known.
        modified_secs_ago: Option<u64>,
        /// Human-readable explanation (success or error message).
        message: String,
    },

    // --- Live server control responses ---
    /// Live status snapshot (map, game type, scoreboard, etc.).
    LiveStatus {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        map: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        game_type: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hostname: Option<String>,
        player_count: u32,
        max_clients: u32,
        players: Vec<LivePlayer>,
        /// Extra RCON/state fields (cvar snapshot) that the client chose to
        /// include. Free-form to keep the payload future-proof.
        #[serde(default)]
        extra: serde_json::Value,
    },
    /// Connected-player list (lighter than LiveStatus).
    Players { players: Vec<LivePlayer> },
    /// List of maps available on the game server.
    MapList { maps: Vec<String> },
    /// Current mapcycle contents.
    Mapcycle {
        /// Path to the mapcycle file on the client filesystem, if known.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<String>,
        maps: Vec<String>,
    },
    /// Current server.cfg file contents.
    ServerCfg {
        path: String,
        contents: String,
    },
    /// List of per-map config entries stored on the client.
    MapConfigs { entries: serde_json::Value },
    /// A `.pk3` was downloaded successfully onto the client filesystem.
    MapDownloaded {
        /// Absolute path to the written file.
        path: String,
        /// Final file size in bytes.
        size: u64,
    },

    // --- Install wizard responses ---
    /// Local install-state marker + suggested defaults for the wizard.
    InstallDefaults {
        /// Current install-state marker contents (null if marker is missing).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        state: Option<UrtInstallState>,
        /// Default install path suggestion (`$HOME/urbanterror-<slug>`).
        suggested_install_path: String,
        /// Default free UDP port suggestion.
        suggested_port: u16,
        /// Suggested slug (derived from this client's own service slug).
        suggested_slug: String,
        /// Default server-name suggestion.
        suggested_server_name: String,
        /// Whether the one-time systemd scaffolding (urt@.service template +
        /// sudoers drop-in) is present on this host. If false, register_systemd
        /// requests will fail and the UI should surface a hint.
        scaffolding_present: bool,
    },
    /// Result of a port-availability probe.
    PortReport {
        kind: PortKind,
        results: Vec<PortProbeResult>,
    },
    /// Wizard install complete: returns final paths + (optional) service name.
    InstallWizardComplete {
        install_path: String,
        server_cfg_path: String,
        mapcycle_path: Option<String>,
        game_log: String,
        service_name: Option<String>,
    },
    /// Status of the managed `urt@<slug>.service` after an action.
    GameServerServiceStatus {
        service_name: String,
        action: String,
        active: bool,
        enabled: bool,
        /// Trimmed last lines from `systemctl status` for display.
        status_excerpt: String,
    },

    /// Generic success acknowledgement.
    Ok {
        #[serde(default)]
        message: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        data: Option<serde_json::Value>,
    },

    /// Error response.
    Error { message: String },
}

/// A single connected player as reported by the client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LivePlayer {
    pub cid: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team: Option<String>,
    #[serde(default)]
    pub score: i32,
    #[serde(default)]
    pub ping: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub db_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<u32>,
}

/// A config file found during filesystem scan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigFileEntry {
    pub path: String,
    pub size: u64,
    pub modified: Option<String>,
}

/// A directory entry returned by the file browser.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
}

/// A health check result from parsing a server.cfg.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigCheck {
    pub key: String,
    pub status: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix_value: Option<String>,
}

/// A single key-value setting from a server.cfg file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CfgSetting {
    pub key: String,
    pub value: String,
}

/// On-disk install-state marker for the client bot's managed UrT instance.
/// Lives at `$INSTALL_DIR/state/urt-install.json`. Written by the shell
/// installer (initially) and updated by the wizard once configuration
/// completes successfully.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UrtInstallState {
    /// Instance slug (shared with `r3-<slug>.service`).
    #[serde(default)]
    pub slug: String,
    /// Whether UrT game files are present on disk.
    #[serde(default)]
    pub files_present: bool,
    /// Install path if the files are present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub install_path: Option<String>,
    /// Whether a server.cfg has been generated and the instance is ready
    /// to run (optionally as a managed service).
    #[serde(default)]
    pub configured: bool,
    /// systemd service name (`urt@<slug>.service`) if register_systemd was
    /// used, otherwise `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_name: Option<String>,
    /// UDP port recorded at configuration time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    /// Path to the generated server.cfg (for convenience in the UI).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_cfg_path: Option<String>,
    /// Path to the games.log file for this instance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game_log: Option<String>,
}

/// Result of probing a single port for availability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortProbeResult {
    pub port: u16,
    /// True if the port appears free (not in use and bindable by us).
    pub available: bool,
    /// True if `ss` reports the port as bound by some process.
    pub ss_bound: bool,
    /// True if we successfully opened and released a socket on this port.
    pub bind_succeeded: bool,
    /// Human-readable explanation (mostly for in-use ports: process, owner).
    pub detail: String,
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

    // -- Bidirectional request/response --
    /// Request from master to client.
    Request {
        request_id: String,
        request: ClientRequest,
    },
    /// Response from client to master.
    Response {
        request_id: String,
        response: ClientResponse,
    },
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
    /// Fully uninstall the R3 client from the host: stop + disable the
    /// systemd unit, remove install dir, certs, DB, and optionally the
    /// UrT game server files. The client process exits as part of the
    /// handler; master must not expect a response.
    SelfUninstall {
        #[serde(default = "default_true_bool")]
        remove_gameserver: bool,
    },
}

fn default_true_bool() -> bool {
    true
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

// ---------------------------------------------------------------------------
// Request polling (client polls master for pending requests)
// ---------------------------------------------------------------------------

/// Response from `GET /internal/requests/:server_id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingRequestsResponse {
    pub requests: Vec<PendingRequestItem>,
}

/// A single pending request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingRequestItem {
    pub request_id: String,
    pub request: ClientRequest,
}

/// Sent by client via `POST /internal/responses`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientResponseSubmission {
    pub request_id: String,
    pub response: ClientResponse,
}

// ---------------------------------------------------------------------------
// Hub orchestration (master ↔ hub)
// ---------------------------------------------------------------------------

/// Static + slow-changing host facts reported by a hub.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HostInfoPayload {
    pub hostname: String,
    pub os: String,
    pub kernel: String,
    pub cpu_model: String,
    pub cpu_cores: u32,
    pub total_ram_bytes: u64,
    pub disk_total_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_ip: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_ip: Option<String>,
    /// JSON-encoded list of detected UrT installations (`[{path, version, size_bytes}]`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub urt_installs_json: Option<String>,
}

/// Periodic host metric sample reported on each heartbeat.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HostMetricSample {
    pub cpu_pct: f32,
    pub mem_pct: f32,
    pub disk_pct: f32,
    pub load1: f32,
    pub load5: f32,
    pub load15: f32,
    pub uptime_s: u64,
}

/// Per-client status reported by a hub on each heartbeat.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubClientStatus {
    /// systemd instance slug (`r3-client@<slug>.service`).
    pub slug: String,
    /// Master-assigned `server_id` (None until first successful pair).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_id: Option<i64>,
    /// systemd active state (active, inactive, failed, ...).
    pub systemd_state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rss_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_log_line: Option<String>,
}

/// Initial registration of a hub with the master after pairing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubRegisterRequest {
    pub hub_name: String,
    pub address: String,
    pub cert_fingerprint: String,
    pub version: String,
    pub build_hash: String,
    pub host_info: HostInfoPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubRegisterResponse {
    pub hub_id: i64,
}

/// Periodic hub heartbeat.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubHeartbeatRequest {
    pub hub_id: i64,
    /// Re-sent only when something changed since last heartbeat.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_info: Option<HostInfoPayload>,
    pub metrics: HostMetricSample,
    pub clients: Vec<HubClientStatus>,
    pub version: String,
    pub build_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubHeartbeatResponse {
    pub ok: bool,
    /// Pending actions queued by the master for this hub.
    #[serde(default)]
    pub pending_actions: Vec<PendingHubActionItem>,
    /// Authoritative release channel the master wants this hub to follow.
    /// When set, hubs should adopt it for their own auto-update loop and
    /// persist it locally so it survives restarts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update_channel: Option<String>,
}

/// Actions the master can ask a hub to perform.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action_type", content = "params")]
pub enum HubAction {
    /// Install a new R3 client on the hub host.
    InstallClient {
        slug: String,
        server_name: String,
        /// Optional immediate game-server install; if `Some`, the hub will
        /// also install UrT and wire the client to it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        game_server: Option<GameServerWizardParams>,
        #[serde(default = "default_register_systemd")]
        register_systemd: bool,
    },
    UninstallClient {
        slug: String,
        #[serde(default)]
        remove_data: bool,
    },
    StartClient { slug: String },
    StopClient { slug: String },
    RestartClient { slug: String },
    /// Install only a UrT game server (no client).
    InstallGameServer {
        slug: String,
        params: GameServerWizardParams,
    },
    RemoveGameServer { slug: String },
    /// Force the per-client R3 binary to re-pull and apply an update.
    UpdateClient { slug: String },
    /// Read the last `tail_lines` of a client's journald log.
    GetClientLogs {
        slug: String,
        #[serde(default = "default_tail_lines")]
        tail_lines: u32,
    },
    /// Report the hub's own version/build.
    GetHubVersion,
    /// Restart the hub process itself (re-exec).
    Restart,
    /// Force the hub to check for and apply an R3 update.
    ForceUpdate {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        update_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        channel: Option<String>,
    },
    /// Fully uninstall the hub (and every client it manages) from the
    /// host. The hub iterates its managed `r3-client@*` units, runs
    /// per-client cleanup, then spawns the uninstaller as a transient
    /// systemd unit and exits.
    SelfUninstall {
        #[serde(default = "default_true_bool")]
        remove_gameserver: bool,
        /// Base URL of the master's update endpoint, e.g.
        /// `https://r3.pugbot.net/api/updates`. The hub appends
        /// `/uninstall-r3.sh` to fetch the uninstaller. When `None`
        /// the hub falls back to its configured default.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        update_url: Option<String>,
    },
}

fn default_register_systemd() -> bool {
    true
}

fn default_tail_lines() -> u32 {
    200
}

/// A queued hub action with its tracking ID.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingHubActionItem {
    pub action_id: String,
    pub action: HubAction,
}

/// Result envelope returned by the hub for each completed action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubResponse {
    pub action_id: String,
    pub ok: bool,
    pub message: String,
    /// Optional structured payload (logs, version info, install paths, ...).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// Intermediate progress event pushed by the hub while executing a
/// long-running action (e.g. `InstallClient`). The master stores these
/// in an in-memory per-action log so the UI can stream them to the admin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubProgressEvent {
    pub action_id: String,
    /// Monotonically increasing counter starting at 0, used by the UI to
    /// dedupe and order events.
    pub seq: u32,
    /// Short machine-readable step identifier (e.g. "mint_cert",
    /// "install_client", "gs_install").
    pub step: String,
    /// Human-readable message for this step.
    pub message: String,
    /// Optional progress percent (0–100) for steps that can report it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub percent: Option<u8>,
    pub ts: chrono::DateTime<chrono::Utc>,
}

/// Hub asks the master to mint a fresh client certificate for a sub-client
/// it is provisioning. Authenticated by the hub's own mTLS cert.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MintClientCertRequest {
    pub hub_id: i64,
    pub slug: String,
    pub server_name: String,
    pub address: String,
    #[serde(default)]
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MintClientCertResponse {
    pub server_id: i64,
    pub ca_cert: String,
    pub client_cert: String,
    pub client_key: String,
    pub master_sync_url: String,
}
