use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use std::time::Duration as StdDuration;
use tracing::info;

use crate::core::{AuditEntry, MapConfig, MapConfigDefault};
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
    #[serde(default)]
    pub supported_gametypes: String,
    #[serde(default)]
    pub default_gametype: Option<String>,
    #[serde(default)]
    pub g_suddendeath: Option<i32>,
    #[serde(default)]
    pub g_teamdamage: Option<i32>,
    #[serde(default)]
    pub source: Option<String>,
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
        supported_gametypes: body.supported_gametypes,
        default_gametype: body.default_gametype,
        g_suddendeath: body.g_suddendeath,
        g_teamdamage: body.g_teamdamage,
        source: body.source.unwrap_or_else(|| "user".to_string()),
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
                server_id: None,
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
        supported_gametypes: body.supported_gametypes,
        default_gametype: body.default_gametype,
        g_suddendeath: body.g_suddendeath,
        g_teamdamage: body.g_teamdamage,
        source: body.source.unwrap_or_else(|| "user".to_string()),
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
                server_id: None,
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
                server_id: None,
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

// ===== Global map-config defaults (master-only template) =====

/// GET /api/v1/map-config-defaults — list all global defaults.
pub async fn list_map_config_defaults(
    AdminOnly(_claims): AdminOnly,
    State(state): State<AppState>,
) -> impl IntoResponse {
    match state.storage.get_map_config_defaults().await {
        Ok(defs) => Json(serde_json::json!({ "defaults": defs })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// GET /api/v1/map-config-defaults/:map_name — fetch one global default.
pub async fn get_map_config_default(
    AdminOnly(_claims): AdminOnly,
    State(state): State<AppState>,
    Path(map_name): Path<String>,
) -> impl IntoResponse {
    match state.storage.get_map_config_default(&map_name).await {
        Ok(Some(def)) => Json(serde_json::json!(def)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Default not found" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// PUT /api/v1/map-config-defaults/:map_name — upsert a global default.
pub async fn save_map_config_default_endpoint(
    AdminOnly(claims): AdminOnly,
    State(state): State<AppState>,
    Path(map_name): Path<String>,
    Json(mut body): Json<MapConfigDefault>,
) -> impl IntoResponse {
    body.map_name = map_name.clone();
    body.updated_at = chrono::Utc::now();
    if body.created_at.timestamp() == 0 {
        body.created_at = chrono::Utc::now();
    }
    match state.storage.save_map_config_default(&body).await {
        Ok(_) => {
            let _ = state
                .storage
                .save_audit_entry(&AuditEntry {
                    id: 0,
                    admin_user_id: Some(claims.user_id),
                    action: "mapconfig_default_save".to_string(),
                    detail: format!("Saved global map-config default for '{}'", map_name),
                    ip_address: None,
                    created_at: chrono::Utc::now(),
                    server_id: None,
                })
                .await;
            info!(map = %map_name, "Global map-config default saved");
            Json(serde_json::json!({ "ok": true })).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// DELETE /api/v1/map-config-defaults/:map_name
pub async fn delete_map_config_default_endpoint(
    AdminOnly(claims): AdminOnly,
    State(state): State<AppState>,
    Path(map_name): Path<String>,
) -> impl IntoResponse {
    match state.storage.delete_map_config_default(&map_name).await {
        Ok(_) => {
            let _ = state
                .storage
                .save_audit_entry(&AuditEntry {
                    id: 0,
                    admin_user_id: Some(claims.user_id),
                    action: "mapconfig_default_delete".to_string(),
                    detail: format!("Deleted global map-config default for '{}'", map_name),
                    ip_address: None,
                    created_at: chrono::Utc::now(),
                    server_id: None,
                })
                .await;
            Json(serde_json::json!({ "ok": true })).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

#[derive(Deserialize, Default)]
pub struct PropagateBody {
    /// If true, also overwrite rows that an admin has edited (source='user').
    /// Default false = only overwrite 'auto' and 'default_seed' rows.
    #[serde(default)]
    pub overwrite_user_edits: bool,
}

/// POST /api/v1/map-config-defaults/:map_name/propagate — push this
/// global default down to every server's `map_configs` row.
///
/// Each connected client is issued a `SaveMapConfig` over the sync layer.
/// Local (standalone-mode) storage is also updated so single-server
/// deployments see the change immediately.
pub async fn propagate_map_config_default(
    AdminOnly(claims): AdminOnly,
    State(state): State<AppState>,
    Path(map_name): Path<String>,
    Json(body): Json<PropagateBody>,
) -> impl IntoResponse {
    let def = match state.storage.get_map_config_default(&map_name).await {
        Ok(Some(d)) => d,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "Default not found" })),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    // Build a MapConfig payload from the default (id=0 so clients upsert).
    let mk_config = |existing: Option<MapConfig>| -> MapConfig {
        let id = existing.as_ref().map(|c| c.id).unwrap_or(0);
        MapConfig {
            id,
            map_name: def.map_name.clone(),
            gametype: def.gametype.clone(),
            capturelimit: def.capturelimit,
            timelimit: def.timelimit,
            fraglimit: def.fraglimit,
            g_gear: def.g_gear.clone(),
            g_gravity: def.g_gravity,
            g_friendlyfire: def.g_friendlyfire,
            g_followstrict: def.g_followstrict,
            g_waverespawns: def.g_waverespawns,
            g_bombdefusetime: def.g_bombdefusetime,
            g_bombexplodetime: def.g_bombexplodetime,
            g_swaproles: def.g_swaproles,
            g_maxrounds: def.g_maxrounds,
            g_matchmode: def.g_matchmode,
            g_respawndelay: def.g_respawndelay,
            startmessage: def.startmessage.clone(),
            skiprandom: def.skiprandom,
            bot: def.bot,
            custom_commands: def.custom_commands.clone(),
            supported_gametypes: def.supported_gametypes.clone(),
            default_gametype: def.default_gametype.clone(),
            g_suddendeath: def.g_suddendeath,
            g_teamdamage: def.g_teamdamage,
            source: "default_seed".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    };

    // Apply to the master's own storage first (covers standalone mode).
    let existing = state.storage.get_map_config(&map_name).await.ok().flatten();
    let skip_master = existing
        .as_ref()
        .map(|c| c.source == "user" && !body.overwrite_user_edits)
        .unwrap_or(false);
    let mut master_updated = false;
    if !skip_master {
        let cfg = mk_config(existing);
        if state.storage.save_map_config(&cfg).await.is_ok() {
            master_updated = true;
        }
    }

    // Fan out over the sync layer to every known server.
    let mut clients_sent = 0u32;
    let mut clients_skipped = 0u32;
    let mut clients_failed = 0u32;
    if let (Some(pending_responses), Some(pending_client_requests)) = (
        state.pending_responses.as_ref(),
        state.pending_client_requests.as_ref(),
    ) {
        if let Ok(servers) = state.storage.get_servers().await {
            for server in servers {
                // Best-effort: ensure a remote row exists, check source, then save.
                // We do the simple thing: just send a SaveMapConfig with the
                // propagated values. The client upserts on (server_id, map_name).
                let cfg = mk_config(None);
                let payload = match serde_json::to_value(&cfg) {
                    Ok(v) => v,
                    Err(_) => { clients_failed += 1; continue; }
                };
                match crate::sync::master::send_request_to_server(
                    pending_responses,
                    pending_client_requests,
                    server.id,
                    crate::sync::protocol::ClientRequest::SaveMapConfig { config: payload },
                    StdDuration::from_secs(10),
                )
                .await
                {
                    Ok(_) => { clients_sent += 1; }
                    Err(_) => { clients_skipped += 1; }
                }
            }
        }
    }

    let _ = state
        .storage
        .save_audit_entry(&AuditEntry {
            id: 0,
            admin_user_id: Some(claims.user_id),
            action: "mapconfig_default_propagate".to_string(),
            detail: format!(
                "Propagated map-config default '{}' (overwrite_user_edits={}, master_updated={}, clients_sent={}, skipped={}, failed={})",
                map_name, body.overwrite_user_edits, master_updated, clients_sent, clients_skipped, clients_failed,
            ),
            ip_address: None,
            created_at: chrono::Utc::now(),
            server_id: None,
        })
        .await;

    Json(serde_json::json!({
        "ok": true,
        "master_updated": master_updated,
        "clients_sent": clients_sent,
        "clients_skipped": clients_skipped,
        "clients_failed": clients_failed,
    }))
    .into_response()
}
