use axum::{extract::State, response::IntoResponse, Json};

use crate::web::auth::AuthUser;
use crate::web::state::AppState;

/// GET /api/v1/groups
pub async fn list_groups(
    AuthUser(_claims): AuthUser,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let groups = state.storage.get_groups().await.unwrap_or_default();
    Json(serde_json::json!({"groups": groups}))
}
