use async_trait::async_trait;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use crate::core::context::BotContext;
use crate::events::{Event, EventData};
use crate::plugins::{Plugin, PluginInfo};

/// The Discord plugin — relays game events to Discord via webhooks.
/// Combines the functionality of B3's "discord" and "b3todiscord" plugins.
///
/// Supports multiple webhook channels:
/// - `chat_webhook_url`: Chat messages from in-game
/// - `admin_webhook_url`: Admin actions (kicks, bans, warnings)
/// - `events_webhook_url`: Game events (map changes, joins, kills)
/// - `webhook_url`: Fallback for all events if specific ones aren't set
pub struct DiscordPlugin {
    enabled: bool,
    /// Fallback webhook URL for all events
    webhook_url: String,
    /// Dedicated chat webhook URL (overrides fallback for chat events)
    chat_webhook_url: String,
    /// Dedicated admin actions webhook URL
    admin_webhook_url: String,
    /// Dedicated game events webhook URL
    events_webhook_url: String,
    /// Bot display name in Discord
    bot_name: String,
    /// Whether to relay player chat messages
    relay_chat: bool,
    /// Whether to relay kill events
    relay_kills: bool,
    /// Whether to relay join/disconnect events
    relay_connections: bool,
    /// Whether to relay admin actions (kicks/bans/warnings)
    relay_admin_actions: bool,
    /// Whether to relay map changes
    relay_map_changes: bool,
    /// HTTP client for webhook requests
    client: reqwest::Client,
    /// Rate-limit tracker: webhook URL -> last send timestamp
    last_send: RwLock<HashMap<String, std::time::Instant>>,
    /// Minimum interval between messages to the same webhook (ms)
    rate_limit_ms: u64,
}

impl DiscordPlugin {
    pub fn new() -> Self {
        Self {
            enabled: true,
            webhook_url: String::new(),
            chat_webhook_url: String::new(),
            admin_webhook_url: String::new(),
            events_webhook_url: String::new(),
            bot_name: "R3 Bot".to_string(),
            relay_chat: true,
            relay_kills: false,
            relay_connections: true,
            relay_admin_actions: true,
            relay_map_changes: true,
            client: reqwest::Client::new(),
            last_send: RwLock::new(HashMap::new()),
            rate_limit_ms: 1000,
        }
    }

    /// Strip UrT color codes (^0 through ^9, ^x, etc.) from text for Discord display.
    fn strip_colors(text: &str) -> String {
        let mut result = String::with_capacity(text.len());
        let mut chars = text.chars();
        while let Some(c) = chars.next() {
            if c == '^' {
                // Skip the next character (color code)
                chars.next();
            } else {
                result.push(c);
            }
        }
        result
    }

    /// Get the appropriate webhook URL for a given event category.
    fn webhook_for(&self, category: &str) -> &str {
        let specific = match category {
            "chat" => &self.chat_webhook_url,
            "admin" => &self.admin_webhook_url,
            "events" => &self.events_webhook_url,
            _ => "",
        };
        if specific.is_empty() {
            &self.webhook_url
        } else {
            specific
        }
    }

    /// Send a message to a Discord webhook with rate limiting.
    async fn send_webhook(&self, category: &str, content: &str) {
        let url = self.webhook_for(category);
        if url.is_empty() {
            return;
        }

        // Rate limiting
        {
            let last = self.last_send.read().await;
            if let Some(ts) = last.get(url) {
                if ts.elapsed().as_millis() < self.rate_limit_ms as u128 {
                    return; // Too fast, skip
                }
            }
        }

        let payload = serde_json::json!({
            "username": self.bot_name,
            "content": content,
        });

        match self.client.post(url).json(&payload).send().await {
            Ok(resp) => {
                if !resp.status().is_success() {
                    warn!(
                        status = %resp.status(),
                        category = category,
                        "Discord webhook returned non-success status"
                    );
                }
            }
            Err(e) => {
                error!(error = %e, category = category, "Failed to send Discord webhook");
            }
        }

        // Update last send time
        let mut last = self.last_send.write().await;
        last.insert(url.to_string(), std::time::Instant::now());
    }
}

