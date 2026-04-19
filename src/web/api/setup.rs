//! First-run setup wizard API endpoints.
//!
//! These endpoints are unauthenticated and only available before the first
//! admin user has been created. Once setup is complete, they return 403.

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::web::state::AppState;

// ---- Request / Response types ----

#[derive(Debug, Serialize)]
pub struct SetupStatusResponse {
    pub needs_setup: bool,
    pub mode: String,
    pub version: String,
}

#[derive(Debug, Deserialize)]
pub struct CompleteSetupRequest {
    /// Admin username to create.
    pub admin_username: String,
    /// Admin password.
    pub admin_password: String,
    /// Optional: bot name override.
    #[serde(default)]
    pub bot_name: Option<String>,
    /// Optional: game server IP (standalone/client).
    #[serde(default)]
    pub server_ip: Option<String>,
    /// Optional: game server port (standalone/client).
    #[serde(default)]
    pub server_port: Option<u16>,
    /// Optional: RCON password (standalone/client).
    #[serde(default)]
    pub rcon_password: Option<String>,
    /// Optional: game log path (standalone/client).
    #[serde(default)]
    pub game_log: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CompleteSetupResponse {
    pub ok: bool,
    pub message: String,
}

// ---- Helpers ----

/// Check whether setup is needed (no admin users exist).
async fn is_setup_needed(state: &AppState) -> bool {
    match state.storage.get_admin_users().await {
        Ok(users) => users.is_empty(),
        Err(_) => true, // If we can't check, assume setup is needed
    }
}

/// Detect the current run mode from config.
fn detect_mode(state: &AppState) -> &'static str {
    if state.config.client.is_some() {
        "client"
    } else if state.config.master.is_some() {
        "master"
    } else {
        "standalone"
    }
}

// ---- Endpoints ----

/// GET /api/v1/setup/status — Check whether first-run setup is needed.
pub async fn setup_status(State(state): State<AppState>) -> impl IntoResponse {
    let needs_setup = is_setup_needed(&state).await;
    let mode = detect_mode(&state).to_string();

    Json(SetupStatusResponse {
        needs_setup,
        mode,
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// POST /api/v1/setup/complete — Complete the first-run setup.
///
/// This endpoint only works when no admin users exist. It creates the first
/// admin account and optionally applies configuration overrides.
pub async fn complete_setup(
    State(state): State<AppState>,
    Json(body): Json<CompleteSetupRequest>,
) -> impl IntoResponse {
    // Guard: only available during first-run setup
    if !is_setup_needed(&state).await {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error": "Setup has already been completed"})),
        )
            .into_response();
    }

    // Validate inputs
    if body.admin_username.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Admin username is required"})),
        )
            .into_response();
    }
    if body.admin_password.len() < 6 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Password must be at least 6 characters"})),
        )
            .into_response();
    }

    // Create admin user
    let password_hash = match bcrypt::hash(&body.admin_password, bcrypt::DEFAULT_COST) {
        Ok(h) => h,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to hash password: {}", e)})),
            )
                .into_response();
        }
    };

    let admin = crate::core::AdminUser {
        id: 0,
        username: body.admin_username.trim().to_string(),
        password_hash,
        role: "admin".to_string(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    if let Err(e) = state.storage.save_admin_user(&admin).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to create admin user: {}", e)})),
        )
            .into_response();
    }

    // Apply optional config overrides
    let has_config_changes = body.bot_name.is_some()
        || body.server_ip.is_some()
        || body.server_port.is_some()
        || body.rcon_password.is_some()
        || body.game_log.is_some();

    if has_config_changes {
        if let Ok(content) = std::fs::read_to_string(&state.config_path) {
            if let Ok(mut doc) = content.parse::<toml::Value>() {
                if let Some(referee) = doc.get_mut("referee").and_then(|v| v.as_table_mut()) {
                    if let Some(name) = &body.bot_name {
                        referee.insert("bot_name".to_string(), toml::Value::String(name.clone()));
                    }
                }
                if let Some(server) = doc.get_mut("server").and_then(|v| v.as_table_mut()) {
                    if let Some(ip) = &body.server_ip {
                        server
                            .insert("public_ip".to_string(), toml::Value::String(ip.clone()));
                    }
                    if let Some(port) = body.server_port {
                        server.insert(
                            "port".to_string(),
                            toml::Value::Integer(port as i64),
                        );
                    }
                    if let Some(rcon) = &body.rcon_password {
                        server.insert(
                            "rcon_password".to_string(),
                            toml::Value::String(rcon.clone()),
                        );
                    }
                    if let Some(log) = &body.game_log {
                        server.insert(
                            "game_log".to_string(),
                            toml::Value::String(log.clone()),
                        );
                    }
                }

                if let Ok(output) = toml::to_string_pretty(&doc) {
                    let _ = std::fs::write(&state.config_path, &output);
                }
            }
        }
    }

    info!(
        admin = %body.admin_username,
        mode = detect_mode(&state),
        "First-run setup completed"
    );

    Json(CompleteSetupResponse {
        ok: true,
        message: "Setup complete! You can now log in.".to_string(),
    })
    .into_response()
}
