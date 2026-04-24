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
    /// Wizard-derived fields populated on completion so the master can
    /// auto-persist a full `ServerConfigPayload` without a second round trip.
    pub port: Option<u16>,
    pub rcon_password: Option<String>,
    pub server_cfg_path: Option<String>,
    pub public_ip: Option<String>,
    pub service_name: Option<String>,
    pub slug: Option<String>,
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
            port: None,
            rcon_password: None,
            server_cfg_path: None,
            public_ip: None,
            service_name: None,
            slug: None,
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
        server_cfg_path: Some(path.to_string()),
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

/// Archive format for a download mirror.
#[derive(Debug, Clone, Copy)]
enum ArchiveKind {
    /// gzipped tar: `Quake3-UrT-Ded.*` + `q3ut4/` at top level.
    TarGz,
    /// zip: full UrT package; we extract and keep only `q3ut4/` + binary.
    Zip,
}

/// A single download source.
struct UrtMirror {
    url: &'static str,
    kind: ArchiveKind,
    /// Minimum expected size in bytes — anything smaller is almost certainly
    /// an HTML error page or a partial response.
    min_bytes: u64,
}

/// Mirror list, tried in order. pugbot.net is primary because it's the only
/// mirror that reliably serves the real bytes (the official urbanterror.info
/// URL returns CMS HTML for dedicated tarballs).
/// Mirror list, tried in order. pugbot.net is primary (known-good, serves
/// the real zip bytes); mirror2.urbanterror.info is a working secondary
/// hosted by the UrT project.
const URT_MIRRORS: &[UrtMirror] = &[
    UrtMirror {
        url: "https://maps.pugbot.net/q3ut4/UrbanTerror434_full.zip",
        kind: ArchiveKind::Zip,
        min_bytes: 500 * 1024 * 1024,
    },
    UrtMirror {
        url: "https://mirror2.urbanterror.info/UrbanTerror434_full.zip",
        kind: ArchiveKind::Zip,
        min_bytes: 500 * 1024 * 1024,
    },
];

/// Download UrT 4.3 to `install_path`, trying each mirror in order. Each
/// candidate is validated (HTTP 2xx, minimum size, magic bytes, archive
/// listable) before extraction. Returns a human-readable error only if
/// every mirror fails — the error lists every mirror's failure reason.
pub async fn download_and_extract_urt(install_path: &str) -> Result<(), String> {
    download_and_extract_urt_cached(install_path, None).await
}

/// Same as [`download_and_extract_urt`] but with an optional persistent
/// cache directory for the downloaded archive. When `cache_dir` is
/// provided, the archive is stored there (named after the mirror's
/// URL basename) and reused on subsequent calls if it still validates
/// (size + magic + archive integrity). When `None`, the archive is
/// written to `/tmp` and deleted after extraction.
pub async fn download_and_extract_urt_cached(
    install_path: &str,
    cache_dir: Option<&Path>,
) -> Result<(), String> {
    let target = Path::new(install_path);
    tokio::fs::create_dir_all(target)
        .await
        .map_err(|e| format!("Failed to create {}: {}", install_path, e))?;

    if let Some(dir) = cache_dir {
        tokio::fs::create_dir_all(dir)
            .await
            .map_err(|e| format!("Failed to create cache dir {}: {}", dir.display(), e))?;
    }

    let mut per_mirror: Vec<String> = Vec::new();
    for mirror in URT_MIRRORS {
        info!(url = mirror.url, "Trying UrT mirror");
        match try_mirror(mirror, install_path, cache_dir).await {
            Ok(()) => {
                info!(url = mirror.url, "Mirror succeeded");
                return Ok(());
            }
            Err(e) => {
                warn!(url = mirror.url, error = %e, "Mirror failed, trying next");
                per_mirror.push(format!("  - {}: {}", mirror.url, e));
            }
        }
    }
    Err(format!(
        "All {} UrT 4.3 download mirrors failed:\n{}",
        URT_MIRRORS.len(),
        per_mirror.join("\n")
    ))
}

