//! UrT 4.3 game-server install/remove on the hub host.
//!
//! For the initial scaffold this just shells out to the existing
//! `install-r3.sh` / `urt@.service` machinery laid down by the standalone
//! installer. A future iteration will lift the wizard logic out of
//! `src/sync/handlers.rs` so it can be invoked context-free.

use std::path::PathBuf;

use tracing::info;

use crate::config::HubSection;
use crate::sync::protocol::GameServerWizardParams;

/// Compute the per-instance install path for a slug under `urt_install_root`.
pub fn install_path(hub_cfg: &HubSection, slug: &str) -> PathBuf {
    PathBuf::from(&hub_cfg.urt_install_root).join(slug)
}

/// Install a UrT 4.3 dedicated server for the given slug. Stub: future work
/// will replace this with a context-free port of the wizard.
pub async fn install_game_server(
    hub_cfg: &HubSection,
    slug: &str,
    _params: &GameServerWizardParams,
) -> anyhow::Result<PathBuf> {
    let path = install_path(hub_cfg, slug);
    std::fs::create_dir_all(&path)?;
    info!(%slug, path = %path.display(), "Game server install requested (stub)");
    Ok(path)
}

/// Remove the install dir for the given slug.
pub async fn remove_game_server(hub_cfg: &HubSection, slug: &str) -> anyhow::Result<()> {
    let path = install_path(hub_cfg, slug);
    if path.exists() {
        std::fs::remove_dir_all(&path)?;
    }
    Ok(())
}
