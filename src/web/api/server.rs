use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};

use crate::core::AuditEntry;
use crate::web::auth::{AdminOnly, AuthUser};
use crate::web::state::AppState;

/// GET /api/v1/server/status
pub async fn server_status(
    AuthUser(_claims): AuthUser,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let game = state.ctx.game.read().await;
    let connected = state.ctx.clients.get_all().await;

    Json(serde_json::json!({
        "game_name": game.game_name,
        "map_name": game.map_name,
        "game_type": game.game_type,
        "player_count": connected.len(),
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