async fn try_mirror(
    mirror: &UrtMirror,
    install_path: &str,
    cache_dir: Option<&Path>,
) -> Result<(), String> {
    let ext = match mirror.kind {
        ArchiveKind::TarGz => "tar.gz",
        ArchiveKind::Zip => "zip",
    };

    // Derive a stable filename from the mirror URL basename so that
    // identical archives across mirrors share a cache slot.
    let fname = mirror
        .url
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("urt43_download")
        .to_string();

    // When caching, persist to the cache dir; otherwise use /tmp and
    // delete on exit. `keep_on_success` tracks whether we should leave
    // the archive on disk after a successful extraction.
    let (archive_path, keep_on_success): (String, bool) = match cache_dir {
        Some(dir) => (dir.join(&fname).to_string_lossy().into_owned(), true),
        None => (
            format!("/tmp/urt43_dl_{}.{}", std::process::id(), ext),
            false,
        ),
    };

    // If a cached archive already exists, try to reuse it. We validate
    // size + magic + archive integrity before skipping the download.
    let mut have_cached = false;
    if keep_on_success {
        if let Ok(meta) = tokio::fs::metadata(&archive_path).await {
            if meta.len() >= mirror.min_bytes
                && validate_archive(&archive_path, mirror).await.is_ok()
            {
                info!(
                    url = mirror.url,
                    path = %archive_path,
                    bytes = meta.len(),
                    "Reusing cached UrT archive"
                );
                have_cached = true;
            } else {
                info!(
                    url = mirror.url,
                    path = %archive_path,
                    "Cached archive invalid or too small; re-downloading"
                );
                let _ = tokio::fs::remove_file(&archive_path).await;
            }
        }
    } else {
        // Non-cache path: always start clean.
        let _ = tokio::fs::remove_file(&archive_path).await;
    }
    let tmp_path = archive_path;

    // Helper: remove the archive unless we're keeping it in the cache.
    let cleanup = |path: String, keep: bool| async move {
        if !keep {
            let _ = tokio::fs::remove_file(&path).await;
        }
    };

    if !have_cached {
        // Probe HEAD first so we can fail fast with a clear HTTP status before
        // downloading half a gigabyte.
        let probe = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(format!(
                "curl -fsSIL -A 'R3-Wizard/1.0' --max-time 20 -o /dev/null \
                 -w 'http_code=%{{http_code}} final_url=%{{url_effective}} size=%{{size_download}} \
dl_size=%{{size_header}} time=%{{time_total}}' '{url}' 2>&1",
                url = mirror.url
            ))
            .output()
            .await
            .map_err(|e| format!("probe spawn failed: {}", e))?;
        let probe_out = String::from_utf8_lossy(&probe.stdout).trim().to_string();
        if !probe.status.success() {
            let err_text = String::from_utf8_lossy(&probe.stderr);
            let combined = if err_text.trim().is_empty() {
                probe_out.clone()
            } else {
                format!("{} :: {}", probe_out, err_text.trim())
            };
            return Err(format!("HEAD probe failed ({})", combined));
        }
        info!(url = mirror.url, probe = %probe_out, "Mirror HEAD probe ok");

        // Download. Capture combined stdout+stderr via `2>&1` into a variable so
        // we can report the real reason if curl (and wget fallback) both fail.
        // `set -o pipefail` is bash-only so we stick with plain sh and check each
        // command's output.
        let dl_script = format!(
            "out=$(curl -fL --connect-timeout 30 --max-time 1800 --retry 2 --retry-delay 3 \
             -A 'R3-Wizard/1.0' -o '{tmp}' '{url}' 2>&1); rc=$?; \
             if [ $rc -ne 0 ]; then \
               echo \"curl rc=$rc: $out\"; \
               out2=$(wget --timeout=60 --tries=2 --user-agent='R3-Wizard/1.0' \
                     -O '{tmp}' '{url}' 2>&1); rc2=$?; \
               if [ $rc2 -ne 0 ]; then echo \"wget rc=$rc2: $out2\"; exit $rc2; fi; \
             fi; exit 0",
            tmp = tmp_path,
            url = mirror.url
        );
        let dl = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(&dl_script)
            .output()
            .await
            .map_err(|e| format!("spawn failed: {}", e))?;
        if !dl.status.success() {
            let combined = String::from_utf8_lossy(&dl.stdout);
            let stderr = String::from_utf8_lossy(&dl.stderr);
            let _ = tokio::fs::remove_file(&tmp_path).await;
            let msg = if combined.trim().is_empty() {
                stderr.trim().to_string()
            } else {
                combined.trim().to_string()
            };
            return Err(format!(
                "download transport failure (exit {}): {}",
                dl.status.code().unwrap_or(-1),
                if msg.is_empty() { "no output captured".to_string() } else { msg }
            ));
        }

        // Validate size.
        let meta = tokio::fs::metadata(&tmp_path)
            .await
            .map_err(|e| format!("stat {} failed: {}", tmp_path, e))?;
        if meta.len() < mirror.min_bytes {
            let _ = tokio::fs::remove_file(&tmp_path).await;
            return Err(format!(
                "file too small ({} bytes, expected >= {}) — mirror likely returned an HTML error page",
                meta.len(),
                mirror.min_bytes
            ));
        }

        // Validate magic bytes.
        let mut header = [0u8; 4];
        {
            use tokio::io::AsyncReadExt;
            let mut f = tokio::fs::File::open(&tmp_path)
                .await
                .map_err(|e| format!("open tmp failed: {}", e))?;
            f.read_exact(&mut header)
                .await
                .map_err(|e| format!("read magic failed: {}", e))?;
        }
        match mirror.kind {
            ArchiveKind::TarGz => {
                if &header[..2] != [0x1f, 0x8b] {
                    let _ = tokio::fs::remove_file(&tmp_path).await;
                    return Err(format!(
                        "not a gzip archive (magic={:02x}{:02x}) — mirror returned something else",
                        header[0], header[1]
                    ));
                }
            }
            ArchiveKind::Zip => {
                if header != [0x50, 0x4b, 0x03, 0x04] {
                    let _ = tokio::fs::remove_file(&tmp_path).await;
                    return Err(format!(
                        "not a zip archive (magic={:02x}{:02x}{:02x}{:02x})",
                        header[0], header[1], header[2], header[3]
                    ));
                }
            }
        }

        // Archive-integrity check for freshly-downloaded archives only
        // (cached archives were already validated above).
        let check_cmd = match mirror.kind {
            ArchiveKind::TarGz => format!("tar -tzf '{}' >/dev/null", tmp_path),
            ArchiveKind::Zip => format!("unzip -tq '{}' >/dev/null", tmp_path),
        };
        let check = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(&check_cmd)
            .output()
            .await
            .map_err(|e| format!("integrity check spawn failed: {}", e))?;
        if !check.status.success() {
            let _ = tokio::fs::remove_file(&tmp_path).await;
            return Err(format!(
                "archive integrity check failed: {}",
                String::from_utf8_lossy(&check.stderr).trim()
            ));
        }
    }

    // Extract.
    let extract_cmd = match mirror.kind {
        ArchiveKind::TarGz => format!(
            "tar xzf '{tmp}' -C '{dst}' --strip-components=1 2>&1 || \
             tar xzf '{tmp}' -C '{dst}' 2>&1",
            tmp = tmp_path,
            dst = install_path
        ),
        ArchiveKind::Zip => format!(
            "unzip -q -o '{}' -d '{}' 2>&1",
            tmp_path, install_path
        ),
    };
    let ex = tokio::process::Command::new("sh")
        .arg("-c")
        .arg(&extract_cmd)
        .output()
        .await
        .map_err(|e| format!("extract spawn failed: {}", e))?;
    cleanup(tmp_path.clone(), keep_on_success).await;
    if !ex.status.success() {
        return Err(format!(
            "extraction failed: {}",
            String::from_utf8_lossy(&ex.stderr).trim()
        ));
    }

    // Flatten single-top-dir zips so q3ut4/ ends up at $install_path/q3ut4/.
    if matches!(mirror.kind, ArchiveKind::Zip) {
        flatten_single_top_dir(install_path).await;
    }

    // Final sanity: q3ut4/ must exist after extract.
    let q3ut4 = Path::new(install_path).join("q3ut4");
    if !q3ut4.is_dir() {
        // List what's actually there so admins can tell what went wrong
        // (zip structure changed, flatten failed, wrong archive, etc.).
        let mut listing = Vec::new();
        if let Ok(mut rd) = tokio::fs::read_dir(install_path).await {
            while let Ok(Some(ent)) = rd.next_entry().await {
                let kind = if ent.path().is_dir() { "d" } else { "f" };
                listing.push(format!("{}:{}", kind, ent.file_name().to_string_lossy()));
            }
        }
        return Err(format!(
            "extraction produced no q3ut4/ directory under {} (contents: [{}])",
            install_path,
            listing.join(", ")
        ));
    }

    Ok(())
}

/// Validate that `archive_path` is a well-formed archive of the given
/// `mirror.kind`: checks magic bytes and runs the archive tool's integrity
/// listing (`tar -tzf` / `unzip -tq`). Used to decide whether a cached
/// archive can be reused without re-downloading.
async fn validate_archive(archive_path: &str, mirror: &UrtMirror) -> Result<(), String> {
    let mut header = [0u8; 4];
    {
        use tokio::io::AsyncReadExt;
        let mut f = tokio::fs::File::open(archive_path)
            .await
            .map_err(|e| format!("open failed: {}", e))?;
        f.read_exact(&mut header)
            .await
            .map_err(|e| format!("read magic failed: {}", e))?;
    }
    match mirror.kind {
        ArchiveKind::TarGz => {
            if &header[..2] != [0x1f, 0x8b] {
                return Err("not a gzip archive".to_string());
            }
        }
        ArchiveKind::Zip => {
            if header != [0x50, 0x4b, 0x03, 0x04] {
                return Err("not a zip archive".to_string());
            }
        }
    }
    let check_cmd = match mirror.kind {
        ArchiveKind::TarGz => format!("tar -tzf '{}' >/dev/null", archive_path),
        ArchiveKind::Zip => format!("unzip -tq '{}' >/dev/null", archive_path),
    };
    let check = tokio::process::Command::new("sh")
        .arg("-c")
        .arg(&check_cmd)
        .output()
        .await
        .map_err(|e| format!("integrity spawn failed: {}", e))?;
    if !check.status.success() {
        return Err(format!(
            "integrity check failed: {}",
            String::from_utf8_lossy(&check.stderr).trim()
        ));
    }
    Ok(())
}

/// If `install_path` contains exactly one directory entry (a zip wrapper
/// like `UrbanTerror43/`), move its contents up a level and remove it.
/// Uses `bash -c` with dotglob/nullglob because `/bin/sh` on Ubuntu is
/// dash, which doesn't support `shopt`.
async fn flatten_single_top_dir(install_path: &str) {
    let path = Path::new(install_path);
    let mut entries = match tokio::fs::read_dir(path).await {
        Ok(e) => e,
        Err(e) => {
            warn!(path = %install_path, error = %e, "flatten: read_dir failed");
            return;
        }
    };
    let mut only: Option<PathBuf> = None;
    let mut count = 0usize;
    while let Ok(Some(ent)) = entries.next_entry().await {
        count += 1;
        if count > 1 {
            info!(path = %install_path, "flatten: multiple top-level entries, nothing to flatten");
            return;
        }
        only = Some(ent.path());
    }
    let Some(dir) = only else {
        warn!(path = %install_path, "flatten: install_path is empty after extract");
        return;
    };
    if !dir.is_dir() {
        return;
    }
    info!(wrapper = %dir.display(), dst = %install_path, "flatten: moving contents up one level");
    let out = tokio::process::Command::new("bash")
        .arg("-c")
        .arg(format!(
            "shopt -s dotglob nullglob && mv -- '{src}'/* '{dst}/' && rmdir -- '{src}'",
            src = dir.display(),
            dst = install_path
        ))
        .output()
        .await;
    match out {
        Ok(o) if o.status.success() => {
            info!(wrapper = %dir.display(), "flatten: succeeded");
        }
        Ok(o) => {
            warn!(
                wrapper = %dir.display(),
                stderr = %String::from_utf8_lossy(&o.stderr).trim(),
                stdout = %String::from_utf8_lossy(&o.stdout).trim(),
                "flatten: mv command exited non-zero"
            );
        }
        Err(e) => warn!(wrapper = %dir.display(), error = %e, "flatten: bash spawn failed"),
    }
}

/// Start downloading and installing a UrT 4.3 dedicated server.
/// This spawns a background task and updates the shared install state.
pub fn start_install_game_server(install_path: String, state: SharedInstallState) {
    tokio::spawn(async move {
        run_install(install_path, state).await;
    });
}

