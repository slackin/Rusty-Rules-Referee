//! UrT 4.3 game-server install/remove on the hub host.
//!
//! Installs are staged under `<urt_install_root>/<slug>/` and registered
//! with systemd as `urt@<slug>.service` via the template unit laid down
//! by `install-r3.sh --add-urt`. The heavy lifting (mirror fetch,
//! archive validation, extraction) is delegated to the shared
//! `handlers::download_and_extract_urt` helper so hub and standalone
//! paths share one tested implementation.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tracing::{info, warn};

use crate::config::HubSection;
use crate::sync::handlers::download_and_extract_urt;
use crate::sync::protocol::GameServerWizardParams;
use crate::sync::urt_cfg;

/// Compute the per-instance install path for a slug under `urt_install_root`.
pub fn install_path(hub_cfg: &HubSection, slug: &str) -> PathBuf {
    PathBuf::from(&hub_cfg.urt_install_root).join(slug)
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
        info!(%slug, path = %path.display(), "Downloading UrT 4.3 for hub-managed game server");
        let path_str = path.to_string_lossy().to_string();
        download_and_extract_urt(&path_str)
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
        if let Err(e) = register_urt_instance(slug, &abs_path, params.port).await {
            warn!(%slug, error = %e, "urt@ systemd registration failed");
            return Err(e);
        }
    }

    Ok(path)
}

/// Remove the install dir for the given slug. Also tears down the systemd
/// drop-in and unit for `urt@<slug>.service` if present.
pub async fn remove_game_server(hub_cfg: &HubSection, slug: &str) -> anyhow::Result<()> {
    let unit = format!("urt@{}.service", slug);
    let _ = run_sudo(&["systemctl", "disable", "--now", &unit]).await;
    let dropin = format!("/etc/systemd/system/urt@.service.d/{}.conf", slug);
    let _ = run_sudo(&["rm", "-f", &dropin]).await;
    let _ = run_sudo(&["systemctl", "daemon-reload"]).await;

    let path = install_path(hub_cfg, slug);
    if path.exists() {
        std::fs::remove_dir_all(&path)?;
    }
    Ok(())
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

/// Write `/etc/systemd/system/urt@.service.d/<slug>.conf`, reload systemd,
/// then enable + start `urt@<slug>.service`.
async fn register_urt_instance(
    slug: &str,
    install_path: &Path,
    port: u16,
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

    let dropin = format!(
        "# Generated by R3 hub for instance {slug}.\n\
         [Service]\n\
         User={user}\n\
         WorkingDirectory={install}\n\
         ReadWritePaths={install}\n\
         Environment=URT_PORT={port}\n\
         ExecStart={binary} +set fs_homepath {install} +set fs_basepath {install} \
         +set dedicated 2 +set net_port {port} +exec server.cfg\n",
        slug = slug,
        user = user,
        install = install_path.display(),
        port = port,
        binary = binary.display(),
    );

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
