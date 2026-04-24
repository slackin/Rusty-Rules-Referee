//! UrT 4.3 game-server install/remove on the hub host.
//!
//! Installs are staged under `<urt_install_root>/<slug>/` and registered
//! with systemd as `urt@<slug>.service` via the template unit laid down
//! by `install-r3.sh --add-urt`. The heavy lifting (mirror fetch,
//! archive validation, extraction) is delegated to the shared
//! `handlers::download_and_extract_urt_cached` helper so hub and
//! standalone paths share one tested implementation. The hub passes
//! a persistent cache dir (`<urt_install_root>/.cache/`) so subsequent
//! installs on the same host reuse the already-downloaded archive
//! instead of hitting the mirror again.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tracing::{info, warn};

use crate::config::HubSection;
use crate::sync::handlers::download_and_extract_urt_cached;
use crate::sync::protocol::GameServerWizardParams;
use crate::sync::urt_cfg;

/// Compute the per-instance install path for a slug under `urt_install_root`.
pub fn install_path(hub_cfg: &HubSection, slug: &str) -> PathBuf {
    PathBuf::from(&hub_cfg.urt_install_root).join(slug)
}

/// Directory used to cache the downloaded UrT 4.3 archive so subsequent
/// installs on the same hub don't re-download hundreds of MB from the
/// mirror. Located as `<urt_install_root>/.cache/` so it lives alongside
/// the per-slug installs and is covered by the same disk/backup policy.
pub fn cache_dir(hub_cfg: &HubSection) -> PathBuf {
    PathBuf::from(&hub_cfg.urt_install_root).join(".cache")
}

/// Install a UrT 4.3 dedicated server for the given slug.
///
/// Steps:
///   1. Download + extract UrT 4.3 files into `<urt_install_root>/<slug>/`
///      (skipped if `q3ut4/` already exists and `force_download` is false).
///   2. Render `server.cfg` from the wizard params and write it (+ a
///      default mapcycle and empty games.log) into `q3ut4/`.
///   3. If `register_systemd` is set, drop a `/etc/systemd/system/urt@.service.d/<slug>.conf`
///      overriding User/WorkingDirectory/ExecStart, reload systemd, enable
///      and start the `urt@<slug>.service` unit.
pub async fn install_game_server(
    hub_cfg: &HubSection,
    slug: &str,
    params: &GameServerWizardParams,
) -> anyhow::Result<PathBuf> {
    // Always install into the hub-managed path for this slug. We ignore
    // `params.install_path` to keep hub-managed servers consistently
    // under `urt_install_root`.
    let path = install_path(hub_cfg, slug);
    std::fs::create_dir_all(&path)?;

    // Validate/render cfg up-front so we fail fast on bad params before
    // paying the download cost.
    let rendered_cfg = urt_cfg::generate(params, &params.hostname)
        .map_err(|e| anyhow::anyhow!("Config validation failed: {}", e))?;

    // Download only when missing, or if explicitly forced by the caller.
    let q3ut4 = path.join("q3ut4");
    let have_files = q3ut4.is_dir();
    if !have_files || params.force_download {
        let cache = cache_dir(hub_cfg);
        info!(
            %slug,
            path = %path.display(),
            cache = %cache.display(),
            "Downloading UrT 4.3 for hub-managed game server (cached)"
        );
        let path_str = path.to_string_lossy().to_string();
        download_and_extract_urt_cached(&path_str, Some(&cache))
            .await
            .map_err(|e| anyhow::anyhow!("UrT download failed: {}", e))?;
    } else {
        info!(%slug, path = %path.display(), "UrT files already present — skipping download");
    }

    // Write server.cfg (0600), mapcycle.txt, games.log.
    let written = tokio::task::block_in_place(|| urt_cfg::write_to_disk(&path, &rendered_cfg))
        .map_err(|e| anyhow::anyhow!("Writing server.cfg failed: {}", e))?;
    info!(%slug, cfg = %written.server_cfg.display(), "Wrote server.cfg");

    if params.register_systemd {
        // Systemd wants an absolute WorkingDirectory/ExecStart. Canonicalize
        // so hub configs with relative `urt_install_root` (e.g. "urbanterror")
        // still produce a valid drop-in.
        let abs_path = path.canonicalize().unwrap_or_else(|_| path.clone());
        let exec = UrtExecParams::new_simple(params.port);
        if let Err(e) = register_urt_instance(slug, &abs_path, &exec).await {
            warn!(%slug, error = %e, "urt@ systemd registration failed");
            return Err(e);
        }
    }

    Ok(path)
}

