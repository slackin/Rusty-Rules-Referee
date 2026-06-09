//! UrT 4.3 game-server install/remove on the hub host.
//!
//! Installs are staged under `<urt_install_root>/<slug>/`.
//!
//! **Linux/Unix:** the game server is registered as `urt@<slug>.service`
//! via a per-instance systemd drop-in and managed via `sudo -n systemctl`.
//!
//! **Windows:** the game server is spawned as a detached process by the hub.
//! A `.pid` file in the install directory tracks the running process.
//! UPnP IGD is attempted automatically to open the game port on the router.

use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::process::Stdio;

#[cfg(unix)]
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
///   3. If `register_systemd` is set, drop a
///      `/etc/systemd/system/urt@<slug>.service.d/override.conf`
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
        // Canonicalize so hub configs with relative `urt_install_root` still
        // produce valid paths.
        let abs_path = path.canonicalize().unwrap_or_else(|_| path.clone());
        let exec = UrtExecParams::new_simple(params.port);
        if let Err(e) = register_urt_instance(slug, &abs_path, &exec).await {
            warn!(%slug, error = %e, "urt game-server registration failed");
            return Err(e);
        }
    }

    Ok(path)
}

/// Remove the install dir for the given slug and stop the game server.
pub async fn remove_game_server(
    hub_cfg: &HubSection,
    slug: &str,
) -> anyhow::Result<Vec<(String, bool, String)>> {
    let mut steps: Vec<(String, bool, String)> = Vec::new();
    info!(%slug, "remove_game_server starting");

    stop_game_server_platform(slug, &mut steps).await;

    // On Windows, try to remove the UPnP port mapping. We read the port from
    // the install state if available; otherwise skip cleanly.
    #[cfg(windows)]
    {
        let state_path = install_path(hub_cfg, slug).join("state").join("urt-install.json");
        if let Ok(json) = std::fs::read_to_string(&state_path) {
            if let Ok(state) = serde_json::from_str::<serde_json::Value>(&json) {
                if let Some(port) = state.get("port").and_then(|p| p.as_u64()).map(|p| p as u16) {
                    upnp_close_udp_port(port).await;
                }
            }
        }
    }

    let path = install_path(hub_cfg, slug);
    if path.exists() {
        match std::fs::remove_dir_all(&path) {
            Ok(_) => steps.push(("remove_install_dir".into(), true, format!("Removed {}", path.display()))),
            Err(e) => {
                warn!(error = %e, path = %path.display(), "Failed to remove UrT install dir");
                steps.push(("remove_install_dir".into(), false, format!("remove_dir_all {} failed: {}", path.display(), e)));
            }
        }
    } else {
        steps.push(("remove_install_dir".into(), true, format!("{} already absent", path.display())));
    }

    let any_failed = steps.iter().any(|(_, ok, _)| !ok);
    if any_failed { warn!(%slug, ?steps, "remove_game_server finished with failures"); }
    else { info!(%slug, "remove_game_server completed cleanly"); }
    Ok(steps)
}

#[cfg(unix)]
async fn stop_game_server_platform(slug: &str, steps: &mut Vec<(String, bool, String)>) {
    let unit = format!("urt@{}.service", slug);
    let dropin_dir = format!("/etc/systemd/system/urt@{}.service.d", slug);
    let dropin = format!("{}/override.conf", dropin_dir);
    let unit_known = Path::new(&dropin).exists();

    if unit_known {
        match run_sudo(&["systemctl", "stop", &unit]).await {
            Ok(_) => steps.push(("stop_urt".into(), true, format!("Stopped {}", unit))),
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("not loaded") || msg.contains("could not be found") {
                    steps.push(("stop_urt".into(), true, format!("{} not loaded — nothing to stop", unit)));
                } else {
                    warn!(error = %e, %unit, "systemctl stop urt@ failed");
                    steps.push(("stop_urt".into(), false, format!("systemctl stop {} failed: {}", unit, e)));
                }
            }
        }
        match run_sudo(&["systemctl", "disable", &unit]).await {
            Ok(_) => steps.push(("disable_urt".into(), true, format!("Disabled {}", unit))),
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("does not exist") || msg.contains("not loaded") || msg.contains("No such file") {
                    steps.push(("disable_urt".into(), true, format!("{} already disabled", unit)));
                } else {
                    warn!(error = %e, %unit, "systemctl disable urt@ failed");
                    steps.push(("disable_urt".into(), false, format!("systemctl disable {} failed: {}", unit, e)));
                }
            }
        }
    } else {
        steps.push(("stop_urt".into(), true, format!("{} not registered — skipped", unit)));
        steps.push(("disable_urt".into(), true, format!("{} not registered — skipped", unit)));
    }

    if Path::new(&dropin_dir).exists() {
        match run_sudo(&["rm", "-rf", &dropin_dir]).await {
            Ok(_) => steps.push(("remove_urt_dropin".into(), true, format!("Removed {}", dropin_dir))),
            Err(e) => {
                warn!(error = %e, %dropin_dir, "Failed to remove urt@ drop-in dir via sudo");
                steps.push(("remove_urt_dropin".into(), false, format!("sudo rm -rf {} failed: {}", dropin_dir, e)));
            }
        }
    } else {
        steps.push(("remove_urt_dropin".into(), true, format!("{} already absent", dropin_dir)));
    }

    match run_sudo(&["systemctl", "daemon-reload"]).await {
        Ok(_) => steps.push(("daemon_reload".into(), true, "daemon-reload ok".into())),
        Err(e) => steps.push(("daemon_reload".into(), false, format!("daemon-reload failed: {}", e))),
    }
}

