use async_trait::async_trait;
use tracing::info;

use crate::core::context::BotContext;
use crate::events::Event;
use crate::plugins::{Plugin, PluginInfo};

/// VPN/proxy detection plugin — kicks clients connecting from known VPN/proxy IP ranges.
pub struct VpncheckPlugin {
    enabled: bool,
    /// Blocked IP ranges as (start, end) pairs of u32-encoded IPv4 addresses.
    blocked_ranges: Vec<(u32, u32)>,
    kick_reason: String,
}

impl VpncheckPlugin {
    pub fn new() -> Self {
        Self {
            enabled: true,
            blocked_ranges: Vec::new(),
            kick_reason: "VPN/Proxy connections are not allowed on this server.".to_string(),
        }
    }

    /// Add a blocked IP range. Start and end are u32-encoded IPv4 addresses.
    pub fn add_blocked_range(&mut self, start: u32, end: u32) {
        self.blocked_ranges.push((start, end));
    }

    /// Convert a dotted-quad IPv4 string to a u32.
    fn ip_to_u32(ip: &str) -> Option<u32> {
        let parts: Vec<&str> = ip.split('.').collect();
        if parts.len() != 4 {
            return None;
        }
        let a = parts[0].parse::<u32>().ok()?;
        let b = parts[1].parse::<u32>().ok()?;
        let c = parts[2].parse::<u32>().ok()?;
        let d = parts[3].parse::<u32>().ok()?;
        if a > 255 || b > 255 || c > 255 || d > 255 {
            return None;
        }
        Some((a << 24) | (b << 16) | (c << 8) | d)
    }

    /// Check if an IP address falls within any blocked range.
    fn is_blocked(&self, ip: &str) -> bool {
        let Some(ip_num) = Self::ip_to_u32(ip) else {
            return false;
        };
        self.blocked_ranges
            .iter()
            .any(|&(start, end)| ip_num >= start && ip_num <= end)
    }
}

impl Default for VpncheckPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Plugin for VpncheckPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "vpncheck",
            description: "Detects and kicks clients connecting from VPN/proxy IP ranges",
            requires_config: true,
            requires_plugins: &[],
            requires_parsers: &[],
            requires_storage: &[],
            load_after: &[],
        }
    }

    async fn on_load_config(&mut self, settings: Option<&toml::Table>) -> anyhow::Result<()> {
        if let Some(s) = settings {
            if let Some(v) = s.get("kick_reason").and_then(|v| v.as_str()) {
                self.kick_reason = v.to_string();
            }
            if let Some(arr) = s.get("blocked_ranges").and_then(|v| v.as_array()) {
                self.blocked_ranges.clear();
                for item in arr {
                    if let Some(range_str) = item.as_str() {
                        // Support "start-end" format with dotted-quad IPs
                        if let Some((start_str, end_str)) = range_str.split_once('-') {
                            if let (Some(start), Some(end)) = (Self::ip_to_u32(start_str.trim()), Self::ip_to_u32(end_str.trim())) {
                                self.blocked_ranges.push((start, end));
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    async fn on_startup(&mut self) -> anyhow::Result<()> {
        info!(
            blocked_ranges = self.blocked_ranges.len(),
            "Vpncheck plugin started"
        );
        Ok(())
    }

    async fn on_event(&self, event: &Event, ctx: &BotContext) -> anyhow::Result<()> {
        let Some(event_key) = ctx.event_registry.get_key(event.event_type) else {
            return Ok(());
        };

        match event_key {
            "EVT_CLIENT_AUTH" => {
                let Some(client_id) = event.client_id else {
                    return Ok(());
                };

                let cid_str = client_id.to_string();
                let Some(client) = ctx.clients.get_by_cid(&cid_str).await else {
                    return Ok(());
                };

                if let Some(ip) = client.ip {
                    let ip_str = ip.to_string();

                    if self.is_blocked(&ip_str) {
                        info!(
                            client = client_id,
                            name = %client.name,
                            ip = %ip_str,
                            "VPN/proxy detected, kicking client"
                        );
                        ctx.kick(&cid_str, &self.kick_reason).await?;
                    }
                }
            }
            _ => {}
        }

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
        Some(vec!["EVT_CLIENT_AUTH".to_string()])
    }
}
