//! First-run setup wizard API endpoints.
//!
//! These endpoints are unauthenticated and only available before the first
//! admin user has been created. Once setup is complete, they return 403.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::sync::protocol::{CfgSetting, ConfigCheck, ConfigFileEntry};
use crate::web::state::AppState;

// ---- Request / Response types ----

#[derive(Debug, Serialize)]
pub struct SetupStatusResponse {
    pub needs_setup: bool,
    pub mode: String,
    pub version: String,
    pub build_hash: String,
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
        build_hash: env!("BUILD_HASH").to_string(),
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

// ---------------------------------------------------------------------------
// Setup file browser & config scanning endpoints
// ---------------------------------------------------------------------------

/// POST /api/v1/setup/browse — browse the server filesystem for .cfg files.
///
/// Only available during first-run setup (no admin users exist).
/// Browsing is restricted to the user's home directory and its subdirectories.
pub async fn setup_browse(
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    if !is_setup_needed(&state).await {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error": "Setup has already been completed"})),
        )
            .into_response();
    }

    let home = match home_dir() {
        Some(h) => h,
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Cannot determine home directory"})),
            )
                .into_response();
        }
    };

    let path_str = match body.get("path").and_then(|v| v.as_str()) {
        Some(p) if !p.is_empty() => p.to_string(),
        _ => home.to_string_lossy().to_string(),
    };

    let path = Path::new(&path_str);

    if !path.is_absolute() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Path must be absolute"})),
        )
            .into_response();
    }

    let canonical = match path.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": format!("Invalid path: {}", e)})),
            )
                .into_response();
        }
    };

    // Restrict to home directory and subdirectories
    if !canonical.starts_with(&home) {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error": "Browsing is restricted to the home directory"})),
        )
            .into_response();
    }

    if !canonical.is_dir() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Path is not a directory"})),
        )
            .into_response();
    }

    let dir = match std::fs::read_dir(&canonical) {
        Ok(d) => d,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": format!("Cannot read directory: {}", e)})),
            )
                .into_response();
        }
    };

    let mut entries: Vec<serde_json::Value> = Vec::new();
    for entry in dir.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let is_dir = meta.is_dir();
        if is_dir || name.ends_with(".cfg") {
            entries.push(serde_json::json!({
                "name": name,
                "is_dir": is_dir,
                "size": if is_dir { 0 } else { meta.len() },
            }));
        }
    }

    entries.sort_by(|a, b| {
        let a_dir = a["is_dir"].as_bool().unwrap_or(false);
        let b_dir = b["is_dir"].as_bool().unwrap_or(false);
        match (a_dir, b_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => {
                let a_name = a["name"].as_str().unwrap_or("");
                let b_name = b["name"].as_str().unwrap_or("");
                a_name.to_lowercase().cmp(&b_name.to_lowercase())
            }
        }
    });

    // Only allow navigating up within home directory
    let parent = if canonical != home {
        canonical.parent().map(|p| p.to_string_lossy().to_string())
    } else {
        None
    };

    Json(serde_json::json!({
        "path": canonical.to_string_lossy(),
        "home": home.to_string_lossy(),
        "parent": parent,
        "entries": entries,
    }))
    .into_response()
}

