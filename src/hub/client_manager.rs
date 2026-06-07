//! Per-client lifecycle management for hub-managed R3 clients.
//!
//! Each managed client lives at `<clients_root>/<slug>/` with its own
//! `r3.toml`, certs, and database.
//!
//! **Linux/Unix:** clients are started as `r3-client@<slug>.service`
//! systemd template instances managed via `sudo -n systemctl`.
//!
//! **Windows:** clients are spawned as detached processes by the hub.
//! A `.pid` file in the client directory tracks the running process.
//! The hub's watchdog (in `hub/mod.rs`) polls these PIDs and restarts
//! any process that has exited.

use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::process::Stdio;

#[cfg(unix)]
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tracing::{info, warn};

use crate::config::HubSection;
use crate::sync::protocol::HubClientStatus;

/// Compute the per-client install directory for a slug.
pub fn client_dir(hub_cfg: &HubSection, slug: &str) -> PathBuf {
    Path::new(&hub_cfg.clients_root).join(slug)
}

/// systemd template name (`r3-client@.service` → unit `r3-client@<slug>.service`).
pub fn unit_name(hub_cfg: &HubSection, slug: &str) -> String {
    let template = hub_cfg
        .systemd_unit_template
        .strip_suffix(".service")
        .unwrap_or(&hub_cfg.systemd_unit_template);
    let prefix = template.strip_suffix('@').unwrap_or(template);
    format!("{}@{}.service", prefix, slug)
}

/// Discover all currently-installed client slugs by scanning `<clients_root>`.
pub fn list_installed_slugs(hub_cfg: &HubSection) -> Vec<String> {
    let root = Path::new(&hub_cfg.clients_root);
    let Ok(rd) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut slugs = Vec::new();
    for entry in rd.flatten() {
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            if let Some(name) = entry.file_name().to_str() {
                if !name.starts_with('.') {
                    slugs.push(name.to_string());
                }
            }
        }
    }
    slugs
}

/// Build a `HubClientStatus` for each client install dir.
pub async fn list_client_statuses(hub_cfg: &HubSection) -> Vec<HubClientStatus> {
    let slugs = list_installed_slugs(hub_cfg);
    let mut out = Vec::with_capacity(slugs.len());
    for slug in slugs {
        let state = client_running_state(hub_cfg, &slug).await;
        out.push(HubClientStatus {
            slug,
            server_id: None,
            systemd_state: state,
            pid: None,
            rss_bytes: None,
            last_log_line: None,
        });
    }
    out
}

/// Returns the running state string for a client ("active", "inactive", etc.)
#[cfg(unix)]
async fn client_running_state(hub_cfg: &HubSection, slug: &str) -> String {
    systemctl_active_state(&unit_name(hub_cfg, slug)).await
}

#[cfg(windows)]
async fn client_running_state(hub_cfg: &HubSection, slug: &str) -> String {
    let dir = client_dir(hub_cfg, slug);
    if client_process_alive(&dir).is_some() {
        "active".to_string()
    } else {
        "inactive".to_string()
    }
}

async fn systemctl_active_state(unit: &str) -> String {
    let out = Command::new("systemctl")
        .args(["is-active", unit])
        .stderr(std::process::Stdio::null())
        .output()
        .await;
    match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        Err(_) => "unknown".to_string(),
    }
}

/// Run `sudo -n <args...>` and return stdout on success. The hub process runs
/// as an unprivileged user; the installer lays down a narrow sudoers drop-in
/// allowing only systemctl + drop-in writes for `r3-client@*.service`.
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

/// Write `content` to `path` via `sudo -n tee` (narrow NOPASSWD rule) on Unix,
/// or direct file write on Windows.
#[cfg(unix)]
async fn sudo_tee_write(path: &Path, content: &str) -> anyhow::Result<()> {
    let path_str = path.to_string_lossy().to_string();
    let mut child = Command::new("sudo")
        .args(["-n", "tee", &path_str])
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
            path.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(())
}

#[cfg(windows)]
async fn sudo_tee_write(path: &Path, content: &str) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)?;
    Ok(())
}

/// Run `sudo -n <action> <unit>`. Used by start/stop/restart actions.
#[cfg(unix)]
pub async fn systemctl_action(unit: &str, action: &str) -> anyhow::Result<()> {
    run_sudo(&["systemctl", action, unit]).await?;
    Ok(())
}

/// On Windows the hub spawns clients directly; systemctl is not used.
#[cfg(windows)]
pub async fn systemctl_action(_unit: &str, _action: &str) -> anyhow::Result<()> {
    Ok(())
}

