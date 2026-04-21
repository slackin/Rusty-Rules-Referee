//! Multi-server management API endpoints (master mode).
//!
//! These endpoints allow the web UI to list, inspect, and send commands
//! to connected game-server bots through the master's sync layer.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};

use crate::sync::protocol::{RemoteAction, RemoteCommand, SyncMessage, ServerConfigPayload, ConfigSync, ClientRequest, ClientResponse};
use crate::web::state::AppState;

// ---- Response types ----

#[derive(Debug, Serialize)]
pub struct ServerInfo {
    pub id: i64,
    pub name: String,
    pub address: String,
    pub port: u16,
    pub status: String,
    pub current_map: Option<String>,
    pub player_count: u32,
    pub max_clients: u32,
    pub last_seen: Option<String>,
    pub online: bool,
    /// Release channel this server's bot follows for updates.
    pub update_channel: String,
    /// Last client-reported build hash (from heartbeat).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_hash: Option<String>,
    /// Last client-reported version string (from heartbeat).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ServersListResponse {
    pub servers: Vec<ServerInfo>,
}

#[derive(Debug, Serialize)]
pub struct CommandResponse {
    pub ok: bool,
    pub message: String,
}

// ---- Request types ----

#[derive(Debug, Deserialize)]
pub struct RconRequest {
    pub command: String,
}

