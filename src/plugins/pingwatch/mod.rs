use async_trait::async_trait;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::core::context::BotContext;
use crate::events::Event;
use crate::plugins::{Plugin, PluginInfo};

/// The PingWatch plugin — monitors player pings and kicks high-ping players.
/// Checks pings periodically and warns/kicks players above the threshold.
pub struct PingWatchPlugin {
    enabled: bool,
    max_ping: u32,
    warn_threshold: u32,
    max_warnings: u32,
    /// Per-client warning counts.
    warnings: RwLock<HashMap<i64, u32>>,
}

impl PingWatchPlugin {
    pub fn new() -> Self {
        Self {
            enabled: true,
            max_ping: 250,
            warn_threshold: 200,
            max_warnings: 3,
            warnings: RwLock::new(HashMap::new()),
        }
    }

    pub fn with_max_ping(mut self, max: u32) -> Self {
        self.max_ping = max;
        self
    }

    /// Run the periodic ping check. Call this from a spawned task.
    pub async fn check_pings(&self, ctx: &BotContext) -> anyhow::Result<()> {
        // Get status output which typically includes ping info
        let status = ctx.rcon.send("status").await?;

        for line in status.lines() {
            let fields: Vec<&str> = line.split_whitespace().collect();
            // Typical status line: "num score ping guid name lastmsg address qport rate"
            if fields.len() >= 5 {
                if let (Ok(cid), Ok(ping)) = (fields[0].parse::<i64>(), fields[2].parse::<u32>()) {
                    if ping >= self.max_ping {
                        let (count, should_kick) = {
                            let mut warnings = self.warnings.write().await;
                            let count = warnings.entry(cid).or_insert(0);
                            *count += 1;
                            (*count, *count >= self.max_warnings)
                        };

                        if should_kick {
                            warn!(cid = cid, ping = ping, "Kicking high-ping player");
                            ctx.kick(
                                &cid.to_string(),
                                &format!("High ping ({}ms > {}ms limit)", ping, self.max_ping),
                            )
                            .await?;
                        } else {
                            info!(cid = cid, ping = ping, warns = count, "High ping warning");
                            ctx.message(
                                &cid.to_string(),
                                &format!(
                                    "^3Warning: Your ping ({}ms) is too high. Max: {}ms ({}/{})",
                                    ping, self.max_ping, count, self.max_warnings
                                ),
                            )
                            .await?;
                        }
                    } else if ping < self.warn_threshold {
                        // Ping recovered — reset warnings
                        let mut warnings = self.warnings.write().await;
                        warnings.remove(&cid);
                    }
                }
            }
        }

        Ok(())
    }
}

impl Default for PingWatchPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Plugin for PingWatchPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "pingwatch",
            description: "Monitors and kicks high-ping players",
            requires_config: false,
            requires_plugins: &[],
            requires_parsers: &[],
            requires_storage: &[],
            load_after: &[],
        }
    }

    async fn on_startup(&mut self) -> anyhow::Result<()> {
        info!(max_ping = self.max_ping, "PingWatch plugin started");
        Ok(())
    }

    async fn on_event(&self, _event: &Event, _ctx: &BotContext) -> anyhow::Result<()> {
        // PingWatch works via periodic polling (check_pings), not events.
        // It's called from a spawned timer task in main.
        Ok(())
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn on_enable(&mut self) {
        self.enabled = true;
    }

    fn on_disable(&mut self) {
        self.enabled = false;
    }

    fn subscribed_events(&self) -> Option<Vec<String>> {
        Some(vec![]) // No event subscriptions — runs on its own timer
    }
}
