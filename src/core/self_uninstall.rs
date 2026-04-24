//! Self-uninstall helper used by client and hub modes.
//!
//! When the master deletes a server or hub row it also wants the remote
//! host to fully clean itself up (systemd unit + install dir +
//! optionally the UrT game server). We fetch the master's published
//! `uninstall-r3.sh` and run it via `systemd-run --no-block --collect`
//! so the transient unit survives *our* own process being stopped by
//! `systemctl stop`. On hosts without `sudo systemd-run` privileges we
//! fall back to `nohup setsid bash` — good enough for clean shutdowns
//! but will be killed if systemd reaps our cgroup.

use std::fs;
use std::process::{Command, Stdio};

use tracing::{info, warn};

/// Detect the systemd unit hosting this process by inspecting
/// `/proc/self/cgroup`. Returns e.g. `r3.service` or
/// `r3-client@my-server.service`. Returns `None` when we're not running
/// under systemd (dev shells, containers, etc.).
pub fn detect_unit_name() -> Option<String> {
    let raw = fs::read_to_string("/proc/self/cgroup").ok()?;
    // Typical cgroup-v2 line: `0::/system.slice/r3-client@foo.service`
    for line in raw.lines() {
        if let Some(path) = line.rsplit(':').next() {
            for seg in path.rsplit('/') {
                if seg.ends_with(".service") {
                    return Some(seg.to_string());
                }
            }
        }
    }
    None
}

/// Spawn the uninstaller as a transient systemd unit and return
/// immediately. The caller should log the result and exit.
///
/// * `update_url` — base URL (matches `config.update.url`), e.g.
///   `https://r3.pugbot.net/api/updates`. We append `/uninstall-r3.sh`.
/// * `all` — if true pass `--all` (hub mode: blow everything away).
///   Otherwise pass the detected unit name so only this instance is
///   removed.
/// * `remove_gameserver` — when true pass `--remove-gameserver`.
pub fn trigger(update_url: &str, all: bool, remove_gameserver: bool) -> anyhow::Result<String> {
    let base = update_url.trim_end_matches('/');
    let uninstaller_url = format!("{}/uninstall-r3.sh", base);
    let script_path = "/tmp/r3-uninstall.sh";
    let log_path = "/tmp/r3-self-uninstall.log";

    // Build the uninstaller argv as a shell string.
    let mut args = String::new();
    if all {
        args.push_str("--all ");
    } else if let Some(unit) = detect_unit_name() {
        // Pass just the short unit name (uninstall-r3.sh strips
        // `.service` internally).
        let short = unit.strip_suffix(".service").unwrap_or(&unit);
        args.push_str(short);
        args.push(' ');
    } else {
        anyhow::bail!(
            "Cannot detect systemd unit for this process and --all not set; refusing to self-uninstall"
        );
    }
    args.push_str("-y");
    if remove_gameserver {
        args.push_str(" --remove-gameserver");
    }

    // The inner shell:
    //   1. Give our parent process a brief moment to send any final
    //      response back to the master before we pull the rug.
    //   2. Download the uninstaller from the master's update endpoint.
    //   3. Run it, teeing output to a log for post-mortem debugging.
    let inner = format!(
        "sleep 3; curl -fsSL {url} -o {script} && chmod +x {script} && bash {script} {args} >> {log} 2>&1",
        url = shell_escape(&uninstaller_url),
        script = script_path,
        args = args,
        log = log_path,
    );

    let pid = std::process::id();
    let unit_name = format!("r3-self-uninstall-{}.service", pid);

    // Preferred path: sudo systemd-run transient unit.
    let status = Command::new("sudo")
        .args([
            "-n",
            "systemd-run",
            "--no-block",
            "--collect",
            "--unit",
            &unit_name,
            "/bin/bash",
            "-c",
            &inner,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    match status {
        Ok(s) if s.success() => {
            info!(unit = %unit_name, "Self-uninstall transient unit dispatched");
            return Ok(format!(
                "Self-uninstall scheduled as {} (log: {})",
                unit_name, log_path
            ));
        }
        Ok(s) => {
            warn!(
                exit = ?s.code(),
                "sudo systemd-run failed; falling back to nohup setsid"
            );
        }
        Err(e) => {
            warn!(error = %e, "sudo systemd-run not available; falling back to nohup setsid");
        }
    }

    // Fallback: double-fork via setsid so the child survives our own exit.
    // Will still be killed if systemd tears down our cgroup, but on hosts
    // without the sudoers drop-in this is the best we can do.
    let fallback_cmd = format!(
        "nohup setsid bash -c {inner} < /dev/null > /dev/null 2>&1 &",
        inner = shell_escape(&inner),
    );
    Command::new("/bin/sh")
        .args(["-c", &fallback_cmd])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| anyhow::anyhow!("nohup fallback failed: {}", e))?;

    Ok(format!("Self-uninstall dispatched via fallback (log: {})", log_path))
}

/// Minimal POSIX single-quote escaping for use inside `bash -c '…'`.
fn shell_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        if c == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}