#[derive(Debug, Deserialize)]
pub struct KickRequest {
    pub cid: String,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BanRequest {
    pub cid: String,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub duration_minutes: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct SayRequest {
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct MessageRequest {
    pub cid: String,
    pub message: String,
}

// ---- Helpers ----

/// Send a command to a connected server. Returns error if server not found or not connected.
async fn send_to_server(
    state: &AppState,
    server_id: i64,
    action: RemoteAction,
) -> Result<(), (StatusCode, String)> {
    let clients = state
        .connected_clients
        .as_ref()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "Not running in master mode".to_string()))?;

    let clients_guard = clients.read().await;
    let client = clients_guard
        .get(&server_id)
        .ok_or((StatusCode::NOT_FOUND, format!("Server {} is not connected", server_id)))?;

    let cmd = SyncMessage::Command(RemoteCommand {
        command_id: uuid::Uuid::new_v4().to_string(),
        action,
    });

    client.tx.send(cmd).await.map_err(|e| {
        warn!(server_id, error = %e, "Failed to send command to server");
        (StatusCode::INTERNAL_SERVER_ERROR, "Failed to send command".to_string())
    })
}

// ---- Endpoints ----

/// Check if a server is online: either via WebSocket connection or recent heartbeat (REST polling).
fn is_server_online(ws_connected: bool, last_seen: Option<chrono::DateTime<chrono::Utc>>) -> bool {
    if ws_connected {
        return true;
    }
    // REST-polling clients send heartbeats every ~10s; consider online if seen in last 60s
    if let Some(ts) = last_seen {
        let age = chrono::Utc::now() - ts;
        return age.num_seconds() < 60;
    }
    false
}

/// GET /api/v1/servers — list all registered servers with live status.
pub async fn list_servers(
    State(state): State<AppState>,
) -> Result<Json<ServersListResponse>, StatusCode> {
    let servers = state.storage.get_servers().await.map_err(|e| {
        error!(error = %e, "Failed to list servers");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let connected = state.connected_clients.as_ref();
    let connected_ids: std::collections::HashSet<i64> = if let Some(c) = connected {
        c.read().await.keys().copied().collect()
    } else {
        std::collections::HashSet::new()
    };

    let versions: std::collections::HashMap<i64, (Option<String>, Option<String>)> =
        if let Some(map) = state.client_versions.as_ref() {
            map.read()
                .await
                .iter()
                .map(|(k, v)| (*k, (v.build_hash.clone(), v.version.clone())))
                .collect()
        } else {
            std::collections::HashMap::new()
        };

    let servers = servers
        .into_iter()
        .map(|s| {
            let online = is_server_online(connected_ids.contains(&s.id), s.last_seen);
            let (build_hash, version) = versions
                .get(&s.id)
                .cloned()
                .unwrap_or((None, None));
            ServerInfo {
                id: s.id,
                name: s.name,
                address: s.address,
                port: s.port,
                status: if online { s.status } else { "offline".to_string() },
                current_map: s.current_map,
                player_count: s.player_count,
                max_clients: s.max_clients,
                last_seen: s.last_seen.map(|t| t.to_rfc3339()),
                online,
                update_channel: s.update_channel,
                build_hash,
                version,
            }
        })
        .collect();

    Ok(Json(ServersListResponse { servers }))
}

/// GET /api/v1/servers/:id — get a single server's details.
pub async fn get_server(
    State(state): State<AppState>,
    Path(server_id): Path<i64>,
) -> Result<Json<ServerInfo>, StatusCode> {
    let s = state.storage.get_server(server_id).await.map_err(|_| StatusCode::NOT_FOUND)?;

    let ws_connected = if let Some(c) = state.connected_clients.as_ref() {
        c.read().await.contains_key(&server_id)
    } else {
        false
    };
    let online = is_server_online(ws_connected, s.last_seen);

    let (build_hash, version) = if let Some(map) = state.client_versions.as_ref() {
        map.read()
            .await
            .get(&server_id)
            .map(|v| (v.build_hash.clone(), v.version.clone()))
            .unwrap_or((None, None))
    } else {
        (None, None)
    };

    Ok(Json(ServerInfo {
        id: s.id,
        name: s.name,
        address: s.address,
        port: s.port,
        status: if online { s.status } else { "offline".to_string() },
        current_map: s.current_map,
        player_count: s.player_count,
        max_clients: s.max_clients,
        last_seen: s.last_seen.map(|t| t.to_rfc3339()),
        online,
        update_channel: s.update_channel,
        build_hash,
        version,
    }))
}

/// DELETE /api/v1/servers/:id — remove a server.
pub async fn delete_server(
    State(state): State<AppState>,
    Path(server_id): Path<i64>,
) -> Result<StatusCode, StatusCode> {
    state.storage.delete_server(server_id).await.map_err(|e| {
        error!(error = %e, "Failed to delete server");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    info!(server_id, "Server removed from master");
    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/v1/servers/:id/rcon — send an RCON command to a server.
pub async fn server_rcon(
    State(state): State<AppState>,
    Path(server_id): Path<i64>,
    Json(req): Json<RconRequest>,
) -> impl IntoResponse {
    info!(server_id, command = %req.command, "Remote RCON command");
    match send_to_server(&state, server_id, RemoteAction::Rcon { command: req.command }).await {
        Ok(()) => Json(CommandResponse { ok: true, message: "Command sent".to_string() }).into_response(),
        Err((status, msg)) => (status, Json(CommandResponse { ok: false, message: msg })).into_response(),
    }
}

/// POST /api/v1/servers/:id/kick — kick a player on a server.
pub async fn server_kick(
    State(state): State<AppState>,
    Path(server_id): Path<i64>,
    Json(req): Json<KickRequest>,
) -> impl IntoResponse {
    info!(server_id, cid = %req.cid, "Remote kick");
    match send_to_server(&state, server_id, RemoteAction::Kick {
        cid: req.cid,
        reason: req.reason.unwrap_or_default(),
    }).await {
        Ok(()) => Json(CommandResponse { ok: true, message: "Kick sent".to_string() }).into_response(),
        Err((status, msg)) => (status, Json(CommandResponse { ok: false, message: msg })).into_response(),
    }
}

/// POST /api/v1/servers/:id/ban — ban a player on a server.
pub async fn server_ban(
    State(state): State<AppState>,
    Path(server_id): Path<i64>,
    Json(req): Json<BanRequest>,
) -> impl IntoResponse {
    info!(server_id, cid = %req.cid, "Remote ban");
    let action = if let Some(mins) = req.duration_minutes {
        RemoteAction::TempBan {
            cid: req.cid,
            reason: req.reason.unwrap_or_default(),
            duration_minutes: mins,
        }
    } else {
        RemoteAction::Ban {
            cid: req.cid,
            reason: req.reason.unwrap_or_default(),
        }
    };
    match send_to_server(&state, server_id, action).await {
        Ok(()) => Json(CommandResponse { ok: true, message: "Ban sent".to_string() }).into_response(),
        Err((status, msg)) => (status, Json(CommandResponse { ok: false, message: msg })).into_response(),
    }
}

/// POST /api/v1/servers/:id/say — broadcast a message on a server.
pub async fn server_say(
    State(state): State<AppState>,
    Path(server_id): Path<i64>,
    Json(req): Json<SayRequest>,
) -> impl IntoResponse {
    info!(server_id, "Remote say");
    match send_to_server(&state, server_id, RemoteAction::Say { message: req.message }).await {
        Ok(()) => Json(CommandResponse { ok: true, message: "Message sent".to_string() }).into_response(),
        Err((status, msg)) => (status, Json(CommandResponse { ok: false, message: msg })).into_response(),
    }
}

/// POST /api/v1/servers/:id/message — private message a player on a server.
pub async fn server_message(
    State(state): State<AppState>,
    Path(server_id): Path<i64>,
    Json(req): Json<MessageRequest>,
) -> impl IntoResponse {
    info!(server_id, cid = %req.cid, "Remote message");
    match send_to_server(&state, server_id, RemoteAction::Message {
        cid: req.cid,
        message: req.message,
    }).await {
        Ok(()) => Json(CommandResponse { ok: true, message: "Message sent".to_string() }).into_response(),
        Err((status, msg)) => (status, Json(CommandResponse { ok: false, message: msg })).into_response(),
    }
}

// ---- Server config management ----

#[derive(Debug, Serialize)]
pub struct ServerConfigResponse {
    pub server_id: i64,
    pub config: Option<ServerConfigPayload>,
    pub config_version: i64,
}

/// GET /api/v1/servers/:id/config — get a server's game server configuration.
pub async fn get_server_config(
    State(state): State<AppState>,
    Path(server_id): Path<i64>,
) -> Result<Json<ServerConfigResponse>, StatusCode> {
    let server = state.storage.get_server(server_id).await.map_err(|_| StatusCode::NOT_FOUND)?;

    let config: Option<ServerConfigPayload> = server
        .config_json
        .as_deref()
        .and_then(|json| serde_json::from_str(json).ok());

    Ok(Json(ServerConfigResponse {
        server_id: server.id,
        config,
        config_version: server.config_version,
    }))
}

/// PUT /api/v1/servers/:id/config — update a server's game server configuration.
pub async fn update_server_config(
    State(state): State<AppState>,
    Path(server_id): Path<i64>,
    Json(mut payload): Json<ServerConfigPayload>,
) -> impl IntoResponse {
    // Validate
    if payload.address.is_empty() || payload.address == "0.0.0.0" {
        return (
            StatusCode::BAD_REQUEST,
            Json(CommandResponse { ok: false, message: "Game server address is required".to_string() }),
        ).into_response();
    }
    if payload.port == 0 {
        return (
            StatusCode::BAD_REQUEST,
            Json(CommandResponse { ok: false, message: "Game server port is required".to_string() }),
        ).into_response();
    }
    if payload.rcon_password.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(CommandResponse { ok: false, message: "RCON password is required".to_string() }),
        ).into_response();
    }

    // If the game_log path is a bare filename (no directory), assume the
    // default Urban Terror dedicated-server log location on Linux.
    if let Some(log) = payload.game_log.as_ref() {
        let trimmed = log.trim();
        if !trimmed.is_empty() && !trimmed.contains('/') && !trimmed.contains('\\') {
            let resolved = format!("~/.q3a/q3ut4/{}", trimmed);
            info!(original = %trimmed, resolved = %resolved, "Resolving bare game_log filename to default Linux path");
            payload.game_log = Some(resolved);
        } else if trimmed.is_empty() {
            payload.game_log = None;
        }
    }

    let mut server = match state.storage.get_server(server_id).await {
        Ok(s) => s,
        Err(_) => {
            return (StatusCode::NOT_FOUND, Json(CommandResponse {
                ok: false,
                message: "Server not found".to_string(),
            })).into_response();
        }
    };

    let config_json = match serde_json::to_string(&payload) {
        Ok(j) => j,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(CommandResponse {
                ok: false,
                message: format!("Failed to serialize config: {}", e),
            })).into_response();
        }
    };

    server.config_json = Some(config_json.clone());
    server.config_version += 1;
    server.address = payload.address.clone();
    server.port = payload.port;

    if let Err(e) = state.storage.save_server(&server).await {
        error!(error = %e, "Failed to save server config");
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(CommandResponse {
            ok: false,
            message: "Failed to save configuration".to_string(),
        })).into_response();
    }

    // Push to connected client if online
    if let Some(clients) = state.connected_clients.as_ref() {
        let clients_guard = clients.read().await;
        if let Some(client) = clients_guard.get(&server_id) {
            let msg = SyncMessage::ConfigUpdate(ConfigSync {
                server_id,
                config_json,
                config_version: server.config_version,
            });
            let _ = client.tx.send(msg).await;
            info!(server_id, "Config update pushed to connected client");
        }
    }

    info!(server_id, version = server.config_version, "Server config updated");

    Json(CommandResponse {
        ok: true,
        message: "Configuration saved and pushed".to_string(),
    }).into_response()
}

// ---- Server setup endpoints (config scan, install, browse) ----

#[derive(Debug, Deserialize)]
pub struct ParseConfigRequest {
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct BrowseFilesRequest {
    #[serde(default)]
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct InstallServerRequest {
    pub install_path: String,
}

#[derive(Debug, Deserialize)]
pub struct CheckGameLogRequest {
    pub path: String,
}

/// Helper: send a ClientRequest to a server via the polling infrastructure and await the response.
pub(crate) async fn send_client_request(
    state: &AppState,
    server_id: i64,
    request: ClientRequest,
    timeout: std::time::Duration,
) -> Result<ClientResponse, (StatusCode, String)> {
    let pending_responses = state
        .pending_responses
        .as_ref()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "Not running in master mode".to_string()))?;
    let pending_client_requests = state
        .pending_client_requests
        .as_ref()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "Not running in master mode".to_string()))?;

    crate::sync::master::send_request_to_server(
        pending_responses,
        pending_client_requests,
        server_id,
        request,
        timeout,
    )
    .await
    .map_err(|e| (StatusCode::GATEWAY_TIMEOUT, e))
}

