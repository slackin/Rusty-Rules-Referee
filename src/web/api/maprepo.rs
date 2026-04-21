//! Master-side map repository browser endpoints.
//!
//! Backed by the `map_repo_entries` cache populated by
//! [`crate::maprepo::refresh_all`].

use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::web::auth::{AdminOnly, AuthUser};
use crate::web::state::AppState;

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub offset: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub entries: Vec<serde_json::Value>,
    pub total: u64,
    pub limit: u32,
    pub offset: u32,
}

/// GET /api/v1/map-repo?q=&limit=&offset=
pub async fn search_map_repo(
    AuthUser(_claims): AuthUser,
    State(state): State<AppState>,
    Query(q): Query<SearchQuery>,
) -> impl IntoResponse {
    let query = q.q.unwrap_or_default();
    let limit = q.limit.unwrap_or(50).clamp(1, 500);
    let offset = q.offset.unwrap_or(0);
    match state.storage.search_map_repo(&query, limit, offset).await {
        Ok((entries, total)) => {
            let entries: Vec<serde_json::Value> = entries
                .into_iter()
                .map(|e| {
                    serde_json::json!({
                        "filename": e.filename,
                        "size": e.size,
                        "mtime": e.mtime,
                        "source_url": e.source_url,
                        "last_seen_at": e.last_seen_at.to_rfc3339(),
                    })
                })
                .collect();
            Json(SearchResponse {
                entries,
                total,
                limit,
                offset,
            })
            .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// POST /api/v1/map-repo/refresh — trigger a background refresh. Returns
/// immediately; the updated cache becomes visible via `GET /map-repo/status`.
pub async fn refresh_map_repo(
    AdminOnly(_claims): AdminOnly,
    State(state): State<AppState>,
) -> impl IntoResponse {
    if !state.config.map_repo.enabled {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "map_repo is disabled in config"
            })),
        )
            .into_response();
    }
    let storage: Arc<dyn crate::storage::Storage> = state.storage.clone();
    let sources = state.config.map_repo.sources.clone();
    tokio::spawn(async move {
        crate::maprepo::refresh_all(storage, &sources).await;
    });
    info!("map-repo refresh triggered");
    Json(serde_json::json!({ "ok": true, "message": "Refresh started" })).into_response()
}

/// GET /api/v1/map-repo/status — cache stats + last refresh time.
pub async fn map_repo_status(
    AuthUser(_claims): AuthUser,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let count = state.storage.count_map_repo_entries().await.unwrap_or(0);
    let last = state
        .storage
        .latest_map_repo_refresh()
        .await
        .ok()
        .flatten()
        .map(|t| t.to_rfc3339());
    Json(serde_json::json!({
        "enabled": state.config.map_repo.enabled,
        "sources": state.config.map_repo.sources,
        "refresh_interval_hours": state.config.map_repo.refresh_interval_hours,
        "entry_count": count,
        "last_refresh": last,
    }))
    .into_response()
}
