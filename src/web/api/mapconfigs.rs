use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use tracing::info;

use crate::core::{AuditEntry, MapConfig};
use crate::web::auth::{AdminOnly, AuthUser};
use crate::web::state::AppState;

/// GET /api/v1/map-configs — list all map configurations.
pub async fn list_map_configs(
    AuthUser(_claims): AuthUser,
    State(state): State<AppState>,
) -> impl IntoResponse {
    match state.storage.get_map_configs().await {
        Ok(configs) => Json(serde_json::json!({ "configs": configs })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ).into_response(),
    }
}

/// GET /api/v1/map-configs/:id — get a single map config by ID.
pub async fn get_map_config(
    AuthUser(_claims): AuthUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match state.storage.get_map_config_by_id(id).await {
        Ok(config) => Json(serde_json::json!(config)).into_response(),
        Err(crate::storage::StorageError::NotFound) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Map config not found" })),
        ).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ).into_response(),
    }
}

#[derive(Deserialize)]
pub struct MapConfigBody {
    pub map_name: String,
    #[serde(default)]
    pub gametype: String,
    pub capturelimit: Option<i32>,
    pub timelimit: Option<i32>,
    pub fraglimit: Option<i32>,
    #[serde(default)]
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
    #[serde(default)]
    pub startmessage: String,
    #[serde(default)]
    pub skiprandom: i32,
    #[serde(default)]
    pub bot: i32,
    #[serde(default)]
    pub custom_commands: String,
}

/// POST /api/v1/map-configs — create a new map config.
pub async fn create_map_config(
    AdminOnly(claims): AdminOnly,
    State(state): State<AppState>,
    Json(body): Json<MapConfigBody>,
) -> impl IntoResponse {
    let map_name = body.map_name.trim().to_string();
    if map_name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "map_name is required" })),
        ).into_response();
    }

    // Check for duplicate
    if let Ok(Some(_)) = state.storage.get_map_config(&map_name).await {
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({ "error": format!("Config for '{}' already exists", map_name) })),
        ).into_response();
    }

    let config = MapConfig {
        id: 0,
        map_name: map_name.clone(),
        gametype: body.gametype,
        capturelimit: body.capturelimit,
        timelimit: body.timelimit,
        fraglimit: body.fraglimit,
        g_gear: body.g_gear,
        g_gravity: body.g_gravity,
        g_friendlyfire: body.g_friendlyfire,
        g_followstrict: body.g_followstrict,
        g_waverespawns: body.g_waverespawns,
        g_bombdefusetime: body.g_bombdefusetime,
        g_bombexplodetime: body.g_bombexplodetime,
        g_swaproles: body.g_swaproles,
        g_maxrounds: body.g_maxrounds,
        g_matchmode: body.g_matchmode,
        g_respawndelay: body.g_respawndelay,
        startmessage: body.startmessage,
        skiprandom: body.skiprandom,
        bot: body.bot,
        custom_commands: body.custom_commands,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    match state.storage.save_map_config(&config).await {
        Ok(id) => {
            let _ = state.storage.save_audit_entry(&AuditEntry {
                id: 0,
                admin_user_id: Some(claims.user_id),
                action: "mapconfig_create".to_string(),
                detail: format!("Created map config for '{}'", map_name),
                ip_address: None,
                created_at: chrono::Utc::now(),
            }).await;
            info!(map = %map_name, "Map config created via web UI");
            (StatusCode::CREATED, Json(serde_json::json!({ "id": id }))).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ).into_response(),
    }
}

/// PUT /api/v1/map-configs/:id — update an existing map config.
pub async fn update_map_config(
    AdminOnly(claims): AdminOnly,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<MapConfigBody>,
) -> impl IntoResponse {
    let map_name = body.map_name.trim().to_string();
    if map_name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "map_name is required" })),
        ).into_response();
    }

    // Verify exists
    if let Err(crate::storage::StorageError::NotFound) = state.storage.get_map_config_by_id(id).await {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Map config not found" })),
        ).into_response();
    }

    let config = MapConfig {
        id,
        map_name: map_name.clone(),
        gametype: body.gametype,
        capturelimit: body.capturelimit,
        timelimit: body.timelimit,
        fraglimit: body.fraglimit,
        g_gear: body.g_gear,
        g_gravity: body.g_gravity,
        g_friendlyfire: body.g_friendlyfire,
        g_followstrict: body.g_followstrict,
        g_waverespawns: body.g_waverespawns,
        g_bombdefusetime: body.g_bombdefusetime,
        g_bombexplodetime: body.g_bombexplodetime,
        g_swaproles: body.g_swaproles,
        g_maxrounds: body.g_maxrounds,
        g_matchmode: body.g_matchmode,
        g_respawndelay: body.g_respawndelay,
        startmessage: body.startmessage,
        skiprandom: body.skiprandom,
        bot: body.bot,
        custom_commands: body.custom_commands,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    match state.storage.save_map_config(&config).await {
        Ok(_) => {
            let _ = state.storage.save_audit_entry(&AuditEntry {
                id: 0,
                admin_user_id: Some(claims.user_id),
                action: "mapconfig_update".to_string(),
                detail: format!("Updated map config for '{}'", map_name),
                ip_address: None,
                created_at: chrono::Utc::now(),
            }).await;
            info!(map = %map_name, "Map config updated via web UI");
            Json(serde_json::json!({ "ok": true })).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ).into_response(),
    }
}

/// DELETE /api/v1/map-configs/:id — delete a map config.
pub async fn delete_map_config(
    AdminOnly(claims): AdminOnly,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let config = match state.storage.get_map_config_by_id(id).await {
        Ok(c) => c,
        Err(crate::storage::StorageError::NotFound) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "Map config not found" })),
            ).into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            ).into_response();
        }
    };

    match state.storage.delete_map_config(id).await {
        Ok(_) => {
            let _ = state.storage.save_audit_entry(&AuditEntry {
                id: 0,
                admin_user_id: Some(claims.user_id),
                action: "mapconfig_delete".to_string(),
                detail: format!("Deleted map config for '{}'", config.map_name),
                ip_address: None,
                created_at: chrono::Utc::now(),
            }).await;
            info!(map = %config.map_name, "Map config deleted via web UI");
            Json(serde_json::json!({ "ok": true })).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ).into_response(),
    }
}
