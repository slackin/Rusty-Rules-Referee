//! UrT 4.3 `server.cfg` generator for the install wizard.
//!
//! Embeds [`deploy/urt-default.cfg`] via `include_str!` so operators can
//! adjust the baseline without recompiling (then rebuild to roll it out to
//! all clients). Substitutes `{{...}}` placeholders with wizard inputs.

use std::path::{Path, PathBuf};

use super::protocol::GameServerWizardParams;

/// Embedded baseline cfg template (compiled into the binary).
const DEFAULT_CFG_TEMPLATE: &str = include_str!("../../deploy/urt-default.cfg");

/// Embedded baseline mapcycle (compiled into the binary).
const DEFAULT_MAPCYCLE: &str = include_str!("../../deploy/urt-default-mapcycle.txt");

/// Translate a human game-mode label to the numeric `g_gametype` cvar value.
///
/// Returns `None` for unrecognised labels — the caller should surface this
/// as a validation error rather than silently picking a default.
pub fn game_mode_to_gametype(label: &str) -> Option<u8> {
    let normalised = label.trim().to_ascii_uppercase();
    match normalised.as_str() {
        "FFA" | "DM" | "DEATHMATCH" | "0" => Some(0),
        "LMS" | "LASTMANSTANDING" | "1" => Some(1),
        "TDM" | "TEAMDEATHMATCH" | "3" => Some(3),
        "TS" | "TEAMSURVIVOR" | "4" => Some(4),
        "FTL" | "FOLLOWTHELEADER" | "5" => Some(5),
        "CAH" | "CAPTUREANDHOLD" | "6" => Some(6),
        "CTF" | "CAPTURETHEFLAG" | "7" => Some(7),
        "BOMB" | "BOMBMODE" | "8" => Some(8),
        "JUMP" | "JUMPMODE" | "9" => Some(9),
        "FREEZE" | "FREEZETAG" | "10" => Some(10),
        "GUNGAME" | "GG" | "11" => Some(11),
        _ => None,
    }
}

/// Validation error from [`generate`].
#[derive(Debug, thiserror::Error)]
pub enum CfgError {
    #[error("rcon_password must not be empty")]
    EmptyRcon,
    #[error("unknown game_mode '{0}' — expected one of FFA/TDM/TS/FTL/CAH/CTF/BOMB/JUMP/FREEZE/LMS/GUNGAME")]
    UnknownGameMode(String),
    #[error("max_clients must be between 2 and 64, got {0}")]
    BadMaxClients(u16),
    #[error("rcon_password contains characters that would break cvar quoting (newline, quote)")]
    RconHasUnsafeChars,
    #[error("hostname contains characters that would break cvar quoting")]
    HostnameUnsafeChars,
}

fn contains_cfg_breaking_chars(s: &str) -> bool {
    s.contains('\n') || s.contains('\r') || s.contains('"')
}

/// Render the cfg with `params` applied. Does not touch the filesystem.
pub fn generate(params: &GameServerWizardParams, server_name: &str) -> Result<String, CfgError> {
    if params.rcon_password.is_empty() {
        return Err(CfgError::EmptyRcon);
    }
    if contains_cfg_breaking_chars(&params.rcon_password) {
        return Err(CfgError::RconHasUnsafeChars);
    }
    if contains_cfg_breaking_chars(&params.hostname) {
        return Err(CfgError::HostnameUnsafeChars);
    }
    if !(2..=64).contains(&params.max_clients) {
        return Err(CfgError::BadMaxClients(params.max_clients));
    }
    let gt = game_mode_to_gametype(&params.game_mode)
        .ok_or_else(|| CfgError::UnknownGameMode(params.game_mode.clone()))?;

    let admin_line = match params.admin_password.as_deref() {
        Some(p) if !p.is_empty() && !contains_cfg_breaking_chars(p) => {
            format!("set auth_owners                \"{}\"", p)
        }
        _ => String::from("// set auth_owners <password>  (optional)"),
    };

    // Sanitize server name for the join message (strip quotes).
    let safe_server_name = server_name.replace('"', "'");

    let mut out = DEFAULT_CFG_TEMPLATE.to_string();
    out = out.replace("{{SV_HOSTNAME}}", &params.hostname);
    out = out.replace("{{SERVER_NAME}}", &safe_server_name);
    out = out.replace("{{PORT}}", &params.port.to_string());
    out = out.replace("{{MAX_CLIENTS}}", &params.max_clients.to_string());
    out = out.replace("{{RCON_PASSWORD}}", &params.rcon_password);
    out = out.replace("{{G_GAMETYPE}}", &gt.to_string());
    out = out.replace("{{ADMIN_PASSWORD_LINE}}", &admin_line);
    Ok(out)
}

