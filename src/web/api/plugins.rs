use axum::{extract::State, response::IntoResponse, Json};

use crate::web::auth::AuthUser;
use crate::web::state::AppState;

/// GET /api/v1/plugins — list all plugins.
pub async fn list_plugins(
    AuthUser(_claims): AuthUser,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let plugins: Vec<serde_json::Value> = state.config.plugins.iter().map(|p| {
        serde_json::json!({
            "name": p.name,
            "enabled": p.enabled,
            "config_file": p.config_file,
        })
    }).collect();

    Json(serde_json::json!({"plugins": plugins}))
}
