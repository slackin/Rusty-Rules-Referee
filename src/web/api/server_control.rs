// =====================================================================
// Phase 3 — Full per-server control endpoints (standalone UI parity)
// =====================================================================
//
// Each of these endpoints is scoped under /api/v1/servers/:id/... and
// dispatches a `ClientRequest` over the sync channel to the bot that
// owns that server, returning the response (or a storage-backed history
// view for penalties / chat / audit).

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use std::time::Duration as StdDuration;

use crate::sync::protocol::{ClientRequest, ServerConfigPayload};
use crate::web::api::servers::{CommandResponse, send_client_request};
use crate::web::state::AppState;

// ---- Live state & player control ----

/// GET /api/v1/servers/:id/live — scoreboard + map/gametype/hostname.
pub async fn server_live_status(
    State(state): State<AppState>,
    Path(server_id): Path<i64>,
) -> impl IntoResponse {
    match send_client_request(
        &state,
        server_id,
        ClientRequest::GetLiveStatus,
        StdDuration::from_secs(10),
    )
    .await
    {
        Ok(resp) => Json(serde_json::to_value(&resp).unwrap_or_default()).into_response(),
        Err((status, msg)) => (status, Json(CommandResponse { ok: false, message: msg })).into_response(),
    }
}

/// GET /api/v1/servers/:id/players — raw scoreboard only.
pub async fn server_players(
    State(state): State<AppState>,
    Path(server_id): Path<i64>,
) -> impl IntoResponse {
    match send_client_request(
        &state,
        server_id,
        ClientRequest::GetPlayers,
        StdDuration::from_secs(10),
    )
    .await
    {
        Ok(resp) => Json(serde_json::to_value(&resp).unwrap_or_default()).into_response(),
        Err((status, msg)) => (status, Json(CommandResponse { ok: false, message: msg })).into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct PlayerCidPath {
    pub cid: String,
}

/// POST /api/v1/servers/:id/players/:cid/mute
pub async fn server_player_mute(
    State(state): State<AppState>,
    Path((server_id, cid)): Path<(i64, String)>,
) -> impl IntoResponse {
    match send_client_request(
        &state,
        server_id,
        ClientRequest::MutePlayer { cid },
        StdDuration::from_secs(10),
    )
    .await
    {
        Ok(resp) => Json(serde_json::to_value(&resp).unwrap_or_default()).into_response(),
        Err((status, msg)) => (status, Json(CommandResponse { ok: false, message: msg })).into_response(),
    }
}

/// POST /api/v1/servers/:id/players/:cid/unmute
pub async fn server_player_unmute(
    State(state): State<AppState>,
    Path((server_id, cid)): Path<(i64, String)>,
) -> impl IntoResponse {
    match send_client_request(
        &state,
        server_id,
        ClientRequest::UnmutePlayer { cid },
        StdDuration::from_secs(10),
    )
    .await
    {
        Ok(resp) => Json(serde_json::to_value(&resp).unwrap_or_default()).into_response(),
        Err((status, msg)) => (status, Json(CommandResponse { ok: false, message: msg })).into_response(),
    }
}

// ---- Map control ----

#[derive(Debug, Deserialize)]
pub struct ChangeMapRequest {
    pub map: String,
}

/// GET /api/v1/servers/:id/maps — cached list of installed maps on the game
/// server. Backed by the `server_maps` table, populated asynchronously by
/// [`crate::mapscan`]. For a live refresh use POST `…/maps/refresh`.
pub async fn server_maps(
    State(state): State<AppState>,
    Path(server_id): Path<i64>,
) -> impl IntoResponse {
    let maps = state
        .storage
        .list_server_maps(server_id)
        .await
        .unwrap_or_default();
    let status = state
        .storage
        .get_server_map_scan(server_id)
        .await
        .ok()
        .flatten();
    let current_map = state
        .storage
        .get_server(server_id)
        .await
        .ok()
        .and_then(|s| s.current_map);
    Json(serde_json::json!({
        "maps": maps,
        "current_map": current_map,
        "last_scan_at": status.as_ref().and_then(|s| s.last_scan_at),
        "last_scan_ok": status.as_ref().map(|s| s.last_scan_ok).unwrap_or(false),
        "last_scan_error": status.as_ref().and_then(|s| s.last_scan_error.clone()),
        "map_count": status.as_ref().map(|s| s.map_count).unwrap_or(0),
    }))
    .into_response()
}

/// POST /api/v1/servers/:id/maps/refresh — force an immediate RCON scan
/// (bypassing the periodic scheduler). Rate-limited inside mapscan.
pub async fn server_maps_refresh(
    State(state): State<AppState>,
    Path(server_id): Path<i64>,
) -> impl IntoResponse {
    let (Some(pending_responses), Some(pending_client_requests)) = (
        state.pending_responses.clone(),
        state.pending_client_requests.clone(),
    ) else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(CommandResponse {
                ok: false,
                message: "Master sync channel not available".to_string(),
            }),
        )
            .into_response();
    };

    match crate::mapscan::scan_remote_server(
        state.storage.clone(),
        pending_responses,
        pending_client_requests,
        server_id,
    )
    .await
    {
        Ok(count) => Json(serde_json::json!({
            "ok": true,
            "map_count": count,
        }))
        .into_response(),
        Err(msg) => (
            StatusCode::BAD_GATEWAY,
            Json(CommandResponse {
                ok: false,
                message: msg,
            }),
        )
            .into_response(),
    }
}

