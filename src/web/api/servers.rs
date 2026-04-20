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

    let servers = servers
        .into_iter()
        .map(|s| {
            let online = connected_ids.contains(&s.id);
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

    let online = if let Some(c) = state.connected_clients.as_ref() {
        c.read().await.contains_key(&server_id)
    } else {
        false
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
    Json(payload): Json<ServerConfigPayload>,
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

/// Helper: send a ClientRequest to a server via the polling infrastructure and await the response.
async fn send_client_request(
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