/// Reload systemd unit files (after writing a new drop-in / instance config).
#[cfg(unix)]
pub async fn systemctl_daemon_reload() -> anyhow::Result<()> {
    run_sudo(&["systemctl", "daemon-reload"]).await?;
    Ok(())
}

/// No-op on Windows — no systemd.
#[cfg(windows)]
pub async fn systemctl_daemon_reload() -> anyhow::Result<()> {
    Ok(())
}

/// Provision a new client at `<clients_root>/<slug>/` with the given
/// `r3.toml`, cert, and key. Writes platform-specific launch config and
/// starts the process.
pub async fn install_client(
    hub_cfg: &HubSection,
    slug: &str,
    r3_toml: &str,
    ca_cert_pem: &str,
    client_cert_pem: &str,
    client_key_pem: &str,
) -> anyhow::Result<()> {
    let dir = client_dir(hub_cfg, slug);
    std::fs::create_dir_all(dir.join("certs"))?;

    std::fs::write(dir.join("r3.toml"), r3_toml)?;
    std::fs::write(dir.join("certs").join("ca.crt"), ca_cert_pem)?;
    std::fs::write(dir.join("certs").join("client.crt"), client_cert_pem)?;
    let key_path = dir.join("certs").join("client.key");
    std::fs::write(&key_path, client_key_pem)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600));
    }

    install_client_platform(hub_cfg, slug, &dir).await?;

    info!(%slug, "Client installed and started");
    Ok(())
}

/// Unix: write a systemd drop-in and enable+start via systemctl.
#[cfg(unix)]
async fn install_client_platform(
    hub_cfg: &HubSection,
    slug: &str,
    _dir: &Path,
) -> anyhow::Result<()> {
    if let Err(e) = write_client_dropin(hub_cfg, slug).await {
        warn!(error = %e, %slug, "Could not write systemd drop-in");
    }
    let unit = unit_name(hub_cfg, slug);
    if let Err(e) = systemctl_daemon_reload().await {
        warn!(error = %e, "systemctl daemon-reload failed");
    }
    if let Err(e) = run_sudo(&["systemctl", "enable", "--now", &unit]).await {
        warn!(error = %e, %unit, "systemctl enable --now failed");
    }
    Ok(())
}

/// Windows: spawn the client as a detached process and record its PID.
#[cfg(windows)]
async fn install_client_platform(
    hub_cfg: &HubSection,
    slug: &str,
    dir: &Path,
) -> anyhow::Result<()> {
    spawn_client_process(hub_cfg, slug, dir).await
}

// ---- Windows process helpers ------------------------------------------------

/// Path to the PID file for a managed client on Windows.
#[cfg(windows)]
pub fn client_pid_file(dir: &Path) -> PathBuf {
    dir.join(".pid")
}

/// Spawn the R3 client binary as a detached process on Windows, writing
/// its PID to `<client_dir>/.pid` so the hub watchdog can monitor it.
#[cfg(windows)]
pub async fn spawn_client_process(
    hub_cfg: &HubSection,
    slug: &str,
    dir: &Path,
) -> anyhow::Result<()> {

    // Resolve the binary path: prefer the hub's own executable so both
    // always run the same build. Fall back to the configured path.
    let binary = hub_cfg
        .r3_binary_path
        .as_deref()
        .map(PathBuf::from)
        .filter(|p| p.exists())
        .or_else(|| std::env::current_exe().ok())
        .ok_or_else(|| anyhow::anyhow!("Cannot locate rusty-rules-referee binary"))?;

    let conf = dir.join("r3.toml");
    let log_file = dir.join("r3.log");

    // CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS so the child survives
    // if the hub is restarted.
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
    const DETACHED_PROCESS: u32 = 0x00000008;

    let stdout = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)?;
    let stderr = stdout.try_clone()?;

    let child = tokio::process::Command::new(&binary)
        .args(["--mode", "client", conf.to_string_lossy().as_ref()])
        .current_dir(dir)
        .stdout(stdout)
        .stderr(stderr)
        .creation_flags(CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS)
        .spawn()?;

    let pid = child.id().unwrap_or(0);
    // Detach — the process is now independent of this handle.
    drop(child);

    std::fs::write(client_pid_file(dir), pid.to_string())?;
    info!(%slug, %pid, "Spawned R3 client process (Windows)");
    Ok(())
}

/// Check if the PID stored in `<client_dir>/.pid` is still alive.
#[cfg(windows)]
pub fn client_process_alive(dir: &Path) -> Option<u32> {
    let pid_str = std::fs::read_to_string(client_pid_file(dir)).ok()?;
    let pid: u32 = pid_str.trim().parse().ok()?;
    if pid == 0 {
        return None;
    }
    // Use `tasklist /FI "PID eq <n>" /NH` to check liveness.
    let out = std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {}", pid), "/NH", "/FO", "CSV"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    // tasklist returns "INFO: No tasks..." when the PID is gone.
    if text.contains(&pid.to_string()) && !text.contains("No tasks") {
        Some(pid)
    } else {
        None
    }
}