/// POST /api/v1/servers/:id/map — change the map.
pub async fn server_change_map(
    State(state): State<AppState>,
    Path(server_id): Path<i64>,
    Json(req): Json<ChangeMapRequest>,
) -> impl IntoResponse {
    match send_client_request(
        &state,
        server_id,
        ClientRequest::ChangeMap { map: req.map },
        StdDuration::from_secs(15),
    )
    .await
    {
        Ok(resp) => Json(serde_json::to_value(&resp).unwrap_or_default()).into_response(),
        Err((status, msg)) => (status, Json(CommandResponse { ok: false, message: msg })).into_response(),
    }
}

/// GET /api/v1/servers/:id/mapcycle
pub async fn server_get_mapcycle(
    State(state): State<AppState>,
    Path(server_id): Path<i64>,
) -> impl IntoResponse {
    match send_client_request(
        &state,
        server_id,
        ClientRequest::GetMapcycle,
        StdDuration::from_secs(15),
    )
    .await
    {
        Ok(resp) => Json(serde_json::to_value(&resp).unwrap_or_default()).into_response(),
        Err((status, msg)) => (status, Json(CommandResponse { ok: false, message: msg })).into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct SetMapcycleRequest {
    pub maps: Vec<String>,
}

/// PUT /api/v1/servers/:id/mapcycle
pub async fn server_set_mapcycle(
    State(state): State<AppState>,
    Path(server_id): Path<i64>,
    Json(req): Json<SetMapcycleRequest>,
) -> impl IntoResponse {
    match send_client_request(
        &state,
        server_id,
        ClientRequest::SetMapcycle { maps: req.maps },
        StdDuration::from_secs(15),
    )
    .await
    {
        Ok(resp) => Json(serde_json::to_value(&resp).unwrap_or_default()).into_response(),
        Err((status, msg)) => (status, Json(CommandResponse { ok: false, message: msg })).into_response(),
    }
}

// ---- server.cfg editor ----

/// GET /api/v1/servers/:id/server-cfg
pub async fn server_get_server_cfg(
    State(state): State<AppState>,
    Path(server_id): Path<i64>,
) -> impl IntoResponse {
    match send_client_request(
        &state,
        server_id,
        ClientRequest::GetServerCfg,
        StdDuration::from_secs(15),
    )
    .await
    {
        Ok(resp) => Json(serde_json::to_value(&resp).unwrap_or_default()).into_response(),
        Err((status, msg)) => (status, Json(CommandResponse { ok: false, message: msg })).into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct SaveConfigFileRequest {
    pub path: String,
    pub contents: String,
}

/// PUT /api/v1/servers/:id/server-cfg — save arbitrary .cfg file (path validated by client).
pub async fn server_save_server_cfg(
    State(state): State<AppState>,
    Path(server_id): Path<i64>,
    Json(req): Json<SaveConfigFileRequest>,
) -> impl IntoResponse {
    match send_client_request(
        &state,
        server_id,
        ClientRequest::SaveConfigFile {
            path: req.path,
            contents: req.contents,
        },
        StdDuration::from_secs(15),
    )
    .await
    {
        Ok(resp) => Json(serde_json::to_value(&resp).unwrap_or_default()).into_response(),
        Err((status, msg)) => (status, Json(CommandResponse { ok: false, message: msg })).into_response(),
    }
}

// ---- Map configs (CRUD proxied to client DB) ----

/// GET /api/v1/servers/:id/map-configs
pub async fn server_list_map_configs(
    State(state): State<AppState>,
    Path(server_id): Path<i64>,
) -> impl IntoResponse {
    match send_client_request(
        &state,
        server_id,
        ClientRequest::ListMapConfigs,
        StdDuration::from_secs(15),
    )
    .await
    {
        Ok(resp) => Json(serde_json::to_value(&resp).unwrap_or_default()).into_response(),
        Err((status, msg)) => (status, Json(CommandResponse { ok: false, message: msg })).into_response(),
    }
}

/// POST/PUT /api/v1/servers/:id/map-configs — upsert a map_config row.
pub async fn server_save_map_config(
    State(state): State<AppState>,
    Path(server_id): Path<i64>,
    Json(config): Json<serde_json::Value>,
) -> impl IntoResponse {
    match send_client_request(
        &state,
        server_id,
        ClientRequest::SaveMapConfig { config },
        StdDuration::from_secs(15),
    )
    .await
    {
        Ok(resp) => Json(serde_json::to_value(&resp).unwrap_or_default()).into_response(),
        Err((status, msg)) => (status, Json(CommandResponse { ok: false, message: msg })).into_response(),
    }
}

/// DELETE /api/v1/servers/:id/map-configs/:map_config_id
pub async fn server_delete_map_config(
    State(state): State<AppState>,
    Path((server_id, map_config_id)): Path<(i64, i64)>,
) -> impl IntoResponse {
    match send_client_request(
        &state,
        server_id,
        ClientRequest::DeleteMapConfig { id: map_config_id },
        StdDuration::from_secs(15),
    )
    .await
    {
        Ok(resp) => Json(serde_json::to_value(&resp).unwrap_or_default()).into_response(),
        Err((status, msg)) => (status, Json(CommandResponse { ok: false, message: msg })).into_response(),
    }
}

/// GET /api/v1/servers/:id/map-configs/by-name/:map_name — fetch the
/// map_config for `map_name`, creating one from defaults if absent.
pub async fn server_ensure_map_config(
    State(state): State<AppState>,
    Path((server_id, map_name)): Path<(i64, String)>,
) -> impl IntoResponse {
    match send_client_request(
        &state,
        server_id,
        ClientRequest::EnsureMapConfig { map_name },
        StdDuration::from_secs(15),
    )
    .await
    {
        Ok(resp) => Json(serde_json::to_value(&resp).unwrap_or_default()).into_response(),
        Err((status, msg)) => (status, Json(CommandResponse { ok: false, message: msg })).into_response(),
    }
}

/// POST /api/v1/servers/:id/map-configs/by-name/:map_name/apply — apply
/// the map_config immediately without waiting for a map change.
pub async fn server_apply_map_config(
    State(state): State<AppState>,
    Path((server_id, map_name)): Path<(i64, String)>,
) -> impl IntoResponse {
    match send_client_request(
        &state,
        server_id,
        ClientRequest::ApplyMapConfig { map_name },
        StdDuration::from_secs(30),
    )
    .await
    {
        Ok(resp) => Json(serde_json::to_value(&resp).unwrap_or_default()).into_response(),
        Err((status, msg)) => (status, Json(CommandResponse { ok: false, message: msg })).into_response(),
    }
}

/// POST /api/v1/servers/:id/map-configs/by-name/:map_name/reset — reset
/// the map_config back to its default / built-in values.
pub async fn server_reset_map_config(
    State(state): State<AppState>,
    Path((server_id, map_name)): Path<(i64, String)>,
) -> impl IntoResponse {
    match send_client_request(
        &state,
        server_id,
        ClientRequest::ResetMapConfig { map_name },
        StdDuration::from_secs(15),
    )
    .await
    {
        Ok(resp) => Json(serde_json::to_value(&resp).unwrap_or_default()).into_response(),
        Err((status, msg)) => (status, Json(CommandResponse { ok: false, message: msg })).into_response(),
    }
}

// ---- Historical / persisted views (read directly from master DB) ----

#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    #[serde(default = "default_limit")]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
    #[serde(default)]
    pub before_id: Option<i64>,
}
fn default_limit() -> u32 { 50 }

/// GET /api/v1/servers/:id/penalties?limit=&offset=
pub async fn server_penalties_history(
    State(state): State<AppState>,
    Path(server_id): Path<i64>,
    axum::extract::Query(q): axum::extract::Query<PaginationQuery>,
) -> impl IntoResponse {
    match state
        .storage
        .get_penalties_by_server(server_id, q.limit.min(500))
        .await
    {
        Ok(rows) => Json(serde_json::json!({"penalties": rows})).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// GET /api/v1/servers/:id/chat?limit=&before_id=
pub async fn server_chat_history(
    State(state): State<AppState>,
    Path(server_id): Path<i64>,
    axum::extract::Query(q): axum::extract::Query<PaginationQuery>,
) -> impl IntoResponse {
    match state
        .storage
        .get_chat_messages_by_server(server_id, q.limit.min(500), q.before_id)
        .await
    {
        Ok(rows) => Json(serde_json::json!({"messages": rows})).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// GET /api/v1/servers/:id/audit-log?limit=&offset=
pub async fn server_audit_log(
    State(state): State<AppState>,
    Path(server_id): Path<i64>,
    axum::extract::Query(q): axum::extract::Query<PaginationQuery>,
) -> impl IntoResponse {
    match state
        .storage
        .get_audit_log_by_server(server_id, q.limit.min(500), q.offset)
        .await
    {
        Ok(rows) => Json(serde_json::json!({"entries": rows})).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// ---- Plugin config (stored in servers.config_json on the master) ----

/// GET /api/v1/servers/:id/plugins — list of plugin configs from stored config_json.
pub async fn server_list_plugins(
    State(state): State<AppState>,
    Path(server_id): Path<i64>,
) -> impl IntoResponse {
    let server = match state.storage.get_server(server_id).await {
        Ok(s) => s,
        Err(_) => return (StatusCode::NOT_FOUND, Json(CommandResponse { ok: false, message: "Server not found".into() })).into_response(),
    };
    let plugins = server
        .config_json
        .as_deref()
        .and_then(|j| serde_json::from_str::<ServerConfigPayload>(j).ok())
        .and_then(|p| p.plugins)
        .unwrap_or_default();
    Json(serde_json::json!({"plugins": plugins})).into_response()
}

#[derive(Debug, Deserialize)]
pub struct UpdatePluginRequest {
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub settings: Option<serde_json::Value>,
}

/// PUT /api/v1/servers/:id/plugins/:plugin_name — update a plugin's enabled/settings
/// in the stored config_json. The updated payload is pushed on next config
/// sync (bump config_version) so the client rewrites its TOML.
pub async fn server_update_plugin(
    State(state): State<AppState>,
    Path((server_id, plugin_name)): Path<(i64, String)>,
    Json(req): Json<UpdatePluginRequest>,
) -> impl IntoResponse {
    let mut server = match state.storage.get_server(server_id).await {
        Ok(s) => s,
        Err(_) => return (StatusCode::NOT_FOUND, Json(CommandResponse { ok: false, message: "Server not found".into() })).into_response(),
    };
    let mut payload: ServerConfigPayload = match server
        .config_json
        .as_deref()
        .and_then(|j| serde_json::from_str(j).ok())
    {
        Some(p) => p,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(CommandResponse { ok: false, message: "Server has no config payload yet".into() }),
            )
                .into_response();
        }
    };
    let plugins = payload.plugins.get_or_insert_with(Vec::new);
    if let Some(p) = plugins.iter_mut().find(|p| p.name == plugin_name) {
        if let Some(enabled) = req.enabled {
            p.enabled = enabled;
        }
        if let Some(settings) = req.settings {
            p.settings = settings;
        }
    } else {
        plugins.push(crate::sync::protocol::PluginConfigPayload {
            name: plugin_name.clone(),
            enabled: req.enabled.unwrap_or(true),
            settings: req.settings.unwrap_or(serde_json::Value::Null),
        });
    }

    let new_json = match serde_json::to_string(&payload) {
        Ok(j) => j,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Serialize failed: {}", e)})),
            )
                .into_response();
        }
    };
    server.config_json = Some(new_json);
    server.config_version += 1;
    if let Err(e) = state.storage.save_server(&server).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Save failed: {}", e)})),
        )
            .into_response();
    }

    Json(serde_json::json!({
        "ok": true,
        "config_version": server.config_version,
        "message": format!("Plugin '{}' updated; client will pick up on next heartbeat", plugin_name),
    }))
    .into_response()
}
