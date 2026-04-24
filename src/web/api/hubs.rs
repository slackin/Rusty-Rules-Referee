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
///
/// Returns `202 Accepted` with `{ "action_id": "..." }` immediately. The
/// UI should then poll `GET /api/v1/hubs/:id/actions/:action_id` to
/// display live progress events and collect the final result.
pub async fn install_client(
    AdminOnly(_): AdminOnly,
    State(state): State<AppState>,
    Path(hub_id): Path<i64>,
    Json(body): Json<InstallClientBody>,
) -> impl IntoResponse {
    if let Err(e) = require_master(&state) {
        return (e.0, Json(serde_json::json!({"error": e.1}))).into_response();
    }
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
    let logs = match &state.hub_action_logs {
        Some(l) => l.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": "Hub orchestration not available"})),
            )
                .into_response();
        }
    };
    let action = HubAction::InstallClient {
        slug: body.slug,
        server_name: body.server_name,
        game_server: body.game_server,
        register_systemd: body.register_systemd,
    };
    let action_id = crate::sync::master::enqueue_hub_action(
        &pending_actions,
        &logs,
        hub_id,
        action,
    )
    .await;
    (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({ "action_id": action_id })),
    )
        .into_response()
}

/// GET /api/v1/hubs/:id/actions/:action_id — return the current progress
/// log and (if available) final result for an enqueued hub action.
pub async fn get_action_progress(
    AdminOnly(_): AdminOnly,
    State(state): State<AppState>,
    Path((hub_id, action_id)): Path<(i64, String)>,
) -> impl IntoResponse {
    if let Err(e) = require_master(&state) {
        return (e.0, Json(serde_json::json!({"error": e.1}))).into_response();
    }
    let logs = match &state.hub_action_logs {
        Some(l) => l.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": "Hub orchestration not available"})),
            )
                .into_response();
        }
    };
    let logs = logs.read().await;
    match logs.get(&action_id) {
        Some(log) if log.hub_id == hub_id => Json(serde_json::json!({
            "action_id": action_id,
            "hub_id": log.hub_id,
            "action_kind": log.action_kind,
            "created_at": log.created_at,
            "events": log.events,
            "result": log.result,
            "done": log.result.is_some(),
        }))
        .into_response(),
        _ => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Unknown or expired action"})),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct UninstallClientQuery {
    #[serde(default)]
    pub remove_data: bool,
}

/// DELETE /api/v1/hubs/:id/clients/:slug — uninstall a hub-managed client.
///
/// Returns `202 Accepted` with `{ "action_id": "..." }` immediately and
/// spawns a background task that, once the hub reports completion,
/// deletes the matching `game_servers` row on the master. The UI polls
/// `GET /api/v1/hubs/:id/actions/:action_id` for step-by-step progress.
pub async fn uninstall_client(
    AdminOnly(_): AdminOnly,
    State(state): State<AppState>,
    Path((hub_id, slug)): Path<(i64, String)>,
    Query(q): Query<UninstallClientQuery>,
) -> impl IntoResponse {
    if let Err(e) = require_master(&state) {
        return (e.0, Json(serde_json::json!({"error": e.1}))).into_response();
    }
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
    let logs = match &state.hub_action_logs {
        Some(l) => l.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": "Hub orchestration not available"})),
            )
                .into_response();
        }
    };
    let action = HubAction::UninstallClient {
        slug: slug.clone(),
        remove_data: q.remove_data,
    };
    let action_id = crate::sync::master::enqueue_hub_action(
        &pending_actions,
        &logs,
        hub_id,
        action,
    )
    .await;

    // Spawn a watcher that waits for the log entry to have a result and
    // then cleans up the master-side `game_servers` row.
    let logs_watch = logs.clone();
    let action_id_watch = action_id.clone();
    let storage = state.storage.clone();
    tokio::spawn(async move {
        use tokio::time::{sleep, Duration};
        let deadline = std::time::Instant::now() + Duration::from_secs(15 * 60);
        loop {
            if std::time::Instant::now() > deadline {
                tracing::warn!(action_id = %action_id_watch, hub_id, slug = %slug,
                    "Uninstall watcher timed out waiting for hub response");
                return;
            }
            let done_result = {
                let guard = logs_watch.read().await;
                guard.get(&action_id_watch).and_then(|l| l.result.clone())
            };
            if let Some(result) = done_result {
                if result.ok {
                    match storage.get_servers().await {
                        Ok(servers) => {
                            for s in servers {
                                if s.hub_id == Some(hub_id)
                                    && s.slug.as_deref() == Some(&slug)
                                {
                                    if let Err(e) = storage.delete_server(s.id).await {
                                        tracing::warn!(server_id = s.id, error = %e,
                                            "Failed to delete game_servers row after uninstall");
                                    } else {
                                        tracing::info!(server_id = s.id, hub_id, slug = %slug,
                                            "Deleted game_servers row after uninstall");
                                    }
                                }
                            }
                        }
                        Err(e) => tracing::warn!(error = %e,
                            "get_servers failed during uninstall cleanup"),
                    }
                } else {
                    tracing::warn!(action_id = %action_id_watch, hub_id, slug = %slug,
                        msg = %result.message, "Hub reported uninstall failure — skipping DB cleanup");
                }
                return;
            }
            sleep(Duration::from_millis(500)).await;
        }
    });

    (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({ "action_id": action_id })),
    )
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