/// Kill a managed client process by PID (Windows).
#[cfg(windows)]
pub fn kill_client_process(dir: &Path) {
    let pid_str = match std::fs::read_to_string(client_pid_file(dir)) {
        Ok(s) => s,
        Err(_) => return,
    };
    if let Ok(pid) = pid_str.trim().parse::<u32>() {
        let _ = std::process::Command::new("taskkill")
            .args(["/F", "/PID", &pid.to_string()])
            .output();
    }
    let _ = std::fs::remove_file(client_pid_file(dir));
}

/// Render the systemd drop-in body for `r3-client@<slug>.service`.
fn render_client_dropin(dir: &Path) -> String {
    // Run the managed client as the hub's user so it can write r3.db,
    // logs, and per-instance state files under its install dir.
    let user = std::env::var("USER").unwrap_or_else(|_| "root".to_string());
    let abs_dir = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    // The bot also needs to write into the UrT install directory
    // (e.g. /home/<user>/urbanterror/<slug>/q3ut4) to import .pk3 files
    // via the map repo browser, and to edit server.cfg / mapcycle.txt.
    // The standalone installer grants the whole user home for this
    // reason; mirror that here so map import works out-of-the-box without
    // a manual unit edit. Falls back to just the install dir if $HOME is
    // unset (rare; e.g. running as root without a login shell).
    let home_rw = std::env::var("HOME")
        .ok()
        .filter(|h| !h.is_empty() && h != "/" && h != &abs_dir.to_string_lossy().to_string())
        .map(|h| format!(" {}", h))
        .unwrap_or_default();
    // NoNewPrivileges=no is required so the sub-client can call
    // `sudo -n systemctl start|stop|restart urt@<slug>.service`. The
    // stock r3-client@.service template sets it to true; this drop-in
    // overrides that without needing a template edit on already-deployed
    // hubs.
    format!(
        "[Service]\n\
         User={user}\n\
         WorkingDirectory={wd}\n\
         ReadWritePaths={wd}{home_rw}\n\
         Environment=R3_CONF={conf}\n\
         NoNewPrivileges=no\n",
        user = user,
        wd = abs_dir.display(),
        home_rw = home_rw,
        conf = dir.join("r3.toml").display(),
    )
}

/// Write (or rewrite) the systemd drop-in for `r3-client@<slug>.service`.
/// Used both at install time and at hub startup (to repair drop-ins
/// generated by older hub builds with narrower `ReadWritePaths`).
#[cfg(unix)]
pub async fn write_client_dropin(hub_cfg: &HubSection, slug: &str) -> anyhow::Result<()> {
    let dir = client_dir(hub_cfg, slug);
    let unit = unit_name(hub_cfg, slug);
    let dropin_dir = PathBuf::from(format!("/etc/systemd/system/{}.d", unit));
    run_sudo(&[
        "install",
        "-d",
        "-m",
        "0755",
        &dropin_dir.to_string_lossy(),
    ])
    .await?;
    let conf = render_client_dropin(&dir);
    let dropin_file = dropin_dir.join("install.conf");
    sudo_tee_write(&dropin_file, &conf).await?;
    Ok(())
}

/// No drop-ins on Windows — process management is handled directly.
#[cfg(windows)]
pub async fn write_client_dropin(_hub_cfg: &HubSection, _slug: &str) -> anyhow::Result<()> {
    Ok(())
}

/// Reconcile managed clients at hub startup.
///
/// Unix: rewrite systemd drop-ins that older builds generated with
/// too-narrow `ReadWritePaths`.
///
/// Windows: restart any client processes that are no longer running.
pub async fn reconcile_client_dropins(hub_cfg: &HubSection) {
    reconcile_clients_platform(hub_cfg).await;
}

#[cfg(unix)]
async fn reconcile_clients_platform(hub_cfg: &HubSection) {
    let slugs = list_installed_slugs(hub_cfg);
    if slugs.is_empty() {
        return;
    }
    let mut changed = 0usize;
    for slug in &slugs {
        let unit = unit_name(hub_cfg, slug);
        let dropin_file =
            PathBuf::from(format!("/etc/systemd/system/{}.d/install.conf", unit));
        let want = render_client_dropin(&client_dir(hub_cfg, slug));
        let have = std::fs::read_to_string(&dropin_file).unwrap_or_default();
        if have == want {
            continue;
        }
        match write_client_dropin(hub_cfg, slug).await {
            Ok(()) => {
                changed += 1;
                info!(%slug, "Updated systemd drop-in for managed client");
            }
            Err(e) => {
                warn!(error = %e, %slug, "Could not refresh systemd drop-in");
            }
        }
    }
    if changed > 0 {
        if let Err(e) = systemctl_daemon_reload().await {
            warn!(error = %e, "systemctl daemon-reload failed after drop-in refresh");
        } else {
            info!(count = changed, "Refreshed managed-client drop-ins");
        }
    }
}