/// POST /api/v1/servers/:id/scan-configs — ask the client to scan for .cfg files.
pub async fn scan_server_configs(
    State(state): State<AppState>,
    Path(server_id): Path<i64>,
) -> impl IntoResponse {
    info!(server_id, "Scanning for config files on client");
    match send_client_request(
        &state,
        server_id,
        ClientRequest::ScanConfigFiles,
        std::time::Duration::from_secs(60),
    ).await {
        Ok(resp) => Json(serde_json::to_value(&resp).unwrap_or_default()).into_response(),
        Err((status, msg)) => (status, Json(CommandResponse { ok: false, message: msg })).into_response(),
    }
}

/// POST /api/v1/servers/:id/browse — browse the client filesystem for config files.
pub async fn browse_server_files(
    State(state): State<AppState>,
    Path(server_id): Path<i64>,
    Json(req): Json<BrowseFilesRequest>,
) -> impl IntoResponse {
    info!(server_id, path = %req.path, "Browsing files on client");
    match send_client_request(
        &state,
        server_id,
        ClientRequest::BrowseFiles { path: req.path },
        std::time::Duration::from_secs(30),
    ).await {
        Ok(resp) => Json(serde_json::to_value(&resp).unwrap_or_default()).into_response(),
        Err((status, msg)) => (status, Json(CommandResponse { ok: false, message: msg })).into_response(),
    }
}

