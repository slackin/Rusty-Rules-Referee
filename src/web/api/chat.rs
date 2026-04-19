use axum::{
    extract::{Query, State},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;

use crate::web::auth::AuthUser;
use crate::web::state::AppState;

#[derive(Deserialize)]
pub struct ChatQuery {
    pub limit: Option<u32>,
    pub before_id: Option<i64>,
}

/// GET /api/v1/chat — recent chat messages (paginated).
pub async fn list_chat(
    AuthUser(_claims): AuthUser,
    State(state): State<AppState>,
    Query(query): Query<ChatQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(50).min(200);
    let messages = state
        .storage
        .get_chat_messages(limit, query.before_id)
        .await
        .unwrap_or_default();
    Json(serde_json::json!({ "messages": messages }))
}