async fn run_install(install_path: String, state: SharedInstallState) {
    // Update: downloading
    {
        let mut s = state.write().await;
        s.stage = "downloading".to_string();
        s.percent = 5;
        s.error = None;
        s.completed = false;
    }

    if let Err(msg) = download_and_extract_urt(&install_path).await {
        let mut s = state.write().await;
        s.stage = "error".to_string();
        s.error = Some(msg);
        return;
    }

    {
        let mut s = state.write().await;
        s.stage = "configuring".to_string();
        s.percent = 90;
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
            port: s.port,
            rcon_password: s.rcon_password.clone(),
            server_cfg_path: s.server_cfg_path.clone(),
            public_ip: s.public_ip.clone(),
            service_name: s.service_name.clone(),
            slug: s.slug.clone(),
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

/// Validate a cvar name — alphanumerics + underscore only. Prevents
/// command injection via the sync protocol.
fn valid_cvar_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Parse a Q3/UrT cvar-query response. Typical shape:
/// `"g_gear" is:"LKQEN^7" default:"0^7"`. Returns the value with trailing
/// `^7` color code stripped. Falls back to the trimmed raw response for
/// unknown formats.
fn parse_cvar_response(raw: &str) -> String {
    let clean = raw.trim().trim_matches('\0');
    // Find the `is:"..."` segment.
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

/// GetCvar — read a single cvar via RCON.
pub async fn handle_get_cvar(ctx: Option<&BotContext>, name: &str) -> ClientResponse {
    let Some(ctx) = ctx else { return unavailable("GetCvar"); };
    if !valid_cvar_name(name) {
        return ClientResponse::Error { message: format!("Invalid cvar name: {}", name) };
    }
    match ctx.rcon.send(name).await {
        Ok(raw) => {
            let value = parse_cvar_response(&raw);
            ClientResponse::Ok {
                message: format!("Read cvar {}", name),
                data: Some(serde_json::json!({
                    "name": name,
                    "value": value,
                    "raw": raw,
                })),
            }
        }
        Err(e) => ClientResponse::Error { message: format!("RCON read failed: {}", e) },
    }
}

/// SetCvar — write a single cvar via RCON. `value` is forwarded verbatim
/// (but quoted in the `set` command) so callers can pass UrT gear strings
/// like "GAIKWNEMLOQURSTUVXZ".
pub async fn handle_set_cvar(ctx: Option<&BotContext>, name: &str, value: &str) -> ClientResponse {
    let Some(ctx) = ctx else { return unavailable("SetCvar"); };
    if !valid_cvar_name(name) {
        return ClientResponse::Error { message: format!("Invalid cvar name: {}", name) };
    }
    // Reject control chars / quotes in the value to keep the `set` command
    // well-formed.
    if value.chars().any(|c| c == '"' || c == '\n' || c == '\r' || (c as u32) < 0x20) {
        return ClientResponse::Error { message: "Invalid cvar value".into() };
    }
    let cmd = format!("set {} \"{}\"", name, value);
    match ctx.rcon.send(&cmd).await {
        Ok(_) => ClientResponse::Ok {
            message: format!("Set {} = {}", name, value),
            data: Some(serde_json::json!({ "name": name, "value": value })),
        },
        Err(e) => ClientResponse::Error { message: format!("RCON set failed: {}", e) },
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

/// GetServerCfg — read the currently active `server.cfg`.
///
/// Resolution order:
///   1. The explicit `server.server_cfg_path` chosen during setup, if it
///      exists on disk.
///   2. `{game_log_dir}/server.cfg` (the home-folder copy UrT writes to).
///   3. Any other `*.cfg` under the game-log directory, preferring filenames
///      that contain "server".
pub async fn handle_get_server_cfg(
    game_log: Option<&str>,
    server_cfg_path: Option<&str>,
) -> ClientResponse {
    // 1. Explicit configured path wins.
    if let Some(cfg) = server_cfg_path.and_then(|s| {
        let t = s.trim();
        if t.is_empty() { None } else { Some(t) }
    }) {
        let p = PathBuf::from(cfg);
        match tokio::fs::read_to_string(&p).await {
            Ok(contents) => {
                return ClientResponse::ServerCfg {
                    path: p.to_string_lossy().to_string(),
                    contents,
                };
            }
            Err(e) => {
                // Configured path was unreadable — surface that rather than
                // silently falling back, so the user can fix the setting.
                return ClientResponse::Error {
                    message: format!(
                        "Configured server_cfg_path {} is not readable: {}",
                        p.display(),
                        e
                    ),
                };
            }
        }
    }

    // 2 + 3. Fall back to games.log-derived home directory.
    let Some(dir) = game_dir_from_log(game_log) else {
        return ClientResponse::Error {
            message: "Cannot determine server directory (set server.server_cfg_path or server.game_log)".to_string(),
        };
    };

    let preferred = dir.join("server.cfg");
    if let Ok(contents) = tokio::fs::read_to_string(&preferred).await {
        return ClientResponse::ServerCfg {
            path: preferred.to_string_lossy().to_string(),
            contents,
        };
    }

    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(mut rd) = tokio::fs::read_dir(&dir).await {
        while let Ok(Some(ent)) = rd.next_entry().await {
            let p = ent.path();
            if p.extension().and_then(|e| e.to_str()) == Some("cfg") {
                candidates.push(p);
            }
        }
    }
    candidates.sort();
    let chosen = candidates
        .iter()
        .find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_ascii_lowercase().contains("server"))
                .unwrap_or(false)
        })
        .cloned()
        .or_else(|| candidates.first().cloned());

    match chosen {
        Some(path) => match tokio::fs::read_to_string(&path).await {
            Ok(contents) => ClientResponse::ServerCfg {
                path: path.to_string_lossy().to_string(),
                contents,
            },
            Err(e) => ClientResponse::Error {
                message: format!("Cannot read {}: {}", path.display(), e),
            },
        },
        None => ClientResponse::Error {
            message: format!(
                "No server.cfg found in {} (also tried any *.cfg)",
                dir.display()
            ),
        },
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

/// EnsureMapConfig — get-or-create a map_config row for `map_name`.
/// Returns the resulting `MapConfig` serialized under `data.config`.
pub async fn handle_ensure_map_config(
    storage: Option<&Arc<dyn Storage>>,
    map_name: &str,
) -> ClientResponse {
    let Some(storage) = storage else { return unavailable("EnsureMapConfig"); };
    match storage.ensure_map_config(map_name).await {
        Ok(cfg) => ClientResponse::Ok {
            message: format!("Ensured map_config for {}", map_name),
            data: Some(serde_json::json!({ "config": cfg })),
        },
        Err(e) => ClientResponse::Error {
            message: format!("Storage error: {}", e),
        },
    }
}

/// ApplyMapConfig — fetch `map_configs[map_name]` and push all its cvars
/// to the live server immediately (no map change required).
pub async fn handle_apply_map_config(
    ctx: Option<&BotContext>,
    storage: Option<&Arc<dyn Storage>>,
    map_name: &str,
) -> ClientResponse {
    let Some(ctx) = ctx else { return unavailable("ApplyMapConfig (no BotContext)"); };
    let Some(storage) = storage else { return unavailable("ApplyMapConfig (no storage)"); };
    let cfg = match storage.ensure_map_config(map_name).await {
        Ok(c) => c,
        Err(e) => return ClientResponse::Error {
            message: format!("Storage error: {}", e),
        },
    };
    crate::plugins::mapconfig::MapconfigPlugin::apply_config(ctx, &cfg).await;
    ClientResponse::Ok {
        message: format!("Applied map config for {}", map_name),
        data: Some(serde_json::json!({ "config": cfg })),
    }
}

/// ResetMapConfig — delete the existing row (if any) and re-ensure it
/// from defaults. Returns the freshly-created `MapConfig`.
pub async fn handle_reset_map_config(
    storage: Option<&Arc<dyn Storage>>,
    map_name: &str,
) -> ClientResponse {
    let Some(storage) = storage else { return unavailable("ResetMapConfig"); };
    if let Ok(Some(existing)) = storage.get_map_config(map_name).await {
        if let Err(e) = storage.delete_map_config(existing.id).await {
            return ClientResponse::Error {
                message: format!("Storage error: {}", e),
            };
        }
    }
    match storage.ensure_map_config(map_name).await {
        Ok(cfg) => ClientResponse::Ok {
            message: format!("Reset map_config for {}", map_name),
            data: Some(serde_json::json!({ "config": cfg })),
        },
        Err(e) => ClientResponse::Error {
            message: format!("Storage error: {}", e),
        },
    }
}

/// DownloadMapPk3 — fetch a `.pk3` from the master-supplied URL and save it
/// into the game server's `q3ut4/` directory (derived from `game_log`).
///
/// Security:
///   * `filename` must match `^[A-Za-z0-9._()+-]+\.pk3$` (no path separators,
///     no hidden files).
///   * `url` scheme must be `https` (or `http` only if the host is localhost).
///   * `url` host must appear in `allowed_hosts` when non-empty (the master
///     supplies its configured `map_repo.sources` hosts).
///   * Target directory must resolve from `game_log`; no arbitrary paths.
///   * Downloads go to `<target>.part` then atomic-rename on success.
/// Extract the current value from a Q3-style cvar RCON response such as
/// `"fs_homepath" is:"/home/rusty/.q3a^7" default:"..."`. Returns `None` if
/// no value is present or the response is empty. Strips trailing `^7`.
fn parse_cvar_value(raw: &str) -> Option<String> {
    let re = regex::Regex::new(r#"is:\"([^"]*?)(?:\^7)?\""#).ok()?;
    let caps = re.captures(raw)?;
    let v = caps.get(1)?.as_str().trim().to_string();
    if v.is_empty() { None } else { Some(v) }
}

/// Try to prove the given directory is writable by the current process by
/// creating (and immediately deleting) a unique zero-byte probe file inside
/// it. Returns `Ok(())` on success, or the underlying IO error otherwise.
async fn probe_writable_dir(dir: &Path) -> std::io::Result<()> {
    let probe = dir.join(format!(
        ".r3-write-probe-{}",
        std::process::id()
    ));
    let f = tokio::fs::File::create(&probe).await?;
    drop(f);
    let _ = tokio::fs::remove_file(&probe).await;
    Ok(())
}

/// Build the ordered list of candidate `q3ut4/` directories to try for a
/// `.pk3` import. Earlier entries take priority. Queries the game server
/// for `fs_homepath` / `fs_basepath` when a `BotContext` is available.
async fn resolve_download_candidates(
    ctx: Option<&BotContext>,
    game_log: Option<&str>,
    override_dir: Option<&str>,
) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    let mut push = |p: PathBuf| {
        if !out.iter().any(|existing| existing == &p) {
            out.push(p);
        }
    };

    if let Some(d) = override_dir.map(str::trim).filter(|s| !s.is_empty()) {
        push(PathBuf::from(d));
    }
    if let Some(d) = game_dir_from_log(game_log) {
        push(d);
    }
    if let Some(ctx) = ctx {
        for cvar in ["fs_homepath", "fs_basepath"] {
            if let Ok(raw) = ctx.get_cvar(cvar).await {
                if let Some(val) = parse_cvar_value(&raw) {
                    push(PathBuf::from(val).join("q3ut4"));
                }
            }
        }
    }
    out
}

pub async fn handle_download_map_pk3(
    ctx: Option<&BotContext>,
    game_log: Option<&str>,
    override_dir: Option<&str>,
    url: &str,
    filename: &str,
    allowed_hosts: &[String],
) -> ClientResponse {
    // Validate filename.
    let name_re = match regex::Regex::new(r"^[A-Za-z0-9._()+\-]+\.pk3$") {
        Ok(r) => r,
        Err(e) => {
            return ClientResponse::Error {
                message: format!("Internal regex error: {}", e),
            };
        }
    };
    if !name_re.is_match(filename) {
        return ClientResponse::Error {
            message: format!("Invalid filename: {}", filename),
        };
    }

    // Validate URL.
    let parsed = match reqwest::Url::parse(url) {
        Ok(u) => u,
        Err(e) => {
            return ClientResponse::Error {
                message: format!("Invalid download URL: {}", e),
            };
        }
    };
    let scheme = parsed.scheme();
    let host = parsed.host_str().unwrap_or("");
    if host.is_empty() {
        return ClientResponse::Error {
            message: "Download URL has no host".into(),
        };
    }
    let is_local = matches!(host, "localhost" | "127.0.0.1" | "::1");
    if scheme != "https" && !(scheme == "http" && is_local) {
        return ClientResponse::Error {
            message: format!("Download URL scheme must be https (got {})", scheme),
        };
    }
    if !allowed_hosts.is_empty() {
        let allowed = allowed_hosts.iter().any(|a| {
            reqwest::Url::parse(a)
                .ok()
                .and_then(|u| u.host_str().map(|h| h.to_string()))
                .is_some_and(|h| h.eq_ignore_ascii_case(host))
        });
        if !allowed {
            return ClientResponse::Error {
                message: format!("Download host '{}' not in allowlist", host),
            };
        }
    }

    // Resolve target directory. We try a prioritized list of candidates
    // (admin override -> game_log parent -> fs_homepath/q3ut4 ->
    // fs_basepath/q3ut4) and pick the first one that both exists and
    // passes a write-probe. This sidesteps the common deployment where the
    // bot process runs as a different OS user than the UrT server, or is
    // sandboxed by systemd (`ProtectHome=yes`, read-only mounts) such that
    // the primary `fs_homepath/q3ut4` is not writable.
    let candidates = resolve_download_candidates(ctx, game_log, override_dir).await;
    if candidates.is_empty() {
        return ClientResponse::Error {
            message: "Cannot determine game server directory (game_log not set, \
                      and no fs_homepath/fs_basepath available). Set \
                      `map_repo.download_dir` in the client's TOML config to a \
                      writable q3ut4 directory.".into(),
        };
    }
    let mut tried: Vec<String> = Vec::new();
    let mut chosen: Option<PathBuf> = None;
    for c in &candidates {
        if !c.exists() {
            tried.push(format!("{} (missing)", c.display()));
            continue;
        }
        match probe_writable_dir(c).await {
            Ok(()) => {
                chosen = Some(c.clone());
                break;
            }
            Err(e) => {
                tried.push(format!("{} ({})", c.display(), e));
            }
        }
    }
    let Some(dir) = chosen else {
        // Fingerprint the common case: every candidate reports EROFS even
        // though the paths exist. That's the systemd `ProtectHome=read-only`
        // sandbox from older installer versions — no chmod/chown can fix it,
        // only a systemd unit update. Give an actionable one-command fix.
        let all_erofs = !tried.is_empty()
            && tried
                .iter()
                .all(|t| t.contains("os error 30") || t.to_lowercase().contains("read-only"));
        let hint = if all_erofs {
            " This bot was installed with an older version whose systemd unit \
             sandboxed /home as read-only. Re-run the R3 installer on the \
             server (`curl -sSL https://r3.pugbot.net/api/updates/install-r3.sh | sudo bash`) \
             to regenerate the service unit with the correct ReadWritePaths."
        } else {
            " Set `map_repo.download_dir` in the client's TOML config to an \
             existing directory the bot user can write to (the game server \
             will still load maps from it)."
        };
        return ClientResponse::Error {
            message: format!(
                "No writable q3ut4 directory found. Tried: {}.{}",
                tried.join("; "),
                hint
            ),
        };
    };
    let final_path = dir.join(filename);
    let part_path = dir.join(format!("{}.part", filename));
    if final_path.exists() {
        return ClientResponse::Error {
            message: format!("{} already exists on server", filename),
        };
    }

    // Fetch.
    let http = match reqwest::Client::builder()
        .user_agent("r3-bot/map-repo")
        .timeout(Duration::from_secs(300))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return ClientResponse::Error {
                message: format!("HTTP client build failed: {}", e),
            };
        }
    };

    info!(url = %url, dest = %final_path.display(), "Downloading .pk3");
    let mut resp = match http.get(url).send().await {
        Ok(r) => r,
        Err(e) => {
            return ClientResponse::Error {
                message: format!("HTTP request failed: {}", e),
            };
        }
    };
    if !resp.status().is_success() {
        return ClientResponse::Error {
            message: format!("HTTP {} downloading {}", resp.status(), url),
        };
    }

    // Refuse obviously-wrong content types (HTML error pages etc).
    if let Some(ct) = resp.headers().get(reqwest::header::CONTENT_TYPE) {
        if let Ok(s) = ct.to_str() {
            let s = s.to_ascii_lowercase();
            if s.contains("text/html") || s.contains("application/json") {
                return ClientResponse::Error {
                    message: format!("Unexpected content-type: {}", s),
                };
            }
        }
    }

    // Cap at 256 MiB — .pk3 files in UrT are rarely over 100 MiB.
    const MAX_SIZE: u64 = 256 * 1024 * 1024;
    if let Some(len) = resp.content_length() {
        if len > MAX_SIZE {
            return ClientResponse::Error {
                message: format!("File too large ({} bytes)", len),
            };
        }
    }

    let mut file = match tokio::fs::File::create(&part_path).await {
        Ok(f) => f,
        Err(e) => {
            return ClientResponse::Error {
                message: format!("Cannot create {}: {}", part_path.display(), e),
            };
        }
    };
    use tokio::io::AsyncWriteExt;
    let mut total: u64 = 0;
    loop {
        let chunk = match resp.chunk().await {
            Ok(Some(c)) => c,
            Ok(None) => break,
            Err(e) => {
                let _ = tokio::fs::remove_file(&part_path).await;
                return ClientResponse::Error {
                    message: format!("Download interrupted: {}", e),
                };
            }
        };
        total += chunk.len() as u64;
        if total > MAX_SIZE {
            let _ = tokio::fs::remove_file(&part_path).await;
            return ClientResponse::Error {
                message: "Download exceeded size cap".into(),
            };
        }
        if let Err(e) = file.write_all(&chunk).await {
            let _ = tokio::fs::remove_file(&part_path).await;
            return ClientResponse::Error {
                message: format!("Write failed: {}", e),
            };
        }
    }
    if let Err(e) = file.flush().await {
        let _ = tokio::fs::remove_file(&part_path).await;
        return ClientResponse::Error {
            message: format!("Flush failed: {}", e),
        };
    }
    drop(file);

    if let Err(e) = tokio::fs::rename(&part_path, &final_path).await {
        let _ = tokio::fs::remove_file(&part_path).await;
        return ClientResponse::Error {
            message: format!("Rename failed: {}", e),
        };
    }

    info!(
        path = %final_path.display(),
        size = total,
        "Downloaded .pk3 into game server"
    );
    ClientResponse::MapDownloaded {
        path: final_path.to_string_lossy().to_string(),
        size: total,
    }
}

// ===========================================================================
// Install wizard (client-side)
//
// Handlers supporting the master-driven install wizard: port probing,
// install-defaults suggestion, full wizard install, and systemd service
// control for the managed `urt@<slug>.service` instance.
// ===========================================================================

use crate::sync::urt_cfg;

/// Everything a wizard-related handler needs to know about *this* client bot.
/// Derived from the bot's own `config_path` so we correctly map a slug to
/// state file, install dir, and home dir even if multiple clients share a
/// user account.
#[derive(Debug, Clone)]
pub struct WizardContext {
    /// Absolute path to the R3 install dir (parent of `r3.toml`).
    pub r3_install_dir: PathBuf,
    /// Slug — usually derived from the install dir basename (`r3-<slug>`).
    pub slug: String,
    /// Bot's configured server name (used as a default in suggestions).
    pub server_name: String,
    /// Path to the state marker JSON file.
    pub state_file: PathBuf,
    /// User home dir.
    pub home: PathBuf,
}

impl WizardContext {
    pub fn from_config(config_path: &str, server_name: &str) -> Option<Self> {
        let r3_install_dir = Path::new(config_path).parent()?.to_path_buf();
        let basename = r3_install_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("r3");
        let slug = basename
            .strip_prefix("r3-")
            .map(|s| s.to_string())
            .unwrap_or_else(|| slugify(server_name));
        let state_file = r3_install_dir.join("state/urt-install.json");
        let home = home_dir()?;
        Some(Self {
            r3_install_dir,
            slug,
            server_name: server_name.to_string(),
            state_file,
            home,
        })
    }
}

/// Lowercase, dash-separated, alnum-only (mirrors the shell installer rules).
fn slugify(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_dash = false;
    for c in input.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

// ---------------------------------------------------------------------------
// Multi-instance safety: detect sibling urt@ installs on the same host
// ---------------------------------------------------------------------------

/// A sibling `urt@<slug>.service` instance found on the host, parsed from
/// its drop-in file under `/etc/systemd/system/urt@.service.d/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiblingInstance {
    pub slug: String,
    pub install_path: Option<String>,
    pub port: Option<u16>,
}

/// Default path to the systemd drop-in directory for the `urt@` template.
pub(crate) const URT_DROPIN_DIR: &str = "/etc/systemd/system/urt@.service.d";

/// Parse a single DropIn `.conf` file to extract install path + port.
/// Returns `None` only if the file can't be read; a file with no
/// recognisable keys returns a `SiblingInstance` with `None` fields so the
/// caller still knows the slug exists.
pub(crate) fn parse_dropin(path: &Path) -> Option<SiblingInstance> {
    let slug = path.file_stem()?.to_str()?.to_string();
    let content = std::fs::read_to_string(path).ok()?;
    let mut install_path: Option<String> = None;
    let mut port: Option<u16> = None;
    for line in content.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("WorkingDirectory=") {
            install_path = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("Environment=URT_PORT=") {
            port = rest.trim().parse().ok();
        } else if port.is_none() {
            // Fallback: parse +set net_port <N> out of ExecStart=.
            if let Some(idx) = line.find("+set net_port ") {
                let tail = &line[idx + "+set net_port ".len()..];
                let tok = tail.split_whitespace().next().unwrap_or("");
                port = tok.parse().ok();
            }
        }
    }
    Some(SiblingInstance {
        slug,
        install_path,
        port,
    })
}

/// Scan a DropIn directory for all `urt@<slug>.service` siblings.
/// Returns an empty vec if the directory is missing or unreadable.
pub(crate) fn scan_sibling_urt_instances(dropins_dir: &Path) -> Vec<SiblingInstance> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dropins_dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.extension().and_then(|s| s.to_str()) != Some("conf") {
            continue;
        }
        if let Some(sib) = parse_dropin(&p) {
            out.push(sib);
        }
    }
    out
}

