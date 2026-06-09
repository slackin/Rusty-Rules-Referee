//! Player Groups API — cross-server shared permission collections.
//!
//! # Endpoints
//!
//! ## Groups (master-level)
//! - GET    /api/v1/player-groups                         list all groups
//! - POST   /api/v1/player-groups                         create group
//! - GET    /api/v1/player-groups/:id                     get single group + members
//! - PUT    /api/v1/player-groups/:id                     update name/description
//! - DELETE /api/v1/player-groups/:id                     delete group (cascades members)
//!
//! ## Members
//! - GET    /api/v1/player-groups/:id/members             list members
//! - POST   /api/v1/player-groups/:id/members             add member
//! - PUT    /api/v1/player-groups/:id/members/:guid       update member level/note
//! - DELETE /api/v1/player-groups/:id/members/:guid       remove member
//!
//! ## Server ↔ Group assignment
//! - GET    /api/v1/servers/:id/player-groups             groups for this server
//! - PUT    /api/v1/servers/:id/player-groups             set groups for this server
//!
//! ## Effective user list
//! - GET    /api/v1/servers/:id/users                     merged effective user list

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;

use crate::web::auth::AdminOnly;
use crate::web::state::AppState;

// ---------------------------------------------------------------------------
// Group CRUD
// ---------------------------------------------------------------------------

/// GET /api/v1/player-groups
pub async fn list_groups(
    AdminOnly(_): AdminOnly,
    State(state): State<AppState>,
) -> impl IntoResponse {
    match state.storage.list_player_groups().await {
        Ok(groups) => Json(serde_json::json!({ "player_groups": groups })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

#[derive(Deserialize)]
pub struct CreateGroupBody {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

/// POST /api/v1/player-groups
pub async fn create_group(
    AdminOnly(_): AdminOnly,
    State(state): State<AppState>,
    Json(body): Json<CreateGroupBody>,
) -> impl IntoResponse {
    if body.name.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": "name is required" }))).into_response();
    }
    match state.storage.create_player_group(body.name.trim(), &body.description).await {
        Ok(id) => Json(serde_json::json!({ "id": id })).into_response(),
        Err(e) => (StatusCode::CONFLICT, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

/// GET /api/v1/player-groups/:id
pub async fn get_group(
    AdminOnly(_): AdminOnly,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let group = match state.storage.get_player_group(id).await {
        Ok(g) => g,
        Err(_) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "Not found" }))).into_response(),
    };
    let members = state.storage.list_player_group_members(id).await.unwrap_or_default();
    // Also list servers that use this group.
    let servers = state.storage.get_servers().await.unwrap_or_default();
    let mut assigned_servers = vec![];
    for srv in servers {
        if let Ok(groups) = state.storage.get_server_player_groups(srv.id).await {
            if groups.iter().any(|g| g.id == id) {
                assigned_servers.push(serde_json::json!({ "id": srv.id, "name": srv.name }));
            }
        }
    }
    Json(serde_json::json!({
        "player_group": group,
        "members": members,
        "assigned_servers": assigned_servers,
    })).into_response()
}

#[derive(Deserialize)]
pub struct UpdateGroupBody {
    pub name: Option<String>,
    pub description: Option<String>,
}

/// PUT /api/v1/player-groups/:id
pub async fn update_group(
    AdminOnly(_): AdminOnly,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<UpdateGroupBody>,
) -> impl IntoResponse {
    let group = match state.storage.get_player_group(id).await {
        Ok(g) => g,
        Err(_) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "Not found" }))).into_response(),
    };
    let name = body.name.as_deref().unwrap_or(&group.name);
    let description = body.description.as_deref().unwrap_or(&group.description);
    match state.storage.update_player_group(id, name, description).await {
        Ok(()) => Json(serde_json::json!({ "status": "ok" })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

/// DELETE /api/v1/player-groups/:id
pub async fn delete_group(
    AdminOnly(_): AdminOnly,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match state.storage.delete_player_group(id).await {
        Ok(()) => Json(serde_json::json!({ "status": "ok" })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

// ---------------------------------------------------------------------------
// Member CRUD
// ---------------------------------------------------------------------------

/// GET /api/v1/player-groups/:id/members
pub async fn list_members(
    AdminOnly(_): AdminOnly,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match state.storage.list_player_group_members(id).await {
        Ok(m) => Json(serde_json::json!({ "members": m })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

#[derive(Deserialize)]
pub struct UpsertMemberBody {
    pub client_guid: Option<String>,   // only required on POST
    pub group_bits: u64,
    #[serde(default)]
    pub note: String,
}

/// POST /api/v1/player-groups/:id/members
pub async fn add_member(
    AdminOnly(_): AdminOnly,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<UpsertMemberBody>,
) -> impl IntoResponse {
    let guid = match &body.client_guid {
        Some(g) if !g.trim().is_empty() => g.trim().to_string(),
        _ => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": "client_guid is required" }))).into_response(),
    };
    match state.storage.upsert_player_group_member(id, &guid, body.group_bits, &body.note).await {
        Ok(row_id) => Json(serde_json::json!({ "id": row_id })).into_response(),
        Err(e) => (StatusCode::CONFLICT, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

/// PUT /api/v1/player-groups/:id/members/:guid
pub async fn update_member(
    AdminOnly(_): AdminOnly,
    State(state): State<AppState>,
    Path((id, guid)): Path<(i64, String)>,
    Json(body): Json<UpsertMemberBody>,
) -> impl IntoResponse {
    match state.storage.upsert_player_group_member(id, &guid, body.group_bits, &body.note).await {
        Ok(_) => Json(serde_json::json!({ "status": "ok" })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

/// DELETE /api/v1/player-groups/:id/members/:guid
pub async fn delete_member(
    AdminOnly(_): AdminOnly,
    State(state): State<AppState>,
    Path((id, guid)): Path<(i64, String)>,
) -> impl IntoResponse {
    match state.storage.delete_player_group_member(id, &guid).await {
        Ok(()) => Json(serde_json::json!({ "status": "ok" })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

// ---------------------------------------------------------------------------
// Server ↔ Group assignment
// ---------------------------------------------------------------------------

/// GET /api/v1/servers/:id/player-groups
pub async fn get_server_groups(
    AdminOnly(_): AdminOnly,
    State(state): State<AppState>,
    Path(server_id): Path<i64>,
) -> impl IntoResponse {
    match state.storage.get_server_player_groups(server_id).await {
        Ok(groups) => Json(serde_json::json!({ "player_groups": groups })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

#[derive(Deserialize)]
pub struct SetServerGroupsBody {
    pub group_ids: Vec<i64>,
}

/// PUT /api/v1/servers/:id/player-groups
pub async fn set_server_groups(
    AdminOnly(_): AdminOnly,
    State(state): State<AppState>,
    Path(server_id): Path<i64>,
    Json(body): Json<SetServerGroupsBody>,
) -> impl IntoResponse {
    match state.storage.set_server_player_groups(server_id, &body.group_ids).await {
        Ok(()) => Json(serde_json::json!({ "status": "ok" })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}

// ---------------------------------------------------------------------------
// Effective user list
// ---------------------------------------------------------------------------

/// GET /api/v1/servers/:id/users
pub async fn get_server_users(
    AdminOnly(_): AdminOnly,
    State(state): State<AppState>,
    Path(server_id): Path<i64>,
) -> impl IntoResponse {
    match state.storage.get_effective_users(server_id).await {
        Ok(users) => Json(serde_json::json!({ "users": users })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    }
}
