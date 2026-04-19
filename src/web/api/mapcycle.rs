use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use regex::Regex;
use serde::Deserialize;
use tracing::info;

use crate::core::AuditEntry;
use crate::web::auth::{AdminOnly, AuthUser};
use crate::web::state::AppState;

/// Parse a cvar value from RCON response like `"g_mapcycle" is:"mapcycle.txt^7", the default`
fn parse_cvar_value(raw: &str) -> Option<String> {
    // Format: "<name>" is:"<value>"  or  "<name>" is:"<value>^7", the default
    let re = Regex::new(r#"is:\"([^"]+?)(?:\^7)?\""#).ok()?;
    re.captures(raw).map(|c| c[1].to_string())
}

fn get_mapcycle_filename(raw_cvar: &str) -> String {
    parse_cvar_value(raw_cvar)
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "mapcycle.txt".to_string())
}

/// GET /api/v1/server/mapcycle — read the mapcycle from the server.
pub async fn get_mapcycle(
    AuthUser(_claims): AuthUser,
    State(state): State<AppState>,
) -> impl IntoResponse {
    // Read g_mapcycle cvar to find the cycle file name
    let cycle_name = match state.ctx.get_cvar("g_mapcycle").await {
        Ok(v) => get_mapcycle_filename(&v),
        Err(_) => "mapcycle.txt".to_string(),
    };

    // Use RCON to read the mapcycle by dumping it
    // UrT doesn't have a direct file-read command, but we can read via cyclemap
    // Instead, try reading the file from the game server's q3ut4 directory
    // For now, we'll use a combination approach:
    // 1. Try to get the mapcycle content via the server's file system path
    // 2. Fall back to getting the current map rotation info

    // The game_log path gives us the server directory
    let game_dir = state.config.server.game_log.as_ref()
        .and_then(|p| {
            let path = std::path::Path::new(p);
            path.parent().map(|d| d.to_path_buf())
        });

    let mut maps = Vec::new();
    let mut raw_content = String::new();

    if let Some(dir) = game_dir {
        let cycle_path = dir.join(&cycle_name);
        if let Ok(content) = tokio::fs::read_to_string(&cycle_path).await {
            raw_content = content.clone();
            for line in content.lines() {
                let trimmed = line.trim();
                if !trimmed.is_empty() && !trimmed.starts_with("//") && !trimmed.starts_with('#') {
                    // Mapcycle lines can be just map names or have additional params
                    let map_name = trimmed.split_whitespace().next().unwrap_or(trimmed);
                    if !map_name.starts_with('{') && !map_name.starts_with('}') {
                        maps.push(map_name.to_string());
                    }
                }
            }
        }
    }

    Json(serde_json::json!({
        "cycle_name": cycle_name,
        "maps": maps,
        "raw": raw_content,
    })).into_response()
}

#[derive(Deserialize)]
pub struct UpdateMapcycleBody {
    pub maps: Vec<String>,
}

/// PUT /api/v1/server/mapcycle — update the mapcycle.
pub async fn update_mapcycle(
    AdminOnly(claims): AdminOnly,
    State(state): State<AppState>,
    Json(body): Json<UpdateMapcycleBody>,
) -> impl IntoResponse {
    // Validate map names
    let valid_map = Regex::new(r"^[a-zA-Z0-9_\-]+$").unwrap();
    for map in &body.maps {
        if !valid_map.is_match(map) {
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "error": format!("Invalid map name: {}", map)
            }))).into_response();
        }
    }

    // Get mapcycle file path
    let cycle_name = match state.ctx.get_cvar("g_mapcycle").await {
        Ok(v) => get_mapcycle_filename(&v),
        Err(_) => "mapcycle.txt".to_string(),
    };

    let game_dir = state.config.server.game_log.as_ref()
        .and_then(|p| {
            let path = std::path::Path::new(p);
            path.parent().map(|d| d.to_path_buf())
        });

    let Some(dir) = game_dir else {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
            "error": "Cannot determine server directory from game_log path"
        }))).into_response();
    };

    let cycle_path = dir.join(&cycle_name);
    let content = body.maps.join("\n") + "\n";

    match tokio::fs::write(&cycle_path, &content).await {
        Ok(_) => {
            let _ = state.storage.save_audit_entry(&AuditEntry {
                id: 0,
                admin_user_id: Some(claims.user_id),
                action: "mapcycle_update".to_string(),
                detail: format!("Updated mapcycle ({} maps): {}", body.maps.len(), body.maps.join(", ")),
                ip_address: None,
                created_at: chrono::Utc::now(),
            }).await;

            info!(maps = body.maps.len(), "Mapcycle updated via web UI");

            Json(serde_json::json!({
                "status": "ok",
                "maps": body.maps,
            })).into_response()
        }
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": format!("Failed to write mapcycle: {}", e)
            }))).into_response()
        }
    }
}
