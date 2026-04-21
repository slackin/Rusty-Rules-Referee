use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use regex::Regex;
use tracing::info;

use crate::core::AuditEntry;
use crate::web::auth::{AdminOnly, AuthUser};
use crate::web::state::AppState;

/// GET /api/v1/server/status — reads from in-memory game state (updated by background poller).
pub async fn server_status(
    AuthUser(_claims): AuthUser,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let ctx = match state.require_ctx() {
        Ok(c) => c,
        Err(_) => return Json(serde_json::json!({"error": "Not available in master mode"})).into_response(),
    };
    let game = ctx.game.read().await;
    let player_count = ctx.clients.count().await;

    Json(serde_json::json!({
        "game_name": game.game_name,
        "map_name": game.map_name,
        "game_type": game.game_type,
        "player_count": player_count,
        "max_clients": game.max_clients,
        "hostname": game.hostname,
        "round_time_start": game.round_time_start,
        "map_time_start": game.map_time_start,
    })).into_response()
}

/// POST /api/v1/server/rcon — execute raw RCON command (admin only).
pub async fn rcon_command(
    AdminOnly(claims): AdminOnly,
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let cmd = match body.get("command").and_then(|v| v.as_str()) {
        Some(c) => c,
        None => {
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "Missing 'command' field"}))).into_response();
        }
    };

    // Audit log
    let _ = state.storage.save_audit_entry(&AuditEntry {
        id: 0,
        admin_user_id: Some(claims.user_id),
        action: "rcon".to_string(),
        detail: format!("RCON: {}", cmd),
        ip_address: None,
        created_at: chrono::Utc::now(),
        server_id: None,
    }).await;

    match state.require_ctx() {
        Ok(ctx) => match ctx.write(cmd).await {
            Ok(response) => {
                Json(serde_json::json!({"response": response})).into_response()
            }
            Err(e) => {
                (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
            }
        },
        Err(status) => (status, Json(serde_json::json!({"error": "Not available in master mode"}))).into_response(),
    }
}

/// GET /api/v1/server/say — send a public message.
pub async fn server_say(
    AdminOnly(_claims): AdminOnly,
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let msg = body.get("message").and_then(|v| v.as_str()).unwrap_or("");
    let ctx = match state.require_ctx() {
        Ok(c) => c,
        Err(status) => return (status, Json(serde_json::json!({"error": "Not available in master mode"}))).into_response(),
    };
    match ctx.say(msg).await {
        Ok(_) => Json(serde_json::json!({"status": "ok"})).into_response(),
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// GET /api/v1/server/maps — list available maps on the server.
pub async fn list_maps(
    AuthUser(_claims): AuthUser,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let ctx = match state.require_ctx() {
        Ok(c) => c,
        Err(_) => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "Not available in master mode"}))).into_response(),
    };
    let response = match ctx.rcon.send("fdir *.bsp").await {
        Ok(r) => r,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response();
        }
    };

    // Parse fdir output: lines like "maps/ut4_abbey.bsp" — strip prefix and suffix
    let maps: Vec<String> = response
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.ends_with(".bsp") {
                // Strip leading "maps/" or any path prefix
                let name = trimmed
                    .rsplit('/')
                    .next()
                    .unwrap_or(trimmed)
                    .trim_end_matches(".bsp");
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
            None
        })
        .collect();

    let current_map = ctx.game.read().await.map_name.clone();

    Json(serde_json::json!({
        "maps": maps,
        "current_map": current_map,
    })).into_response()
}

/// POST /api/v1/server/map — change map or set next map.
pub async fn change_map(
    AdminOnly(claims): AdminOnly,
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let map_name = match body.get("map").and_then(|v| v.as_str()) {
        Some(m) => m,
        None => {
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "Missing 'map' field"}))).into_response();
        }
    };

    let action = match body.get("action").and_then(|v| v.as_str()) {
        Some(a) => a,
        None => {
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "Missing 'action' field"}))).into_response();
        }
    };

    // Validate map name to prevent RCON injection
    let valid_map = Regex::new(r"^[a-zA-Z0-9_\-]+$").unwrap();
    if !valid_map.is_match(map_name) {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "Invalid map name"}))).into_response();
    }

    let ctx = match state.require_ctx() {
        Ok(c) => c,
        Err(status) => return (status, Json(serde_json::json!({"error": "Not available in master mode"}))).into_response(),
    };

    match action {
        "change" => {
            // Audit log
            let _ = state.storage.save_audit_entry(&AuditEntry {
                id: 0,
                admin_user_id: Some(claims.user_id),
                action: "map_change".to_string(),
                detail: format!("Changed map to {}", map_name),
                ip_address: None,
                created_at: chrono::Utc::now(),
                server_id: None,
            }).await;

            let _ = ctx.say(&format!("^7Changing map to ^2{}^7...", map_name)).await;
            match ctx.rcon.send(&format!("map {}", map_name)).await {
                Ok(_) => Json(serde_json::json!({"status": "ok", "message": format!("Changing map to {}", map_name)})).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
            }
        }
        "setnext" => {
            // Audit log
            let _ = state.storage.save_audit_entry(&AuditEntry {
                id: 0,
                admin_user_id: Some(claims.user_id),
                action: "set_next_map".to_string(),
                detail: format!("Set next map to {}", map_name),
                ip_address: None,
                created_at: chrono::Utc::now(),
                server_id: None,
            }).await;

            match ctx.set_cvar("g_nextmap", map_name).await {
                Ok(_) => {
                    let _ = ctx.say(&format!("^7Next map set to ^2{}", map_name)).await;
                    Json(serde_json::json!({"status": "ok", "message": format!("Next map set to {}", map_name)})).into_response()
                }
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
            }
        }
        _ => {
            (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "Invalid action. Use 'change' or 'setnext'"}))).into_response()
        }
    }
}

/// POST /api/v1/server/restart — restart the bot process.
pub async fn restart_bot(
    AdminOnly(claims): AdminOnly,
    State(state): State<AppState>,
) -> impl IntoResponse {
    // Audit log
    let _ = state.storage.save_audit_entry(&AuditEntry {
        id: 0,
        admin_user_id: Some(claims.user_id),
        action: "bot_restart".to_string(),
        detail: "Bot restart triggered via web UI".to_string(),
        ip_address: None,
        created_at: chrono::Utc::now(),
        server_id: None,
    }).await;

    info!("Bot restart requested via web UI by user {}", claims.user_id);

    // Spawn a delayed task that re-launches the process and then exits
    tokio::spawn(async {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        let exe = match std::env::current_exe() {
            Ok(e) => e,
            Err(_) => {
                tracing::error!("Cannot determine current executable path for restart");
                std::process::exit(1);
            }
        };
        let args: Vec<String> = std::env::args().skip(1).collect();

        // Use nohup + shell to detach the new process so it survives this process exiting
        let mut cmd_str = format!("sleep 1 && {:?}", exe);
        for arg in &args {
            cmd_str.push(' ');
            cmd_str.push_str(arg);
        }

        match std::process::Command::new("bash")
            .arg("-c")
            .arg(&cmd_str)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(_) => {
                info!("Replacement process spawned, exiting current process");
                std::process::exit(0);
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to spawn replacement process");
                std::process::exit(1);
            }
        }
    });

    Json(serde_json::json!({
        "status": "ok",
        "message": "Bot is restarting..."
    }))
}
