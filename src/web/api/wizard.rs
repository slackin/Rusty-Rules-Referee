// =====================================================================
// UrT install-wizard endpoints (Phase 5)
//
// The wizard is driven from the master UI and talks to the selected
// client bot over the existing sync protocol. Each endpoint here is a
// thin HTTP->ClientRequest adapter that forwards the payload and
// returns the raw ClientResponse JSON for the UI to render.
// =====================================================================

use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use std::time::Duration as StdDuration;

use crate::sync::protocol::{
    ClientRequest, GameServerWizardParams, PortKind, ServiceAction,
};
use crate::web::api::servers::{send_client_request, CommandResponse};
use crate::web::state::AppState;

// ---------------------------------------------------------------------
// GET/POST /api/v1/servers/:id/wizard/suggest
// ---------------------------------------------------------------------

/// Ask the client bot for the state of its install marker + suggested
/// defaults (free port, recommended install path, current slug, etc.).
pub async fn wizard_suggest(
    State(state): State<AppState>,
    Path(server_id): Path<i64>,
) -> impl IntoResponse {
    match send_client_request(
        &state,
        server_id,
        ClientRequest::SuggestInstallDefaults,
        StdDuration::from_secs(15),
    )
    .await
    {
        Ok(resp) => Json(serde_json::to_value(&resp).unwrap_or_default()).into_response(),
        Err((status, msg)) => {
            (status, Json(CommandResponse { ok: false, message: msg })).into_response()
        }
    }
}

// ---------------------------------------------------------------------
// POST /api/v1/servers/:id/wizard/probe-ports
// ---------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ProbePortsRequest {
    pub ports: Vec<u16>,
    #[serde(default)]
    pub kind: Option<String>, // "udp" (default) or "tcp"
}

/// Ask the client bot to probe a list of ports for availability (both
/// passive `ss` parse and active bind attempt).
pub async fn wizard_probe_ports(
    State(state): State<AppState>,
    Path(server_id): Path<i64>,
    Json(req): Json<ProbePortsRequest>,
) -> impl IntoResponse {
    let kind = match req.kind.as_deref() {
        Some(k) if k.eq_ignore_ascii_case("tcp") => PortKind::Tcp,
        _ => PortKind::Udp,
    };
    if req.ports.is_empty() {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            Json(CommandResponse {
                ok: false,
                message: "ports list is required".to_string(),
            }),
        )
            .into_response();
    }
    if req.ports.len() > 64 {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            Json(CommandResponse {
                ok: false,
                message: "too many ports (max 64)".to_string(),
            }),
        )
            .into_response();
    }
    match send_client_request(
        &state,
        server_id,
        ClientRequest::DetectPorts { ports: req.ports, kind },
        StdDuration::from_secs(30),
    )
    .await
    {
        Ok(resp) => Json(serde_json::to_value(&resp).unwrap_or_default()).into_response(),
        Err((status, msg)) => {
            (status, Json(CommandResponse { ok: false, message: msg })).into_response()
        }
    }
}

// ---------------------------------------------------------------------
// POST /api/v1/servers/:id/wizard/install
// ---------------------------------------------------------------------

/// Kick off the full install wizard on the client. The client spawns the
/// install task in the background and returns `InstallStarted` immediately;
/// the UI then polls `/install-status` for progress.
pub async fn wizard_install(
    State(state): State<AppState>,
    Path(server_id): Path<i64>,
    Json(params): Json<GameServerWizardParams>,
) -> impl IntoResponse {
    if params.rcon_password.trim().is_empty() {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            Json(CommandResponse {
                ok: false,
                message: "RCON password is required".to_string(),
            }),
        )
            .into_response();
    }
    if params.port == 0 {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            Json(CommandResponse {
                ok: false,
                message: "port must be > 0".to_string(),
            }),
        )
            .into_response();
    }
    if params.install_path.trim().is_empty() {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            Json(CommandResponse {
                ok: false,
                message: "install_path is required".to_string(),
            }),
        )
            .into_response();
    }
    match send_client_request(
        &state,
        server_id,
        ClientRequest::InstallGameServerWizard { params },
        StdDuration::from_secs(30),
    )
    .await
    {
        Ok(resp) => Json(serde_json::to_value(&resp).unwrap_or_default()).into_response(),
        Err((status, msg)) => {
            (status, Json(CommandResponse { ok: false, message: msg })).into_response()
        }
    }
}

// ---------------------------------------------------------------------
// POST /api/v1/servers/:id/wizard/service/:action
// ---------------------------------------------------------------------

/// Control the managed `urt@<slug>.service` unit (start/stop/restart/
/// enable/disable/status).
pub async fn wizard_service_action(
    State(state): State<AppState>,
    Path((server_id, action)): Path<(i64, String)>,
) -> impl IntoResponse {
    let action_enum = match action.to_ascii_lowercase().as_str() {
        "start" => ServiceAction::Start,
        "stop" => ServiceAction::Stop,
        "restart" => ServiceAction::Restart,
        "enable" => ServiceAction::Enable,
        "disable" => ServiceAction::Disable,
        "status" => ServiceAction::Status,
        other => {
            return (
                axum::http::StatusCode::BAD_REQUEST,
                Json(CommandResponse {
                    ok: false,
                    message: format!("unknown service action: {}", other),
                }),
            )
                .into_response();
        }
    };
    match send_client_request(
        &state,
        server_id,
        ClientRequest::GameServerService { action: action_enum },
        StdDuration::from_secs(30),
    )
    .await
    {
        Ok(resp) => Json(serde_json::to_value(&resp).unwrap_or_default()).into_response(),
        Err((status, msg)) => {
            (status, Json(CommandResponse { ok: false, message: msg })).into_response()
        }
    }
}
