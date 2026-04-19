use async_trait::async_trait;
use std::collections::HashSet;
use tracing::{info, warn};

use crate::core::context::BotContext;
use crate::events::Event;
use crate::plugins::{Plugin, PluginInfo};

/// The CountryFilter plugin — allows or blocks players based on their IP's country.
/// Uses GeoIP lookup to determine the player's country from their IP address.
///
/// Operates in one of two modes:
/// - **Allowlist**: Only players from listed countries can join.
/// - **Blocklist**: Players from listed countries are kicked.
pub struct CountryFilterPlugin {
    enabled: bool,
    mode: FilterMode,
    countries: HashSet<String>,
    kick_message: String,
}

#[derive(Debug, Clone, Copy)]
enum FilterMode {
    Allowlist,
    Blocklist,
}

impl CountryFilterPlugin {
    pub fn new() -> Self {
        Self {
            enabled: true,
            mode: FilterMode::Blocklist,
            countries: HashSet::new(),
            kick_message: "Your country is not allowed on this server.".to_string(),
        }
    }

    pub fn with_blocklist(mut self, countries: &[&str]) -> Self {
        self.mode = FilterMode::Blocklist;
        self.countries = countries.iter().map(|s| s.to_uppercase()).collect();
        self
    }

    pub fn with_allowlist(mut self, countries: &[&str]) -> Self {
        self.mode = FilterMode::Allowlist;
        self.countries = countries.iter().map(|s| s.to_uppercase()).collect();
        self
    }

    fn should_kick(&self, country_code: &str) -> bool {
        let code = country_code.to_uppercase();
        match self.mode {
            FilterMode::Blocklist => self.countries.contains(&code),
            FilterMode::Allowlist => !self.countries.is_empty() && !self.countries.contains(&code),
        }
    }

    /// Look up country from IP address.
    /// Placeholder — in production, use the `maxminddb` crate with a GeoLite2 database.
    fn lookup_country(&self, ip: &str) -> Option<String> {
        // Placeholder: return None (no GeoIP database loaded).
        // To enable, add `maxminddb` crate and load a GeoLite2-Country.mmdb file.
        // Example:
        //   let reader = maxminddb::Reader::open_readfile("GeoLite2-Country.mmdb")?;
        //   let result: geoip2::Country = reader.lookup(ip.parse()?)?;
        //   result.country.and_then(|c| c.iso_code).map(|s| s.to_string())
        let _ = ip;
        None
    }
}

impl Default for CountryFilterPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Plugin for CountryFilterPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "countryfilter",
            description: "Filters players by country using GeoIP",
            requires_config: true,
            requires_plugins: &[],
            requires_parsers: &[],
            requires_storage: &[],
            load_after: &[],
        }
    }

    async fn on_load_config(&mut self, settings: Option<&toml::Table>) -> anyhow::Result<()> {
        if let Some(s) = settings {
            if let Some(v) = s.get("mode").and_then(|v| v.as_str()) {
                self.mode = match v {
                    "allowlist" => FilterMode::Allowlist,
                    _ => FilterMode::Blocklist,
                };
            }
            if let Some(v) = s.get("kick_message").and_then(|v| v.as_str()) {
                self.kick_message = v.to_string();
            }
            if let Some(arr) = s.get("countries").and_then(|v| v.as_array()) {
                self.countries = arr.iter().filter_map(|v| v.as_str().map(|s| s.to_uppercase())).collect();
            }
        }
        Ok(())
    }

    async fn on_startup(&mut self) -> anyhow::Result<()> {
        info!(
            mode = ?self.mode,
            countries = self.countries.len(),
            "CountryFilter plugin started"
        );
        Ok(())
    }

    async fn on_event(&self, event: &Event, ctx: &BotContext) -> anyhow::Result<()> {
        let Some(client_id) = event.client_id else {
            return Ok(());
        };

        // Try to get the client's IP from storage
        if let Ok(client) = ctx.storage.get_client(client_id).await {
            if let Some(ip) = client.ip {
                if let Some(country) = self.lookup_country(&ip.to_string()) {
                    if self.should_kick(&country) {
                        warn!(
                            client = client_id,
                            country = %country,
                            ip = %ip,
                            "Player blocked by country filter"
                        );
                        ctx.message(&client_id.to_string(), &self.kick_message).await?;
                        ctx.kick(&client_id.to_string(), &format!("Country filter: {}", country)).await?;
                    } else {
                        info!(client = client_id, country = %country, "Country filter passed");
                    }
                }
            }
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
        Some(vec!["EVT_CLIENT_CONNECT".to_string()])
    }
}
