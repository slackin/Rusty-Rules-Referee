//! Per-client systemd lifecycle management for hub-managed R3 clients.
//!
//! Each managed client lives at `<clients_root>/<slug>/` with its own
//! `r3.toml`, certs, and database. systemd starts it as
//! `r3-client@<slug>.service` (template instance).

use std::path::{Path, PathBuf};
use std::process::Stdio;

use tokio::process::Command;
use tracing::{debug, info, warn};

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

/// Run `systemctl <action> <unit>`. Used by start/stop/restart actions.
pub async fn systemctl_action(unit: &str, action: &str) -> anyhow::Result<()> {
    let status = Command::new("systemctl")
        .args([action, unit])
        .status()
        .await?;
    if !status.success() {
        anyhow::bail!("systemctl {} {} failed", action, unit);
    }
    Ok(())
}

/// Reload systemd unit files (after writing a new drop-in / instance config).
pub async fn systemctl_daemon_reload() -> anyhow::Result<()> {
    let status = Command::new("systemctl")
        .args(["daemon-reload"])
        .status()
        .await?;
    if !status.success() {
        anyhow::bail!("systemctl daemon-reload failed");
    }
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
    let unit = unit_name(hub_cfg, slug);
    let dropin_dir = PathBuf::from(format!("/etc/systemd/system/{}.d", unit));
    if let Err(e) = std::fs::create_dir_all(&dropin_dir) {
        warn!(error = %e, "Could not create systemd drop-in dir (non-fatal in dev)");
    }
    let conf = format!(
        "[Service]\nWorkingDirectory={}\nEnvironment=R3_CONF={}\n",
        dir.canonicalize().unwrap_or(dir.clone()).display(),
        dir.join("r3.toml").display(),
    );
    if let Err(e) = std::fs::write(dropin_dir.join("install.conf"), conf) {
        warn!(error = %e, "Could not write systemd drop-in (non-fatal in dev)");
    }

    let _ = systemctl_daemon_reload().await;
    let _ = Command::new("systemctl")
        .args(["enable", "--now", &unit])
        .status()
        .await;

    info!(%slug, "Client installed and enabled");
    Ok(())
}

/// Disable + stop the client unit and optionally remove its install dir.
pub async fn uninstall_client(
    hub_cfg: &HubSection,
    slug: &str,
    remove_data: bool,
) -> anyhow::Result<()> {
    let unit = unit_name(hub_cfg, slug);
    let _ = Command::new("systemctl")
        .args(["disable", "--now", &unit])
        .status()
        .await;

    let dropin_dir = PathBuf::from(format!("/etc/systemd/system/{}.d", unit));
    let _ = std::fs::remove_dir_all(&dropin_dir);
    let _ = systemctl_daemon_reload().await;

    if remove_data {
        let dir = client_dir(hub_cfg, slug);
        if let Err(e) = std::fs::remove_dir_all(&dir) {
            warn!(error = %e, "Failed to remove client dir");
        }
    }
    debug!(%slug, "Client uninstalled");
    Ok(())
}
