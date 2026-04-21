//! Auto-update system for Rusty Rules Referee.
//!
//! Periodically checks a central update server for new builds, downloads
//! the binary, verifies its SHA-256 hash, replaces the current executable,
//! and restarts the process.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::{error, info, warn};

use crate::config::UpdateSection;

// ---------------------------------------------------------------------------
// Manifest types (deserialized from latest.json on the update server)
// ---------------------------------------------------------------------------

/// The update manifest served at `{update_url}/{channel}/latest.json`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UpdateManifest {
    /// Release channel this manifest was published on (production/beta/alpha/dev).
    /// Optional for backward compatibility, but expected on all new manifests.
    #[serde(default)]
    pub channel: Option<String>,
    /// Semantic version string, e.g. "2.0.0".
    pub version: String,
    /// Unique build identifier, e.g. "2.0.0-a1b2c3d4-20260419120000".
    pub build_hash: String,
    /// Short git commit hash.
    pub git_commit: String,
    /// ISO 8601 timestamp of when the build was published.
    pub released_at: String,
    /// Per-platform download information keyed by platform name.
    /// e.g. "linux-x86_64" -> { url, sha256, size }
    pub platforms: HashMap<String, PlatformBinary>,
}

/// Download information for a specific platform.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlatformBinary {
    /// Full URL to download the binary.
    pub url: String,
    /// SHA-256 hex digest of the binary file.
    pub sha256: String,
    /// File size in bytes.
    pub size: u64,
}

/// Result of an update check.
pub struct UpdateAvailable {
    pub manifest: UpdateManifest,
    pub platform: PlatformBinary,
}

// ---------------------------------------------------------------------------
// Platform detection
// ---------------------------------------------------------------------------

/// Determine the platform key for the current binary.
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

// ---------------------------------------------------------------------------
// Update check
// ---------------------------------------------------------------------------

/// Fetch the update manifest for the given channel and check if a newer build is available.
pub async fn check_for_update(
    base_url: &str,
    channel: &str,
    current_build_hash: &str,
) -> anyhow::Result<Option<UpdateAvailable>> {
    let manifest_url = format!(
        "{}/{}/latest.json",
        base_url.trim_end_matches('/'),
        channel
    );

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    // Bypass any HTTP/CDN caches so we always see the freshly published
    // manifest — the update server overwrites latest.json in place.
    let resp = client
        .get(&manifest_url)
        .header("Cache-Control", "no-cache")
        .header("Pragma", "no-cache")
        .query(&[("_ts", chrono::Utc::now().timestamp().to_string())])
        .send()
        .await?;
    if !resp.status().is_success() {
        anyhow::bail!("Update server returned HTTP {}", resp.status());
    }

    let manifest: UpdateManifest = resp.json().await?;

    // Defense-in-depth: if the manifest advertises a channel, it must match what we requested.
    if let Some(m_channel) = manifest.channel.as_deref() {
        if m_channel != channel {
            warn!(
                requested = channel,
                got = m_channel,
                "Update manifest channel mismatch — skipping"
            );
            return Ok(None);
        }
    }

    // Same build hash → up to date
    if manifest.build_hash == current_build_hash {
        return Ok(None);
    }

    let platform = current_platform();
    let binary_info = match manifest.platforms.get(platform) {
        Some(b) => b.clone(),
        None => {
            warn!(
                platform = platform,
                "Update available ({}) but no binary for this platform",
                manifest.build_hash
            );
            return Ok(None);
        }
    };

    Ok(Some(UpdateAvailable {
        manifest,
        platform: binary_info,
    }))
}

// ---------------------------------------------------------------------------
// Download & verify
// ---------------------------------------------------------------------------