/// GET /api/v1/hubs/:id/version — current hub build + master-side latest manifest.
pub async fn get_hub_version(
    AdminOnly(_): AdminOnly,
    State(state): State<AppState>,
    Path(hub_id): Path<i64>,
) -> impl IntoResponse {
    if let Err(e) = require_master(&state) {
        return (e.0, Json(serde_json::json!({"error": e.1}))).into_response();
    }

    // Cached heartbeat-reported version.
    let cached = if let Some(map) = state.hub_versions.as_ref() {
        map.read().await.get(&hub_id).map(|v| {
            serde_json::json!({
                "build_hash": v.build_hash,
                "version": v.version,
                "reported_at": v.reported_at.to_rfc3339(),
            })
        })
    } else {
        None
    };

    // Channel comes from the hub row in the DB.
    let (channel, db_build) = match state.storage.get_hub(hub_id).await {
        Ok(h) => (h.update_channel, h.build_hash),
        Err(_) => (state.config.update.channel.clone(), None),
    };

    let update_url = state.config.update.url.clone();
    let latest = match crate::update::check_for_update(&update_url, &channel, "").await {
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
        "hub_id": hub_id,
        "cached": cached,
        "db_build_hash": db_build,
        "channel": channel,
        "latest": latest,
        "master_update_url": update_url,
    }))
    .into_response()
}

/// POST /api/v1/hubs/:id/force-update — ask the hub to download + apply + restart.
pub async fn force_hub_update(
    AdminOnly(_): AdminOnly,
    State(state): State<AppState>,
    Path(hub_id): Path<i64>,
) -> impl IntoResponse {
    if let Err(e) = require_master(&state) {
        return (e.0, Json(serde_json::json!({"error": e.1}))).into_response();
    }
    let update_url = state.config.update.url.clone();
    let channel = state
        .storage
        .get_hub(hub_id)
        .await
        .ok()
        .map(|h| h.update_channel)
        .unwrap_or_else(|| state.config.update.channel.clone());

    enqueue_action(
        &state,
        hub_id,
        HubAction::ForceUpdate {
            update_url: Some(update_url),
            channel: Some(channel),
        },
    )
    .await
    .into_response()
}

#[derive(Debug, Deserialize)]
pub struct SetHubUpdateChannelBody {
    pub channel: String,
}

/// PUT /api/v1/hubs/:id/update-channel
pub async fn set_hub_update_channel(
    AdminOnly(_): AdminOnly,
    State(state): State<AppState>,
    Path(hub_id): Path<i64>,
    Json(body): Json<SetHubUpdateChannelBody>,
) -> impl IntoResponse {
    if let Err(e) = require_master(&state) {
        return (e.0, Json(serde_json::json!({"error": e.1}))).into_response();
    }
    let channel = body.channel.trim().to_string();
    if !crate::config::VALID_UPDATE_CHANNELS.contains(&channel.as_str()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": format!(
                    "Invalid channel '{}' — expected one of: {}",
                    channel,
                    crate::config::VALID_UPDATE_CHANNELS.join(", ")
                )
            })),
        )
            .into_response();
    }
    if let Err(e) = state.storage.set_hub_update_channel(hub_id, &channel).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response();
    }
    Json(serde_json::json!({
        "ok": true,
        "message": format!("Channel set to {}", channel),
        "channel": channel,
    }))
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

    // Best-effort: ask the hub to uninstall itself (and every sub-client it
    // manages) before we forget it. A dead or disconnected hub simply
    // won't respond — we continue and delete the row anyway so the admin
    // doesn't end up with orphaned master rows.
    let pending_actions_opt = state.pending_hub_actions.clone();
    let pending_responses_opt = state.pending_hub_responses.clone();
    if let (Some(pending_actions), Some(pending_responses)) =
        (pending_actions_opt, pending_responses_opt)
    {
        let update_url = state.config.update.url.clone();
        let action = HubAction::SelfUninstall {
            remove_gameserver: true,
            update_url: Some(update_url),
        };
        match send_action_to_hub(
            &pending_responses,
            &pending_actions,
            hub_id,
            action,
            Duration::from_secs(30),
        )
        .await
        {
            Ok(resp) => tracing::info!(hub_id, ?resp, "Hub self-uninstall acknowledged"),
            Err(e) => warn!(
                hub_id,
                error = %e,
                "Hub self-uninstall not acknowledged; deleting row anyway"
            ),
        }
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
