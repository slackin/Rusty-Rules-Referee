//! Admin REST API for hub orchestrators.
//!
//! All endpoints require AdminOnly auth and are only meaningful when the
//! server is running in master mode (state.connected_hubs is set).

use std::time::Duration;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use tracing::warn;

use crate::sync::master::send_action_to_hub;
use crate::sync::protocol::{GameServerWizardParams, HubAction};
use crate::web::auth::AdminOnly;
use crate::web::state::AppState;

fn require_master(state: &AppState) -> Result<(), (StatusCode, String)> {
    if state.connected_hubs.is_some() {
        Ok(())
    } else {
        Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "Hub orchestration is only available in master mode".to_string(),
        ))
    }
}

/// GET /api/v1/hubs — list all paired hubs.
pub async fn list_hubs(
    AdminOnly(_): AdminOnly,
    State(state): State<AppState>,
) -> impl IntoResponse {
    if let Err(e) = require_master(&state) {
        return (e.0, Json(serde_json::json!({"error": e.1}))).into_response();
    }
    match state.storage.get_hubs().await {
        Ok(hubs) => Json(hubs).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// GET /api/v1/hubs/:id — full hub detail (host info + clients).
pub async fn get_hub(
    AdminOnly(_): AdminOnly,
    State(state): State<AppState>,
    Path(hub_id): Path<i64>,
) -> impl IntoResponse {
    if let Err(e) = require_master(&state) {
        return (e.0, Json(serde_json::json!({"error": e.1}))).into_response();
    }
    let hub = match state.storage.get_hub(hub_id).await {
        Ok(h) => h,
        Err(_) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Not found"}))).into_response(),
    };
    let host_info = state.storage.get_host_info(hub_id).await.ok().flatten();
    let clients = state
        .storage
        .list_clients_for_hub(hub_id)
        .await
        .unwrap_or_default();
    Json(serde_json::json!({
        "hub": hub,
        "host_info": host_info,
        "clients": clients,
    }))
    .into_response()
}

#[derive(Debug, Deserialize)]
pub struct MetricsQuery {
    /// Time range, one of: "1h" (default), "6h", "24h", "7d".
    #[serde(default)]
    pub range: Option<String>,
}

/// GET /api/v1/hubs/:id/metrics?range=1h
pub async fn get_hub_metrics(
    AdminOnly(_): AdminOnly,
    State(state): State<AppState>,
    Path(hub_id): Path<i64>,
    Query(q): Query<MetricsQuery>,
) -> impl IntoResponse {
    if let Err(e) = require_master(&state) {
        return (e.0, Json(serde_json::json!({"error": e.1}))).into_response();
    }
    let dur = match q.range.as_deref().unwrap_or("1h") {
        "6h" => chrono::Duration::hours(6),
        "24h" => chrono::Duration::hours(24),
        "7d" => chrono::Duration::days(7),
        _ => chrono::Duration::hours(1),
    };
    let since = chrono::Utc::now() - dur;
    match state.storage.get_host_metrics(hub_id, since).await {
        Ok(metrics) => Json(metrics).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct InstallClientBody {
    pub slug: String,
    pub server_name: String,
    #[serde(default)]
    pub game_server: Option<GameServerWizardParams>,
    #[serde(default = "default_true")]
    pub register_systemd: bool,
}

fn default_true() -> bool {
    true
}

async fn enqueue_action(state: &AppState, hub_id: i64, action: HubAction) -> impl IntoResponse {
    let pending_actions = match &state.pending_hub_actions {
        Some(a) => a.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": "Hub orchestration not available"})),
            )
                .into_response();
        }
    };
    let pending_responses = match &state.pending_hub_responses {
        Some(r) => r.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": "Hub orchestration not available"})),
            )
                .into_response();
        }
    };

    match send_action_to_hub(
        &pending_responses,
        &pending_actions,
        hub_id,
        action,
        Duration::from_secs(60),
    )
    .await
    {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => {
            warn!(hub_id, error = %e, "Hub action failed");
            (
                StatusCode::GATEWAY_TIMEOUT,
                Json(serde_json::json!({"error": e})),
            )
                .into_response()
        }
    }
}

