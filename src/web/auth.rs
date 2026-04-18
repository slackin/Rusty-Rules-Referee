use axum::{
    async_trait,
    extract::{FromRequestParts, State},
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

use super::state::AppState;

/// JWT claims.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,       // username
    pub user_id: i64,
    pub role: String,
    pub exp: usize,        // expiry (unix timestamp)
    pub iat: usize,        // issued at
}

/// Request body for login.
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// Response body for login.
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user: UserInfo,
}

#[derive(Debug, Serialize)]
pub struct UserInfo {
    pub id: i64,
    pub username: String,
    pub role: String,
}

/// Create a JWT token for the given user.
pub fn create_token(secret: &str, user_id: i64, username: &str, role: &str) -> Result<String, jsonwebtoken::errors::Error> {
    let now = chrono::Utc::now().timestamp() as usize;
    let claims = Claims {
        sub: username.to_string(),
        user_id,
        role: role.to_string(),
        exp: now + 86400, // 24 hours
        iat: now,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
}

/// Validate and decode a JWT token.
pub fn decode_token(secret: &str, token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )?;
    Ok(data.claims)
}

/// Extractor: authenticated user from Authorization header.
pub struct AuthUser(pub Claims);

#[async_trait]
impl FromRequestParts<AppState> for AuthUser {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        let token = if let Some(t) = auth_header.strip_prefix("Bearer ") {
            t
        } else {
            return Err((StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "Missing or invalid Authorization header"}))).into_response());
        };

        match decode_token(&state.jwt_secret, token) {
            Ok(claims) => Ok(AuthUser(claims)),
            Err(_) => Err((StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "Invalid or expired token"}))).into_response()),
        }
    }
}

/// Extractor: require admin role.
pub struct AdminOnly(pub Claims);

#[async_trait]
impl FromRequestParts<AppState> for AdminOnly {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let AuthUser(claims) = AuthUser::from_request_parts(parts, state).await?;
        if claims.role != "admin" {
            return Err((StatusCode::FORBIDDEN, Json(serde_json::json!({"error": "Admin access required"}))).into_response());
        }
        Ok(AdminOnly(claims))
    }
}

/// POST /api/v1/auth/login
pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> impl IntoResponse {
    let user = match state.storage.get_admin_user(&body.username).await {
        Ok(u) => u,
        Err(_) => {
            return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "Invalid credentials"}))).into_response();
        }
    };

    match bcrypt::verify(&body.password, &user.password_hash) {
        Ok(true) => {}
        _ => {
            return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "Invalid credentials"}))).into_response();
        }
    }

    match create_token(&state.jwt_secret, user.id, &user.username, &user.role) {
        Ok(token) => {
            Json(LoginResponse {
                token,
                user: UserInfo {
                    id: user.id,
                    username: user.username,
                    role: user.role,
                },
            }).into_response()
        }
        Err(_) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to create token"}))).into_response()
        }
    }
}

/// GET /api/v1/auth/me
pub async fn me(AuthUser(claims): AuthUser) -> impl IntoResponse {
    Json(serde_json::json!({
        "id": claims.user_id,
        "username": claims.sub,
        "role": claims.role,
    }))
}
