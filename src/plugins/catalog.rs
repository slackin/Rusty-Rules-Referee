//! Static plugin catalog — single source of truth for plugin metadata,
//! settings schemas, and defaults used by the web UI.
//!
//! The catalog is **static** (no `&self`, no `PluginRegistry` needed) so
//! it can be served by the master binary, which never instantiates plugin
//! objects. Standalone and client binaries can also consume it.
//!
//! To add a new plugin's schema: add a `fn <name>_schema() -> PluginSchema`
//! below and include it in [`catalog`].

use serde::Serialize;
use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// One catalog entry: plugin name, description, and settings schema.
#[derive(Debug, Clone, Serialize)]
pub struct PluginCatalogEntry {
    pub name: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub schema: PluginSchema,
}

/// A plugin's settings schema — ordered list of fields.
#[derive(Debug, Clone, Serialize)]
pub struct PluginSchema {
    pub fields: Vec<SettingField>,
}

/// One settings field rendered by the UI.
#[derive(Debug, Clone, Serialize)]
pub struct SettingField {
    pub key: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    /// Field widget type. Kept as a string to match the existing UI
    /// `pluginMeta` vocabulary verbatim (text, textarea, number, boolean,
    /// select, string_list, key_value, key_value_table, key_value_list,
    /// task_list).
    #[serde(rename = "type")]
    pub field_type: &'static str,
    pub default: Value,
    /// Select options (for `select` fields) or column headers (for
    /// `key_value_table`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<&'static str>>,
    /// Number input step (e.g. 0.1 for floats).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<f64>,
}

impl SettingField {
    const fn new(
        key: &'static str,
        field_type: &'static str,
        label: &'static str,
        description: &'static str,
        default: Value,
    ) -> Self {
        Self {
            key,
            field_type,
            label,
            description,
            default,
            options: None,
            step: None,
        }
    }

    fn with_options(mut self, options: Vec<&'static str>) -> Self {
        self.options = Some(options);
        self
    }

    fn with_step(mut self, step: f64) -> Self {
        self.step = Some(step);
        self
    }
}

// ---------------------------------------------------------------------------
// Tiny helpers
// ---------------------------------------------------------------------------

fn text(
    key: &'static str,
    label: &'static str,
    description: &'static str,
    default: &str,
) -> SettingField {
    SettingField::new(key, "text", label, description, json!(default))
}

fn textarea(
    key: &'static str,
    label: &'static str,
    description: &'static str,
    default: &str,
) -> SettingField {
    SettingField::new(key, "textarea", label, description, json!(default))
}

fn number(
    key: &'static str,
    label: &'static str,
    description: &'static str,
    default: f64,
) -> SettingField {
    SettingField::new(key, "number", label, description, json!(default))
}

fn boolean(
    key: &'static str,
    label: &'static str,
    description: &'static str,
    default: bool,
) -> SettingField {
    SettingField::new(key, "boolean", label, description, json!(default))
}

fn select(
    key: &'static str,
    label: &'static str,
    description: &'static str,
    default: &str,
    options: Vec<&'static str>,
) -> SettingField {
    SettingField::new(key, "select", label, description, json!(default)).with_options(options)
}

fn string_list(key: &'static str, label: &'static str, description: &'static str) -> SettingField {
    SettingField::new(key, "string_list", label, description, json!([]))
}

fn key_value(key: &'static str, label: &'static str, description: &'static str) -> SettingField {
    SettingField::new(key, "key_value", label, description, json!({}))
}

fn key_value_table(
    key: &'static str,
    label: &'static str,
    description: &'static str,
) -> SettingField {
    SettingField::new(key, "key_value_table", label, description, json!({}))
}

fn key_value_list(
    key: &'static str,
    label: &'static str,
    description: &'static str,
) -> SettingField {
    SettingField::new(key, "key_value_list", label, description, json!({}))
}

fn task_list(key: &'static str, label: &'static str, description: &'static str) -> SettingField {
    SettingField::new(key, "task_list", label, description, json!([]))
}

// ---------------------------------------------------------------------------
// Catalog aggregator
// ---------------------------------------------------------------------------

