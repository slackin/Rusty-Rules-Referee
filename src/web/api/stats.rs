use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;

use crate::web::auth::AuthUser;
use crate::web::state::AppState;

#[derive(Deserialize)]
pub struct LeaderboardQuery {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// GET /api/v1/stats/leaderboard
pub async fn leaderboard(
    AuthUser(_claims): AuthUser,
    State(state): State<AppState>,
    Query(query): Query<LeaderboardQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(50);
    let offset = query.offset.unwrap_or(0);
    let data = state.storage.get_xlr_leaderboard(limit, offset).await.unwrap_or_default();
    Json(serde_json::json!({"leaderboard": data}))
}

/// GET /api/v1/stats/player/:id
pub async fn player_stats(
    AuthUser(_claims): AuthUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let stats = state.storage.get_xlr_player_stats(id).await.unwrap_or(None);
    let weapons = state.storage.get_xlr_weapon_stats(Some(id)).await.unwrap_or_default();
    Json(serde_json::json!({
        "stats": stats,
        "weapons": weapons,
    }))
}

/// GET /api/v1/stats/weapons
pub async fn weapon_stats(
    AuthUser(_claims): AuthUser,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let data = state.storage.get_xlr_weapon_stats(None).await.unwrap_or_default();
    Json(serde_json::json!({"weapons": data}))
}

/// GET /api/v1/stats/maps
pub async fn map_stats(
    AuthUser(_claims): AuthUser,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let data = state.storage.get_xlr_map_stats().await.unwrap_or_default();
    Json(serde_json::json!({"maps": data}))
}
