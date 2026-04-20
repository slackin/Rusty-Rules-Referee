// Build script for Rusty Rules Referee
// Embeds a unique build hash into every compiled binary.
// Format: {version}-{git_short}-{timestamp}  e.g. "2.0.0-a1b2c3d4-20260419120000"

use std::process::Command;
use std::time::SystemTime;

fn main() {
    // Get version from Cargo.toml
    let version = env!("CARGO_PKG_VERSION");

    // Get git short commit hash (8 chars)
    let git_hash = Command::new("git")
        .args(["rev-parse", "--short=8", "HEAD"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout).ok().map(|s| s.trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "nogit".to_string());

    // Get build timestamp as YYYYMMDDHHMMSS (UTC)
    let timestamp = {
        let dur = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();
        let secs = dur.as_secs();
        // Manual UTC breakdown (no chrono at build time)
        let days = secs / 86400;
        let time_of_day = secs % 86400;
        let hours = time_of_day / 3600;
        let minutes = (time_of_day % 3600) / 60;
        let seconds = time_of_day % 60;

        // Days since epoch to Y/M/D
        let mut y = 1970i64;
        let mut remaining = days as i64;
        loop {
            let days_in_year = if is_leap(y) { 366 } else { 365 };
            if remaining < days_in_year {
                break;
            }
            remaining -= days_in_year;
            y += 1;
        }
        let leap = is_leap(y);
        let month_days: [i64; 12] = [
            31,
            if leap { 29 } else { 28 },
            31, 30, 31, 30, 31, 31, 30, 31, 30, 31,
        ];
        let mut m = 0usize;
        for md in &month_days {
            if remaining < *md {
                break;
            }
            remaining -= *md;
            m += 1;
        }
        let d = remaining + 1;
        format!(
            "{:04}{:02}{:02}{:02}{:02}{:02}",
            y,
            m + 1,
            d,
            hours,
            minutes,
            seconds
        )
    };

    let build_hash = format!("{}-{}-{}", version, git_hash, timestamp);

    println!("cargo:rustc-env=BUILD_HASH={}", build_hash);
    println!("cargo:rustc-env=GIT_COMMIT={}", git_hash);
    println!("cargo:rustc-env=BUILD_TIMESTAMP={}", timestamp);

    // Rebuild when git HEAD changes or on explicit force
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/heads/");
    println!("cargo:rerun-if-env-changed=FORCE_REBUILD");
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}