/// Download a binary from `url`, verify its SHA-256, and return the temp file path.
pub async fn download_and_verify(
    url: &str,
    expected_sha256: &str,
) -> anyhow::Result<PathBuf> {
    info!(url = url, "Downloading update...");

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(300))
        .build()?;

    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        anyhow::bail!("Download failed with HTTP {}", resp.status());
    }

    let bytes = resp.bytes().await?;
    info!(size = bytes.len(), "Download complete, verifying SHA-256...");

    // Verify SHA-256
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let actual_sha256 = format!("{:x}", hasher.finalize());

    if actual_sha256 != expected_sha256 {
        anyhow::bail!(
            "SHA-256 mismatch: expected {}, got {}",
            expected_sha256,
            actual_sha256
        );
    }
    info!("SHA-256 verified OK");

    // Write to temp file next to current executable
    let current_exe = std::env::current_exe()?;
    let parent = current_exe.parent().unwrap_or_else(|| std::path::Path::new("."));
    let temp_path = parent.join(".r3-update-tmp");

    std::fs::write(&temp_path, &bytes)?;

    // Set executable permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&temp_path, std::fs::Permissions::from_mode(0o755))?;
    }

    Ok(temp_path)
}

// ---------------------------------------------------------------------------
// Apply update (atomic replace)
// ---------------------------------------------------------------------------

/// Replace the current binary with the downloaded update.
pub fn apply_update(temp_path: &PathBuf) -> anyhow::Result<PathBuf> {
    let current_exe = std::env::current_exe()?;

    // On Unix: rename temp over current binary (atomic on same filesystem)
    // Keep a backup just in case
    let backup_path = current_exe.with_extension("old");
    if backup_path.exists() {
        std::fs::remove_file(&backup_path)?;
    }

    // Move current → .old backup
    std::fs::rename(&current_exe, &backup_path)?;

    // Move temp → current
    match std::fs::rename(temp_path, &current_exe) {
        Ok(()) => {
            info!(path = %current_exe.display(), "Binary updated successfully");
            // Clean up backup
            let _ = std::fs::remove_file(&backup_path);
            Ok(current_exe)
        }
        Err(e) => {
            // Rollback: restore backup
            error!(error = %e, "Failed to install update, rolling back");
            let _ = std::fs::rename(&backup_path, &current_exe);
            Err(e.into())
        }
    }
}

// ---------------------------------------------------------------------------
// Restart
// ---------------------------------------------------------------------------

