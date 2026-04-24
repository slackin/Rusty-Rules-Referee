//! Per-client systemd lifecycle management for hub-managed R3 clients.
//!
//! Each managed client lives at `<clients_root>/<slug>/` with its own
//! `r3.toml`, certs, and database. systemd starts it as
//! `r3-client@<slug>.service` (template instance).

use std::path::{Path, PathBuf};
use std::process::Stdio;

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

/// Build a `HubClientStatus` for each client install dir, querying systemd
/// for liveness when available.
pub async fn list_client_statuses(hub_cfg: &HubSection) -> Vec<HubClientStatus> {
    let slugs = list_installed_slugs(hub_cfg);
    let mut out = Vec::with_capacity(slugs.len());
    for slug in slugs {
        let unit = unit_name(hub_cfg, &slug);
        let systemd_state = systemctl_active_state(&unit).await;
        out.push(HubClientStatus {
            slug,
            server_id: None, // TODO: read from per-client r3.db / state file
            systemd_state,
            pid: None,
            rss_bytes: None,
            last_log_line: None,
        });
    }
    out
}

async fn systemctl_active_state(unit: &str) -> String {
    let out = Command::new("systemctl")
        .args(["is-active", unit])
        .stderr(Stdio::null())
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

/// Write `content` to `path` via `sudo -n tee` (narrow NOPASSWD rule).
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

/// Run `sudo -n <action> <unit>`. Used by start/stop/restart actions.
pub async fn systemctl_action(unit: &str, action: &str) -> anyhow::Result<()> {
    run_sudo(&["systemctl", action, unit]).await?;
    Ok(())
}

/// Reload systemd unit files (after writing a new drop-in / instance config).
pub async fn systemctl_daemon_reload() -> anyhow::Result<()> {
    run_sudo(&["systemctl", "daemon-reload"]).await?;
    Ok(())
}

/// Provision a new client at `<clients_root>/<slug>/` with the given
/// `r3.toml`, cert, and key. Writes a systemd drop-in pointing the
/// `r3-client@<slug>.service` instance at this directory.
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

    // Write systemd drop-in to point the template unit at this directory.
    // The hub runs as an unprivileged user, so the drop-in directory and
    // file are created/written via `sudo -n` against the narrow NOPASSWD
    // sudoers rule installed by install-r3.sh (hub mode).
    let unit = unit_name(hub_cfg, slug);
    let dropin_dir = PathBuf::from(format!("/etc/systemd/system/{}.d", unit));
    if let Err(e) = run_sudo(&[
        "install",
        "-d",
        "-m",
        "0755",
        &dropin_dir.to_string_lossy(),
    ])
    .await
    {
        warn!(error = %e, "Could not create systemd drop-in dir via sudo");
    }
    let conf = {
        // Run the managed client as the hub's user so it can write r3.db,
        // logs, and per-instance state files under its install dir.
        let user = std::env::var("USER").unwrap_or_else(|_| "root".to_string());
        let abs_dir = dir.canonicalize().unwrap_or(dir.clone());
        format!(
            "[Service]\n\
             User={user}\n\
             WorkingDirectory={wd}\n\
             ReadWritePaths={wd}\n\
             Environment=R3_CONF={conf}\n",
            user = user,
            wd = abs_dir.display(),
            conf = dir.join("r3.toml").display(),
        )
    };
    let dropin_file = dropin_dir.join("install.conf");
    if let Err(e) = sudo_tee_write(&dropin_file, &conf).await {
        warn!(error = %e, "Could not write systemd drop-in via sudo");
    }

    if let Err(e) = systemctl_daemon_reload().await {
        warn!(error = %e, "systemctl daemon-reload failed");
    }
    if let Err(e) = run_sudo(&["systemctl", "enable", "--now", &unit]).await {
        warn!(error = %e, %unit, "systemctl enable --now failed");
    }

    info!(%slug, "Client installed and enabled");
    Ok(())
}

/// Disable + stop the client unit and optionally remove its install dir.
///
/// Returns a per-step log as `(step, ok, message)` tuples so callers can
/// relay detailed progress back to the master UI instead of swallowing
/// errors. Returning `Err` is reserved for truly fatal problems (e.g.
/// filesystem errors we can't recover from); `sudo`/systemctl failures
/// are captured in the step log so the admin can see exactly which
/// command failed.
pub async fn uninstall_client(
    hub_cfg: &HubSection,
    slug: &str,
    remove_data: bool,
) -> anyhow::Result<Vec<(String, bool, String)>> {
    let mut steps: Vec<(String, bool, String)> = Vec::new();
    let unit = unit_name(hub_cfg, slug);
    info!(%slug, %unit, remove_data, "uninstall_client starting");

    match run_sudo(&["systemctl", "disable", "--now", &unit]).await {
        Ok(_) => steps.push((
            "disable_unit".into(),
            true,
            format!("Disabled + stopped {}", unit),
        )),
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
        Ok(_) => steps.push((
            "remove_dropin".into(),
            true,
            format!("Removed {}", dropin_dir.display()),
        )),
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
        Err(e) => steps.push((
            "daemon_reload".into(),
            false,
            format!("daemon-reload failed: {}", e),
        )),
    }

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
        warn!(%slug, %unit, ?steps, "uninstall_client finished with failures");
    } else {
        info!(%slug, %unit, "uninstall_client completed cleanly");
    }
    Ok(steps)
}
