//! Client-side request handlers for master-initiated operations.
//!
//! These run on the client bot when the master sends a Request message
//! via the sync WebSocket. Each handler performs a local operation
//! (filesystem scan, config parsing, game server installation) and
//! returns a ClientResponse.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;
use tracing::{error, info, warn};

use super::protocol::*;
use crate::core::context::BotContext;
use crate::core::MapConfig;
use crate::storage::Storage;

// ---------------------------------------------------------------------------
// Install state (shared across poll requests)
// ---------------------------------------------------------------------------

/// Tracks the progress of a game server installation.
#[derive(Debug, Clone)]
pub struct InstallState {
    pub stage: String,
    pub percent: u8,
    pub error: Option<String>,
    pub completed: bool,
    pub install_path: Option<String>,
    pub game_log: Option<String>,
}

impl Default for InstallState {
    fn default() -> Self {
        Self {
            stage: "idle".to_string(),
            percent: 0,
            error: None,
            completed: false,
            install_path: None,
            game_log: None,
        }
    }
}

/// Shared install state that persists across requests.
pub type SharedInstallState = Arc<RwLock<InstallState>>;

pub fn new_install_state() -> SharedInstallState {
    Arc::new(RwLock::new(InstallState::default()))
}

// ---------------------------------------------------------------------------
// Config file scanning
// ---------------------------------------------------------------------------

