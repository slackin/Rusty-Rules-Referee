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
    pub query: Option<String>,
    pub client_id: Option<i64>,
}

/// GET /api/v1/chat — recent chat messages (paginated, searchable).
pub async fn list_chat(
    AuthUser(_claims): AuthUser,
    State(state): State<AppState>,
    Query(params): Query<ChatQuery>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(50).min(200);
    let query_str = params.query.as_deref().filter(|s| !s.is_empty());
    let messages = state
        .storage
        .search_chat_messages(query_str, params.client_id, limit, params.before_id)
        .await
        .unwrap_or_default();
    Json(serde_json::json!({ "messages": messages }))
}