/// POST /api/v1/hubs/:id/clients — install a new R3 client on the hub.
pub async fn install_client(
    AdminOnly(_): AdminOnly,
    State(state): State<AppState>,
    Path(hub_id): Path<i64>,
    Json(body): Json<InstallClientBody>,
) -> impl IntoResponse {
    if let Err(e) = require_master(&state) {
        return (e.0, Json(serde_json::json!({"error": e.1}))).into_response();
    }
    enqueue_action(
        &state,
        hub_id,
        HubAction::InstallClient {
            slug: body.slug,
            server_name: body.server_name,
            game_server: body.game_server,
            register_systemd: body.register_systemd,
        },
    )
    .await
    .into_response()
}

#[derive(Debug, Deserialize)]
pub struct UninstallClientQuery {
    #[serde(default)]
    pub remove_data: bool,
}

/// DELETE /api/v1/hubs/:id/clients/:slug
pub async fn uninstall_client(
    AdminOnly(_): AdminOnly,
    State(state): State<AppState>,
    Path((hub_id, slug)): Path<(i64, String)>,
    Query(q): Query<UninstallClientQuery>,
) -> impl IntoResponse {
    if let Err(e) = require_master(&state) {
        return (e.0, Json(serde_json::json!({"error": e.1}))).into_response();
    }
    enqueue_action(
        &state,
        hub_id,
        HubAction::UninstallClient {
            slug,
            remove_data: q.remove_data,
        },
    )
    .await
    .into_response()
}

#[derive(Debug, Deserialize)]
pub struct ClientActionBody {
    /// One of: "start", "stop", "restart".
    pub action: String,
}

/// POST /api/v1/hubs/:id/clients/:slug/action
pub async fn client_action(
    AdminOnly(_): AdminOnly,
    State(state): State<AppState>,
    Path((hub_id, slug)): Path<(i64, String)>,
    Json(body): Json<ClientActionBody>,
) -> impl IntoResponse {
    if let Err(e) = require_master(&state) {
        return (e.0, Json(serde_json::json!({"error": e.1}))).into_response();
    }
    let action = match body.action.as_str() {
        "start" => HubAction::StartClient { slug },
        "stop" => HubAction::StopClient { slug },
        "restart" => HubAction::RestartClient { slug },
        other => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": format!("unknown action: {}", other)})),
            )
                .into_response();
        }
    };
    enqueue_action(&state, hub_id, action).await.into_response()
}

#[derive(Debug, Deserialize)]
pub struct InstallGameServerBody {
    pub slug: String,
    pub params: GameServerWizardParams,
}

/// POST /api/v1/hubs/:id/game-server
pub async fn install_game_server(
    AdminOnly(_): AdminOnly,
    State(state): State<AppState>,
    Path(hub_id): Path<i64>,
    Json(body): Json<InstallGameServerBody>,
) -> impl IntoResponse {
    if let Err(e) = require_master(&state) {
        return (e.0, Json(serde_json::json!({"error": e.1}))).into_response();
    }
    enqueue_action(
        &state,
        hub_id,
        HubAction::InstallGameServer {
            slug: body.slug,
            params: body.params,
        },
    )
    .await
    .into_response()
}

/// POST /api/v1/hubs/:id/restart
pub async fn restart_hub(
    AdminOnly(_): AdminOnly,
    State(state): State<AppState>,
    Path(hub_id): Path<i64>,
) -> impl IntoResponse {
    if let Err(e) = require_master(&state) {
        return (e.0, Json(serde_json::json!({"error": e.1}))).into_response();
    }
    enqueue_action(&state, hub_id, HubAction::Restart)
        .await
        .into_response()
}

/// DELETE /api/v1/hubs/:id — forget a hub on the master (does not touch host).
pub async fn delete_hub(
    AdminOnly(_): AdminOnly,
    State(state): State<AppState>,
    Path(hub_id): Path<i64>,
) -> impl IntoResponse {
    if let Err(e) = require_master(&state) {
        return (e.0, Json(serde_json::json!({"error": e.1}))).into_response();
    }
    match state.storage.delete_hub(hub_id).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}