/// Paths produced by [`write_to_disk`].
#[derive(Debug, Clone)]
pub struct WrittenPaths {
    pub server_cfg: PathBuf,
    pub mapcycle: PathBuf,
}

/// Write the generated cfg and a stub mapcycle into `<install_path>/q3ut4/`.
///
/// Creates the directory if missing. Overwrites an existing `server.cfg` —
/// callers should confirm with the user before invoking.
pub fn write_to_disk(install_path: &Path, rendered_cfg: &str) -> std::io::Result<WrittenPaths> {
    let q3ut4 = install_path.join("q3ut4");
    std::fs::create_dir_all(&q3ut4)?;

    let server_cfg = q3ut4.join("server.cfg");
    std::fs::write(&server_cfg, rendered_cfg)?;
    // 0600 — server.cfg contains the rcon password.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&server_cfg, std::fs::Permissions::from_mode(0o600));
    }

    let mapcycle = q3ut4.join("mapcycle.txt");
    // Only write the stub mapcycle if one doesn't already exist — operators
    // frequently carry their own mapcycle across reinstalls.
    if !mapcycle.exists() {
        std::fs::write(&mapcycle, DEFAULT_MAPCYCLE)?;
    }

    // Ensure an empty games.log exists so the bot's log tailer has a file to
    // attach to immediately (UrT will open it in append mode).
    let games_log = q3ut4.join("games.log");
    if !games_log.exists() {
        std::fs::write(&games_log, b"")?;
    }

    Ok(WrittenPaths { server_cfg, mapcycle })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_params() -> GameServerWizardParams {
        GameServerWizardParams {
            install_path: "/tmp/urt".to_string(),
            hostname: "^1R3 ^7CTF".to_string(),
            public_ip: "1.2.3.4".to_string(),
            port: 27960,
            rcon_password: "s3cret".to_string(),
            game_mode: "CTF".to_string(),
            max_clients: 16,
            admin_password: Some("a".to_string()),
            register_systemd: false,
            slug: None,
            force_download: false,
        }
    }

    #[test]
    fn generates_cfg_with_substitutions() {
        let out = generate(&sample_params(), "test").unwrap();
        assert!(out.contains("set sv_hostname              \"^1R3 ^7CTF\""));
        assert!(out.contains("set net_port                 27960"));
        assert!(out.contains("set sv_maxclients            16"));
        assert!(out.contains("set rconPassword             \"s3cret\""));
        assert!(out.contains("set g_gametype               7"));
        // R3-required logging invariants
        assert!(out.contains("set g_logsync                1"));
        assert!(out.contains("set logfile                  2"));
        assert!(out.contains("set g_log                    \"games.log\""));
    }

    #[test]
    fn rejects_unsafe_rcon() {
        let mut p = sample_params();
        p.rcon_password = "abc\"def".to_string();
        assert!(matches!(generate(&p, "x"), Err(CfgError::RconHasUnsafeChars)));
    }

    #[test]
    fn rejects_empty_rcon() {
        let mut p = sample_params();
        p.rcon_password = String::new();
        assert!(matches!(generate(&p, "x"), Err(CfgError::EmptyRcon)));
    }

    #[test]
    fn rejects_unknown_game_mode() {
        let mut p = sample_params();
        p.game_mode = "KABOOM".to_string();
        assert!(matches!(generate(&p, "x"), Err(CfgError::UnknownGameMode(_))));
    }

    #[test]
    fn rejects_bad_max_clients() {
        let mut p = sample_params();
        p.max_clients = 100;
        assert!(matches!(generate(&p, "x"), Err(CfgError::BadMaxClients(100))));
    }

    #[test]
    fn game_mode_aliases_all_map() {
        assert_eq!(game_mode_to_gametype("CTF"), Some(7));
        assert_eq!(game_mode_to_gametype("ctf"), Some(7));
        assert_eq!(game_mode_to_gametype("TDM"), Some(3));
        assert_eq!(game_mode_to_gametype("7"), Some(7));
        assert_eq!(game_mode_to_gametype("nope"), None);
    }
}