/// POST /api/v1/setup/scan-configs — auto-discover .cfg files in known UrT directories.
///
/// Scans the user's home directory for common UrT game server config locations.
/// Only available during first-run setup.
pub async fn setup_scan_configs(
    State(state): State<AppState>,
) -> impl IntoResponse {
    if !is_setup_needed(&state).await {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error": "Setup has already been completed"})),
        )
            .into_response();
    }

    let home = match home_dir() {
        Some(h) => h,
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Cannot determine home directory"})),
            )
                .into_response();
        }
    };

    // Known directories where UrT server configs might live
    let search_dirs = vec![
        home.join(".q3a/q3ut4"),
        home.join("q3ut4"),
        home.join("urbanterror/q3ut4"),
        home.join("urbanterror/UrbanTerror43/q3ut4"),
        PathBuf::from("/opt/urbanterror/q3ut4"),
        PathBuf::from("/usr/local/games/urbanterror/q3ut4"),
    ];

    // Also scan home directory one level deep for */q3ut4/ patterns
    let mut extra_dirs = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&home) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                let candidate = p.join("q3ut4");
                if candidate.is_dir() && !search_dirs.contains(&candidate) {
                    extra_dirs.push(candidate);
                }
            }
        }
    }

    let all_dirs: Vec<PathBuf> = search_dirs
        .into_iter()
        .chain(extra_dirs)
        .filter(|d| d.is_dir())
        .collect();

    let mut files: Vec<ConfigFileEntry> = Vec::new();

    for dir in &all_dirs {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        if ext == "cfg" {
                            if let Ok(meta) = std::fs::metadata(&path) {
                                let modified = meta.modified().ok().map(|t| {
                                    let dt: chrono::DateTime<chrono::Utc> = t.into();
                                    dt.to_rfc3339()
                                });
                                files.push(ConfigFileEntry {
                                    path: path.to_string_lossy().to_string(),
                                    size: meta.len(),
                                    modified,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    files.sort_by(|a, b| a.path.cmp(&b.path));

    info!(count = files.len(), dirs = all_dirs.len(), "Setup: scanned for config files");

    Json(serde_json::json!({
        "files": files,
        "directories_searched": all_dirs.iter().map(|d| d.to_string_lossy().to_string()).collect::<Vec<_>>(),
    }))
    .into_response()
}

/// POST /api/v1/setup/analyze-cfg — parse a server.cfg file and extract settings.
///
/// Reads and parses a .cfg file, extracting RCON password, port, game log path,
/// and running health checks. Only available during first-run setup.
pub async fn setup_analyze_cfg(
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    if !is_setup_needed(&state).await {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error": "Setup has already been completed"})),
        )
            .into_response();
    }

    let path_str = match body.get("path").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Missing 'path' field"})),
            )
                .into_response();
        }
    };

    let file_path = Path::new(path_str);

    // Security: only allow .cfg files
    match file_path.extension().and_then(|e| e.to_str()) {
        Some("cfg") => {}
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Only .cfg files can be read"})),
            )
                .into_response();
        }
    }

    // Security: restrict to home directory
    let home = match home_dir() {
        Some(h) => h,
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Cannot determine home directory"})),
            )
                .into_response();
        }
    };

    let canonical = match file_path.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": format!("Invalid path: {}", e)})),
            )
                .into_response();
        }
    };

    if !canonical.starts_with(&home) {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error": "File must be within the home directory"})),
        )
            .into_response();
    }

    let content = match std::fs::read_to_string(&canonical) {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": format!("Cannot read file: {}", e)})),
            )
                .into_response();
        }
    };

    // Parse all "set <key> <value>" and "seta <key> <value>" lines
    let mut all_settings: Vec<CfgSetting> = Vec::new();
    let mut setting_map: HashMap<String, String> = HashMap::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }

        if trimmed.starts_with("set ") || trimmed.starts_with("seta ") {
            let rest = if trimmed.starts_with("seta ") {
                &trimmed[5..]
            } else {
                &trimmed[4..]
            };
            let rest = rest.trim();
            if let Some((key, val_raw)) = rest.split_once(char::is_whitespace) {
                let val = val_raw.trim().trim_matches('"');
                all_settings.push(CfgSetting {
                    key: key.to_string(),
                    value: val.to_string(),
                });
                setting_map.insert(key.to_string(), val.to_string());
            }
        }
    }

    // Run health checks
    let checks = run_config_checks(&setting_map);

    // Extract key settings for auto-fill
    let rcon_password = setting_map
        .get("rconPassword")
        .or_else(|| setting_map.get("sv_rconPassword"))
        .cloned()
        .unwrap_or_default();

    let port = setting_map
        .get("net_port")
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(27960);

    let game_log = setting_map.get("g_log").cloned().filter(|v| !v.is_empty());

    // Resolve game_log to absolute path relative to the cfg file's directory
    let game_log = game_log.map(|log| {
        let log_path = Path::new(&log);
        if log_path.is_absolute() {
            log
        } else if let Some(parent) = canonical.parent() {
            let resolved = parent.join(&log);
            resolved.to_string_lossy().to_string()
        } else {
            log
        }
    });

    let hostname = setting_map.get("sv_hostname").cloned();
    let gametype = setting_map.get("g_gametype").cloned();

    info!(path = path_str, settings_count = all_settings.len(), "Setup: analyzed config file");

    Json(serde_json::json!({
        "settings": {
            "rcon_password": rcon_password,
            "port": port,
            "game_log": game_log,
            "hostname": hostname,
            "gametype": gametype,
        },
        "checks": checks,
        "all_settings": all_settings,
    }))
    .into_response()
}

// ---------------------------------------------------------------------------
// Shared helpers for config health checks
// ---------------------------------------------------------------------------

