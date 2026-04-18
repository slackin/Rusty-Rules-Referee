use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;

use crate::web::auth::AuthUser;
use crate::web::state::AppState;

#[derive(Deserialize)]
pub struct AliasQuery {
    pub client_id: Option<i64>,
}

/// GET /api/v1/aliases?client_id=X
pub async fn list_aliases(
    AuthUser(_claims): AuthUser,
    State(state): State<AppState>,
    Query(query): Query<AliasQuery>,
) -> impl IntoResponse {
    if let Some(cid) = query.client_id {
        let aliases = state.storage.get_aliases(cid).await.unwrap_or_default();
        return Json(serde_json::json!({"aliases": aliases}));
    }
    Json(serde_json::json!({"aliases": []}))
}