/// Remove the install dir for the given slug. Also tears down the systemd
/// drop-in and unit for `urt@<slug>.service` if present.
///
/// Returns a per-step log so callers can surface exactly which sub-step
/// failed (sudoers rules for `urt@` differ from `r3-client@` — notably
/// no `disable --now` — so we stop and disable separately).
pub async fn remove_game_server(
    hub_cfg: &HubSection,
    slug: &str,
) -> anyhow::Result<Vec<(String, bool, String)>> {
    let mut steps: Vec<(String, bool, String)> = Vec::new();
    let unit = format!("urt@{}.service", slug);
    let dropin = format!("/etc/systemd/system/urt@.service.d/{}.conf", slug);
    info!(%slug, %unit, "remove_game_server starting");

    // The urt@<slug> unit only exists as a usable service when a drop-in
    // <slug>.conf is present (install-time artifact). If the drop-in is
    // missing, the client never finished its game-server install — skip
    // stop/disable so we don't surface spurious "not loaded" errors.
    let unit_known = Path::new(&dropin).exists();

    if unit_known {
        match run_sudo(&["systemctl", "stop", &unit]).await {
            Ok(_) => steps.push(("stop_urt".into(), true, format!("Stopped {}", unit))),
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("not loaded") || msg.contains("could not be found") {
                    steps.push((
                        "stop_urt".into(),
                        true,
                        format!("{} not loaded — nothing to stop", unit),
                    ));
                } else {
                    warn!(error = %e, %unit, "systemctl stop urt@ failed");
                    steps.push((
                        "stop_urt".into(),
                        false,
                        format!("systemctl stop {} failed: {}", unit, e),
                    ));
                }
            }
        }

        match run_sudo(&["systemctl", "disable", &unit]).await {
            Ok(_) => steps.push(("disable_urt".into(), true, format!("Disabled {}", unit))),
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("does not exist")
                    || msg.contains("not loaded")
                    || msg.contains("No such file")
                {
                    steps.push((
                        "disable_urt".into(),
                        true,
                        format!("{} already disabled", unit),
                    ));
                } else {
                    warn!(error = %e, %unit, "systemctl disable urt@ failed");
                    steps.push((
                        "disable_urt".into(),
                        false,
                        format!("systemctl disable {} failed: {}", unit, e),
                    ));
                }
            }
        }
    } else {
        steps.push((
            "stop_urt".into(),
            true,
            format!("{} not registered — skipped", unit),
        ));
        steps.push((
            "disable_urt".into(),
            true,
            format!("{} not registered — skipped", unit),
        ));
    }

    // Remove the per-instance drop-in file if it exists. (urt@ sudoers only
    // permits removing *.conf files under the shared drop-in directory.)
    if Path::new(&dropin).exists() {
        match run_sudo(&["rm", &dropin]).await {
            Ok(_) => steps.push(("remove_urt_dropin".into(), true, format!("Removed {}", dropin))),
            Err(e) => {
                warn!(error = %e, %dropin, "Failed to remove urt@ drop-in via sudo");
                steps.push((
                    "remove_urt_dropin".into(),
                    false,
                    format!("sudo rm {} failed: {}", dropin, e),
                ));
            }
        }
    } else {
        steps.push((
            "remove_urt_dropin".into(),
            true,
            format!("{} already absent", dropin),
        ));
    }

    match run_sudo(&["systemctl", "daemon-reload"]).await {
        Ok(_) => steps.push(("daemon_reload".into(), true, "daemon-reload ok".into())),
        Err(e) => steps.push((
            "daemon_reload".into(),
            false,
            format!("daemon-reload failed: {}", e),
        )),
    }

    let path = install_path(hub_cfg, slug);
    if path.exists() {
        match std::fs::remove_dir_all(&path) {
            Ok(_) => steps.push((
                "remove_install_dir".into(),
                true,
                format!("Removed {}", path.display()),
            )),
            Err(e) => {
                warn!(error = %e, path = %path.display(), "Failed to remove UrT install dir");
                steps.push((
                    "remove_install_dir".into(),
                    false,
                    format!("remove_dir_all {} failed: {}", path.display(), e),
                ));
            }
        }
    } else {
        steps.push((
            "remove_install_dir".into(),
            true,
            format!("{} already absent", path.display()),
        ));
    }

    let any_failed = steps.iter().any(|(_, ok, _)| !ok);
    if any_failed {
        warn!(%slug, ?steps, "remove_game_server finished with failures");
    } else {
        info!(%slug, "remove_game_server completed cleanly");
    }
    Ok(steps)
}

