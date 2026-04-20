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

use crate::sync::protocol::{RemoteAction, RemoteCommand, SyncMessage};
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
