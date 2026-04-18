use std::collections::HashMap;

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};

use crate::core::AuditEntry;
use crate::web::auth::{AdminOnly, AuthUser};
use crate::web::state::AppState;

/// Parse RCON serverinfo output into key-value pairs.
fn parse_serverinfo(raw: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("Server info") {
            continue;
        }
        // Format: "key             value"
        let mut parts = line.splitn(2, char::is_whitespace);
        if let (Some(key), Some(val)) = (parts.next(), parts.next()) {
            map.insert(key.trim().to_string(), val.trim().to_string());
        }
    }
    map
}

/// GET /api/v1/server/status
pub async fn server_status(
    AuthUser(_claims): AuthUser,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let game = state.ctx.game.read().await;
    let connected = state.ctx.clients.get_all().await;

    // Fetch live data from RCON serverinfo
    let rcon_info = match state.ctx.rcon.send("serverinfo").await {
        Ok(raw) => parse_serverinfo(&raw),
        Err(_) => HashMap::new(),
    };

    let map_name = rcon_info.get("mapname").cloned()
        .or_else(|| game.map_name.clone());
    let game_type = rcon_info.get("g_gametype").cloned()
        .or_else(|| game.game_type.clone());
    let max_clients = rcon_info.get("sv_maxclients")
        .and_then(|v| v.parse::<u32>().ok());
    let hostname = rcon_info.get("sv_hostname").cloned();

    Json(serde_json::json!({
        "game_name": game.game_name,
        "map_name": map_name,
        "game_type": game_type,
        "player_count": connected.len(),
        "max_clients": max_clients,
        "hostname": hostname,
        "round_time_start": game.round_time_start,
        "map_time_start": game.map_time_start,
    }))
}

/// POST /api/v1/server/rcon — execute raw RCON command (admin only).
pub async fn rcon_command(
    AdminOnly(claims): AdminOnly,
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let cmd = match body.get("command").and_then(|v| v.as_str()) {
        Some(c) => c,
        None => {
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "Missing 'command' field"}))).into_response();
        }
    };

    // Audit log
    let _ = state.storage.save_audit_entry(&AuditEntry {
        id: 0,
        admin_user_id: Some(claims.user_id),
        action: "rcon".to_string(),
        detail: format!("RCON: {}", cmd),
        ip_address: None,
        created_at: chrono::Utc::now(),
    }).await;

    match state.ctx.write(cmd).await {
        Ok(response) => {
            Json(serde_json::json!({"response": response})).into_response()
        }
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// GET /api/v1/server/say — send a public message.
pub async fn server_say(
    AdminOnly(_claims): AdminOnly,
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let msg = body.get("message").and_then(|v| v.as_str()).unwrap_or("");
    match state.ctx.say(msg).await {
        Ok(_) => Json(serde_json::json!({"status": "ok"})).into_response(),
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}