/// Locate the UrT dedicated server binary under `install_path`.
fn find_urt_binary(install_path: &Path) -> Option<PathBuf> {
    for name in [
        "Quake3-UrT-Ded.x86_64",
        "Quake3-UrT-Ded.x86",
        "Quake3-UrT-Ded.i386",
        "Quake3-UrT-Ded",
    ] {
        let p = install_path.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

/// Start-time options baked into the `urt@<slug>.service` drop-in's
/// `ExecStart=`. All three fields are set by the install wizard and can
/// be changed later via `reconfigure_game_server`.
#[derive(Debug, Clone)]
pub struct UrtExecParams {
    pub port: u16,
    /// Bind IP for `+set net_ip`. Empty or `"0.0.0.0"` means "bind all".
    pub net_ip: String,
    /// Extra ExecStart tokens appended after `+exec server.cfg`. Must pass
    /// [`validate_extra_args`] first.
    pub extra_args: Vec<String>,
}

impl UrtExecParams {
    pub fn new_simple(port: u16) -> Self {
        Self {
            port,
            net_ip: String::new(),
            extra_args: Vec::new(),
        }
    }
}

/// Validator for admin-supplied extra ExecStart tokens. Each token must be
/// printable ASCII with no shell metacharacters, and the joined string
/// must not exceed 1024 bytes. Returns `Ok(())` or a human-readable error.
pub fn validate_extra_args(args: &[String]) -> Result<(), String> {
    const MAX_TOTAL: usize = 1024;
    // Forbidden anywhere in a token: shell metacharacters + control chars.
    // (`+`, `.`, `/`, `:`, `-`, `=`, digits/letters/underscore are allowed.)
    const DENY: &[char] = &[
        '\n', '\r', '\t', '\0', '`', '$', ';', '&', '|', '<', '>', '"', '\'', '\\', '(', ')', '{',
        '}', '[', ']', '*', '?', '#', '!',
    ];
    let mut total = 0usize;
    for (i, tok) in args.iter().enumerate() {
        if tok.is_empty() {
            return Err(format!("extra arg #{} is empty", i + 1));
        }
        for ch in tok.chars() {
            if !ch.is_ascii() || ch.is_ascii_control() {
                return Err(format!(
                    "extra arg #{} contains non-printable or non-ASCII character",
                    i + 1
                ));
            }
            if DENY.contains(&ch) {
                return Err(format!(
                    "extra arg #{} contains disallowed character '{}'",
                    i + 1,
                    ch
                ));
            }
            if ch == ' ' {
                return Err(format!(
                    "extra arg #{} contains a space — split on whitespace into separate tokens",
                    i + 1
                ));
            }
        }
        total += tok.len() + 1;
    }
    if total > MAX_TOTAL {
        return Err(format!(
            "extra args total length {} exceeds {} bytes",
            total, MAX_TOTAL
        ));
    }
    Ok(())
}

/// Build the `ExecStart=` line for the `urt@<slug>.service` drop-in.
/// Returns just the command (no `ExecStart=` prefix). `extra_args` must
/// already be validated via [`validate_extra_args`].
fn build_exec_start(binary: &Path, install_path: &Path, exec: &UrtExecParams) -> String {
    let mut out = format!(
        "{binary} +set fs_homepath {install} +set fs_basepath {install} \
         +set dedicated 2 +set net_port {port}",
        binary = binary.display(),
        install = install_path.display(),
        port = exec.port,
    );
    let ip = exec.net_ip.trim();
    if !ip.is_empty() && ip != "0.0.0.0" {
        out.push_str(&format!(" +set net_ip {}", ip));
    }
    out.push_str(" +exec server.cfg");
    for tok in &exec.extra_args {
        out.push(' ');
        out.push_str(tok);
    }
    out
}

/// Render the full `/etc/systemd/system/urt@.service.d/<slug>.conf` body.
fn render_dropin(
    slug: &str,
    user: &str,
    install_path: &Path,
    binary: &Path,
    exec: &UrtExecParams,
) -> String {
    let exec_start = build_exec_start(binary, install_path, exec);
    format!(
        "# Generated by R3 hub for instance {slug}.\n\
         [Service]\n\
         User={user}\n\
         WorkingDirectory={install}\n\
         ReadWritePaths={install}\n\
         Environment=URT_PORT={port}\n\
         ExecStart={exec_start}\n",
        slug = slug,
        user = user,
        install = install_path.display(),
        port = exec.port,
        exec_start = exec_start,
    )
}

/// Write `/etc/systemd/system/urt@.service.d/<slug>.conf`, reload systemd,
/// then enable + start `urt@<slug>.service`.
async fn register_urt_instance(
    slug: &str,
    install_path: &Path,
    exec: &UrtExecParams,
) -> anyhow::Result<()> {
    if !Path::new("/etc/systemd/system/urt@.service").exists() {
        anyhow::bail!(
            "systemd scaffolding is missing on this host. Run \
             'sudo bash install-r3.sh --add-urt' on the hub host, then retry."
        );
    }
    let binary = find_urt_binary(install_path).ok_or_else(|| {
        anyhow::anyhow!(
            "No UrT dedicated binary found under {} (looked for Quake3-UrT-Ded*).",
            install_path.display()
        )
    })?;
    let user = std::env::var("USER").unwrap_or_else(|_| "nobody".to_string());

    let dropin = render_dropin(slug, &user, install_path, &binary, exec);

    let dropin_dir = "/etc/systemd/system/urt@.service.d";
    // Best-effort mkdir; fine if it already exists.
    let _ = run_sudo(&["install", "-d", "-m", "0755", dropin_dir]).await;

    let dropin_path = format!("{}/{}.conf", dropin_dir, slug);
    sudo_tee_write(&dropin_path, &dropin).await?;

    run_sudo(&["systemctl", "daemon-reload"]).await?;
    let unit = format!("urt@{}.service", slug);
    run_sudo(&["systemctl", "enable", &unit]).await?;
    // Start is best-effort: a failing start shouldn't roll back the cfg
    // write; the admin can inspect journalctl and retry.
    if let Err(e) = run_sudo(&["systemctl", "start", &unit]).await {
        warn!(%unit, error = %e, "systemctl start failed (unit is enabled; start can be retried)");
    }
    Ok(())
}

/// Rewrite an existing `urt@<slug>.service` drop-in with new start-time
/// options and restart the unit. Returns a per-step log so callers can
/// surface partial failures. Errors out if the instance isn't installed
/// (no drop-in to replace) or if the requested port is held by some
/// other process on the host.
pub async fn reconfigure_game_server(
    hub_cfg: &HubSection,
    slug: &str,
    exec: &UrtExecParams,
) -> anyhow::Result<Vec<(String, bool, String)>> {
    let mut steps: Vec<(String, bool, String)> = Vec::new();
    let dropin_path = format!("/etc/systemd/system/urt@.service.d/{}.conf", slug);
    let unit = format!("urt@{}.service", slug);

    if !Path::new(&dropin_path).exists() {
        anyhow::bail!(
            "urt@{} is not installed on this host (no drop-in at {}). Install it via the wizard first.",
            slug,
            dropin_path
        );
    }

    validate_extra_args(&exec.extra_args).map_err(|e| anyhow::anyhow!(e))?;

    // -- Port conflict check: if the requested port is bound by something
    //    other than this very unit, refuse. Use `ss -ulnp` to identify
    //    listeners; the unit's own PID(s) are tolerated.
    let current_pids = current_pids_for_unit(&unit).await.unwrap_or_default();
    match check_port_available(exec.port, &current_pids).await {
        Ok(()) => steps.push((
            "probe_port".into(),
            true,
            format!("UDP port {} is available", exec.port),
        )),
        Err(msg) => {
            let suggestions = suggest_free_ports(exec.port, 3).await;
            let hint = if suggestions.is_empty() {
                String::new()
            } else {
                format!(
                    " Try one of: {}",
                    suggestions
                        .iter()
                        .map(|p| p.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            anyhow::bail!("{}{}", msg, hint);
        }
    }

    // -- Resolve install path from the hub config for this slug.
    let install_path = install_path(hub_cfg, slug);
    let abs_install = install_path
        .canonicalize()
        .unwrap_or_else(|_| install_path.clone());
    let binary = find_urt_binary(&abs_install).ok_or_else(|| {
        anyhow::anyhow!(
            "No UrT dedicated binary found under {} — install appears incomplete.",
            abs_install.display()
        )
    })?;
    let user = std::env::var("USER").unwrap_or_else(|_| "nobody".to_string());
    let dropin = render_dropin(slug, &user, &abs_install, &binary, exec);

    match sudo_tee_write(&dropin_path, &dropin).await {
        Ok(_) => steps.push((
            "write_dropin".into(),
            true,
            format!("Rewrote {}", dropin_path),
        )),
        Err(e) => {
            steps.push((
                "write_dropin".into(),
                false,
                format!("sudo tee {} failed: {}", dropin_path, e),
            ));
            return Ok(steps);
        }
    }

    match run_sudo(&["systemctl", "daemon-reload"]).await {
        Ok(_) => steps.push(("daemon_reload".into(), true, "daemon-reload ok".into())),
        Err(e) => {
            steps.push((
                "daemon_reload".into(),
                false,
                format!("daemon-reload failed: {}", e),
            ));
            return Ok(steps);
        }
    }

    match run_sudo(&["systemctl", "restart", &unit]).await {
        Ok(_) => steps.push(("restart_unit".into(), true, format!("Restarted {}", unit))),
        Err(e) => steps.push((
            "restart_unit".into(),
            false,
            format!("systemctl restart {} failed: {}", unit, e),
        )),
    }

    Ok(steps)
}

/// Read the systemd unit's `MainPID` and any child PIDs (best-effort) so
/// port-conflict checks can tolerate the unit's own socket during a
/// same-port reconfigure (no-op case).
async fn current_pids_for_unit(unit: &str) -> anyhow::Result<std::collections::HashSet<u32>> {
    let mut pids = std::collections::HashSet::new();
    let out = Command::new("systemctl")
        .args(["show", "-p", "MainPID", "--value", unit])
        .output()
        .await?;
    if out.status.success() {
        if let Ok(s) = std::str::from_utf8(&out.stdout) {
            if let Ok(pid) = s.trim().parse::<u32>() {
                if pid > 0 {
                    pids.insert(pid);
                }
            }
        }
    }
    Ok(pids)
}

/// Check if a UDP port is free for this unit to take. If `ss` reports a
/// listener whose PID is in `own_pids`, we treat the port as available
/// (the unit itself currently holds it — a same-port reconfigure is a
/// no-op with respect to binding).
async fn check_port_available(
    port: u16,
    own_pids: &std::collections::HashSet<u32>,
) -> Result<(), String> {
    let out = match Command::new("ss").args(["-Hlunp"]).output().await {
        Ok(o) => o,
        Err(_) => {
            // Fall back to a pure bind probe.
            return match std::net::UdpSocket::bind(format!("0.0.0.0:{}", port)) {
                Ok(_) => Ok(()),
                Err(e) => Err(format!("UDP port {} is in use: {}", port, e)),
            };
        }
    };
    if !out.status.success() {
        return match std::net::UdpSocket::bind(format!("0.0.0.0:{}", port)) {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("UDP port {} is in use: {}", port, e)),
        };
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let target = format!(":{}", port);
    let mut foreign_holder: Option<String> = None;
    for line in stdout.lines() {
        // Find a local-addr token ending in :<port>
        let mut has_port = false;
        for tok in line.split_whitespace() {
            if let Some((_, p)) = tok.rsplit_once(':') {
                if p.parse::<u16>().ok() == Some(port) {
                    has_port = true;
                    break;
                }
            }
        }
        if !has_port && !line.contains(&target) {
            continue;
        }
        // Extract pid=<N> occurrences; if every pid is one of own_pids,
        // the port belongs to this very unit and is fine to keep.
        let mut all_own = true;
        let mut any_pid = false;
        for part in line.split(|c: char| !c.is_ascii_alphanumeric() && c != '=') {
            if let Some(rest) = part.strip_prefix("pid=") {
                any_pid = true;
                if let Ok(pid) = rest.parse::<u32>() {
                    if !own_pids.contains(&pid) {
                        all_own = false;
                    }
                } else {
                    all_own = false;
                }
            }
        }
        if any_pid && all_own {
            continue;
        }
        foreign_holder = Some(line.trim().to_string());
        break;
    }
    if let Some(detail) = foreign_holder {
        return Err(format!(
            "UDP port {} is already in use by another process: {}",
            port, detail
        ));
    }
    // No ss record for the port — still try a live bind to catch races.
    match std::net::UdpSocket::bind(format!("0.0.0.0:{}", port)) {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("UDP port {} bind failed: {}", port, e)),
    }
}

/// Suggest up to `count` free UDP ports near the requested one.
async fn suggest_free_ports(requested: u16, count: usize) -> Vec<u16> {
    let mut out = Vec::with_capacity(count);
    let empty = std::collections::HashSet::new();
    let window = (requested.saturating_add(1)..=requested.saturating_add(50))
        .chain(27960u16..=28050);
    for p in window {
        if p == requested {
            continue;
        }
        if check_port_available(p, &empty).await.is_ok() {
            if !out.contains(&p) {
                out.push(p);
                if out.len() >= count {
                    break;
                }
            }
        }
    }
    out
}

/// Run `sudo -n <args...>`. The hub relies on the narrow NOPASSWD sudoers
/// drop-in installed by `install-r3.sh` (hub mode).
async fn run_sudo(args: &[&str]) -> anyhow::Result<String> {
    let mut full = vec!["-n"];
    full.extend_from_slice(args);
    let out = Command::new("sudo").args(&full).output().await?;
    if !out.status.success() {
        anyhow::bail!(
            "sudo {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

async fn sudo_tee_write(path: &str, content: &str) -> anyhow::Result<()> {
    let mut child = Command::new("sudo")
        .args(["-n", "tee", path])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(content.as_bytes()).await?;
        let _ = stdin.shutdown().await;
    }
    let out = child.wait_with_output().await?;
    if !out.status.success() {
        anyhow::bail!(
            "sudo tee {} failed: {}. Is the R3 sudoers drop-in installed?",
            path,
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(())
}