/// Return the full list of plugin catalog entries in the order plugins are
/// registered in `main.rs`.
pub fn catalog() -> Vec<PluginCatalogEntry> {
    vec![
        PluginCatalogEntry {
            name: "admin",
            label: "Admin",
            description: "Core administration commands (kick, ban, warn, etc.)",
            schema: PluginSchema { fields: vec![
                SettingField::new(
                    "warn_reason", "text", "Default Warn Reason",
                    "Default reason used when warning a player",
                    json!("Server Rule Violation"),
                ),
                number("max_warnings", "Max Warnings",
                    "Number of warnings before automatic action", 3.0),
                string_list("rules", "Server Rules",
                    "Rules displayed via !rules command (one per line)"),
                key_value("spam_messages", "Spam Messages",
                    "Quick message keywords and their text (e.g. \"rules\" → message)"),
                key_value_table("warn_reasons", "Warn Reasons",
                    "Predefined warn keywords with duration (mins) and reason text"),
            ]},
        },
        PluginCatalogEntry {
            name: "poweradminurt",
            label: "Power Admin URT",
            description: "Urban Terror specific administration features",
            schema: PluginSchema { fields: vec![
                boolean("team_balance_enabled", "Team Balance",
                    "Automatically balance teams", true),
                number("team_diff", "Max Team Difference",
                    "Maximum allowed team size difference", 1.0),
                boolean("rsp_enable", "Radio Spam Protection",
                    "Mute players who spam radio commands", false),
                number("rsp_mute_duration", "RSP Mute Duration",
                    "Mute duration in seconds", 2.0),
                number("rsp_max_spamins", "RSP Spam Threshold",
                    "Spam count before muting", 10.0),
                number("rsp_falloff_rate", "RSP Falloff Rate",
                    "Spam counter decay rate", 2.0),
                number("full_ident_level", "Full Ident Level",
                    "Min admin level to see IP/GUID in !ident", 60.0),
            ]},
        },
        PluginCatalogEntry {
            name: "adv",
            label: "Advertisements",
            description: "Rotating server advertisement messages",
            schema: PluginSchema { fields: vec![
                number("interval_secs", "Interval (seconds)",
                    "Seconds between advertisement rotations", 120.0),
                string_list("messages", "Messages",
                    "Rotating advertisement messages (URT color codes supported)"),
            ]},
        },
        PluginCatalogEntry {
            name: "afk",
            label: "AFK Detection",
            description: "Detect and handle AFK (away from keyboard) players",
            schema: PluginSchema { fields: vec![
                number("afk_threshold_secs", "AFK Threshold (seconds)",
                    "Seconds of inactivity before player is considered AFK", 300.0),
                number("min_players", "Min Players",
                    "Minimum players online before AFK kicks activate", 4.0),
                number("check_interval_secs", "Check Interval (seconds)",
                    "How often to check for AFK players", 60.0),
                boolean("move_to_spec", "Move to Spectator",
                    "Move AFK players to spectator instead of kicking", true),
                text("afk_message", "AFK Message",
                    "Message shown to AFK players",
                    "^7AFK: You have been inactive too long"),
            ]},
        },
        PluginCatalogEntry {
            name: "spawnkill",
            label: "Spawn Kill Protection",
            description: "Detect and punish spawn killing",
            schema: PluginSchema { fields: vec![
                number("grace_period_secs", "Grace Period (seconds)",
                    "Protection window after spawning", 3.0),
                number("max_spawnkills", "Max Spawn Kills",
                    "Spawn kills before action is taken", 3.0),
                select("action", "Action",
                    "Punishment for exceeding spawn kill limit",
                    "warn", vec!["warn", "kick", "tempban"]),
                number("tempban_duration", "Tempban Duration (minutes)",
                    "Ban duration if action is tempban", 5.0),
            ]},
        },
        PluginCatalogEntry {
            name: "spree",
            label: "Kill Spree",
            description: "Announce kill spree milestones",
            schema: PluginSchema { fields: vec![
                number("min_spree", "Min Spree Count",
                    "Minimum kills for a spree announcement", 5.0),
                key_value("spree_messages", "Spree Messages",
                    "Kill count → announcement message (e.g. \"5\" → \"KILLING SPREE!\")"),
            ]},
        },
        PluginCatalogEntry {
            name: "xlrstats",
            label: "XLR Stats",
            description: "Extended live ranking and statistics system",
            schema: PluginSchema { fields: vec![
                number("kill_bonus", "Kill Bonus",
                    "Skill calculation multiplier for kills", 1.2).with_step(0.1),
                number("assist_bonus", "Assist Bonus",
                    "Point multiplier for assists", 0.5).with_step(0.1),
                number("min_kills", "Min Kills",
                    "Minimum kills before stats are displayed", 50.0),
            ]},
        },
        PluginCatalogEntry {
            name: "makeroom",
            label: "Make Room",
            description: "Reserve slots for admins by kicking lowest-level players",
            schema: PluginSchema { fields: vec![
                number("min_admin_level", "Min Admin Level",
                    "Minimum level that triggers room-making", 20.0),
                number("max_players", "Max Players",
                    "Server player capacity", 32.0),
            ]},
        },
        PluginCatalogEntry {
            name: "customcommands",
            label: "Custom Commands",
            description: "Define custom chat commands with text responses",
            schema: PluginSchema { fields: vec![
                key_value("commands", "Commands",
                    "Command name → response text (e.g. \"rules\" → \"No camping, no spawn killing\")"),
            ]},
        },
        PluginCatalogEntry {
            name: "callvote",
            label: "Call Vote Control",
            description: "Control and restrict in-game voting",
            schema: PluginSchema { fields: vec![
                number("min_level", "Min Level to Vote",
                    "Minimum player level to call votes", 0.0),
                number("max_votes_per_round", "Max Votes per Round",
                    "Maximum votes a player can call per round", 3.0),
                string_list("blocked_votes", "Blocked Vote Types",
                    "Vote types to block (e.g. \"kick\", \"map\", \"gametype\")"),
            ]},
        },
        PluginCatalogEntry {
            name: "censor",
            label: "Chat Censor",
            description: "Filter bad words from chat messages",
            schema: PluginSchema { fields: vec![
                text("warn_message", "Warning Message",
                    "Message sent to the player when censored",
                    "Watch your language!"),
                number("max_warnings", "Max Warnings",
                    "Warnings before kicking the player", 3.0),
                string_list("bad_words", "Bad Words",
                    "Regex patterns for forbidden words in chat (case-insensitive)"),
                string_list("bad_names", "Bad Names",
                    "Regex patterns for forbidden player names (case-insensitive)"),
            ]},
        },
        PluginCatalogEntry {
            name: "censorurt",
            label: "Name Censor (URT)",
            description: "Filter offensive player names and clan tags",
            schema: PluginSchema { fields: vec![
                string_list("banned_names", "Banned Name Patterns",
                    "Regex patterns for banned names (case-insensitive)"),
            ]},
        },
        PluginCatalogEntry {
            name: "spamcontrol",
            label: "Spam Control",
            description: "Prevent players from spamming chat",
            schema: PluginSchema { fields: vec![
                number("max_messages", "Max Messages",
                    "Maximum messages in the time window", 5.0),
                number("time_window_secs", "Time Window (seconds)",
                    "Time window for counting messages", 10.0),
                number("max_repeats", "Max Repeats",
                    "Maximum consecutive repeated messages", 3.0),
            ]},
        },
        PluginCatalogEntry {
            name: "tk",
            label: "Team Kill Tracking",
            description: "Track and punish excessive team killing",
            schema: PluginSchema { fields: vec![
                number("max_team_kills", "Max Team Kills",
                    "Team kills per round before action", 5.0),
                number("max_team_damage", "Max Team Damage",
                    "Team damage per round before action", 300.0).with_step(10.0),
            ]},
        },
        PluginCatalogEntry {
            name: "welcome",
            label: "Welcome Messages",
            description: "Greet players when they join the server",
            schema: PluginSchema { fields: vec![
                textarea("new_player_message", "New Player Message",
                    "Message for first-time players. Variables: $name",
                    "^7Welcome to the server, ^2$name^7! Type ^3!help^7 for commands."),
                textarea("returning_player_message", "Returning Player Message",
                    "Message for returning players. Variables: $name, $last_visit",
                    "^7Welcome back, ^2$name^7! You were last seen ^3$last_visit^7."),
            ]},
        },
        PluginCatalogEntry {
            name: "chatlogger",
            label: "Chat Logger",
            description: "Log all chat messages to files",
            schema: PluginSchema { fields: vec![
                text("log_dir", "Log Directory",
                    "Directory for chat log files", "chat_logs"),
            ]},
        },
        PluginCatalogEntry {
            name: "stats",
            label: "Basic Stats",
            description: "Track basic in-round player statistics",
            schema: PluginSchema { fields: vec![] },
        },
        PluginCatalogEntry {
            name: "firstkill",
            label: "First Kill",
            description: "Announce the first kill of each round",
            schema: PluginSchema { fields: vec![] },
        },
        PluginCatalogEntry {
            name: "flagannounce",
            label: "Flag Announce",
            description: "Announce CTF flag captures, returns, and drops",
            schema: PluginSchema { fields: vec![] },
        },
        PluginCatalogEntry {
            name: "scheduler",
            label: "Scheduler",
            description: "Run actions on game events (round start, map change, etc.)",
            schema: PluginSchema { fields: vec![
                task_list("tasks", "Scheduled Tasks",
                    "Actions triggered by game events"),
            ]},
        },
        PluginCatalogEntry {
            name: "mapconfig",
            label: "Map Config",
            description: "Apply per-map server configurations",
            schema: PluginSchema { fields: vec![
                key_value_list("map_configs", "Map Configs",
                    "Map name → list of RCON commands to execute on map change"),
            ]},
        },
        PluginCatalogEntry {
            name: "vpncheck",
            label: "VPN Check",
            description: "Detect and block VPN/proxy connections",
            schema: PluginSchema { fields: vec![
                text("kick_reason", "Kick Reason",
                    "Message shown when kicking VPN users",
                    "VPN/Proxy connections are not allowed on this server."),
                string_list("blocked_ranges", "Blocked IP Ranges",
                    "IP ranges to block (format: \"start.ip - end.ip\")"),
            ]},
        },
        PluginCatalogEntry {
            name: "countryfilter",
            label: "Country Filter",
            description: "Allow or block connections by country",
            schema: PluginSchema { fields: vec![
                select("mode", "Filter Mode",
                    "Allowlist only allows listed countries; blocklist blocks them",
                    "blocklist", vec!["allowlist", "blocklist"]),
                text("kick_message", "Kick Message",
                    "Message shown to filtered players",
                    "Your country is not allowed on this server."),
                string_list("countries", "Country Codes",
                    "ISO 3166-1 alpha-2 country codes (e.g. US, DE, FR)"),
            ]},
        },
        PluginCatalogEntry {
            name: "pingwatch",
            label: "Ping Watch",
            description: "Monitor and kick high-ping players",
            schema: PluginSchema { fields: vec![
                number("max_ping", "Max Ping (ms)",
                    "Ping threshold for kicking", 250.0),
                number("warn_threshold", "Warn Threshold (ms)",
                    "Ping threshold for warnings", 200.0),
                number("max_warnings", "Max Warnings",
                    "Warnings before kick", 3.0),
            ]},
        },
        PluginCatalogEntry {
            name: "login",
            label: "Login",
            description: "Require password authentication for admin commands",
            schema: PluginSchema { fields: vec![
                number("min_level", "Min Level",
                    "Minimum admin level requiring login", 20.0),
            ]},
        },
        PluginCatalogEntry {
            name: "follow",
            label: "Follow",
            description: "Follow a player and receive notifications about their activity",
            schema: PluginSchema { fields: vec![] },
        },
        PluginCatalogEntry {
            name: "nickreg",
            label: "Nick Registration",
            description: "Protect registered nicknames from impostors",
            schema: PluginSchema { fields: vec![
                boolean("warn_before_kick", "Warn Before Kick",
                    "Warn players before kicking for nick violation", true),
            ]},
        },
        PluginCatalogEntry {
            name: "namechecker",
            label: "Name Checker",
            description: "Check for forbidden names, duplicates, and name spam",
            schema: PluginSchema { fields: vec![
                number("max_name_changes", "Max Name Changes",
                    "Maximum name changes allowed in the time window", 5.0),
                number("name_change_window", "Name Change Window (seconds)",
                    "Time window for counting name changes", 300.0),
                boolean("check_duplicates", "Check Duplicates",
                    "Kick players with duplicate names", true),
                string_list("forbidden_patterns", "Forbidden Name Patterns",
                    "Regex patterns for forbidden names (case-insensitive)"),
            ]},
        },
        PluginCatalogEntry {
            name: "specchecker",
            label: "Spectator Checker",
            description: "Kick spectators who idle too long when the server is busy",
            schema: PluginSchema { fields: vec![
                number("max_spec_time", "Max Spec Time (seconds)",
                    "Seconds before kicking a spectator", 300.0),
                number("min_players", "Min Players",
                    "Only enforce when server has this many players", 8.0),
                number("warn_interval", "Warn Interval (seconds)",
                    "Seconds between warnings", 60.0),
                number("immune_level", "Immune Level",
                    "Admin level immune to spec kicks", 20.0),
            ]},
        },
        PluginCatalogEntry {
            name: "headshotcounter",
            label: "Headshot Counter",
            description: "Track headshot ratios and detect possible aimbots",
            schema: PluginSchema { fields: vec![
                number("warn_ratio", "Warn Ratio",
                    "Headshot ratio threshold for warning (0.0-1.0)", 0.70).with_step(0.01),
                number("ban_ratio", "Ban Ratio",
                    "Headshot ratio threshold for auto-tempban (0.0-1.0)", 0.85).with_step(0.01),
                number("min_kills", "Min Kills",
                    "Minimum kills before ratio checks activate", 15.0),
                number("ban_duration", "Ban Duration (minutes)",
                    "Temp-ban duration when ban ratio is exceeded", 60.0),
                number("announce_interval", "Announce Interval",
                    "Announce headshot streaks every N headshots", 10.0),
            ]},
        },
        PluginCatalogEntry {
            name: "discord",
            label: "Discord",
            description: "Relay game events (chat, kills, bans, map changes) to Discord via webhooks",
            schema: PluginSchema { fields: vec![
                text("webhook_url", "Webhook URL",
                    "Default Discord webhook URL for all events", ""),
                text("chat_webhook_url", "Chat Webhook URL",
                    "Dedicated webhook for chat messages (overrides default)", ""),
                text("admin_webhook_url", "Admin Webhook URL",
                    "Dedicated webhook for admin actions (kicks, bans, warns)", ""),
                text("events_webhook_url", "Events Webhook URL",
                    "Dedicated webhook for game events (connections, map changes)", ""),
                text("bot_name", "Bot Display Name",
                    "Name shown in Discord for webhook messages", "R3 Bot"),
                boolean("relay_chat", "Relay Chat",
                    "Send player chat messages to Discord", true),
                boolean("relay_kills", "Relay Kills",
                    "Send kill events to Discord", false),
                boolean("relay_connections", "Relay Connections",
                    "Send player join/leave events to Discord", true),
                boolean("relay_admin_actions", "Relay Admin Actions",
                    "Send kicks, bans, and warnings to Discord", true),
                boolean("relay_map_changes", "Relay Map Changes",
                    "Send map change and round start events to Discord", true),
                number("rate_limit_ms", "Rate Limit (ms)",
                    "Minimum milliseconds between webhook messages", 1000.0),
            ]},
        },
        PluginCatalogEntry {
            name: "geowelcome",
            label: "Geo Welcome",
            description: "Greet players with their country when they connect (GeoIP lookup)",
            schema: PluginSchema { fields: vec![
                textarea("welcome_message", "Welcome Message",
                    "Message template. Variables: $name, $country, $country_code",
                    "^7Player ^2$name ^7connected from ^3$country"),
                boolean("announce_public", "Announce Public",
                    "Announce to the whole server (true) or just the player (false)", true),
                text("geoip_api_url", "GeoIP API URL",
                    "GeoIP lookup URL template. $ip will be replaced with the player IP",
                    "http://ip-api.com/json/$ip?fields=status,country,countryCode"),
            ]},
        },
    ]
}