impl Default for DiscordPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Plugin for DiscordPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "discord",
            description: "Relays game events to Discord via webhooks",
            requires_config: true,
            requires_plugins: &[],
            requires_parsers: &[],
            requires_storage: &[],
            load_after: &["admin"],
        }
    }

    async fn on_load_config(&mut self, settings: Option<&toml::Table>) -> anyhow::Result<()> {
        if let Some(s) = settings {
            if let Some(v) = s.get("webhook_url").and_then(|v| v.as_str()) {
                self.webhook_url = v.to_string();
            }
            if let Some(v) = s.get("chat_webhook_url").and_then(|v| v.as_str()) {
                self.chat_webhook_url = v.to_string();
            }
            if let Some(v) = s.get("admin_webhook_url").and_then(|v| v.as_str()) {
                self.admin_webhook_url = v.to_string();
            }
            if let Some(v) = s.get("events_webhook_url").and_then(|v| v.as_str()) {
                self.events_webhook_url = v.to_string();
            }
            if let Some(v) = s.get("bot_name").and_then(|v| v.as_str()) {
                self.bot_name = v.to_string();
            }
            if let Some(v) = s.get("relay_chat").and_then(|v| v.as_bool()) {
                self.relay_chat = v;
            }
            if let Some(v) = s.get("relay_kills").and_then(|v| v.as_bool()) {
                self.relay_kills = v;
            }
            if let Some(v) = s.get("relay_connections").and_then(|v| v.as_bool()) {
                self.relay_connections = v;
            }
            if let Some(v) = s.get("relay_admin_actions").and_then(|v| v.as_bool()) {
                self.relay_admin_actions = v;
            }
            if let Some(v) = s.get("relay_map_changes").and_then(|v| v.as_bool()) {
                self.relay_map_changes = v;
            }
            if let Some(v) = s.get("rate_limit_ms").and_then(|v| v.as_integer()) {
                self.rate_limit_ms = v as u64;
            }
        }

        if self.webhook_url.is_empty()
            && self.chat_webhook_url.is_empty()
            && self.admin_webhook_url.is_empty()
            && self.events_webhook_url.is_empty()
        {
            warn!("Discord plugin: no webhook URLs configured — messages will not be sent");
        }

        Ok(())
    }

    async fn on_startup(&mut self) -> anyhow::Result<()> {
        info!(
            fallback = !self.webhook_url.is_empty(),
            chat = !self.chat_webhook_url.is_empty(),
            admin = !self.admin_webhook_url.is_empty(),
            events = !self.events_webhook_url.is_empty(),
            "Discord plugin started"
        );
        Ok(())
    }

    async fn on_event(&self, event: &Event, ctx: &BotContext) -> anyhow::Result<()> {
        let evt_key = ctx
            .event_registry
            .get_key(event.event_type)
            .unwrap_or("UNKNOWN");

        match evt_key {
            // --- Chat events ---
            "EVT_CLIENT_SAY" | "EVT_CLIENT_TEAM_SAY" if self.relay_chat => {
                if let EventData::Text(ref text) = event.data {
                    if let Some(cid) = event.client_id {
                        let name = ctx
                            .clients
                            .get_by_id(cid)
                            .await
                            .map(|c| Self::strip_colors(&c.name))
                            .unwrap_or_else(|| format!("Player#{}", cid));
                        let channel_tag = if evt_key == "EVT_CLIENT_TEAM_SAY" {
                            "[TEAM] "
                        } else {
                            ""
                        };
                        let clean_text = Self::strip_colors(text);
                        self.send_webhook(
                            "chat",
                            &format!("{}**{}**: {}", channel_tag, name, clean_text),
                        )
                        .await;
                    }
                }
            }

            // --- Connection events ---
            "EVT_CLIENT_AUTH" if self.relay_connections => {
                if let Some(cid) = event.client_id {
                    let name = ctx
                        .clients
                        .get_by_id(cid)
                        .await
                        .map(|c| Self::strip_colors(&c.name))
                        .unwrap_or_else(|| format!("Player#{}", cid));
                    self.send_webhook("events", &format!("📥 **{}** connected", name))
                        .await;
                }
            }

            "EVT_CLIENT_DISCONNECT" if self.relay_connections => {
                if let Some(cid) = event.client_id {
                    let name = ctx
                        .clients
                        .get_by_id(cid)
                        .await
                        .map(|c| Self::strip_colors(&c.name))
                        .unwrap_or_else(|| format!("Player#{}", cid));
                    self.send_webhook("events", &format!("📤 **{}** disconnected", name))
                        .await;
                }
            }

            // --- Kill events ---
            "EVT_CLIENT_KILL" if self.relay_kills => {
                if let EventData::Kill {
                    ref weapon,
                    ref hit_location,
                    ..
                } = event.data
                {
                    let killer = event
                        .client_id
                        .and_then(|id| {
                            // We can't await inside and_then, so we'll handle it outside
                            Some(id)
                        });
                    let target = event.target_id;
                    if let (Some(kid), Some(tid)) = (killer, target) {
                        let kname = ctx
                            .clients
                            .get_by_id(kid)
                            .await
                            .map(|c| Self::strip_colors(&c.name))
                            .unwrap_or_else(|| format!("Player#{}", kid));
                        let tname = ctx
                            .clients
                            .get_by_id(tid)
                            .await
                            .map(|c| Self::strip_colors(&c.name))
                            .unwrap_or_else(|| format!("Player#{}", tid));
                        let loc_str = if hit_location.is_empty() {
                            String::new()
                        } else {
                            format!(" ({})", hit_location)
                        };
                        self.send_webhook(
                            "events",
                            &format!(
                                "💀 **{}** killed **{}** with {}{}",
                                kname,
                                tname,
                                weapon,
                                loc_str
                            ),
                        )
                        .await;
                    }
                }
            }

            // --- Map change ---
            "EVT_GAME_MAP_CHANGE" if self.relay_map_changes => {
                if let EventData::MapChange { ref new, ref old } = event.data {
                    let old_str = old.as_deref().unwrap_or("unknown");
                    self.send_webhook(
                        "events",
                        &format!("🗺️ Map changed: **{}** → **{}**", old_str, new),
                    )
                    .await;
                }
            }

            // --- Admin actions ---
            "EVT_CLIENT_KICK" if self.relay_admin_actions => {
                if let Some(cid) = event.client_id {
                    let name = ctx
                        .clients
                        .get_by_id(cid)
                        .await
                        .map(|c| Self::strip_colors(&c.name))
                        .unwrap_or_else(|| format!("Player#{}", cid));
                    let reason = if let EventData::Text(ref r) = event.data {
                        Self::strip_colors(r)
                    } else {
                        String::new()
                    };
                    self.send_webhook(
                        "admin",
                        &format!("🚫 **{}** was kicked{}", name, if reason.is_empty() { String::new() } else { format!(": {}", reason) }),
                    )
                    .await;
                }
            }

            "EVT_CLIENT_BAN" | "EVT_CLIENT_BAN_TEMP" if self.relay_admin_actions => {
                if let Some(cid) = event.client_id {
                    let name = ctx
                        .clients
                        .get_by_id(cid)
                        .await
                        .map(|c| Self::strip_colors(&c.name))
                        .unwrap_or_else(|| format!("Player#{}", cid));
                    let ban_type = if evt_key == "EVT_CLIENT_BAN" {
                        "banned"
                    } else {
                        "temp-banned"
                    };
                    let reason = if let EventData::Text(ref r) = event.data {
                        Self::strip_colors(r)
                    } else {
                        String::new()
                    };
                    self.send_webhook(
                        "admin",
                        &format!(
                            "⛔ **{}** was {}{}",
                            name,
                            ban_type,
                            if reason.is_empty() {
                                String::new()
                            } else {
                                format!(": {}", reason)
                            }
                        ),
                    )
                    .await;
                }
            }

            "EVT_CLIENT_WARN" if self.relay_admin_actions => {
                if let Some(cid) = event.client_id {
                    let name = ctx
                        .clients
                        .get_by_id(cid)
                        .await
                        .map(|c| Self::strip_colors(&c.name))
                        .unwrap_or_else(|| format!("Player#{}", cid));
                    let reason = if let EventData::Text(ref r) = event.data {
                        Self::strip_colors(r)
                    } else {
                        String::new()
                    };
                    self.send_webhook(
                        "admin",
                        &format!(
                            "⚠️ **{}** was warned{}",
                            name,
                            if reason.is_empty() {
                                String::new()
                            } else {
                                format!(": {}", reason)
                            }
                        ),
                    )
                    .await;
                }
            }

            // --- Round events ---
            "EVT_GAME_ROUND_START" if self.relay_map_changes => {
                let map = ctx
                    .game
                    .read()
                    .await
                    .map_name
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string());
                let count = ctx.clients.count().await;
                self.send_webhook(
                    "events",
                    &format!("🏁 Round started on **{}** ({} players)", map, count),
                )
                .await;
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
        Some(vec![
            "EVT_CLIENT_SAY".to_string(),
            "EVT_CLIENT_TEAM_SAY".to_string(),
            "EVT_CLIENT_AUTH".to_string(),
            "EVT_CLIENT_DISCONNECT".to_string(),
            "EVT_CLIENT_KILL".to_string(),
            "EVT_GAME_MAP_CHANGE".to_string(),
            "EVT_CLIENT_KICK".to_string(),
            "EVT_CLIENT_BAN".to_string(),
            "EVT_CLIENT_BAN_TEMP".to_string(),
            "EVT_CLIENT_WARN".to_string(),
            "EVT_GAME_ROUND_START".to_string(),
        ])
    }
}