/// Check the discovered sibling instances for conflicts with the install we
/// are about to perform. Callers that pass `force == true` accept slug
/// overwrites but still get errors for port / install-path collisions with
/// *different* slugs (those are always unsafe).
pub(crate) fn check_sibling_conflicts(
    siblings: &[SiblingInstance],
    my_slug: &str,
    my_install_path: &str,
    my_port: u16,
    force: bool,
) -> Result<(), String> {
    // Normalise install path to its string form; comparisons are exact so
    // callers should canonicalise ahead of time if necessary.
    for sib in siblings {
        if sib.slug == my_slug {
            // Same slug → usually a re-install by this very client. The
            // refuse-re-install guard in `run_wizard_install` handles the
            // "already configured locally" case; here we only need to refuse
            // if the sibling points at a *different* install dir (meaning
            // another client previously claimed this slug on the host).
            if let Some(sib_path) = sib.install_path.as_deref() {
                if sib_path != my_install_path && !force {
                    return Err(format!(
                        "Another managed game server already uses slug '{}' at '{}'. \
                         Choose a different slug or uninstall the other instance first.",
                        my_slug, sib_path
                    ));
                }
            }
            continue;
        }
        if sib.install_path.as_deref() == Some(my_install_path) {
            return Err(format!(
                "Install path '{}' is already used by managed instance 'urt@{}.service'. \
                 Each client must have its own install directory.",
                my_install_path, sib.slug
            ));
        }
        if sib.port == Some(my_port) {
            return Err(format!(
                "UDP port {} is already claimed by managed instance 'urt@{}.service'. \
                 Pick a different port.",
                my_port, sib.slug
            ));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// State file I/O
// ---------------------------------------------------------------------------

fn read_install_state(path: &Path) -> Option<UrtInstallState> {
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str::<UrtInstallState>(&raw).ok()
}

fn write_install_state(path: &Path, state: &UrtInstallState) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(state)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    std::fs::write(path, json)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Port detection (active bind probe + passive `ss` parse)
// ---------------------------------------------------------------------------

/// Run `ss` once and collect the set of locally-bound port numbers for the
/// given protocol. Returns `None` if `ss` is not on PATH or fails entirely;
/// callers should treat that as "no passive data, rely on bind probe".
async fn ss_bound_ports(kind: PortKind) -> Option<std::collections::HashMap<u16, String>> {
    // -H: no header, -l: listening, -n: numeric, -p: process info (best-effort).
    let flag = match kind {
        PortKind::Udp => "-Hlunp",
        PortKind::Tcp => "-Hlntp",
    };
    let output = tokio::process::Command::new("ss")
        .arg(flag)
        .output()
        .await
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut map = std::collections::HashMap::new();
    for line in stdout.lines() {
        // Columns vary slightly across ss versions, but the local-address
        // column is typically index 4 for UDP (listening) and index 3 for TCP.
        // Parse robustly by finding the first "addr:port" looking token.
        let fields: Vec<&str> = line.split_whitespace().collect();
        for f in &fields {
            if let Some(port) = parse_listen_port(f) {
                let detail = line.trim().to_string();
                map.insert(port, detail);
                break;
            }
        }
    }
    Some(map)
}

/// Extract a trailing :PORT from an `ss` address column.
fn parse_listen_port(token: &str) -> Option<u16> {
    // IPv6 like `[::]:27960` or IPv4 like `0.0.0.0:27960` or `*:27960`.
    // Strip any bracketed v6 prefix.
    let addr = token.rsplit_once(':')?.1;
    addr.parse::<u16>().ok().filter(|p| *p > 0)
}

fn try_bind(port: u16, kind: PortKind) -> (bool, String) {
    let addr = format!("0.0.0.0:{}", port);
    match kind {
        PortKind::Udp => match std::net::UdpSocket::bind(&addr) {
            Ok(_) => (true, "UDP bind ok".to_string()),
            Err(e) => (false, format!("UDP bind failed: {}", e)),
        },
        PortKind::Tcp => match std::net::TcpListener::bind(&addr) {
            Ok(_) => (true, "TCP bind ok".to_string()),
            Err(e) => (false, format!("TCP bind failed: {}", e)),
        },
    }
}

pub async fn handle_detect_ports(ports: &[u16], kind: PortKind) -> ClientResponse {
    let ss_map = ss_bound_ports(kind).await;
    let mut results = Vec::with_capacity(ports.len());
    for &port in ports {
        let ss_bound = ss_map
            .as_ref()
            .map(|m| m.contains_key(&port))
            .unwrap_or(false);
        let ss_detail = ss_map.as_ref().and_then(|m| m.get(&port).cloned());
        let (bind_ok, bind_detail) = try_bind(port, kind);
        let available = !ss_bound && bind_ok;
        let detail = match (ss_bound, bind_ok) {
            (true, _) => ss_detail.unwrap_or_else(|| "in use (ss-reported)".to_string()),
            (false, false) => bind_detail,
            (false, true) => "available".to_string(),
        };
        results.push(PortProbeResult {
            port,
            available,
            ss_bound,
            bind_succeeded: bind_ok,
            detail,
        });
    }
    ClientResponse::PortReport { kind, results }
}

/// Find the lowest available UDP port in `range` (inclusive start, exclusive
/// end). Uses the active bind probe only for speed.
fn first_available_udp(range: std::ops::Range<u16>) -> Option<u16> {
    range.into_iter().find(|&p| try_bind(p, PortKind::Udp).0)
}

// ---------------------------------------------------------------------------
// Install defaults
// ---------------------------------------------------------------------------

/// Return the current state + suggested defaults for the wizard.
pub async fn handle_suggest_install_defaults(ctx: &WizardContext) -> ClientResponse {
    let state = read_install_state(&ctx.state_file);
    let suggested_install_path = ctx
        .home
        .join(format!("urbanterror-{}", ctx.slug))
        .to_string_lossy()
        .to_string();
    let suggested_port = first_available_udp(27960..28000).unwrap_or(27960);
    let scaffolding_present = Path::new("/etc/systemd/system/urt@.service").exists();
    ClientResponse::InstallDefaults {
        state,
        suggested_install_path,
        suggested_port,
        suggested_slug: ctx.slug.clone(),
        suggested_server_name: ctx.server_name.clone(),
        scaffolding_present,
    }
}

// ---------------------------------------------------------------------------
// Wizard install
// ---------------------------------------------------------------------------

/// Starts the wizard install in a background task (mirrors the behaviour of
/// `start_install_game_server` so the existing poll-based progress UI keeps
/// working).
pub fn start_install_wizard(
    params: GameServerWizardParams,
    ctx: WizardContext,
    state: SharedInstallState,
) {
    tokio::spawn(async move {
        run_wizard_install(params, ctx, state).await;
    });
}

async fn run_wizard_install(
    params: GameServerWizardParams,
    ctx: WizardContext,
    state: SharedInstallState,
) {
    // -- Refuse re-run if already configured (one game server per client) --
    if let Some(existing) = read_install_state(&ctx.state_file) {
        if existing.configured && !params.force_download {
            let mut s = state.write().await;
            s.stage = "error".to_string();
            s.error = Some(
                "This client already has a configured game server. Uninstall it first.".to_string(),
            );
            return;
        }
    }

    // -- Resolve public IP: user-provided value wins, otherwise probe via
    //    a lightweight HTTPS lookup. Blank on failure — the master will
    //    fall back to the client's TLS peer address.
    let mut effective_params = params;
    if effective_params.public_ip.trim().is_empty() {
        if let Some(ip) = detect_public_ip().await {
            info!(%ip, "Auto-detected public IP for wizard install");
            effective_params.public_ip = ip;
        } else {
            warn!("Public IP auto-detection failed; master will fall back to client peer IP");
        }
    }
    let params = effective_params;

    // -- Multi-instance safety: scan sibling urt@ drop-ins for conflicts --
    let slug = params.slug.clone().unwrap_or_else(|| ctx.slug.clone());
    let siblings = scan_sibling_urt_instances(Path::new(URT_DROPIN_DIR));
    if let Err(msg) = check_sibling_conflicts(
        &siblings,
        &slug,
        &params.install_path,
        params.port,
        params.force_download,
    ) {
        let mut s = state.write().await;
        s.stage = "error".to_string();
        s.error = Some(msg);
        return;
    }

    // -- Active port guard: make sure the chosen UDP port is free right now.
    //    Catches races where a sibling isn't yet registered via DropIn but
    //    already bound the port (e.g. started by hand).
    {
        let (ok, detail) = try_bind(params.port, PortKind::Udp);
        if !ok {
            let mut s = state.write().await;
            s.stage = "error".to_string();
            s.error = Some(format!(
                "UDP port {} is not available on this host: {}",
                params.port, detail
            ));
            return;
        }
    }

    // -- Validate params early by rendering the cfg in-memory --
    let rendered_cfg = match urt_cfg::generate(&params, &ctx.server_name) {
        Ok(s) => s,
        Err(e) => {
            let mut s = state.write().await;
            s.stage = "error".to_string();
            s.error = Some(format!("Config validation failed: {}", e));
            return;
        }
    };

    // -- Reset progress --
    {
        let mut s = state.write().await;
        s.stage = "starting".to_string();
        s.percent = 2;
        s.error = None;
        s.completed = false;
        s.install_path = None;
        s.game_log = None;
    }

    // -- Download files if missing or forced --
    let install_path = Path::new(&params.install_path);
    let q3ut4 = install_path.join("q3ut4");
    let have_files = q3ut4.is_dir();
    if !have_files || params.force_download {
        {
            let mut s = state.write().await;
            s.stage = "downloading".to_string();
            s.percent = 10;
        }
        if let Err(msg) = download_and_extract(&params.install_path).await {
            let mut s = state.write().await;
            s.stage = "error".to_string();
            s.error = Some(msg);
            return;
        }
    } else {
        info!(path = %install_path.display(), "UrT files already present — skipping download");
    }

    // -- Write cfg --
    {
        let mut s = state.write().await;
        s.stage = "configuring".to_string();
        s.percent = 75;
    }
    let written = match tokio::task::block_in_place(|| {
        urt_cfg::write_to_disk(install_path, &rendered_cfg)
    }) {
        Ok(w) => w,
        Err(e) => {
            let mut s = state.write().await;
            s.stage = "error".to_string();
            s.error = Some(format!("Writing server.cfg failed: {}", e));
            return;
        }
    };
    let games_log = install_path.join("q3ut4/games.log");

    // -- Optionally register systemd unit --
    let service_name = if params.register_systemd {
        {
            let mut s = state.write().await;
            s.stage = "registering-service".to_string();
            s.percent = 88;
        }
        match register_systemd_instance(&ctx, &params, install_path).await {
            Ok(name) => Some(name),
            Err(msg) => {
                let mut s = state.write().await;
                s.stage = "error".to_string();
                s.error = Some(msg);
                return;
            }
        }
    } else {
        None
    };

    // -- Update state marker --
    let new_state = UrtInstallState {
        slug: ctx.slug.clone(),
        files_present: true,
        install_path: Some(params.install_path.clone()),
        configured: true,
        service_name: service_name.clone(),
        port: Some(params.port),
        server_cfg_path: Some(written.server_cfg.to_string_lossy().to_string()),
        game_log: Some(games_log.to_string_lossy().to_string()),
    };
    if let Err(e) = write_install_state(&ctx.state_file, &new_state) {
        warn!(error = %e, "Failed to persist install-state marker (install still succeeded)");
    }

    {
        let mut s = state.write().await;
        s.stage = "complete".to_string();
        s.percent = 100;
        s.completed = true;
        s.install_path = Some(params.install_path.clone());
        s.game_log = Some(games_log.to_string_lossy().to_string());
        s.port = Some(params.port);
        s.rcon_password = Some(params.rcon_password.clone());
        s.server_cfg_path = Some(written.server_cfg.to_string_lossy().to_string());
        s.public_ip = if params.public_ip.trim().is_empty() {
            None
        } else {
            Some(params.public_ip.clone())
        };
        s.service_name = service_name.clone();
        s.slug = Some(params.slug.clone().unwrap_or_else(|| ctx.slug.clone()));
        s.error = None;
    }

    info!(
        install_path = %params.install_path,
        service = service_name.as_deref().unwrap_or("(unmanaged)"),
        "Install wizard completed successfully"
    );
}

/// Download the UrT 4.3 archive (from the first working mirror) and extract
/// into `install_path`. Returns a human-readable error string on failure.
/// Delegates to the shared `download_and_extract_urt()` helper so both the
/// legacy `InstallGameServer` path and the wizard use the same validated
/// mirror list.
async fn download_and_extract(install_path: &str) -> Result<(), String> {
    download_and_extract_urt(install_path).await
}

/// Try to discover the host's public IPv4 via a short list of well-known
/// echo services. Returns `None` if every probe fails (offline host, DNS
/// broken, captive portal, etc.). Kept deliberately simple and bounded —
/// the master has a peer-IP fallback if this fails.
async fn detect_public_ip() -> Option<String> {
    // Services that return a bare IPv4 as plain text. Ordered by reliability.
    const PROBES: &[&str] = &[
        "https://api.ipify.org",
        "https://ifconfig.me/ip",
        "https://ipv4.icanhazip.com",
    ];
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .user_agent(concat!("rusty-rules-referee/", env!("CARGO_PKG_VERSION")))
        .build()
        .ok()?;
    for url in PROBES {
        match client.get(*url).send().await {
            Ok(r) if r.status().is_success() => {
                if let Ok(body) = r.text().await {
                    let trimmed = body.trim();
                    if is_plausible_ipv4(trimmed) {
                        return Some(trimmed.to_string());
                    }
                }
            }
            _ => continue,
        }
    }
    None
}

/// Quick sanity check that a string looks like a dotted-quad IPv4 address
/// (guards against captive portals returning HTML).
fn is_plausible_ipv4(s: &str) -> bool {
    s.parse::<std::net::Ipv4Addr>()
        .map(|ip| !ip.is_loopback() && !ip.is_unspecified() && !ip.is_link_local())
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// systemd drop-in registration
// ---------------------------------------------------------------------------

/// Locate the UrT dedicated server binary under `install_path`.
fn find_urt_binary(install_path: &Path) -> Option<PathBuf> {
    let candidates = [
        "Quake3-UrT-Ded.x86_64",
        "Quake3-UrT-Ded.x86",
        "Quake3-UrT-Ded.i386",
        "Quake3-UrT-Ded",
    ];
    for name in candidates {
        let p = install_path.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

async fn register_systemd_instance(
    ctx: &WizardContext,
    params: &GameServerWizardParams,
    install_path: &Path,
) -> Result<String, String> {
    // Guard: require the one-time scaffolding. Fail loudly with a useful hint.
    if !Path::new("/etc/systemd/system/urt@.service").exists() {
        return Err(
            "systemd scaffolding is missing on this host. Run 'sudo bash install-r3.sh --add-urt' \
             on the client machine to install it, then retry."
                .to_string(),
        );
    }
    let binary = find_urt_binary(install_path).ok_or_else(|| {
        format!(
            "No UrT dedicated binary found under {} (looked for Quake3-UrT-Ded*).",
            install_path.display()
        )
    })?;
    let user = std::env::var("USER").unwrap_or_else(|_| "nobody".to_string());

    // Drop-in content — overrides User/Group/WorkingDirectory/ExecStart of the
    // urt@.service template for this instance.
    let dropin = format!(
        "# Generated by R3 install wizard for instance {slug}.\n\
         [Service]\n\
         User={user}\n\
         WorkingDirectory={install}\n\
         ReadWritePaths={install}\n\
         Environment=URT_PORT={port}\n\
         ExecStart={binary} +set fs_homepath {install} +set fs_basepath {install} \
         +set dedicated 2 +set net_port {port} +exec server.cfg\n",
        slug = params.slug.clone().unwrap_or_else(|| ctx.slug.clone()),
        user = user,
        install = install_path.display(),
        port = params.port,
        binary = binary.display(),
    );
    let slug_for_unit = params.slug.clone().unwrap_or_else(|| ctx.slug.clone());
    let dropin_path = format!(
        "/etc/systemd/system/urt@.service.d/{}.conf",
        slug_for_unit
    );
    sudo_tee_write(&dropin_path, &dropin).await?;

    // Reload systemd and enable the instance.
    run_sudo(&["systemctl", "daemon-reload"]).await?;
    let unit = format!("urt@{}.service", slug_for_unit);
    run_sudo(&["systemctl", "enable", &unit]).await?;

    Ok(unit)
}

/// Write `content` to `path` via `sudo -n tee`. Uses the narrow NOPASSWD
/// sudoers drop-in installed by `install-r3.sh`.
async fn sudo_tee_write(path: &str, content: &str) -> Result<(), String> {
    use tokio::io::AsyncWriteExt;
    use tokio::process::Command;
    let mut child = Command::new("sudo")
        .args(["-n", "tee", path])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn sudo tee: {}", e))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(content.as_bytes())
            .await
            .map_err(|e| format!("Failed to write to sudo tee stdin: {}", e))?;
        // Explicit drop via shutdown so tee sees EOF.
        let _ = stdin.shutdown().await;
    }
    let out = child
        .wait_with_output()
        .await
        .map_err(|e| format!("sudo tee wait failed: {}", e))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!(
            "sudo tee {} failed: {}. Is the R3 sudoers drop-in installed?",
            path,
            err.trim()
        ));
    }
    Ok(())
}

async fn run_sudo(args: &[&str]) -> Result<String, String> {
    let mut full_args = vec!["-n"];
    full_args.extend_from_slice(args);
    let out = tokio::process::Command::new("sudo")
        .args(&full_args)
        .output()
        .await
        .map_err(|e| format!("Failed to spawn sudo: {}", e))?;
    if !out.status.success() {
        return Err(format!(
            "sudo {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

// ---------------------------------------------------------------------------
// systemd service control
// ---------------------------------------------------------------------------

pub async fn handle_game_server_service(
    action: ServiceAction,
    ctx: &WizardContext,
) -> ClientResponse {
    // Find the configured service from the state marker.
    let state = match read_install_state(&ctx.state_file) {
        Some(s) if s.configured => s,
        _ => {
            return ClientResponse::Error {
                message: "No configured game server on this client — run the install wizard first."
                    .to_string(),
            };
        }
    };
    let service_name = match state.service_name {
        Some(n) => n,
        None => {
            return ClientResponse::Error {
                message: "This install is not managed by systemd. Re-run the wizard with \
                          'Register systemd service' enabled to manage it from the UI."
                    .to_string(),
            };
        }
    };

    let subcommand = match action {
        ServiceAction::Start => "start",
        ServiceAction::Stop => "stop",
        ServiceAction::Restart => "restart",
        ServiceAction::Enable => "enable",
        ServiceAction::Disable => "disable",
        ServiceAction::Status => "status",
    };

    if !matches!(action, ServiceAction::Status) {
        if let Err(msg) = run_sudo(&["systemctl", subcommand, &service_name]).await {
            return ClientResponse::Error {
                message: format!("systemctl {} failed: {}", subcommand, msg),
            };
        }
    }

    // Always fetch status afterwards for a consistent response payload.
    let status_out = tokio::process::Command::new("sudo")
        .args(["-n", "systemctl", "status", &service_name, "--no-pager", "--lines=10"])
        .output()
        .await;
    let status_excerpt = match &status_out {
        Ok(o) => {
            let mut s = String::from_utf8_lossy(&o.stdout).to_string();
            if s.is_empty() {
                s = String::from_utf8_lossy(&o.stderr).to_string();
            }
            s.lines().take(20).collect::<Vec<_>>().join("\n")
        }
        Err(e) => format!("(status unavailable: {})", e),
    };

    let active = tokio::process::Command::new("sudo")
        .args(["-n", "systemctl", "is-active", &service_name])
        .output()
        .await
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "active")
        .unwrap_or(false);
    let enabled = tokio::process::Command::new("sudo")
        .args(["-n", "systemctl", "is-enabled", &service_name])
        .output()
        .await
        .map(|o| {
            let s = String::from_utf8_lossy(&o.stdout);
            matches!(s.trim(), "enabled" | "alias" | "static" | "enabled-runtime")
        })
        .unwrap_or(false);

    ClientResponse::GameServerServiceStatus {
        service_name,
        action: subcommand.to_string(),
        active,
        enabled,
        status_excerpt,
    }
}

// ===========================================================================
// Tests (Phase 7) — multi-instance safety
// ===========================================================================

#[cfg(test)]
mod wizard_safety_tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    /// Minimal RAII tempdir that avoids pulling in the `tempfile` crate
    /// (the build server runs `cargo build --release` offline against a
    /// pre-warmed target/ and can't fetch new deps).
    struct TmpDir(PathBuf);
    impl TmpDir {
        fn new() -> Self {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let n = TEST_DIR_COUNTER.fetch_add(1, Ordering::SeqCst);
            let dir = std::env::temp_dir().join(format!(
                "r3-wizard-test-{}-{}-{}",
                std::process::id(),
                nanos,
                n
            ));
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TmpDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write_dropin(dir: &Path, slug: &str, install: &str, port: u16) {
        let content = format!(
            "[Service]\n\
             User=bob\n\
             WorkingDirectory={install}\n\
             ReadWritePaths={install}\n\
             Environment=URT_PORT={port}\n\
             ExecStart=/opt/urt/Quake3-UrT-Ded +set fs_homepath {install} +set net_port {port} +exec server.cfg\n",
            install = install,
            port = port,
        );
        fs::write(dir.join(format!("{}.conf", slug)), content).unwrap();
    }

    #[test]
    fn parse_dropin_extracts_install_and_port() {
        let tmp = TmpDir::new();
        write_dropin(tmp.path(), "alpha", "/opt/urt-alpha", 27960);
        let p = tmp.path().join("alpha.conf");
        let sib = parse_dropin(&p).unwrap();
        assert_eq!(sib.slug, "alpha");
        assert_eq!(sib.install_path.as_deref(), Some("/opt/urt-alpha"));
        assert_eq!(sib.port, Some(27960));
    }

    #[test]
    fn scan_picks_up_all_conf_files_and_ignores_others() {
        let tmp = TmpDir::new();
        write_dropin(tmp.path(), "alpha", "/opt/urt-alpha", 27960);
        write_dropin(tmp.path(), "bravo", "/opt/urt-bravo", 27961);
        fs::write(tmp.path().join("README.txt"), "ignore me").unwrap();
        let mut sibs = scan_sibling_urt_instances(tmp.path());
        sibs.sort_by(|a, b| a.slug.cmp(&b.slug));
        assert_eq!(sibs.len(), 2);
        assert_eq!(sibs[0].slug, "alpha");
        assert_eq!(sibs[1].slug, "bravo");
    }

    #[test]
    fn scan_returns_empty_when_dir_missing() {
        let tmp = TmpDir::new();
        let missing = tmp.path().join("does-not-exist");
        assert!(scan_sibling_urt_instances(&missing).is_empty());
    }

    #[test]
    fn conflict_detects_port_reuse_by_different_slug() {
        let siblings = vec![SiblingInstance {
            slug: "alpha".into(),
            install_path: Some("/opt/urt-alpha".into()),
            port: Some(27960),
        }];
        let err = check_sibling_conflicts(&siblings, "bravo", "/opt/urt-bravo", 27960, false)
            .unwrap_err();
        assert!(err.contains("port 27960"), "got: {}", err);
        assert!(err.contains("alpha"), "got: {}", err);
    }

    #[test]
    fn conflict_detects_install_path_reuse_by_different_slug() {
        let siblings = vec![SiblingInstance {
            slug: "alpha".into(),
            install_path: Some("/opt/shared".into()),
            port: Some(27960),
        }];
        let err = check_sibling_conflicts(&siblings, "bravo", "/opt/shared", 27961, false)
            .unwrap_err();
        assert!(err.contains("/opt/shared"), "got: {}", err);
        assert!(err.contains("alpha"), "got: {}", err);
    }

    #[test]
    fn conflict_detects_slug_reused_with_different_path() {
        // Same slug but claimed by a different install dir → refuse without force.
        let siblings = vec![SiblingInstance {
            slug: "alpha".into(),
            install_path: Some("/opt/urt-alpha".into()),
            port: Some(27960),
        }];
        let err =
            check_sibling_conflicts(&siblings, "alpha", "/opt/other-dir", 27970, false).unwrap_err();
        assert!(err.contains("slug 'alpha'"), "got: {}", err);
    }

    #[test]
    fn conflict_allows_force_override_for_same_slug_redirect() {
        let siblings = vec![SiblingInstance {
            slug: "alpha".into(),
            install_path: Some("/opt/urt-alpha".into()),
            port: Some(27960),
        }];
        // With force=true, redirecting our own slug to a new dir is allowed…
        check_sibling_conflicts(&siblings, "alpha", "/opt/other-dir", 27970, true).unwrap();
    }

    #[test]
    fn conflict_force_does_not_override_different_slug_port_collision() {
        // Even with force, stealing another instance's port must be refused.
        let siblings = vec![SiblingInstance {
            slug: "alpha".into(),
            install_path: Some("/opt/urt-alpha".into()),
            port: Some(27960),
        }];
        let err = check_sibling_conflicts(&siblings, "bravo", "/opt/urt-bravo", 27960, true)
            .unwrap_err();
        assert!(err.contains("port 27960"));
    }

    #[test]
    fn conflict_allows_unique_slug_path_and_port() {
        let siblings = vec![SiblingInstance {
            slug: "alpha".into(),
            install_path: Some("/opt/urt-alpha".into()),
            port: Some(27960),
        }];
        check_sibling_conflicts(&siblings, "bravo", "/opt/urt-bravo", 27961, false).unwrap();
    }

    #[test]
    fn conflict_allows_self_reinstall_same_slug_same_path() {
        // Same slug + same install path = our own instance re-registering.
        // That's fine (re-install guard elsewhere handles intent); conflict
        // check should not complain here.
        let siblings = vec![SiblingInstance {
            slug: "alpha".into(),
            install_path: Some("/opt/urt-alpha".into()),
            port: Some(27960),
        }];
        check_sibling_conflicts(&siblings, "alpha", "/opt/urt-alpha", 27960, false).unwrap();
    }

    #[test]
    fn conflict_ignores_sibling_with_missing_fields() {
        // A malformed DropIn with no WorkingDirectory / URT_PORT shouldn't
        // cause spurious collisions.
        let siblings = vec![SiblingInstance {
            slug: "alpha".into(),
            install_path: None,
            port: None,
        }];
        check_sibling_conflicts(&siblings, "bravo", "/opt/urt-bravo", 27961, false).unwrap();
    }

    #[test]
    fn slugify_normalises_arbitrary_input() {
        assert_eq!(slugify("My Server!"), "my-server");
        assert_eq!(slugify("  Clan-One  "), "clan-one");
        assert_eq!(slugify("a//b//c"), "a-b-c");
        assert_eq!(slugify("Server #42"), "server-42");
    }
}

