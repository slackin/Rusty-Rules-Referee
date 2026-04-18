use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;

use crate::core::AdminUser;
use crate::web::auth::{AdminOnly, AuthUser};
use crate::web::state::AppState;

/// GET /api/v1/users
pub async fn list_users(
    AdminOnly(_claims): AdminOnly,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let users = state.storage.get_admin_users().await.unwrap_or_default();
    Json(serde_json::json!({"users": users}))
}

#[derive(Deserialize)]
pub struct CreateUserBody {
    pub username: String,
    pub password: String,
    pub role: Option<String>,
}

/// POST /api/v1/users
pub async fn create_user(
    AdminOnly(_claims): AdminOnly,
    State(state): State<AppState>,
    Json(body): Json<CreateUserBody>,
) -> impl IntoResponse {
    if body.username.is_empty() || body.password.len() < 6 {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "Username required, password must be at least 6 characters"}))).into_response();
    }

    let hash = match bcrypt::hash(&body.password, bcrypt::DEFAULT_COST) {
        Ok(h) => h,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": format!("Hash error: {}", e)}))).into_response();
        }
    };

    let user = AdminUser {
        id: 0,
        username: body.username.clone(),
        password_hash: hash,
        role: body.role.clone().unwrap_or_else(|| "admin".to_string()),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    match state.storage.save_admin_user(&user).await {
        Ok(id) => {
            Json(serde_json::json!({"id": id, "username": user.username, "role": user.role})).into_response()
        }
        Err(e) => {
            (StatusCode::CONFLICT, Json(serde_json::json!({"error": format!("Failed to create user: {}", e)}))).into_response()
        }
    }
}

#[derive(Deserialize)]
pub struct UpdateUserBody {
    pub password: Option<String>,
    pub role: Option<String>,
}

/// PUT /api/v1/users/:id
pub async fn update_user(
    AdminOnly(_claims): AdminOnly,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<UpdateUserBody>,
) -> impl IntoResponse {
    let mut user = match state.storage.get_admin_user_by_id(id).await {
        Ok(u) => u,
        Err(_) => {
            return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "User not found"}))).into_response();
        }
    };

    if let Some(ref pw) = body.password {
        if pw.len() < 6 {
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "Password must be at least 6 characters"}))).into_response();
        }
        user.password_hash = bcrypt::hash(pw, bcrypt::DEFAULT_COST).unwrap_or_default();
    }

    if let Some(ref role) = body.role {
        user.role = role.clone();
    }

    match state.storage.save_admin_user(&user).await {
        Ok(_) => Json(serde_json::json!({"status": "ok"})).into_response(),
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// DELETE /api/v1/users/:id
pub async fn delete_user(
    AdminOnly(claims): AdminOnly,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    // Prevent deleting yourself
    if claims.user_id == id {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "Cannot delete your own account"}))).into_response();
    }

    match state.storage.delete_admin_user(id).await {
        Ok(_) => Json(serde_json::json!({"status": "ok"})).into_response(),
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

#[derive(Deserialize)]
pub struct ChangePasswordBody {
    pub current_password: String,
    pub new_password: String,
}

/// PUT /api/v1/users/me/password
pub async fn change_password(
    AuthUser(claims): AuthUser,
    State(state): State<AppState>,
    Json(body): Json<ChangePasswordBody>,
) -> impl IntoResponse {
    if body.new_password.len() < 6 {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "New password must be at least 6 characters"}))).into_response();
    }

    let user = match state.storage.get_admin_user_by_id(claims.user_id).await {
        Ok(u) => u,
        Err(_) => {
            return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "User not found"}))).into_response();
        }
    };

    // Verify current password
    match bcrypt::verify(&body.current_password, &user.password_hash) {
        Ok(true) => {}
        _ => {
            return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "Current password is incorrect"}))).into_response();
        }
    }

    let hash = match bcrypt::hash(&body.new_password, bcrypt::DEFAULT_COST) {
        Ok(h) => h,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": format!("Hash error: {}", e)}))).into_response();
        }
    };

    let mut updated = user;
    updated.password_hash = hash;
    updated.updated_at = chrono::Utc::now();

    match state.storage.save_admin_user(&updated).await {
        Ok(_) => Json(serde_json::json!({"status": "ok"})).into_response(),
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}
