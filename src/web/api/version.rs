use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use tracing::{error, info, warn};

use crate::update::{check_for_update, download_and_verify, apply_update, restart};
use crate::web::auth::AdminOnly;
use crate::web::state::AppState;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const BUILD_HASH: &str = env!("BUILD_HASH");
const GIT_COMMIT: &str = env!("GIT_COMMIT");
const BUILD_TIMESTAMP: &str = env!("BUILD_TIMESTAMP");

/// GET /api/v1/version — return current version and build info.
pub async fn get_version() -> impl IntoResponse {
    Json(serde_json::json!({
        "version": VERSION,
        "build_hash": BUILD_HASH,
        "git_commit": GIT_COMMIT,
        "build_timestamp": BUILD_TIMESTAMP,
        "platform": current_platform(),
    }))
}

/// POST /api/v1/version/check — check for updates against the update server.
pub async fn check_update(
    AdminOnly(_claims): AdminOnly,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let update_url = &state.config.update.url;

    match check_for_update(update_url, BUILD_HASH).await {
        Ok(Some(update)) => {
            Json(serde_json::json!({
                "update_available": true,
                "current_version": VERSION,
                "current_build_hash": BUILD_HASH,
                "latest_version": update.manifest.version,
                "latest_build_hash": update.manifest.build_hash,
                "latest_git_commit": update.manifest.git_commit,
                "released_at": update.manifest.released_at,
                "download_size": update.platform.size,
            }))
            .into_response()
        }
        Ok(None) => {
            Json(serde_json::json!({
                "update_available": false,
                "current_version": VERSION,
                "current_build_hash": BUILD_HASH,
            }))
            .into_response()
        }
        Err(e) => {
            warn!(error = %e, "Update check failed");
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({
                    "error": format!("Update check failed: {}", e),
                    "current_version": VERSION,
                    "current_build_hash": BUILD_HASH,
                })),
            )
                .into_response()
        }
    }
}

/// POST /api/v1/version/update — download and apply the latest update.
pub async fn apply_latest_update(
    AdminOnly(claims): AdminOnly,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let update_url = &state.config.update.url;

    // First check if there's actually an update
    let update = match check_for_update(update_url, BUILD_HASH).await {
        Ok(Some(u)) => u,
        Ok(None) => {
            return Json(serde_json::json!({
                "status": "up_to_date",
                "message": "Already running the latest version.",
            }))
            .into_response();
        }
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({ "error": format!("Update check failed: {}", e) })),
            )
                .into_response();
        }
    };

    info!(
        admin = %claims.sub,
        version = %update.manifest.version,
        build = %update.manifest.build_hash,
        "Admin triggered update via dashboard"
    );

    // Audit log
    let _ = state.storage.save_audit_entry(&crate::core::AuditEntry {
        id: 0,
        admin_user_id: Some(claims.user_id),
        action: "update_applied".to_string(),
        detail: format!(
            "Updated from {} to {} ({})",
            BUILD_HASH, update.manifest.build_hash, update.manifest.version
        ),
        ip_address: None,
        created_at: chrono::Utc::now(),
    }).await;

    // Download and verify
    let temp_path = match download_and_verify(&update.platform.url, &update.platform.sha256).await {
        Ok(p) => p,
        Err(e) => {
            error!(error = %e, "Failed to download update");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": format!("Download failed: {}", e) })),
            )
                .into_response();
        }
    };

    // Apply
    match apply_update(&temp_path) {
        Ok(_) => {
            info!("Update applied successfully, restart required");
            Json(serde_json::json!({
                "status": "applied",
                "message": format!("Updated to {} ({}). Restart the bot to activate.", update.manifest.version, update.manifest.build_hash),
                "new_version": update.manifest.version,
                "new_build_hash": update.manifest.build_hash,
            }))
            .into_response()
        }
        Err(e) => {
            error!(error = %e, "Failed to apply update");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": format!("Failed to apply update: {}", e) })),
            )
                .into_response()
        }
    }
}

fn current_platform() -> &'static str {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    { "linux-x86_64" }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    { "linux-aarch64" }
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    { "windows-x86_64" }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    { "macos-x86_64" }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    { "macos-aarch64" }
    #[cfg(not(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "windows", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "aarch64"),
    )))]
    { "unknown" }
}
