use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::net::IpAddr;

/// Player team assignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Team {
    #[default]
    Unknown,
    Free,
    Spectator,
    Red,
    Blue,
}

/// A variable stored by a plugin on a client.
#[derive(Debug, Clone)]
pub struct ClientVar {
    pub value: serde_json::Value,
}

impl ClientVar {
    pub fn new(value: serde_json::Value) -> Self {
        Self { value }
    }

    pub fn as_i64(&self) -> i64 {
        self.value.as_i64().unwrap_or(0)
    }

    pub fn as_str(&self) -> &str {
        self.value.as_str().unwrap_or("")
    }
}

/// A connected (or previously connected) player.
#[derive(Debug, Clone)]
pub struct Client {
    // Database / persistent identity
    pub id: i64,
    pub guid: String,
    pub pbid: String,
    pub name: String,
    pub ip: Option<IpAddr>,
    pub greeting: String,
    pub login: String,
    pub password: String,
    pub group_bits: u64,
    pub auto_login: bool,

    // Session state
    pub cid: Option<String>,
    pub team: Team,
    pub connected: bool,
    pub authed: bool,
    pub authorizing: bool,
    pub bot: bool,
    pub hide: bool,
    pub mask_level: u32,

    // Timestamps
    pub time_add: DateTime<Utc>,
    pub time_edit: DateTime<Utc>,
    pub last_visit: Option<DateTime<Utc>>,

    // Arbitrary data and plugin variables
    pub data: HashMap<String, serde_json::Value>,
    plugin_data: HashMap<String, HashMap<String, ClientVar>>,
}

impl Client {
    pub fn new(guid: &str, name: &str) -> Self {
        let now = Utc::now();
        Self {
            id: 0,
            guid: guid.to_string(),
            pbid: String::new(),
            name: name.to_string(),
            ip: None,
            greeting: String::new(),
            login: String::new(),
            password: String::new(),
            group_bits: 0,
            auto_login: true,
            cid: None,
            team: Team::Unknown,
            connected: true,
            authed: false,
            authorizing: false,
            bot: false,
            hide: false,
            mask_level: 0,
            time_add: now,
            time_edit: now,
            last_visit: None,
            data: HashMap::new(),
            plugin_data: HashMap::new(),
        }
    }

    /// Get the client's effective level (highest group level or mask).
    pub fn max_level(&self) -> u32 {
        if self.mask_level > 0 {
            return self.mask_level;
        }
        // Calculate from group_bits: highest bit position
        if self.group_bits == 0 {
            return 0;
        }
        63 - self.group_bits.leading_zeros()
    }

    // ---- Plugin variable storage ----

    pub fn set_var(&mut self, plugin: &str, key: &str, value: serde_json::Value) {
        self.plugin_data
            .entry(plugin.to_string())
            .or_default()
            .insert(key.to_string(), ClientVar::new(value));
    }

    pub fn get_var(&self, plugin: &str, key: &str) -> Option<&ClientVar> {
        self.plugin_data.get(plugin)?.get(key)
    }

    pub fn has_var(&self, plugin: &str, key: &str) -> bool {
        self.plugin_data
            .get(plugin)
            .map(|m| m.contains_key(key))
            .unwrap_or(false)
    }

    pub fn del_var(&mut self, plugin: &str, key: &str) {
        if let Some(map) = self.plugin_data.get_mut(plugin) {
            map.remove(key);
        }
    }

    /// Strip color codes (e.g., ^1, ^7) from the player name.
    pub fn exact_name(&self) -> String {
        let re = regex::Regex::new(r"\^[0-9a-zA-Z]").unwrap();
        re.replace_all(&self.name, "").to_string()
    }
}
