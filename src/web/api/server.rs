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

/// GET /api/v1/server/maps — cached list of installed maps on the local
/// game server. Backed by the `server_maps` table, populated asynchronously
/// by [`crate::mapscan`]. For a live refresh use POST `/server/maps/refresh`.
pub async fn list_maps(
    AuthUser(_claims): AuthUser,
    State(state): State<AppState>,
) -> impl IntoResponse {
    // Standalone mode uses server_id = 0 as the cache key.
    const STANDALONE_SERVER_ID: i64 = 0;

    let ctx = match state.require_ctx() {
        Ok(c) => c,
        Err(_) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": "Not available in master mode"})),
            )
                .into_response()
        }
    };

    let maps = state
        .storage
        .list_server_maps(STANDALONE_SERVER_ID)
        .await
        .unwrap_or_default();
    let status = state
        .storage
        .get_server_map_scan(STANDALONE_SERVER_ID)
        .await
        .ok()
        .flatten();
    let current_map = ctx.game.read().await.map_name.clone();

    Json(serde_json::json!({
        "maps": maps,
        "current_map": current_map,
        "last_scan_at": status.as_ref().and_then(|s| s.last_scan_at),
        "last_scan_ok": status.as_ref().map(|s| s.last_scan_ok).unwrap_or(false),
        "last_scan_error": status.as_ref().and_then(|s| s.last_scan_error.clone()),
        "map_count": status.as_ref().map(|s| s.map_count).unwrap_or(0),
    }))
    .into_response()
}

/// POST /api/v1/server/maps/refresh — force an immediate RCON scan for the
/// local game server (standalone mode only).
pub async fn refresh_maps(
    AuthUser(_claims): AuthUser,
    State(state): State<AppState>,
) -> impl IntoResponse {
    const STANDALONE_SERVER_ID: i64 = 0;
    let ctx = match state.require_ctx() {
        Ok(c) => c,
        Err(_) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": "Not available in master mode"})),
            )
                .into_response()
        }
    };
    match crate::mapscan::scan_local_server(
        state.storage.clone(),
        ctx,
        STANDALONE_SERVER_ID,
    )
    .await
    {
        Ok(count) => Json(serde_json::json!({
            "ok": true,
            "map_count": count,
        }))
        .into_response(),
        Err(msg) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({"ok": false, "error": msg})),
        )
            .into_response(),
    }
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

// ---------------------------------------------------------------------------
// Map repository import (standalone mode)
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize)]
pub struct ImportMapBody {
    pub filename: String,
}

/// POST /api/v1/server/maps/import — fetch a `.pk3` from the repo cache and
/// place it into the local game server's `q3ut4/` directory.
pub async fn import_map(
    AdminOnly(_claims): AdminOnly,
    State(state): State<AppState>,
    Json(body): Json<ImportMapBody>,
) -> impl IntoResponse {
    if !state.config.map_repo.enabled {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "map_repo is disabled"})),
        )
            .into_response();
    }
    let entry = match state.storage.get_map_repo_entry(&body.filename).await {
        Ok(Some(e)) => e,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": format!("'{}' not found in map repo cache", body.filename)
                })),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };
    let allowed_hosts = state.config.map_repo.sources.clone();
    let game_log = state.config.server.game_log.clone();
    let override_dir = state.config.map_repo.download_dir.clone();
    let ctx_ref = state.ctx.as_deref();
    let resp = crate::sync::handlers::handle_download_map_pk3(
        ctx_ref,
        game_log.as_deref(),
        override_dir.as_deref(),
        &entry.source_url,
        &entry.filename,
        &allowed_hosts,
    )
    .await;

    // On success, flag the map as pending_restart in the per-server cache
    // so the UI can show it right away — the engine won't actually load
    // the new `.pk3` until the next `fs_restart` or process restart.
    if matches!(resp, crate::sync::protocol::ClientResponse::MapDownloaded { .. }) {
        const STANDALONE_SERVER_ID: i64 = 0;
        let map_name = entry
            .filename
            .trim_end_matches(".pk3")
            .trim_end_matches(".PK3")
            .to_string();
        let _ = state
            .storage
            .mark_server_map_pending(
                STANDALONE_SERVER_ID,
                &map_name,
                Some(&entry.filename),
                chrono::Utc::now(),
            )
            .await;
    }

    Json(serde_json::to_value(&resp).unwrap_or_default()).into_response()
}

