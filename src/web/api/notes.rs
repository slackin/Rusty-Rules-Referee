use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;

use crate::web::auth::AuthUser;
use crate::web::state::AppState;

#[derive(Deserialize)]
pub struct NoteBody {
    pub content: String,
}

/// GET /api/v1/notes — get the current admin's personal note.
pub async fn get_note(
    AuthUser(claims): AuthUser,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let note = state
        .storage
        .get_admin_note(claims.user_id)
        .await
        .unwrap_or(None);
    let content = note.map(|n| n.content).unwrap_or_default();
    Json(serde_json::json!({ "content": content }))
}

/// PUT /api/v1/notes — save the current admin's personal note.
pub async fn save_note(
    AuthUser(claims): AuthUser,
    State(state): State<AppState>,
    Json(body): Json<NoteBody>,
) -> impl IntoResponse {
    match state
        .storage
        .save_admin_note(claims.user_id, &body.content)
        .await
    {
        Ok(_) => Json(serde_json::json!({ "ok": true })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}
