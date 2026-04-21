//! External `.pk3` map repository scraper + cache refresher.
//!
//! Runs on the master only. Scrapes each configured HTML autoindex (e.g.
//! Apache/nginx `mod_autoindex`), parses filename / size / mtime from the
//! rows, and upserts them into `map_repo_entries`.
//!
//! The cache is queried by the `/api/v1/map-repo` endpoints to drive the
//! admin UI's map browser. Imports onto a game server are handled by the
//! sync layer (see `crate::sync::handlers::handle_download_map_pk3`).

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use regex::Regex;
use tracing::{debug, info, warn};

use crate::core::MapRepoEntry;
use crate::storage::Storage;

pub mod builtin_defaults;

/// Lightweight summary of the last refresh pass.
#[derive(Debug, Clone, Default)]
pub struct RefreshStats {
    pub sources_ok: u32,
    pub sources_failed: u32,
    pub entries_upserted: u64,
    pub entries_pruned: u64,
}

/// Fetch and parse a single autoindex URL, returning all `.pk3` entries.
pub async fn fetch_index(
    http: &reqwest::Client,
    source_url: &str,
) -> anyhow::Result<Vec<MapRepoEntry>> {
    let base = if source_url.ends_with('/') {
        source_url.to_string()
    } else {
        format!("{}/", source_url)
    };
    let resp = http.get(&base).send().await?;
    if !resp.status().is_success() {
        anyhow::bail!("HTTP {} fetching {}", resp.status(), base);
    }
    let body = resp.text().await?;
    let now = Utc::now();

    // Apache/nginx row regex. Captures:
    //   1: filename (ending in .pk3, case-insensitive)
    //   2: optional date (e.g. 2024-05-01 12:30 or 01-May-2024 12:30)
    //   3: optional size (digits with optional K/M/G suffix, or '-')
    //
    // Example matches:
    //   <a href="ut4_turnpike.pk3">ut4_turnpike.pk3</a>             2024-05-01 12:30   12M
    //   <a href="ut4_abbey.pk3">ut4_abbey.pk3</a>                   01-May-2024 12:30  12345
    let row_re = Regex::new(
        r#"(?i)<a\s+href="([^"?#/][^"?#/]*\.pk3)"[^>]*>[^<]*</a>\s*([^<\r\n]*)"#,
    )?;
    let meta_re = Regex::new(
        r#"(?i)(\d{4}-\d{2}-\d{2}\s+\d{2}:\d{2}(?::\d{2})?|\d{1,2}-[A-Za-z]{3}-\d{4}\s+\d{2}:\d{2})\s+([\d\.]+[KMG]?|-)"#,
    )?;

    let mut out = Vec::new();
    for cap in row_re.captures_iter(&body) {
        let filename = cap.get(1).map(|m| m.as_str()).unwrap_or("").trim();
        if filename.is_empty() || filename.contains('/') || filename.contains('\\') {
            continue;
        }
        let tail = cap.get(2).map(|m| m.as_str()).unwrap_or("");
        let (mtime, size) = if let Some(mc) = meta_re.captures(tail) {
            let mt = mc.get(1).map(|m| m.as_str().to_string());
            let sz_str = mc.get(2).map(|m| m.as_str()).unwrap_or("-");
            (mt, parse_size(sz_str))
        } else {
            (None, None)
        };

        let file_url = format!("{}{}", base, filename);
        out.push(MapRepoEntry {
            filename: filename.to_string(),
            size,
            mtime,
            source_url: file_url,
            last_seen_at: now,
        });
    }

    // De-dup by filename (autoindex sometimes has double links with icons).
    out.sort_by(|a, b| a.filename.cmp(&b.filename));
    out.dedup_by(|a, b| a.filename == b.filename);
    Ok(out)
}

/// Parse a size token like `12M`, `1.5G`, `12345`, or `-` into bytes.
fn parse_size(s: &str) -> Option<i64> {
    let s = s.trim();
    if s == "-" || s.is_empty() {
        return None;
    }
    let (num_part, mult): (&str, i64) = if let Some(rest) = s.strip_suffix(|c: char| c.is_ascii_alphabetic()) {
        let suffix = s.chars().last().unwrap_or(' ').to_ascii_uppercase();
        let mult = match suffix {
            'K' => 1024,
            'M' => 1024 * 1024,
            'G' => 1024 * 1024 * 1024,
            _ => 1,
        };
        (rest, mult)
    } else {
        (s, 1)
    };
    let num: f64 = num_part.parse().ok()?;
    Some((num * mult as f64) as i64)
}

/// Refresh the cache from every configured source.
pub async fn refresh_all(
    storage: Arc<dyn Storage>,
    sources: &[String],
) -> RefreshStats {
    let mut stats = RefreshStats::default();
    if sources.is_empty() {
        return stats;
    }
    let http = match reqwest::Client::builder()
        .user_agent("r3-bot/map-repo")
        .timeout(Duration::from_secs(60))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            warn!(error = %e, "map-repo: failed to build HTTP client");
            return stats;
        }
    };

    let started = Utc::now();
    let mut all_entries: Vec<MapRepoEntry> = Vec::new();
    for src in sources {
        match fetch_index(&http, src).await {
            Ok(mut entries) => {
                info!(source = %src, count = entries.len(), "map-repo: index fetched");
                stats.sources_ok += 1;
                all_entries.append(&mut entries);
            }
            Err(e) => {
                warn!(source = %src, error = %e, "map-repo: index fetch failed");
                stats.sources_failed += 1;
            }
        }
    }

    // Collapse duplicates across mirrors — "last seen wins" on source_url.
    all_entries.sort_by(|a, b| a.filename.cmp(&b.filename));
    all_entries.dedup_by(|a, b| a.filename == b.filename);

    match storage.upsert_map_repo_entries(&all_entries).await {
        Ok(n) => {
            stats.entries_upserted = n;
        }
        Err(e) => {
            warn!(error = %e, "map-repo: upsert failed");
            return stats;
        }
    }

    // Prune entries not seen in this pass (only if at least one source
    // succeeded — otherwise we'd wipe the cache on transient outages).
    if stats.sources_ok > 0 {
        match storage.prune_map_repo_entries(started).await {
            Ok(n) => {
                stats.entries_pruned = n;
                if n > 0 {
                    debug!(pruned = n, "map-repo: pruned stale entries");
                }
            }
            Err(e) => warn!(error = %e, "map-repo: prune failed"),
        }
    }
    info!(
        ok = stats.sources_ok,
        failed = stats.sources_failed,
        upserted = stats.entries_upserted,
        pruned = stats.entries_pruned,
        "map-repo: refresh complete"
    );
    stats
}

/// Spawn the background refresh task. Returns immediately.
pub fn spawn_refresher(
    storage: Arc<dyn Storage>,
    sources: Vec<String>,
    interval_hours: u32,
) {
    if sources.is_empty() {
        info!("map-repo: no sources configured, refresher disabled");
        return;
    }
    tokio::spawn(async move {
        // Small delay on startup so boot-time work isn't blocked on network.
        tokio::time::sleep(Duration::from_secs(30)).await;
        loop {
            let _ = refresh_all(storage.clone(), &sources).await;
            if interval_hours == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_secs(interval_hours as u64 * 3600)).await;
        }
    });
}
