use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;

use crate::core::AuditEntry;
use crate::web::auth::{AdminOnly, AuthUser};
use crate::web::state::AppState;

/// GET /api/v1/players — connected players.
pub async fn list_players(
    AuthUser(_claims): AuthUser,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let connected = state.ctx.clients.get_all().await;
    let players: Vec<serde_json::Value> = connected.iter().map(|c| {
        serde_json::json!({
            "id": c.id,
            "cid": c.cid,
            "name": c.name,
            "guid": c.guid,
            "ip": c.ip.map(|ip| ip.to_string()),
            "team": format!("{:?}", c.team),
            "group_bits": c.group_bits,
            "connected": c.connected,
        })
    }).collect();

    Json(serde_json::json!({"players": players}))
}

/// GET /api/v1/players/:id — player detail.
pub async fn get_player(
    AuthUser(_claims): AuthUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let client = match state.storage.get_client(id).await {
        Ok(c) => c,
        Err(_) => {
            return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Player not found"}))).into_response();
        }
    };

    let aliases = state.storage.get_aliases(id).await.unwrap_or_default();
    let penalties = state.storage.get_penalties(id, None).await.unwrap_or_default();
    let xlr = state.storage.get_xlr_player_stats(id).await.unwrap_or(None);

    Json(serde_json::json!({
        "player": {
            "id": client.id,
            "guid": client.guid,
            "name": client.name,
            "ip": client.ip.map(|ip| ip.to_string()),
            "group_bits": client.group_bits,
            "time_add": client.time_add,
            "time_edit": client.time_edit,
            "last_visit": client.last_visit,
        },
        "aliases": aliases,
        "penalties": penalties,
        "xlr_stats": xlr,
    })).into_response()
}

#[derive(Deserialize)]
pub struct PlayerActionBody {
    pub reason: Option<String>,
    pub duration: Option<u32>,
}

/// POST /api/v1/players/:cid/kick
pub async fn kick_player(
    AdminOnly(claims): AdminOnly,
    State(state): State<AppState>,
    Path(cid): Path<String>,
    Json(body): Json<PlayerActionBody>,
) -> impl IntoResponse {
    let reason = body.reason.as_deref().unwrap_or("Kicked by admin");
    match state.ctx.kick(&cid, reason).await {
        Ok(_) => {
            let _ = state.storage.save_audit_entry(&AuditEntry {
                id: 0,
                admin_user_id: Some(claims.user_id),
                action: "kick".to_string(),
                detail: format!("Kicked player cid={} reason={}", cid, reason),
                ip_address: None,
                created_at: chrono::Utc::now(),
            }).await;
            Json(serde_json::json!({"status": "ok"})).into_response()
        }
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// POST /api/v1/players/:cid/ban
pub async fn ban_player(
    AdminOnly(claims): AdminOnly,
    State(state): State<AppState>,
    Path(cid): Path<String>,
    Json(body): Json<PlayerActionBody>,
) -> impl IntoResponse {
    let reason = body.reason.as_deref().unwrap_or("Banned by admin");
    let result = if let Some(duration) = body.duration {
        state.ctx.temp_ban(&cid, reason, duration).await
    } else {
        state.ctx.ban(&cid, reason).await
    };

    match result {
        Ok(_) => {
            let _ = state.storage.save_audit_entry(&AuditEntry {
                id: 0,
                admin_user_id: Some(claims.user_id),
                action: "ban".to_string(),
                detail: format!("Banned player cid={} duration={:?} reason={}", cid, body.duration, reason),
                ip_address: None,
                created_at: chrono::Utc::now(),
            }).await;
            Json(serde_json::json!({"status": "ok"})).into_response()
        }
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// POST /api/v1/players/:cid/message
pub async fn message_player(
    AdminOnly(_claims): AdminOnly,
    State(state): State<AppState>,
    Path(cid): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let msg = body.get("message").and_then(|v| v.as_str()).unwrap_or("");
    match state.ctx.message(&cid, msg).await {
        Ok(_) => Json(serde_json::json!({"status": "ok"})).into_response(),
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
}

/// GET /api/v1/clients/search?q=name
pub async fn search_clients(
    AuthUser(_claims): AuthUser,
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> impl IntoResponse {
    let q = query.q.as_deref().unwrap_or("");
    if q.is_empty() {
        return Json(serde_json::json!({"clients": []})).into_response();
    }

    let mut results = state.storage.find_clients(q).await.unwrap_or_default();
    let alias_results = state.storage.find_clients_by_alias(q).await.unwrap_or_default();

    // Merge, dedup by id
    for c in alias_results {
        if !results.iter().any(|r| r.id == c.id) {
            results.push(c);
        }
    }

    let clients: Vec<serde_json::Value> = results.iter().map(|c| {
        serde_json::json!({
            "id": c.id,
            "guid": c.guid,
            "name": c.name,
            "group_bits": c.group_bits,
            "last_visit": c.last_visit,
        })
    }).collect();

    Json(serde_json::json!({"clients": clients})).into_response()
}