/// POST /api/v1/servers/:id/parse-config — ask the client to parse a specific config file.
pub async fn parse_server_config(
    State(state): State<AppState>,
    Path(server_id): Path<i64>,
    Json(req): Json<ParseConfigRequest>,
) -> impl IntoResponse {
    info!(server_id, path = %req.path, "Parsing config file on client");
    match send_client_request(
        &state,
        server_id,
        ClientRequest::ParseConfigFile { path: req.path },
        std::time::Duration::from_secs(60),
    ).await {
        Ok(resp) => Json(serde_json::to_value(&resp).unwrap_or_default()).into_response(),
        Err((status, msg)) => (status, Json(CommandResponse { ok: false, message: msg })).into_response(),
    }
}

/// POST /api/v1/servers/:id/install-server — ask the client to install a fresh game server.
pub async fn install_game_server(
    State(state): State<AppState>,
    Path(server_id): Path<i64>,
    Json(req): Json<InstallServerRequest>,
) -> impl IntoResponse {
    info!(server_id, path = %req.install_path, "Starting game server install on client");
    match send_client_request(
        &state,
        server_id,
        ClientRequest::InstallGameServer { install_path: req.install_path },
        std::time::Duration::from_secs(300),
    ).await {
        Ok(resp) => Json(serde_json::to_value(&resp).unwrap_or_default()).into_response(),
        Err((status, msg)) => (status, Json(CommandResponse { ok: false, message: msg })).into_response(),
    }
}

