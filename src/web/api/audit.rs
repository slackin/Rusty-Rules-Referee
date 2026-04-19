use axum::{
    extract::{Query, State},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;

use crate::web::auth::AdminOnly;
use crate::web::state::AppState;

#[derive(Deserialize)]
pub struct AuditQuery {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// GET /api/v1/audit-log — paginated audit trail (admin only).
pub async fn list_audit_log(
    AdminOnly(_claims): AdminOnly,
    State(state): State<AppState>,
    Query(query): Query<AuditQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(50).min(200);
    let offset = query.offset.unwrap_or(0);

    let entries = state
        .storage
        .get_audit_log(limit, offset)
        .await
        .unwrap_or_default();

    // Enrich entries with admin usernames
    let admin_users = state.storage.get_admin_users().await.unwrap_or_default();
    let enriched: Vec<serde_json::Value> = entries
        .iter()
        .map(|e| {
            let admin_name = e
                .admin_user_id
                .and_then(|aid| admin_users.iter().find(|u| u.id == aid))
                .map(|u| u.username.as_str())
                .unwrap_or("system");
            serde_json::json!({
                "id": e.id,
                "admin_user_id": e.admin_user_id,
                "admin_username": admin_name,
                "action": e.action,
                "detail": e.detail,
                "ip_address": e.ip_address,
                "created_at": e.created_at,
            })
        })
        .collect();

    Json(serde_json::json!({ "entries": enriched }))
}