/// Browse a directory on the client filesystem, restricted to the home dir.
pub async fn handle_browse_files(path: &str) -> ClientResponse {
    let home = match home_dir() {
        Some(h) => h,
        None => {
            return ClientResponse::Error {
                message: "Cannot determine home directory".to_string(),
            };
        }
    };

    let browse_path = if path.is_empty() || path == "~" {
        home.clone()
    } else {
        PathBuf::from(path)
    };

    // Security: must be under home directory or common game directories
    let allowed = browse_path.starts_with(&home)
        || browse_path.starts_with("/opt/urbanterror")
        || browse_path.starts_with("/usr/local/games/urbanterror");

    if !allowed {
        return ClientResponse::Error {
            message: "Access denied: path must be under home directory or known game directories".to_string(),
        };
    }

    // Canonicalize to prevent traversal
    let canonical = match browse_path.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            return ClientResponse::Error {
                message: format!("Cannot access path: {}", e),
            };
        }
    };

    if !canonical.starts_with(&home)
        && !canonical.starts_with("/opt/urbanterror")
        && !canonical.starts_with("/usr/local/games/urbanterror")
    {
        return ClientResponse::Error {
            message: "Access denied: resolved path is outside allowed directories".to_string(),
        };
    }

    let entries_iter = match std::fs::read_dir(&canonical) {
        Ok(e) => e,
        Err(e) => {
            return ClientResponse::Error {
                message: format!("Cannot read directory: {}", e),
            };
        }
    };

    let mut entries: Vec<super::protocol::DirEntry> = Vec::new();
    for entry in entries_iter.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        // Skip hidden files
        if name.starts_with('.') {
            continue;
        }
        let meta = entry.metadata().ok();
        let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
        let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
        // Only show directories and .cfg files
        if is_dir || name.ends_with(".cfg") {
            entries.push(super::protocol::DirEntry { name, is_dir, size });
        }
    }

    // Sort: directories first, then files, alphabetically
    entries.sort_by(|a, b| {
        b.is_dir.cmp(&a.is_dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    info!(path = %canonical.display(), count = entries.len(), "Browsed directory");

    ClientResponse::DirectoryListing {
        path: canonical.to_string_lossy().to_string(),
        entries,
    }
}

/// Scan known game directories for .cfg files.
pub async fn handle_scan_config_files() -> ClientResponse {
    let home = match home_dir() {
        Some(h) => h,
        None => {
            return ClientResponse::Error {
                message: "Cannot determine home directory".to_string(),
            };
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

    let mut files = Vec::new();

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

    // Sort by path for consistent ordering
    files.sort_by(|a, b| a.path.cmp(&b.path));

    info!(count = files.len(), dirs = all_dirs.len(), "Scanned for config files");

    ClientResponse::ConfigFiles { files }
}

// ---------------------------------------------------------------------------
// Config file parsing
// ---------------------------------------------------------------------------

/// Read and parse a specific server.cfg file, extracting game server settings.
pub async fn handle_parse_config_file(path: &str) -> ClientResponse {
    let file_path = Path::new(path);

    // Security: only allow .cfg files
    match file_path.extension().and_then(|e| e.to_str()) {
        Some("cfg") => {}
        _ => {
            return ClientResponse::Error {
                message: "Only .cfg files can be read".to_string(),
            };
        }
    }

    // Security: must be under a known game directory
    if !is_allowed_config_path(file_path) {
        return ClientResponse::Error {
            message: "Path is not under a recognized game server directory".to_string(),
        };
    }

    let content = match std::fs::read_to_string(file_path) {
        Ok(c) => c,
        Err(e) => {
            return ClientResponse::Error {
                message: format!("Cannot read file: {}", e),
            };
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

    // Run health checks (same logic as analyze_server_cfg in web API)
    let checks = run_config_checks(&setting_map);

    // Extract ServerConfigPayload values
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

    // Try to resolve game_log to an absolute path relative to the cfg file's directory
    let game_log = game_log.map(|log| {
        let log_path = Path::new(&log);
        if log_path.is_absolute() {
            log
        } else if let Some(parent) = file_path.parent() {
            let resolved = parent.join(&log);
            resolved.to_string_lossy().to_string()
        } else {
            log
        }
    });

    let settings = ServerConfigPayload {
        address: String::new(), // Not available in server.cfg — must be provided separately
        port,
        rcon_password,
        game_log,
        rcon_ip: None,
        rcon_port: None,
        delay: None,
        bot: None,
        plugins: None,
    };

    info!(path, settings_count = all_settings.len(), "Parsed config file");

    ClientResponse::ParsedConfig {
        settings,
        checks,
        all_settings,
        raw: content,
    }
}

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
                message: format!("g_logsync is \"{}\" but must be \"1\" for real-time log reading.", v),
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
                message: format!("g_logroll is \"{}\". Recommend \"0\" to prevent log rotation issues.", v),
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
                message: format!("sv_strictAuth is \"{}\". Recommend \"1\" for player auth tracking.", v),
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
    let rcon = settings.get("rconPassword").or_else(|| settings.get("sv_rconPassword"));
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
                message: "No RCON password set. The bot requires RCON to manage the server.".into(),
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

/// Check if a path is under a recognized game server directory.
fn is_allowed_config_path(path: &Path) -> bool {
    let path_str = path.to_string_lossy();

    // Must contain q3ut4 or urbanterror somewhere in the path
    if path_str.contains("q3ut4")
        || path_str.contains("urbanterror")
        || path_str.contains("q3a")
    {
        return true;
    }

    // Also allow if under home directory (common custom install locations)
    if let Some(home) = home_dir() {
        if path.starts_with(&home) {
            return true;
        }
    }

    false
}

// ---------------------------------------------------------------------------
// Game server installation
// ---------------------------------------------------------------------------

const URT_DOWNLOAD_URL: &str =
    "https://www.urbanterror.info/downloads/software/urt/43/UrbanTerror43_ded.tar.gz";

/// Start downloading and installing a UrT 4.3 dedicated server.
/// This spawns a background task and updates the shared install state.
pub fn start_install_game_server(install_path: String, state: SharedInstallState) {
    tokio::spawn(async move {
        run_install(install_path, state).await;
    });
}

async fn run_install(install_path: String, state: SharedInstallState) {
    let target = Path::new(&install_path);

    // Update: downloading
    {
        let mut s = state.write().await;
        s.stage = "downloading".to_string();
        s.percent = 5;
        s.error = None;
        s.completed = false;
    }

    // Create target directory
    if let Err(e) = tokio::fs::create_dir_all(target).await {
        let mut s = state.write().await;
        s.stage = "error".to_string();
        s.error = Some(format!("Failed to create directory: {}", e));
        return;
    }

    let tmp_path = format!("/tmp/urt43_ded_{}.tar.gz", std::process::id());

    // Download
    info!(url = URT_DOWNLOAD_URL, dest = %tmp_path, "Downloading UrT 4.3 dedicated server");
    {
        let mut s = state.write().await;
        s.stage = "downloading".to_string();
        s.percent = 10;
    }

    let download_result = tokio::process::Command::new("sh")
        .arg("-c")
        .arg(format!(
            "wget -q -O '{}' '{}' 2>&1 || curl -fsSL -o '{}' '{}' 2>&1",
            tmp_path, URT_DOWNLOAD_URL, tmp_path, URT_DOWNLOAD_URL
        ))
        .output()
        .await;

    match download_result {
        Ok(output) if output.status.success() => {
            let mut s = state.write().await;
            s.stage = "extracting".to_string();
            s.percent = 60;
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let mut s = state.write().await;
            s.stage = "error".to_string();
            s.error = Some(format!("Download failed: {}", stderr));
            return;
        }
        Err(e) => {
            let mut s = state.write().await;
            s.stage = "error".to_string();
            s.error = Some(format!("Failed to run download command: {}", e));
            return;
        }
    }

    // Extract
    info!(dest = %install_path, "Extracting UrT 4.3 dedicated server");
    let extract_result = tokio::process::Command::new("sh")
        .arg("-c")
        .arg(format!(
            "tar xzf '{}' -C '{}' --strip-components=1 2>&1 || tar xzf '{}' -C '{}' 2>&1",
            tmp_path, install_path, tmp_path, install_path
        ))
        .output()
        .await;

    // Clean up temp file regardless of result
    let _ = tokio::fs::remove_file(&tmp_path).await;

    match extract_result {
        Ok(output) if output.status.success() => {
            let mut s = state.write().await;
            s.stage = "configuring".to_string();
            s.percent = 90;
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let mut s = state.write().await;
            s.stage = "error".to_string();
            s.error = Some(format!("Extraction failed: {}", stderr));
            return;
        }
        Err(e) => {
            let mut s = state.write().await;
            s.stage = "error".to_string();
            s.error = Some(format!("Failed to run extract command: {}", e));
            return;
        }
    }

    // Auto-detect game log path
    let game_log = {
        let candidate = Path::new(&install_path).join("q3ut4/games.log");
        // Create empty games.log if it doesn't exist (so the path is ready)
        let log_dir = candidate.parent().unwrap();
        let _ = std::fs::create_dir_all(log_dir);
        if !candidate.exists() {
            let _ = std::fs::File::create(&candidate);
        }
        Some(candidate.to_string_lossy().to_string())
    };

    // Done
    {
        let mut s = state.write().await;
        s.stage = "complete".to_string();
        s.percent = 100;
        s.completed = true;
        s.install_path = Some(install_path.clone());
        s.game_log = game_log;
    }

    info!(path = %install_path, "UrT 4.3 dedicated server installation complete");
}

/// Handle an install status poll.
pub async fn handle_install_status(state: &SharedInstallState) -> ClientResponse {
    let s = state.read().await;
    if s.completed {
        ClientResponse::InstallComplete {
            install_path: s.install_path.clone().unwrap_or_default(),
            game_log: s.game_log.clone(),
        }
    } else if s.error.is_some() {
        ClientResponse::InstallProgress {
            stage: s.stage.clone(),
            percent: s.percent,
            error: s.error.clone(),
        }
    } else {
        ClientResponse::InstallProgress {
            stage: s.stage.clone(),
            percent: s.percent,
            error: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Version / force-update handlers
// ---------------------------------------------------------------------------

/// Return the client's current build / version information.
pub async fn handle_get_version() -> ClientResponse {
    ClientResponse::Version {
        version: env!("CARGO_PKG_VERSION").to_string(),
        build_hash: env!("BUILD_HASH").to_string(),
        git_commit: env!("GIT_COMMIT").to_string(),
        build_timestamp: env!("BUILD_TIMESTAMP").to_string(),
        platform: update_platform().to_string(),
    }
}

/// Handle a force-update request from the master.
///
/// Checks the configured update manifest. If a newer build is available,
/// spawns a background task to download, verify, apply, and restart —
/// and returns `UpdateTriggered` immediately so the response reaches
/// the master before this process restarts. If already up to date,
/// returns `AlreadyUpToDate`.
pub async fn handle_force_update(update_url: String, channel: String) -> ClientResponse {
    let current_build = env!("BUILD_HASH").to_string();

    let update = match crate::update::check_for_update(&update_url, &channel, &current_build).await {
        Ok(Some(u)) => u,
        Ok(None) => {
            info!(build = %current_build, "Force-update: already up to date");
            return ClientResponse::AlreadyUpToDate { current_build };
        }
        Err(e) => {
            error!(error = %e, "Force-update: manifest check failed");
            return ClientResponse::Error {
                message: format!("Update check failed: {}", e),
            };
        }
    };

    let target_build = update.manifest.build_hash.clone();
    let target_version = update.manifest.version.clone();
    let download_size = update.platform.size;

    info!(
        current = %current_build,
        target = %target_build,
        "Force-update triggered by master — spawning background apply/restart"
    );

    // Spawn the download + apply + restart asynchronously so we can return
    // the UpdateTriggered response to master before restart() replaces this process.
    let binary_url = update.platform.url.clone();
    let binary_sha = update.platform.sha256.clone();
    tokio::spawn(async move {
        // Give the caller a moment to receive the UpdateTriggered response
        tokio::time::sleep(Duration::from_secs(2)).await;

        match crate::update::download_and_verify(&binary_url, &binary_sha).await {
            Ok(temp_path) => match crate::update::apply_update(&temp_path) {
                Ok(_) => {
                    info!("Force-update applied, restarting...");
                    crate::update::restart();
                }
                Err(e) => {
                    error!(error = %e, "Force-update: apply_update failed");
                    let _ = std::fs::remove_file(&temp_path);
                }
            },
            Err(e) => {
                error!(error = %e, "Force-update: download failed");
            }
        }
    });

    ClientResponse::UpdateTriggered {
        current_build,
        target_build,
        target_version,
        download_size,
    }
}

/// Handle a restart request from the master. Spawns a background task that
/// waits briefly (so the Restarting response can be delivered) and then
/// re-execs the current binary.
pub async fn handle_restart() -> ClientResponse {
    let current_build = env!("BUILD_HASH").to_string();
    info!(build = %current_build, "Restart requested by master");
    tokio::spawn(async {
        // Give the caller a moment to receive the Restarting response
        tokio::time::sleep(Duration::from_secs(2)).await;
        info!("Restarting client process now");
        crate::update::restart();
    });
    ClientResponse::Restarting { current_build }
}

/// Check whether a configured game-log path is valid and readable on the
/// client's filesystem. Returns rich diagnostic fields so the dashboard can
/// show a precise error (missing, not a file, permission denied, stale).
pub async fn handle_check_game_log(path: &str) -> ClientResponse {
    use std::fs;
    use std::time::SystemTime;

    let trimmed = path.trim();
    if trimmed.is_empty() {
        return ClientResponse::GameLogCheck {
            path: path.to_string(),
            resolved_path: None,
            ok: false,
            exists: false,
            is_file: false,
            readable: false,
            size: None,
            modified_secs_ago: None,
            message: "Game log path is empty".to_string(),
        };
    }

    let p = PathBuf::from(trimmed);

    // metadata() follows symlinks — good enough for log files.
    let meta = match fs::metadata(&p) {
        Ok(m) => m,
        Err(e) => {
            let exists = p.exists();
            let message = if e.kind() == std::io::ErrorKind::NotFound || !exists {
                format!("File does not exist: {}", trimmed)
            } else if e.kind() == std::io::ErrorKind::PermissionDenied {
                format!("Permission denied reading {}: {}", trimmed, e)
            } else {
                format!("Cannot stat {}: {}", trimmed, e)
            };
            return ClientResponse::GameLogCheck {
                path: path.to_string(),
                resolved_path: None,
                ok: false,
                exists,
                is_file: false,
                readable: false,
                size: None,
                modified_secs_ago: None,
                message,
            };
        }
    };

    let is_file = meta.is_file();
    let size = Some(meta.len());
    let modified_secs_ago = meta
        .modified()
        .ok()
        .and_then(|m| SystemTime::now().duration_since(m).ok())
        .map(|d| d.as_secs());

    let resolved_path = p
        .canonicalize()
        .ok()
        .map(|c| c.to_string_lossy().to_string());

    if !is_file {
        return ClientResponse::GameLogCheck {
            path: path.to_string(),
            resolved_path,
            ok: false,
            exists: true,
            is_file: false,
            readable: false,
            size,
            modified_secs_ago,
            message: format!("Path exists but is not a regular file: {}", trimmed),
        };
    }

    // Actually open the file to verify we can read it (covers ACLs that
    // metadata alone won't reveal).
    let readable = fs::File::open(&p).is_ok();
    if !readable {
        return ClientResponse::GameLogCheck {
            path: path.to_string(),
            resolved_path,
            ok: false,
            exists: true,
            is_file: true,
            readable: false,
            size,
            modified_secs_ago,
            message: format!("File exists but is not readable (check permissions): {}", trimmed),
        };
    }

    let freshness = match modified_secs_ago {
        Some(s) if s < 60 => "updated in the last minute".to_string(),
        Some(s) if s < 3600 => format!("last updated {} minute(s) ago", s / 60),
        Some(s) if s < 86400 => format!("last updated {} hour(s) ago", s / 3600),
        Some(s) => format!("last updated {} day(s) ago — is the game server running?", s / 86400),
        None => "modification time unknown".to_string(),
    };

    let size_human = size.map(|b| {
        if b < 1024 { format!("{} B", b) }
        else if b < 1024 * 1024 { format!("{:.1} KB", b as f64 / 1024.0) }
        else { format!("{:.1} MB", b as f64 / (1024.0 * 1024.0)) }
    }).unwrap_or_else(|| "unknown".to_string());

    ClientResponse::GameLogCheck {
        path: path.to_string(),
        resolved_path,
        ok: true,
        exists: true,
        is_file: true,
        readable: true,
        size,
        modified_secs_ago,
        message: format!("OK — {}, {}", size_human, freshness),
    }
}

/// Detect the current platform key (mirrors crate::update::current_platform).
fn update_platform() -> &'static str {
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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn home_dir() -> Option<PathBuf> {
    std::env::var("HOME")
        .ok()
        .map(PathBuf::from)
        .or_else(|| std::env::var("USERPROFILE").ok().map(PathBuf::from))
}

// ---------------------------------------------------------------------------
// Live server-control handlers (per-server parity with standalone UI)
//
// These all require a live BotContext — when the client hasn't finished
// initialising, the handlers return `ClientResponse::Error`. The master
// treats that as "server configuring" and typically reports it to the UI.
// ---------------------------------------------------------------------------

fn unavailable(what: &str) -> ClientResponse {
    ClientResponse::Error {
        message: format!("{} is unavailable — bot not fully initialised", what),
    }
}

/// Convert the in-memory `Clients` list into transport `LivePlayer`s.
async fn snapshot_players(ctx: &BotContext) -> Vec<LivePlayer> {
    let groups = ctx.storage.get_groups().await.unwrap_or_default();
    let connected = ctx.clients.get_all().await;
    connected
        .into_iter()
        .map(|c| {
            let level = c.max_level();
            let group_name = groups
                .iter()
                .filter(|g| g.level <= level)
                .max_by_key(|g| g.level)
                .map(|g| g.name.clone());
            let _ = group_name; // currently unused in payload — keep for future
            LivePlayer {
                cid: c.cid.clone().unwrap_or_default(),
                name: c.current_name.clone().unwrap_or_else(|| c.name.clone()),
                guid: Some(c.guid.clone()),
                ip: c.ip.map(|ip| ip.to_string()),
                team: Some(format!("{:?}", c.team)),
                score: c.score,
                ping: c.ping as i32,
                db_id: Some(c.id),
                level: Some(level),
            }
        })
        .collect()
}

/// GetPlayers — return the current in-memory scoreboard.
pub async fn handle_get_players(ctx: Option<&BotContext>) -> ClientResponse {
    let Some(ctx) = ctx else { return unavailable("GetPlayers"); };
    let players = snapshot_players(ctx).await;
    ClientResponse::Players { players }
}

/// GetLiveStatus — map, game type, hostname, scoreboard.
pub async fn handle_get_live_status(ctx: Option<&BotContext>) -> ClientResponse {
    let Some(ctx) = ctx else { return unavailable("GetLiveStatus"); };
    let game = ctx.game.read().await;
    let map = game.map_name.clone().filter(|s| !s.is_empty());
    let game_type = game.game_type.clone().filter(|s| !s.is_empty());
    let hostname = game.hostname.clone().filter(|s| !s.is_empty());
    let max_clients = game.max_clients.unwrap_or(0);
    drop(game);

    let players = snapshot_players(ctx).await;
    let player_count = players.len() as u32;

    ClientResponse::LiveStatus {
        map,
        game_type,
        hostname,
        player_count,
        max_clients,
        players,
        extra: serde_json::Value::Null,
    }
}

/// ListMaps — `fdir *.bsp` on the game server.
pub async fn handle_list_maps(ctx: Option<&BotContext>) -> ClientResponse {
    let Some(ctx) = ctx else { return unavailable("ListMaps"); };
    let raw = match ctx.rcon.send("fdir *.bsp").await {
        Ok(r) => r,
        Err(e) => {
            return ClientResponse::Error {
                message: format!("RCON fdir failed: {}", e),
            };
        }
    };
    let maps: Vec<String> = raw
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.ends_with(".bsp") {
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
    ClientResponse::MapList { maps }
}

/// ChangeMap — validate the map name then issue `map <name>`.
pub async fn handle_change_map(ctx: Option<&BotContext>, map: &str) -> ClientResponse {
    let Some(ctx) = ctx else { return unavailable("ChangeMap"); };
    if !map.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return ClientResponse::Error {
            message: format!("Invalid map name: {}", map),
        };
    }
    match ctx.rcon.send(&format!("map {}", map)).await {
        Ok(_) => ClientResponse::Ok {
            message: format!("Map changed to {}", map),
            data: None,
        },
        Err(e) => ClientResponse::Error {
            message: format!("RCON map change failed: {}", e),
        },
    }
}

/// MutePlayer — issue `mute <cid>` via RCON.
pub async fn handle_mute_player(ctx: Option<&BotContext>, cid: &str) -> ClientResponse {
    let Some(ctx) = ctx else { return unavailable("MutePlayer"); };
    if !cid.chars().all(|c| c.is_ascii_digit()) {
        return ClientResponse::Error {
            message: "Invalid client id".into(),
        };
    }
    match ctx.rcon.send(&format!("mute {}", cid)).await {
        Ok(_) => ClientResponse::Ok {
            message: format!("Muted {}", cid),
            data: None,
        },
        Err(e) => ClientResponse::Error {
            message: format!("RCON mute failed: {}", e),
        },
    }
}

/// UnmutePlayer — UrT's `mute` is a toggle; sending it again unmutes.
pub async fn handle_unmute_player(ctx: Option<&BotContext>, cid: &str) -> ClientResponse {
    handle_mute_player(ctx, cid).await
}

/// Resolve the directory holding the game server's runtime files (where
/// mapcycle.txt and server.cfg live) from the configured game_log path.
fn game_dir_from_log(game_log: Option<&str>) -> Option<PathBuf> {
    let log = game_log?;
    let p = PathBuf::from(log);
    p.parent().map(|d| d.to_path_buf())
}

async fn resolve_mapcycle_name(ctx: &BotContext) -> String {
    match ctx.get_cvar("g_mapcycle").await {
        Ok(v) => {
            // Format: "g_mapcycle" is:"mapcycle.txt^7", the default
            let re = regex::Regex::new(r#"is:\"([^"]+?)(?:\^7)?\""#).ok();
            let parsed = re
                .and_then(|r| r.captures(&v).map(|c| c[1].to_string()))
                .filter(|s| !s.is_empty());
            parsed.unwrap_or_else(|| "mapcycle.txt".to_string())
        }
        Err(_) => "mapcycle.txt".to_string(),
    }
}

/// GetMapcycle — read the mapcycle file from the game server's directory.
pub async fn handle_get_mapcycle(
    ctx: Option<&BotContext>,
    game_log: Option<&str>,
) -> ClientResponse {
    let Some(ctx) = ctx else { return unavailable("GetMapcycle"); };
    let cycle_name = resolve_mapcycle_name(ctx).await;
    let Some(dir) = game_dir_from_log(game_log) else {
        return ClientResponse::Error {
            message: "Cannot determine server directory (game_log not set)".to_string(),
        };
    };
    let cycle_path = dir.join(&cycle_name);
    let content = match tokio::fs::read_to_string(&cycle_path).await {
        Ok(c) => c,
        Err(e) => {
            return ClientResponse::Error {
                message: format!("Cannot read mapcycle {}: {}", cycle_path.display(), e),
            };
        }
    };
    let mut maps = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with("//")
            || trimmed.starts_with('#')
            || trimmed.starts_with('{')
            || trimmed.starts_with('}')
        {
            continue;
        }
        if let Some(name) = trimmed.split_whitespace().next() {
            maps.push(name.to_string());
        }
    }
    ClientResponse::Mapcycle {
        path: Some(cycle_path.to_string_lossy().to_string()),
        maps,
    }
}

/// SetMapcycle — overwrite the mapcycle file.
pub async fn handle_set_mapcycle(
    ctx: Option<&BotContext>,
    game_log: Option<&str>,
    maps: &[String],
) -> ClientResponse {
    let Some(ctx) = ctx else { return unavailable("SetMapcycle"); };
    let valid = regex::Regex::new(r"^[a-zA-Z0-9_\-]+$").expect("static regex");
    for m in maps {
        if !valid.is_match(m) {
            return ClientResponse::Error {
                message: format!("Invalid map name: {}", m),
            };
        }
    }
    let cycle_name = resolve_mapcycle_name(ctx).await;
    let Some(dir) = game_dir_from_log(game_log) else {
        return ClientResponse::Error {
            message: "Cannot determine server directory (game_log not set)".to_string(),
        };
    };
    let cycle_path = dir.join(&cycle_name);
    let content = format!("{}\n", maps.join("\n"));
    match tokio::fs::write(&cycle_path, &content).await {
        Ok(_) => {
            info!(path = %cycle_path.display(), maps = maps.len(), "Mapcycle written by master");
            ClientResponse::Ok {
                message: format!("Wrote {} maps to {}", maps.len(), cycle_path.display()),
                data: None,
            }
        }
        Err(e) => ClientResponse::Error {
            message: format!("Cannot write mapcycle {}: {}", cycle_path.display(), e),
        },
    }
}

/// GetServerCfg — read the currently active `server.cfg` (or first `*.cfg`)
/// from the game server's directory.
pub async fn handle_get_server_cfg(game_log: Option<&str>) -> ClientResponse {
    let Some(dir) = game_dir_from_log(game_log) else {
        return ClientResponse::Error {
            message: "Cannot determine server directory (game_log not set)".to_string(),
        };
    };
    let cfg_path = dir.join("server.cfg");
    let contents = match tokio::fs::read_to_string(&cfg_path).await {
        Ok(c) => c,
        Err(e) => {
            return ClientResponse::Error {
                message: format!("Cannot read {}: {}", cfg_path.display(), e),
            };
        }
    };
    ClientResponse::ServerCfg {
        path: cfg_path.to_string_lossy().to_string(),
        contents,
    }
}

/// SaveConfigFile — write arbitrary contents to a `.cfg` file under an
/// allowed game-server directory. Path is validated via
/// `is_allowed_config_path` (reused from the existing scan feature).
pub async fn handle_save_config_file(path: &str, contents: &str) -> ClientResponse {
    let p = PathBuf::from(path);
    if !is_allowed_config_path(&p) {
        return ClientResponse::Error {
            message: format!("Refusing to write outside game-server directories: {}", path),
        };
    }
    if p.extension().and_then(|e| e.to_str()) != Some("cfg") {
        return ClientResponse::Error {
            message: "Only .cfg files can be written".to_string(),
        };
    }
    match tokio::fs::write(&p, contents.as_bytes()).await {
        Ok(_) => {
            info!(path = %p.display(), bytes = contents.len(), "Config file written by master");
            ClientResponse::Ok {
                message: format!("Wrote {} bytes to {}", contents.len(), p.display()),
                data: None,
            }
        }
        Err(e) => ClientResponse::Error {
            message: format!("Cannot write {}: {}", p.display(), e),
        },
    }
}

// ---------------------------------------------------------------------------
// Map-config DB handlers (proxy into client's local storage)
// ---------------------------------------------------------------------------

/// ListMapConfigs — return the client's local map_configs rows as JSON.
pub async fn handle_list_map_configs(storage: Option<&Arc<dyn Storage>>) -> ClientResponse {
    let Some(storage) = storage else { return unavailable("ListMapConfigs"); };
    match storage.get_map_configs().await {
        Ok(rows) => ClientResponse::MapConfigs {
            entries: serde_json::to_value(&rows).unwrap_or(serde_json::Value::Null),
        },
        Err(e) => ClientResponse::Error {
            message: format!("Storage error: {}", e),
        },
    }
}

/// SaveMapConfig — upsert a map_config row from a JSON payload.
pub async fn handle_save_map_config(
    storage: Option<&Arc<dyn Storage>>,
    config: serde_json::Value,
) -> ClientResponse {
    let Some(storage) = storage else { return unavailable("SaveMapConfig"); };
    let mc: MapConfig = match serde_json::from_value(config) {
        Ok(m) => m,
        Err(e) => {
            return ClientResponse::Error {
                message: format!("Invalid MapConfig payload: {}", e),
            };
        }
    };
    match storage.save_map_config(&mc).await {
        Ok(id) => ClientResponse::Ok {
            message: format!("Saved map_config #{} ({})", id, mc.map_name),
            data: Some(serde_json::json!({"id": id})),
        },
        Err(e) => ClientResponse::Error {
            message: format!("Storage error: {}", e),
        },
    }
}

/// DeleteMapConfig — delete a map_config row by id.
pub async fn handle_delete_map_config(
    storage: Option<&Arc<dyn Storage>>,
    id: i64,
) -> ClientResponse {
    let Some(storage) = storage else { return unavailable("DeleteMapConfig"); };
    match storage.delete_map_config(id).await {
        Ok(_) => ClientResponse::Ok {
            message: format!("Deleted map_config #{}", id),
            data: None,
        },
        Err(e) => ClientResponse::Error {
            message: format!("Storage error: {}", e),
        },
    }
}