/// GET /api/v1/servers/:id/install-status — poll the install progress on the client.
pub async fn install_status(
    State(state): State<AppState>,
    Path(server_id): Path<i64>,
) -> impl IntoResponse {
    match send_client_request(
        &state,
        server_id,
        ClientRequest::InstallStatus,
        std::time::Duration::from_secs(30),
    ).await {
        Ok(resp) => Json(serde_json::to_value(&resp).unwrap_or_default()).into_response(),
        Err((status, msg)) => (status, Json(CommandResponse { ok: false, message: msg })).into_response(),
    }
}

// ---------------------------------------------------------------------------
// Version & force-update
// ---------------------------------------------------------------------------

/// GET /api/v1/servers/:id/version — query the client's current build info
/// and compare against the master's update manifest.
///
/// Returns a JSON object with `client` (live response from the bot), `cached`
/// (last heartbeat-reported version), and `latest` (master's view of the
/// newest available build). Any section may be absent if the data is not yet
/// available — for example if the client is offline the `client` field will
/// contain an `error` message rather than a Version response.
pub async fn get_server_version(
    State(state): State<AppState>,
    Path(server_id): Path<i64>,
) -> impl IntoResponse {
    // Cached heartbeat-reported version (always available once the client
    // has posted at least one heartbeat with build_hash included).
    let cached = if let Some(map) = state.client_versions.as_ref() {
        map.read().await.get(&server_id).map(|v| {
            serde_json::json!({
                "build_hash": v.build_hash,
                "version": v.version,
                "reported_at": v.reported_at.to_rfc3339(),
            })
        })
    } else {
        None
    };

    // Live version query to the client. Short timeout so offline clients
    // fail fast and we still return the cached value.
    let live = match send_client_request(
        &state,
        server_id,
        ClientRequest::GetVersion,
        std::time::Duration::from_secs(10),
    ).await {
        Ok(resp) => serde_json::json!({ "ok": true, "response": resp }),
        Err((status, msg)) => serde_json::json!({
            "ok": false,
            "status": status.as_u16(),
            "error": msg,
        }),
    };

    // Master-side view of the latest available build (manifest lookup).
    let update_url = state.config.update.url.clone();
    let update_channel = state.config.update.channel.clone();
    let latest = match crate::update::check_for_update(&update_url, &update_channel, "").await {
        // We pass an empty current_build so check_for_update always returns
        // Some(update) if the manifest loads successfully.
        Ok(Some(u)) => serde_json::json!({
            "ok": true,
            "version": u.manifest.version,
            "build_hash": u.manifest.build_hash,
            "git_commit": u.manifest.git_commit,
            "released_at": u.manifest.released_at,
            "download_size": u.platform.size,
        }),
        Ok(None) => serde_json::json!({ "ok": true, "up_to_date": true }),
        Err(e) => serde_json::json!({
            "ok": false,
            "error": format!("Manifest check failed: {}", e),
        }),
    };

    Json(serde_json::json!({
        "server_id": server_id,
        "client": live,
        "cached": cached,
        "latest": latest,
        "master_update_url": update_url,
    }))
    .into_response()
}

