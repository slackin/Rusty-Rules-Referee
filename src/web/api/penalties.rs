use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;

use crate::core::PenaltyType;
use crate::web::auth::{AdminOnly, AuthUser};
use crate::web::state::AppState;

#[derive(Deserialize)]
pub struct PenaltyQuery {
    pub client_id: Option<i64>,
    #[serde(rename = "type")]
    pub penalty_type: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// GET /api/v1/penalties
pub async fn list_penalties(
    AuthUser(_claims): AuthUser,
    State(state): State<AppState>,
    Query(query): Query<PenaltyQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(50);

    if let Some(client_id) = query.client_id {
        let pt = query.penalty_type.as_deref().and_then(str_to_pt);
        let penalties = state.storage.get_penalties(client_id, pt).await.unwrap_or_default();
        return Json(serde_json::json!({"penalties": penalties})).into_response();
    }

    // If no client_id, return recent bans
    let bans = state.storage.get_last_bans(limit).await.unwrap_or_default();
    Json(serde_json::json!({"penalties": bans})).into_response()
}

/// POST /api/v1/penalties/:id/disable
pub async fn disable_penalty(
    AdminOnly(claims): AdminOnly,
    State(state): State<AppState>,
    Path(penalty_client_id): Path<i64>,
) -> impl IntoResponse {
    // Disable all bans for this client
    let ban_count = state.storage.disable_all_penalties_of_type(penalty_client_id, PenaltyType::Ban).await.unwrap_or(0);
    let tb_count = state.storage.disable_all_penalties_of_type(penalty_client_id, PenaltyType::TempBan).await.unwrap_or(0);

    let _ = state.storage.save_audit_entry(&crate::core::AuditEntry {
        id: 0,
        admin_user_id: Some(claims.user_id),
        action: "unban".to_string(),
        detail: format!("Disabled bans for client_id={} (bans={}, tempbans={})", penalty_client_id, ban_count, tb_count),
        ip_address: None,
        created_at: chrono::Utc::now(),
    }).await;

    Json(serde_json::json!({"status": "ok", "disabled_bans": ban_count, "disabled_tempbans": tb_count}))
}

fn str_to_pt(s: &str) -> Option<PenaltyType> {
    match s {
        "Warning" => Some(PenaltyType::Warning),
        "Notice" => Some(PenaltyType::Notice),
        "Kick" => Some(PenaltyType::Kick),
        "Ban" => Some(PenaltyType::Ban),
        "TempBan" => Some(PenaltyType::TempBan),
        "Mute" => Some(PenaltyType::Mute),
        _ => None,
    }
}
