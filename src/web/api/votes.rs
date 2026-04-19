use axum::{
    extract::{Query, State},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;

use crate::web::auth::AuthUser;
use crate::web::state::AppState;

#[derive(Deserialize)]
pub struct VotesQuery {
    pub limit: Option<u32>,
}

/// GET /api/v1/votes — recent map/game votes.
pub async fn list_votes(
    AuthUser(_claims): AuthUser,
    State(state): State<AppState>,
    Query(query): Query<VotesQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(20).min(100);
    let votes = state
        .storage
        .get_recent_votes(limit)
        .await
        .unwrap_or_default();
    Json(serde_json::json!({ "votes": votes }))
}