#[derive(serde::Deserialize)]
pub struct MissingMapsBody {
    pub maps: Vec<String>,
}

/// POST /api/v1/server/maps/missing — diff the given list against the
/// server's installed maps and annotate with repo availability.
pub async fn missing_maps(
    AuthUser(_claims): AuthUser,
    State(state): State<AppState>,
    Json(body): Json<MissingMapsBody>,
) -> impl IntoResponse {
    let ctx = match state.require_ctx() {
        Ok(c) => c,
        Err(_) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": "Not available in master mode"})),
            )
                .into_response();
        }
    };
    let raw = match ctx.rcon.send("fdir *.bsp").await {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };
    let installed: std::collections::HashSet<String> = raw
        .lines()
        .filter_map(|line| {
            let t = line.trim();
            if !t.ends_with(".bsp") {
                return None;
            }
            let name = t.rsplit('/').next().unwrap_or(t).trim_end_matches(".bsp");
            if name.is_empty() {
                None
            } else {
                Some(name.to_lowercase())
            }
        })
        .collect();

    let mut missing = Vec::new();
    for m in &body.maps {
        let key = m.trim().to_lowercase();
        if key.is_empty() || installed.contains(&key) {
            continue;
        }
        let repo = state
            .storage
            .get_map_repo_entry(&format!("{}.pk3", key))
            .await
            .ok()
            .flatten();
        missing.push(serde_json::json!({
            "map": m,
            "repo_filename": repo.as_ref().map(|e| e.filename.clone()),
            "repo_size": repo.as_ref().and_then(|e| e.size),
        }));
    }
    Json(serde_json::json!({ "missing": missing })).into_response()
}

// ---- Live cvar read/write (standalone) --------------------------------

fn valid_cvar_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn parse_cvar_response(raw: &str) -> String {
    let clean = raw.trim().trim_matches('\0');
    if let Some(idx) = clean.find("is:\"") {
        let after = &clean[idx + 4..];
        if let Some(end) = after.find('"') {
            let mut val = after[..end].to_string();
            if val.ends_with("^7") {
                val.truncate(val.len() - 2);
            }
            return val;
        }
    }
    clean.to_string()
}

/// GET /api/v1/server/cvar/:name — read a single cvar from the live server.
pub async fn get_cvar(
    AuthUser(_claims): AuthUser,
    State(state): State<AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> impl IntoResponse {
    if !valid_cvar_name(&name) {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "Invalid cvar name"}))).into_response();
    }
    let ctx = match state.require_ctx() {
        Ok(c) => c,
        Err(_) => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "Not available in master mode"}))).into_response(),
    };
    match ctx.write(&name).await {
        Ok(raw) => {
            let value = parse_cvar_response(&raw);
            Json(serde_json::json!({ "name": name, "value": value, "raw": raw })).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
    }
}

/// PUT /api/v1/server/cvar/:name — set a single cvar on the live server.
/// Body: `{ "value": "..." }`.
pub async fn set_cvar(
    AdminOnly(claims): AdminOnly,
    State(state): State<AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    if !valid_cvar_name(&name) {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "Invalid cvar name"}))).into_response();
    }
    let value = match body.get("value").and_then(|v| v.as_str()) {
        Some(v) => v.to_string(),
        None => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "Missing 'value' field"}))).into_response(),
    };
    if value.chars().any(|c| c == '"' || c == '\n' || c == '\r' || (c as u32) < 0x20) {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "Invalid cvar value"}))).into_response();
    }
    let ctx = match state.require_ctx() {
        Ok(c) => c,
        Err(_) => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "Not available in master mode"}))).into_response(),
    };

    let _ = state.storage.save_audit_entry(&AuditEntry {
        id: 0,
        admin_user_id: Some(claims.user_id),
        action: "set_cvar".to_string(),
        detail: format!("set {} \"{}\"", name, value),
        ip_address: None,
        created_at: chrono::Utc::now(),
        server_id: None,
    }).await;

    let cmd = format!("set {} \"{}\"", name, value);
    match ctx.write(&cmd).await {
        Ok(_) => Json(serde_json::json!({ "name": name, "value": value, "ok": true })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
    }
}