/// POST /api/v1/servers/:id/force-update — tell the client bot to download
/// and apply the latest update immediately, then restart itself.
///
/// The master passes its own `update.url` to the client so the client uses
/// the same binary source the master publishes to.
pub async fn force_server_update(
    State(state): State<AppState>,
    Path(server_id): Path<i64>,
) -> impl IntoResponse {
    let update_url = state.config.update.url.clone();

    // Use the per-server channel stored in the DB (set from the master UI)
    // and fall back to the master's own configured channel if the row is missing.
    let channel = state
        .storage
        .get_server(server_id)
        .await
        .ok()
        .map(|s| s.update_channel)
        .unwrap_or_else(|| state.config.update.channel.clone());

    info!(server_id, url = %update_url, channel = %channel, "Force-update requested for client");

    match send_client_request(
        &state,
        server_id,
        ClientRequest::ForceUpdate {
            update_url: Some(update_url),
            channel: Some(channel),
        },
        std::time::Duration::from_secs(90),
    ).await {
        Ok(resp) => Json(serde_json::to_value(&resp).unwrap_or_default()).into_response(),
        Err((status, msg)) => {
            warn!(server_id, error = %msg, "Force-update request failed");
            (status, Json(CommandResponse { ok: false, message: msg })).into_response()
        }
    }
}

/// POST /api/v1/servers/:id/restart — ask the client bot to re-exec its
/// own process (a clean restart, no update).
pub async fn restart_server(
    State(state): State<AppState>,
    Path(server_id): Path<i64>,
) -> impl IntoResponse {
    info!(server_id, "Restart requested for client");
    match send_client_request(
        &state,
        server_id,
        ClientRequest::Restart,
        std::time::Duration::from_secs(15),
    ).await {
        Ok(resp) => Json(serde_json::to_value(&resp).unwrap_or_default()).into_response(),
        Err((status, msg)) => {
            warn!(server_id, error = %msg, "Restart request failed");
            (status, Json(CommandResponse { ok: false, message: msg })).into_response()
        }
    }
}

/// POST /api/v1/servers/:id/check-game-log — ask the client to verify that
/// the given game log path exists and is readable on its filesystem.
pub async fn check_server_game_log(
    State(state): State<AppState>,
    Path(server_id): Path<i64>,
    Json(req): Json<CheckGameLogRequest>,
) -> impl IntoResponse {
    info!(server_id, path = %req.path, "Checking game log on client");
    match send_client_request(
        &state,
        server_id,
        ClientRequest::CheckGameLog { path: req.path },
        std::time::Duration::from_secs(15),
    ).await {
        Ok(resp) => Json(serde_json::to_value(&resp).unwrap_or_default()).into_response(),
        Err((status, msg)) => (status, Json(CommandResponse { ok: false, message: msg })).into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct SetUpdateChannelRequest {
    pub channel: String,
}

/// PUT /api/v1/servers/:id/update-channel — set the release channel this
/// server's bot follows for updates. The change is persisted in the DB and
/// picked up by the client on its next heartbeat (no restart required).
pub async fn set_server_update_channel(
    State(state): State<AppState>,
    Path(server_id): Path<i64>,
    Json(req): Json<SetUpdateChannelRequest>,
) -> impl IntoResponse {
    let channel = req.channel.trim().to_string();
    if !crate::config::VALID_UPDATE_CHANNELS.contains(&channel.as_str()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(CommandResponse {
                ok: false,
                message: format!(
                    "Invalid channel '{}' — expected one of: {}",
                    channel,
                    crate::config::VALID_UPDATE_CHANNELS.join(", ")
                ),
            }),
        )
            .into_response();
    }

    if let Err(e) = state.storage.set_server_update_channel(server_id, &channel).await {
        error!(error = %e, server_id, "Failed to update server update_channel");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(CommandResponse { ok: false, message: e.to_string() }),
        )
            .into_response();
    }

    info!(server_id, %channel, "Server update channel changed");
    Json(CommandResponse {
        ok: true,
        message: format!("Release channel set to '{}'. Applied on next heartbeat.", channel),
    })
    .into_response()
}