/// Run health checks on parsed server.cfg settings.
fn run_config_checks(settings: &HashMap<String, String>) -> Vec<ConfigCheck> {
    let mut checks = Vec::new();

    // 1. g_log — must be set
    match settings.get("g_log") {
        Some(v) if !v.is_empty() => {
            checks.push(ConfigCheck {
                key: "g_log".into(),
                status: "ok".into(),
                message: format!("Game log enabled: \"{}\"", v),
                fix_key: None,
                fix_value: None,
            });
        }
        _ => {
            checks.push(ConfigCheck {
                key: "g_log".into(),
                status: "error".into(),
                message: "g_log is not set. The bot requires game logging to be enabled.".into(),
                fix_key: Some("g_log".into()),
                fix_value: Some("games.log".into()),
            });
        }
    }

    // 2. g_logsync — must be 1
    match settings.get("g_logsync").map(|s| s.as_str()) {
        Some("1") => {
            checks.push(ConfigCheck {
                key: "g_logsync".into(),
                status: "ok".into(),
                message: "Log sync is enabled (writes flushed immediately).".into(),
                fix_key: None,
                fix_value: None,
            });
        }
        Some(v) => {
            checks.push(ConfigCheck {
                key: "g_logsync".into(),
                status: "error".into(),
                message: format!(
                    "g_logsync is \"{}\" but must be \"1\" for real-time log reading.",
                    v
                ),
                fix_key: Some("g_logsync".into()),
                fix_value: Some("1".into()),
            });
        }
        None => {
            checks.push(ConfigCheck {
                key: "g_logsync".into(),
                status: "error".into(),
                message: "g_logsync is not set. Must be \"1\" for real-time log reading.".into(),
                fix_key: Some("g_logsync".into()),
                fix_value: Some("1".into()),
            });
        }
    }

    // 3. g_logroll — should be 0
    match settings.get("g_logroll").map(|s| s.as_str()) {
        Some("0") | None => {
            checks.push(ConfigCheck {
                key: "g_logroll".into(),
                status: "ok".into(),
                message: "Log roll is disabled (recommended).".into(),
                fix_key: None,
                fix_value: None,
            });
        }
        Some(v) => {
            checks.push(ConfigCheck {
                key: "g_logroll".into(),
                status: "warning".into(),
                message: format!(
                    "g_logroll is \"{}\". Recommend \"0\" to prevent log rotation issues.",
                    v
                ),
                fix_key: Some("g_logroll".into()),
                fix_value: Some("0".into()),
            });
        }
    }

    // 4. sv_strictAuth — recommended
    match settings.get("sv_strictAuth").map(|s| s.as_str()) {
        Some("1") => {
            checks.push(ConfigCheck {
                key: "sv_strictAuth".into(),
                status: "ok".into(),
                message: "Strict auth is enabled. Player auth names will be tracked.".into(),
                fix_key: None,
                fix_value: None,
            });
        }
        Some(v) => {
            checks.push(ConfigCheck {
                key: "sv_strictAuth".into(),
                status: "warning".into(),
                message: format!(
                    "sv_strictAuth is \"{}\". Recommend \"1\" for player auth tracking.",
                    v
                ),
                fix_key: Some("sv_strictAuth".into()),
                fix_value: Some("1".into()),
            });
        }
        None => {
            checks.push(ConfigCheck {
                key: "sv_strictAuth".into(),
                status: "warning".into(),
                message: "sv_strictAuth not set. Recommend \"1\" for player auth tracking.".into(),
                fix_key: Some("sv_strictAuth".into()),
                fix_value: Some("1".into()),
            });
        }
    }

    // 5. rconPassword — must be set
    let rcon = settings
        .get("rconPassword")
        .or_else(|| settings.get("sv_rconPassword"));
    match rcon {
        Some(v) if !v.is_empty() => {
            checks.push(ConfigCheck {
                key: "rconPassword".into(),
                status: "ok".into(),
                message: "RCON password is set.".into(),
                fix_key: None,
                fix_value: None,
            });
        }
        _ => {
            checks.push(ConfigCheck {
                key: "rconPassword".into(),
                status: "error".into(),
                message: "No RCON password set. The bot requires RCON to manage the server."
                    .into(),
                fix_key: None,
                fix_value: None,
            });
        }
    }

    // 6. g_gametype — informational
    if let Some(gt) = settings.get("g_gametype") {
        let gt_name = match gt.as_str() {
            "0" => "Free For All",
            "1" => "Last Man Standing",
            "3" => "Team Death Match",
            "4" => "Team Survivor",
            "5" => "Follow the Leader",
            "6" => "Capture and Hold",
            "7" => "Capture the Flag",
            "8" => "Bomb Mode",
            "9" => "Jump Mode",
            "10" => "Freeze Tag",
            "11" => "Gun Game",
            _ => "Unknown",
        };
        checks.push(ConfigCheck {
            key: "g_gametype".into(),
            status: "info".into(),
            message: format!("Game type: {} ({})", gt_name, gt),
            fix_key: None,
            fix_value: None,
        });
    }

    checks
}

/// Get the user's home directory.
fn home_dir() -> Option<PathBuf> {
    std::env::var("HOME")
        .ok()
        .map(PathBuf::from)
        .or_else(|| std::env::var("USERPROFILE").ok().map(PathBuf::from))
}