/// Restart the current process by re-executing self with the same arguments.
/// On Unix this uses exec() to replace the process in-place (same PID),
/// so screen/systemd sessions survive.
pub fn restart() -> ! {
    let exe = std::env::current_exe().expect("cannot determine current exe path");
    let args: Vec<String> = std::env::args().collect();

    info!(
        exe = %exe.display(),
        args = ?&args[1..],
        "Restarting with updated binary..."
    );

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // exec() replaces current process — does not return on success
        let err = std::process::Command::new(&exe)
            .args(&args[1..])
            .exec();
        // Only reached if exec() fails
        error!(error = %err, "Failed to exec() updated binary");
        std::process::exit(1);
    }

    #[cfg(not(unix))]
    {
        // On Windows: spawn new process and exit current one
        match std::process::Command::new(&exe).args(&args[1..]).spawn() {
            Ok(_) => std::process::exit(0),
            Err(e) => {
                error!(error = %e, "Failed to spawn updated binary");
                std::process::exit(1);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Startup update check (one-shot, runs before other init)
// ---------------------------------------------------------------------------

/// One-shot update check intended to run at the very start of process
/// startup — BEFORE storage migrations or any other fallible init runs.
/// If an update is available, it is downloaded, verified, applied, and the
/// process restarts immediately. This guarantees that a broken build can
/// always be superseded by a newer one on the configured channel, even
/// when later initialization steps (e.g. DB migrations) would otherwise
/// crash the process before the normal background update loop kicks in.
///
/// Best-effort: network/download/apply errors are logged and swallowed so
/// that normal startup can proceed when the update server is unreachable.
pub async fn startup_update_check(config: &UpdateSection, current_build_hash: &str) {
    if !config.enabled {
        return;
    }

    info!(
        url = %config.url,
        channel = %config.channel,
        build = current_build_hash,
        "Startup update check..."
    );

    let update = match check_for_update(&config.url, &config.channel, current_build_hash).await {
        Ok(Some(u)) => u,
        Ok(None) => {
            info!("Startup update check: up to date");
            return;
        }
        Err(e) => {
            warn!(error = %e, "Startup update check failed (continuing with current build)");
            return;
        }
    };

    info!(
        current = current_build_hash,
        available = %update.manifest.build_hash,
        version = %update.manifest.version,
        "Startup update check: newer build available — applying before init"
    );

    let temp_path = match download_and_verify(&update.platform.url, &update.platform.sha256).await {
        Ok(p) => p,
        Err(e) => {
            warn!(error = %e, "Startup update: download/verify failed — continuing with current build");
            return;
        }
    };

    match apply_update(&temp_path) {
        Ok(_) => {
            info!("Startup update applied, restarting into new binary...");
            restart();
        }
        Err(e) => {
            warn!(error = %e, "Startup update: apply failed — continuing with current build");
            let _ = std::fs::remove_file(&temp_path);
        }
    }
}

// ---------------------------------------------------------------------------
// Background update loop
// ---------------------------------------------------------------------------

/// Run the auto-update checker as a background loop.
/// Checks periodically, downloads + verifies + applies + restarts when an
/// update is available.
pub async fn run_update_loop(config: UpdateSection, current_build_hash: &str) {
    run_update_loop_with_channel(config, current_build_hash, None).await;
}

/// Same as [`run_update_loop`] but the release channel may be overridden at
/// runtime via the supplied `RwLock`. Used in client mode where the master
/// can change this server's channel without restarting the bot.
pub async fn run_update_loop_with_channel(
    config: UpdateSection,
    current_build_hash: &str,
    channel_override: Option<std::sync::Arc<tokio::sync::RwLock<String>>>,
) {
    let interval = Duration::from_secs(config.check_interval);

    info!(
        url = %config.url,
        channel = %config.channel,
        interval_secs = config.check_interval,
        build = current_build_hash,
        "Auto-update checker started"
    );

    // Do an initial check shortly after startup (30 seconds)
    tokio::time::sleep(Duration::from_secs(30)).await;

    loop {
        let channel = match channel_override.as_ref() {
            Some(lock) => lock.read().await.clone(),
            None => config.channel.clone(),
        };
        info!(%channel, "Checking for updates...");

        match check_for_update(&config.url, &channel, current_build_hash).await {
            Ok(Some(update)) => {
                info!(
                    current = current_build_hash,
                    available = %update.manifest.build_hash,
                    version = %update.manifest.version,
                    "Update available!"
                );

                // Download and verify
                match download_and_verify(
                    &update.platform.url,
                    &update.platform.sha256,
                ).await {
                    Ok(temp_path) => {
                        // Apply the update
                        match apply_update(&temp_path) {
                            Ok(_exe_path) => {
                                if config.auto_restart {
                                    info!("Update applied, restarting...");
                                    restart();
                                    // restart() does not return
                                } else {
                                    info!(
                                        "Update applied. Restart the bot to use the new version."
                                    );
                                    // Stop checking — we've already replaced the binary
                                    return;
                                }
                            }
                            Err(e) => {
                                error!(error = %e, "Failed to apply update");
                                // Clean up temp file
                                let _ = std::fs::remove_file(&temp_path);
                            }
                        }
                    }
                    Err(e) => {
                        error!(error = %e, "Failed to download update");
                    }
                }
            }
            Ok(None) => {
                info!("Up to date (build {})", current_build_hash);
            }
            Err(e) => {
                warn!(error = %e, "Update check failed");
            }
        }

        tokio::time::sleep(interval).await;
    }
}