#[cfg(windows)]
async fn reconcile_clients_platform(hub_cfg: &HubSection) {
    let slugs = list_installed_slugs(hub_cfg);
    for slug in &slugs {
        let dir = client_dir(hub_cfg, slug);
        if client_process_alive(&dir).is_none() {
            info!(%slug, "Client process not running — restarting (Windows watchdog)");
            if let Err(e) = spawn_client_process(hub_cfg, slug, &dir).await {
                warn!(error = %e, %slug, "Failed to restart client process");
            }
        }
    }
}

/// Stop the client and optionally remove its install dir.
///
/// Returns a per-step log as `(step, ok, message)` tuples so callers can
/// relay detailed progress back to the master UI.
pub async fn uninstall_client(
    hub_cfg: &HubSection,
    slug: &str,
    remove_data: bool,
) -> anyhow::Result<Vec<(String, bool, String)>> {
    let mut steps: Vec<(String, bool, String)> = Vec::new();
    info!(%slug, remove_data, "uninstall_client starting");

    uninstall_client_platform(hub_cfg, slug, &mut steps).await;

    if remove_data {
        let dir = client_dir(hub_cfg, slug);
        match std::fs::remove_dir_all(&dir) {
            Ok(_) => steps.push((
                "remove_client_dir".into(),
                true,
                format!("Removed {}", dir.display()),
            )),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                steps.push((
                    "remove_client_dir".into(),
                    true,
                    format!("{} already absent", dir.display()),
                ));
            }
            Err(e) => {
                warn!(error = %e, dir = %dir.display(), "Failed to remove client dir");
                steps.push((
                    "remove_client_dir".into(),
                    false,
                    format!("remove_dir_all {} failed: {}", dir.display(), e),
                ));
            }
        }
    } else {
        steps.push((
            "remove_client_dir".into(),
            true,
            "skipped (remove_data=false)".into(),
        ));
    }

    let any_failed = steps.iter().any(|(_, ok, _)| !ok);
    if any_failed {
        warn!(%slug, ?steps, "uninstall_client finished with failures");
    } else {
        info!(%slug, "uninstall_client completed cleanly");
    }
    Ok(steps)
}

#[cfg(unix)]
async fn uninstall_client_platform(
    hub_cfg: &HubSection,
    slug: &str,
    steps: &mut Vec<(String, bool, String)>,
) {
    let unit = unit_name(hub_cfg, slug);
    match run_sudo(&["systemctl", "disable", "--now", &unit]).await {
        Ok(_) => steps.push(("disable_unit".into(), true, format!("Disabled + stopped {}", unit))),
        Err(e) => {
            warn!(error = %e, %unit, "systemctl disable --now failed");
            steps.push((
                "disable_unit".into(),
                false,
                format!("systemctl disable --now {} failed: {}", unit, e),
            ));
        }
    }
    let dropin_dir = PathBuf::from(format!("/etc/systemd/system/{}.d", unit));
    match run_sudo(&["rm", "-rf", &dropin_dir.to_string_lossy()]).await {
        Ok(_) => steps.push(("remove_dropin".into(), true, format!("Removed {}", dropin_dir.display()))),
        Err(e) => {
            warn!(error = %e, dir = %dropin_dir.display(), "Failed to remove drop-in dir via sudo");
            steps.push((
                "remove_dropin".into(),
                false,
                format!("sudo rm -rf {} failed: {}", dropin_dir.display(), e),
            ));
        }
    }
    match systemctl_daemon_reload().await {
        Ok(_) => steps.push(("daemon_reload".into(), true, "daemon-reload ok".into())),
        Err(e) => steps.push(("daemon_reload".into(), false, format!("daemon-reload failed: {}", e))),
    }
}

#[cfg(windows)]
async fn uninstall_client_platform(
    hub_cfg: &HubSection,
    slug: &str,
    steps: &mut Vec<(String, bool, String)>,
) {
    let dir = client_dir(hub_cfg, slug);
    kill_client_process(&dir);
    steps.push(("kill_process".into(), true, format!("Killed client process for {}", slug)));
}