// ---------------------------------------------------------------------------
// Map repository import (per-server)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ImportMapRequest {
    pub filename: String,
}

/// POST /api/v1/servers/:id/maps/import — download a `.pk3` from the cached
/// repo entry onto the target client. Body: `{ filename: "ut4_foo.pk3" }`.
pub async fn import_map(
    State(state): State<AppState>,
    Path(server_id): Path<i64>,
    Json(req): Json<ImportMapRequest>,
) -> impl IntoResponse {
    if !state.config.map_repo.enabled {
        return (
            StatusCode::BAD_REQUEST,
            Json(CommandResponse {
                ok: false,
                message: "map_repo is disabled in config".into(),
            }),
        )
            .into_response();
    }
    let entry = match state.storage.get_map_repo_entry(&req.filename).await {
        Ok(Some(e)) => e,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(CommandResponse {
                    ok: false,
                    message: format!("'{}' not found in map repo cache", req.filename),
                }),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CommandResponse {
                    ok: false,
                    message: format!("Storage error: {}", e),
                }),
            )
                .into_response();
        }
    };

    // Build the host allowlist from configured sources so the client can
    // cross-check and reject SSRF attempts.
    let allowed_hosts: Vec<String> = state.config.map_repo.sources.clone();

    info!(
        server_id,
        filename = %entry.filename,
        url = %entry.source_url,
        "Import map onto server"
    );

    match send_client_request(
        &state,
        server_id,
        ClientRequest::DownloadMapPk3 {
            url: entry.source_url.clone(),
            filename: entry.filename.clone(),
            allowed_hosts,
        },
        std::time::Duration::from_secs(600),
    )
    .await
    {
        Ok(resp) => Json(serde_json::to_value(&resp).unwrap_or_default()).into_response(),
        Err((status, msg)) => (
            status,
            Json(CommandResponse {
                ok: false,
                message: msg,
            }),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct MissingMapsRequest {
    pub maps: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct MissingMapInfo {
    pub map: String,
    /// Matching filename in the repo cache, if found.
    pub repo_filename: Option<String>,
    pub repo_size: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct MissingMapsResponse {
    pub missing: Vec<MissingMapInfo>,
}

/// POST /api/v1/servers/:id/maps/missing — given a candidate list of maps
/// (without the `.pk3` extension), returns those that the server does not
/// currently have installed, enriched with repo-availability info.
pub async fn missing_maps(
    State(state): State<AppState>,
    Path(server_id): Path<i64>,
    Json(req): Json<MissingMapsRequest>,
) -> impl IntoResponse {
    // Query the client for its installed maps.
    let resp = match send_client_request(
        &state,
        server_id,
        ClientRequest::ListMaps,
        std::time::Duration::from_secs(30),
    )
    .await
    {
        Ok(r) => r,
        Err((status, msg)) => {
            return (
                status,
                Json(CommandResponse {
                    ok: false,
                    message: msg,
                }),
            )
                .into_response();
        }
    };
    let installed: Vec<String> = match resp {
        ClientResponse::MapList { maps } => {
            maps.into_iter().map(|m| m.to_lowercase()).collect()
        }
        other => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({
                    "error": "Unexpected response from client",
                    "response": other,
                })),
            )
                .into_response();
        }
    };
    let installed_set: std::collections::HashSet<&str> =
        installed.iter().map(|s| s.as_str()).collect();

    let mut missing = Vec::new();
    for m in &req.maps {
        let key = m.trim().to_lowercase();
        if key.is_empty() || installed_set.contains(key.as_str()) {
            continue;
        }
        // Look up `<map>.pk3` in the repo cache (best-effort).
        let candidate = format!("{}.pk3", key);
        let repo = state
            .storage
            .get_map_repo_entry(&candidate)
            .await
            .ok()
            .flatten();
        missing.push(MissingMapInfo {
            map: m.clone(),
            repo_filename: repo.as_ref().map(|e| e.filename.clone()),
            repo_size: repo.as_ref().and_then(|e| e.size),
        });
    }

    Json(MissingMapsResponse { missing }).into_response()
}
