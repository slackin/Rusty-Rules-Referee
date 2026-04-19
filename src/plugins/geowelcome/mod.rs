use async_trait::async_trait;
use tracing::{info, warn};

use crate::core::context::BotContext;
use crate::events::Event;
use crate::plugins::{Plugin, PluginInfo};

/// The GeoWelcome plugin — greets players with their country when they connect.
/// Uses a GeoIP service to look up the player's country from their IP address
/// and announces it to the server.
pub struct GeowelcomePlugin {
    enabled: bool,
    /// Message template. Variables: $name, $country, $country_code
    welcome_message: String,
    /// Whether to announce to the whole server or just the player
    announce_public: bool,
    /// GeoIP API URL template. $ip will be replaced with the player's IP.
    geoip_api_url: String,
    /// HTTP client
    client: reqwest::Client,
}

impl GeowelcomePlugin {
    pub fn new() -> Self {
        Self {
            enabled: true,
            welcome_message: "^7Player ^2$name ^7connected from ^3$country".to_string(),
            announce_public: true,
            geoip_api_url: "http://ip-api.com/json/$ip?fields=status,country,countryCode".to_string(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(3))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
        }
    }

    /// Look up country from IP via HTTP API.
    async fn lookup_country(&self, ip: &str) -> Option<(String, String)> {
        // Skip private/local IPs
        if ip.starts_with("127.") || ip.starts_with("10.") || ip.starts_with("192.168.") || ip.starts_with("0.") {
            return None;
        }

        let url = self.geoip_api_url.replace("$ip", ip);
        match self.client.get(&url).send().await {
            Ok(resp) => {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    if json.get("status").and_then(|v| v.as_str()) == Some("success") {
                        let country = json
                            .get("country")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Unknown")
                            .to_string();
                        let code = json
                            .get("countryCode")
                            .and_then(|v| v.as_str())
                            .unwrap_or("??")
                            .to_string();
                        return Some((country, code));
                    }
                }
                None
            }
            Err(e) => {
                warn!(error = %e, ip = ip, "GeoIP lookup failed");
                None
            }
        }
    }

    fn format_message(&self, name: &str, country: &str, country_code: &str) -> String {
        self.welcome_message
            .replace("$name", name)
            .replace("$country", country)
            .replace("$country_code", country_code)
    }
}

impl Default for GeowelcomePlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Plugin for GeowelcomePlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "geowelcome",
            description: "Greets players with their country on connect",
            requires_config: false,
            requires_plugins: &[],
            requires_parsers: &[],
            requires_storage: &[],
            load_after: &["welcome"],
        }
    }

    async fn on_load_config(&mut self, settings: Option<&toml::Table>) -> anyhow::Result<()> {
        if let Some(s) = settings {
            if let Some(v) = s.get("welcome_message").and_then(|v| v.as_str()) {
                self.welcome_message = v.to_string();
            }
            if let Some(v) = s.get("announce_public").and_then(|v| v.as_bool()) {
                self.announce_public = v;
            }
            if let Some(v) = s.get("geoip_api_url").and_then(|v| v.as_str()) {
                self.geoip_api_url = v.to_string();
            }
        }
        Ok(())
    }

    async fn on_startup(&mut self) -> anyhow::Result<()> {
        info!(
            announce_public = self.announce_public,
            "GeoWelcome plugin started"
        );
        Ok(())
    }

    async fn on_event(&self, event: &Event, ctx: &BotContext) -> anyhow::Result<()> {
        let Some(client_id) = event.client_id else {
            return Ok(());
        };

        // Only handle auth events
        if let Some(key) = ctx.event_registry.get_key(event.event_type) {
            if key != "EVT_CLIENT_AUTH" {
                return Ok(());
            }
        }

        let cid_str = client_id.to_string();

        // Get client info for name and IP
        if let Some(client) = ctx.clients.get_by_cid(&cid_str).await {
            if let Some(ref ip) = client.ip {
                let ip_str = ip.to_string();
                if let Some((country, code)) = self.lookup_country(&ip_str).await {
                    let msg = self.format_message(&client.name, &country, &code);
                    if self.announce_public {
                        ctx.say(&msg).await?;
                    } else {
                        ctx.message(&cid_str, &msg).await?;
                    }
                    info!(
                        client = client_id,
                        name = %client.name,
                        country = %country,
                        "GeoWelcome: announced player country"
                    );
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
        Some(vec!["EVT_CLIENT_AUTH".to_string()])
    }
}