/// Look up a single plugin's catalog entry by name.
pub fn get(name: &str) -> Option<PluginCatalogEntry> {
    catalog().into_iter().find(|e| e.name == name)
}

/// Build a `serde_json::Map` of default setting values for a plugin.
/// Returns an empty object if the plugin is not in the catalog.
pub fn defaults(name: &str) -> Value {
    match get(name) {
        Some(entry) => {
            let mut obj = serde_json::Map::new();
            for f in entry.schema.fields {
                obj.insert(f.key.to_string(), f.default);
            }
            Value::Object(obj)
        }
        None => Value::Object(Default::default()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_expected_plugins() {
        let c = catalog();
        assert!(c.len() >= 30, "expected at least 30 plugins, got {}", c.len());
        // Every entry has a non-empty name and unique names.
        let mut names = std::collections::HashSet::new();
        for e in &c {
            assert!(!e.name.is_empty());
            assert!(names.insert(e.name), "duplicate plugin name: {}", e.name);
        }
    }

    #[test]
    fn admin_schema_has_warn_reasons() {
        let e = get("admin").expect("admin present");
        assert!(e.schema.fields.iter().any(|f| f.key == "warn_reasons"));
    }

    #[test]
    fn defaults_returns_object_for_known_plugin() {
        let d = defaults("adv");
        assert!(d.is_object());
        assert_eq!(d["interval_secs"], json!(120.0));
    }
}