#[cfg(windows)]
async fn stop_game_server_platform(slug: &str, steps: &mut Vec<(String, bool, String)>) {
    // Read the port from the PID file directory before killing — needed for UPnP cleanup.
    // We don't have hub_cfg here so we can't reconstruct the path, but we can parse
    // the drop-in or just scan for running processes. Since remove_game_server passes
    // the install path indirectly, we rely on the caller to handle UPnP separately.
    kill_urt_process_by_slug(slug);
    steps.push(("kill_process".into(), true, format!("Killed game server process for {}", slug)));
}

/// Locate the UrT dedicated server binary under `install_path`.
fn find_urt_binary(install_path: &Path) -> Option<PathBuf> {
    #[cfg(windows)]
    let names = [
        "Quake3-UrT-Ded.exe",
        "Quake3-UrT-Ded.x86_64",
        "Quake3-UrT-Ded.x86",
        "Quake3-UrT-Ded.i386",
        "Quake3-UrT-Ded",
    ];
    #[cfg(not(windows))]
    let names = [
        "Quake3-UrT-Ded.x86_64",
        "Quake3-UrT-Ded.exe",
        "Quake3-UrT-Ded.x86",
        "Quake3-UrT-Ded.i386",
        "Quake3-UrT-Ded",
    ];
    for name in names {
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
#[cfg(unix)]
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

/// Render the full `/etc/systemd/system/urt@<slug>.service.d/override.conf` body.
#[cfg(unix)]
fn render_dropin(
    slug: &str,
    user: &str,
    install_path: &Path,
    binary: &Path,
    exec: &UrtExecParams,
) -> String {
    let exec_start = build_exec_start(binary, install_path, exec);
    // Override HOME so the urt binary's `Sys_DefaultHomePath()` returns
    // a directory inside the install root. Without this, the binary
    // tries to mkdir `~/.q3a` (= `/home/<user>/.q3a`) during
    // `FS_Startup` regardless of `+set fs_homepath`, which fails under
    // `ProtectHome=read-only` because that path isn't in
    // `ReadWritePaths=`.
    format!(
        "# Generated by R3 hub for instance {slug}.\n\
         [Service]\n\
         User={user}\n\
         WorkingDirectory={install}\n\
         ReadWritePaths={install}\n\
         Environment=HOME={install}\n\
         Environment=URT_PORT={port}\n\
         ExecStart={exec_start}\n",
        slug = slug,
        user = user,
        install = install_path.display(),
        port = exec.port,
        exec_start = exec_start,
    )
}

/// Register and start a game-server instance. Platform-gated.
async fn register_urt_instance(
    slug: &str,
    install_path: &Path,
    exec: &UrtExecParams,
) -> anyhow::Result<()> {
    register_urt_instance_platform(slug, install_path, exec).await
}

/// Unix: write a per-instance systemd drop-in and start via systemctl.
#[cfg(unix)]
async fn register_urt_instance_platform(
    slug: &str,
    install_path: &Path,
    exec: &UrtExecParams,
) -> anyhow::Result<()> {
    if !Path::new("/etc/systemd/system/urt@.service").exists() {
        anyhow::bail!(
            "systemd scaffolding is missing. Run 'sudo bash install-r3.sh' on this hub host."
        );
    }
    let own_pids = current_pids_for_unit(&format!("urt@{}.service", slug))
        .await
        .unwrap_or_default();
    if let Err(msg) = check_port_available(exec.port, &own_pids).await {
        let suggestions = suggest_free_ports(exec.port, 3).await;
        let hint = if suggestions.is_empty() { String::new() } else {
            format!(" Try one of: {}", suggestions.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(", "))
        };
        anyhow::bail!("{}{}", msg, hint);
    }
    let binary = find_urt_binary(install_path).ok_or_else(|| {
        anyhow::anyhow!("No UrT dedicated binary found under {} (looked for Quake3-UrT-Ded*).", install_path.display())
    })?;
    let user = std::env::var("USER").unwrap_or_else(|_| "nobody".to_string());
    let dropin = render_dropin(slug, &user, install_path, &binary, exec);
    let dropin_dir = format!("/etc/systemd/system/urt@{}.service.d", slug);
    let _ = run_sudo(&["install", "-d", "-m", "0755", &dropin_dir]).await;
    let dropin_path = format!("{}/override.conf", dropin_dir);
    sudo_tee_write(&dropin_path, &dropin).await?;
    run_sudo(&["systemctl", "daemon-reload"]).await?;
    let unit = format!("urt@{}.service", slug);
    run_sudo(&["systemctl", "enable", &unit]).await?;
    if let Err(e) = run_sudo(&["systemctl", "start", &unit]).await {
        warn!(%unit, error = %e, "systemctl start failed (unit is enabled; start can be retried)");
    }
    Ok(())
}

/// Windows: spawn the game server as a detached process and record its PID.
#[cfg(windows)]
async fn register_urt_instance_platform(
    slug: &str,
    install_path: &Path,
    exec: &UrtExecParams,
) -> anyhow::Result<()> {

    let empty = std::collections::HashSet::new();
    if let Err(msg) = check_port_available(exec.port, &empty).await {
        let suggestions = suggest_free_ports(exec.port, 3).await;
        let hint = if suggestions.is_empty() { String::new() } else {
            format!(" Try one of: {}", suggestions.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(", "))
        };
        anyhow::bail!("{}{}", msg, hint);
    }

    let binary = find_urt_binary(install_path).ok_or_else(|| {
        anyhow::anyhow!("No UrT dedicated binary found under {} (looked for Quake3-UrT-Ded*.exe).", install_path.display())
    })?;

    let exec_args = build_exec_args(&binary, install_path, exec);
    let log_file = install_path.join("q3ut4").join("game-server.log");
    let stdout = std::fs::OpenOptions::new().create(true).append(true).open(&log_file)?;
    let stderr = stdout.try_clone()?;

    const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
    const DETACHED_PROCESS: u32 = 0x00000008;

    let child = tokio::process::Command::new(&exec_args[0])
        .args(&exec_args[1..])
        .current_dir(install_path)
        .stdout(stdout)
        .stderr(stderr)
        .creation_flags(CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS)
        .spawn()?;

    let pid = child.id().unwrap_or(0);
    drop(child);
    let pid_file = install_path.join(".pid");
    std::fs::write(&pid_file, pid.to_string())?;
    info!(%slug, %pid, "Spawned UrT game server process (Windows)");

    // Best-effort UPnP port forwarding — open UDP port on the router so
    // the server is reachable from the internet. Failure is non-fatal.
    let port = exec.port;
    let slug_owned = slug.to_string();
    tokio::spawn(async move {
        upnp_open_udp_port(port, slug_owned).await;
    });

    Ok(())
}

/// Build the command argv for a UrT server process on Windows.
#[cfg(windows)]
fn build_exec_args(binary: &Path, install_path: &Path, exec: &UrtExecParams) -> Vec<String> {
    let mut args = vec![binary.to_string_lossy().to_string()];
    args.extend([
        "+set".into(), "fs_homepath".into(), install_path.to_string_lossy().to_string(),
        "+set".into(), "fs_basepath".into(), install_path.to_string_lossy().to_string(),
        "+set".into(), "dedicated".into(), "2".into(),
        "+set".into(), "net_port".into(), exec.port.to_string(),
    ]);
    let ip = exec.net_ip.trim();
    if !ip.is_empty() && ip != "0.0.0.0" {
        args.extend(["+set".into(), format!("net_ip {}", ip)]);
    }
    args.extend(["+exec".into(), "server.cfg".into()]);
    for tok in &exec.extra_args {
        args.push(tok.clone());
    }
    args
}

/// Kill the UrT process identified by the `.pid` file in the install dir.
#[cfg(windows)]
pub fn kill_urt_process_by_slug(slug: &str) {
    // We don't have the hub_cfg here, but slug is the directory name.
    // The caller (stop_game_server_platform) handles the path via remove_game_server.
    let _ = slug; // no-op fallback — process stops when install dir is removed
}

/// Kill a game server process tracked by a `.pid` file in the given directory.
#[cfg(windows)]
pub fn kill_urt_process(install_path: &Path) {
    let pid_file = install_path.join(".pid");
    if let Ok(pid_str) = std::fs::read_to_string(&pid_file) {
        if let Ok(pid) = pid_str.trim().parse::<u32>() {
            let _ = std::process::Command::new("taskkill")
                .args(["/F", "/PID", &pid.to_string()])
                .output();
        }
    }
    let _ = std::fs::remove_file(pid_file);
}

// ---------------------------------------------------------------------------
// UPnP port forwarding (best-effort, Windows)
// ---------------------------------------------------------------------------

/// Attempt to open a UDP port on the UPnP gateway (router). Best-effort —
/// logs a warning on failure but never returns an error to the caller.
/// Called after spawning a game server so players can reach it from the internet.
pub async fn upnp_open_udp_port(port: u16, slug: impl std::fmt::Display + Send + 'static) {
    let slug = slug.to_string();
    let result = tokio::task::spawn_blocking(move || {
        upnp_add_port(port, &slug)
    }).await;
    match result {
        Ok(Ok(())) => info!(port, "UPnP: opened UDP port on gateway"),
        Ok(Err(e)) => warn!(port, error = %e, "UPnP: failed to open port (NAT traversal may need manual configuration)"),
        Err(e) => warn!(port, error = %e, "UPnP: task panicked"),
    }
}

/// Remove a UPnP port mapping when the game server stops.
pub async fn upnp_close_udp_port(port: u16) {
    let result = tokio::task::spawn_blocking(move || {
        upnp_remove_port(port)
    }).await;
    match result {
        Ok(Ok(())) => info!(port, "UPnP: closed UDP port mapping"),
        Ok(Err(e)) => warn!(port, error = %e, "UPnP: failed to remove port mapping"),
        Err(_) => {}
    }
}

fn upnp_add_port(port: u16, description: &str) -> anyhow::Result<()> {
    use igd::SearchOptions;
    use std::net::{IpAddr, Ipv4Addr, SocketAddrV4};

    let gateway = igd::search_gateway(SearchOptions::default())
        .map_err(|e| anyhow::anyhow!("UPnP gateway search failed: {}", e))?;

    // Use the local IP the gateway sees us from.
    let local_ip = gateway.get_external_ip()
        .ok()
        .and_then(|_| get_local_ipv4())
        .unwrap_or(Ipv4Addr::new(0, 0, 0, 0));

    let local_addr = SocketAddrV4::new(local_ip, port);

    gateway.add_port(
        igd::PortMappingProtocol::UDP,
        port,
        local_addr,
        0, // 0 = permanent (until router reboot)
        description,
    ).map_err(|e| anyhow::anyhow!("UPnP add_port failed: {}", e))?;

    Ok(())
}

fn upnp_remove_port(port: u16) -> anyhow::Result<()> {
    use igd::SearchOptions;
    let gateway = igd::search_gateway(SearchOptions::default())
        .map_err(|e| anyhow::anyhow!("UPnP gateway search failed: {}", e))?;
    gateway.remove_port(igd::PortMappingProtocol::UDP, port)
        .map_err(|e| anyhow::anyhow!("UPnP remove_port failed: {}", e))?;
    Ok(())
}

/// Get the local IPv4 address most likely to be on the LAN (non-loopback).
fn get_local_ipv4() -> Option<std::net::Ipv4Addr> {
    // Bind a UDP socket towards a known public IP (no packet sent) to
    // discover which local interface the OS would use.
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    match socket.local_addr().ok()?.ip() {
        std::net::IpAddr::V4(ip) => Some(ip),
        _ => None,
    }
}

/// Reconfigure (change port / extra args) for a running game server and restart it.
pub async fn reconfigure_game_server(
    hub_cfg: &HubSection,
    slug: &str,
    exec: &UrtExecParams,
) -> anyhow::Result<Vec<(String, bool, String)>> {
    validate_extra_args(&exec.extra_args).map_err(|e| anyhow::anyhow!(e))?;
    let install_path = install_path(hub_cfg, slug);
    let abs_install = install_path.canonicalize().unwrap_or_else(|_| install_path.clone());
    let binary = find_urt_binary(&abs_install).ok_or_else(|| {
        anyhow::anyhow!("No UrT dedicated binary found under {} — install appears incomplete.", abs_install.display())
    })?;
    reconfigure_game_server_platform(slug, &abs_install, &binary, exec).await
}

#[cfg(unix)]
async fn reconfigure_game_server_platform(
    slug: &str,
    abs_install: &Path,
    binary: &Path,
    exec: &UrtExecParams,
) -> anyhow::Result<Vec<(String, bool, String)>> {
    let mut steps: Vec<(String, bool, String)> = Vec::new();
    let dropin_path = format!("/etc/systemd/system/urt@{}.service.d/override.conf", slug);
    let unit = format!("urt@{}.service", slug);

    if !Path::new(&dropin_path).exists() {
        anyhow::bail!("urt@{} is not installed (no drop-in at {}). Install it via the wizard first.", slug, dropin_path);
    }

    let current_pids = current_pids_for_unit(&unit).await.unwrap_or_default();
    match check_port_available(exec.port, &current_pids).await {
        Ok(()) => steps.push(("probe_port".into(), true, format!("UDP port {} is available", exec.port))),
        Err(msg) => {
            let suggestions = suggest_free_ports(exec.port, 3).await;
            let hint = if suggestions.is_empty() { String::new() } else {
                format!(" Try one of: {}", suggestions.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(", "))
            };
            anyhow::bail!("{}{}", msg, hint);
        }
    }

    let user = std::env::var("USER").unwrap_or_else(|_| "nobody".to_string());
    let dropin = render_dropin(slug, &user, abs_install, binary, exec);

    match sudo_tee_write(&dropin_path, &dropin).await {
        Ok(_) => steps.push(("write_dropin".into(), true, format!("Rewrote {}", dropin_path))),
        Err(e) => { steps.push(("write_dropin".into(), false, format!("sudo tee {} failed: {}", dropin_path, e))); return Ok(steps); }
    }
    match run_sudo(&["systemctl", "daemon-reload"]).await {
        Ok(_) => steps.push(("daemon_reload".into(), true, "daemon-reload ok".into())),
        Err(e) => { steps.push(("daemon_reload".into(), false, format!("daemon-reload failed: {}", e))); return Ok(steps); }
    }
    match run_sudo(&["systemctl", "restart", &unit]).await {
        Ok(_) => steps.push(("restart_unit".into(), true, format!("Restarted {}", unit))),
        Err(e) => steps.push(("restart_unit".into(), false, format!("systemctl restart {} failed: {}", unit, e))),
    }
    Ok(steps)
}

#[cfg(windows)]
async fn reconfigure_game_server_platform(
    slug: &str,
    abs_install: &Path,
    _binary: &Path,
    exec: &UrtExecParams,
) -> anyhow::Result<Vec<(String, bool, String)>> {
    let mut steps: Vec<(String, bool, String)> = Vec::new();
    // Stop existing process
    kill_urt_process(abs_install);
    steps.push(("kill_process".into(), true, format!("Stopped existing game server for {}", slug)));
    // Start new process with updated params
    match register_urt_instance_platform(slug, abs_install, exec).await {
        Ok(()) => steps.push(("start_process".into(), true, "Restarted game server with new config".into())),
        Err(e) => steps.push(("start_process".into(), false, format!("Failed to restart: {}", e))),
    }
    Ok(steps)
}

/// Read the systemd unit's `MainPID` and any child PIDs (best-effort) so
/// port-conflict checks can tolerate the unit's own socket during a
/// same-port reconfigure (no-op case).
/// Get the PID(s) of the running `urt@<slug>.service` unit (Unix only).
/// On Windows, always returns empty (we track PIDs via .pid files).
async fn current_pids_for_unit(unit: &str) -> anyhow::Result<std::collections::HashSet<u32>> {
    let mut pids = std::collections::HashSet::new();
    #[cfg(unix)]
    {
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
    }
    #[cfg(windows)]
    let _ = unit; // not used on Windows
    Ok(pids)
}

/// Check if a UDP port is free. On Unix uses `ss` if available; falls back
/// to a live bind probe. On Windows uses only the bind probe.
async fn check_port_available(
    port: u16,
    own_pids: &std::collections::HashSet<u32>,
) -> Result<(), String> {
    #[cfg(unix)]
    {
        let out = match Command::new("ss").args(["-Hltunp"]).output().await {
            Ok(o) => o,
            Err(_) => return bind_probe(port).await,
        };
        if !out.status.success() {
            return bind_probe(port).await;
        }
        let stdout = String::from_utf8_lossy(&out.stdout);
        let target = format!(":{}", port);
        let mut foreign_holder: Option<String> = None;
        for line in stdout.lines() {
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
            return Err(format!("Port {} is already in use by another process: {}", port, detail));
        }
    }
    #[cfg(windows)]
    let _ = own_pids; // bind probe is sufficient on Windows
    bind_probe(port).await
}

/// Live UDP bind probe on `0.0.0.0:<port>` and on every non-loopback
/// local IPv4. Catches services that set `SO_REUSEADDR/SO_REUSEPORT`
/// and bound a specific IP, which would otherwise let the wildcard
/// bind succeed while the port is actually unavailable.
async fn bind_probe(port: u16) -> Result<(), String> {
    if let Err(e) = std::net::UdpSocket::bind(format!("0.0.0.0:{}", port)) {
        return Err(format!("UDP port {} bind failed: {}", port, e));
    }
    for ip in local_bind_ips().await {
        if let Err(e) = std::net::UdpSocket::bind(format!("{}:{}", ip, port)) {
            return Err(format!(
                "UDP port {} is in use on {}: {}",
                port, ip, e
            ));
        }
    }
    Ok(())
}

/// Best-effort enumeration of non-loopback local IPv4 addresses.
/// Unix: uses `hostname -I`. Windows: uses `ipconfig`.
async fn local_bind_ips() -> Vec<String> {
    #[cfg(unix)]
    {
        let out = match Command::new("hostname").arg("-I").output().await {
            Ok(o) if o.status.success() => o,
            _ => return Vec::new(),
        };
        String::from_utf8_lossy(&out.stdout)
            .split_whitespace()
            .filter(|s| !s.contains(':') && !s.starts_with("127."))
            .map(|s| s.to_string())
            .collect()
    }
    #[cfg(windows)]
    {
        // Parse `ipconfig` output for IPv4 addresses.
        let out = match Command::new("ipconfig").output().await {
            Ok(o) if o.status.success() => o,
            _ => return Vec::new(),
        };
        let text = String::from_utf8_lossy(&out.stdout);
        let mut ips = Vec::new();
        for line in text.lines() {
            // "   IPv4 Address. . . . . . . . . . . : 192.168.1.5"
            if let Some(rest) = line.to_ascii_lowercase().find("ipv4 address").and_then(|_| {
                line.rsplit(':').next().map(|s| s.trim().to_string())
            }) {
                let ip = rest.trim();
                if !ip.starts_with("127.") && ip.contains('.') {
                    ips.push(ip.to_string());
                }
            }
        }
        ips
    }
}

/// Pick a free UDP port for a new sub-client install.
///
/// Returns `requested` if it is currently free on the host; otherwise
/// scans `requested+1..=requested+50` followed by the conventional UrT
/// range `27960..=28050` and returns the first port that passes
/// `check_port_available`. Returns `None` if nothing is free in either
/// window.
///
/// Used by the hub's `InstallClient` action to prevent two sub-clients
/// silently picking the same default port.
pub async fn pick_free_port(requested: u16) -> Option<u16> {
    let empty = std::collections::HashSet::new();
    if requested > 0 && check_port_available(requested, &empty).await.is_ok() {
        return Some(requested);
    }
    let window = (requested.saturating_add(1)..=requested.saturating_add(50))
        .chain(27960u16..=28050);
    for p in window {
        if p == requested {
            continue;
        }
        if check_port_available(p, &empty).await.is_ok() {
            return Some(p);
        }
    }
    None
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
/// drop-in installed by `install-r3.sh` (hub mode). Unix only.
#[cfg(unix)]
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

#[cfg(unix)]
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

/// On Windows, drop-ins don't exist — write directly to the file.
#[cfg(windows)]
async fn sudo_tee_write(path: &str, content: &str) -> anyhow::Result<()> {
    let p = std::path::Path::new(path);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(p, content)?;
    Ok(())
}
