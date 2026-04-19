use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicU32, Ordering};

/// Auto-incrementing event ID generator.
static NEXT_EVENT_ID: AtomicU32 = AtomicU32::new(1);

/// Unique identifier for an event type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EventId(pub u32);

impl fmt::Display for EventId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The event registry — maps event keys to IDs and names.
pub struct EventRegistry {
    events: HashMap<String, EventId>,
    names: HashMap<EventId, String>,
}

impl EventRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            events: HashMap::new(),
            names: HashMap::new(),
        };
        registry.load_defaults();
        registry
    }

    /// Register a new event type, returning its unique ID.
    pub fn create_event(&mut self, key: &str, name: &str) -> EventId {
        if let Some(&id) = self.events.get(key) {
            return id;
        }
        let id = EventId(NEXT_EVENT_ID.fetch_add(1, Ordering::Relaxed));
        self.events.insert(key.to_string(), id);
        self.names.insert(id, name.to_string());
        id
    }

    /// Look up an event ID by its key string.
    pub fn get_id(&self, key: &str) -> Option<EventId> {
        self.events.get(key).copied()
    }

    /// Look up an event key by its numeric ID.
    pub fn get_key(&self, id: EventId) -> Option<&str> {
        self.events
            .iter()
            .find(|(_, &v)| v == id)
            .map(|(k, _)| k.as_str())
    }

    /// Look up an event's human-readable name.
    pub fn get_name(&self, key: &str) -> Option<&str> {
        self.get_id(key)
            .and_then(|id| self.names.get(&id))
            .map(|s| s.as_str())
    }

    /// Load the default events (EVT_* constants).
    fn load_defaults(&mut self) {
        let defaults = [
            ("EVT_EXIT", "Program Exit"),
            ("EVT_STOP", "Stop Process"),
            ("EVT_UNKNOWN", "Unknown Event"),
            ("EVT_CUSTOM", "Custom Event"),
            ("EVT_PLUGIN_ENABLED", "Plugin Enabled"),
            ("EVT_PLUGIN_DISABLED", "Plugin Disabled"),
            ("EVT_PLUGIN_LOADED", "Plugin Loaded"),
            ("EVT_PLUGIN_UNLOADED", "Plugin Unloaded"),
            ("EVT_CLIENT_SAY", "Say"),
            ("EVT_CLIENT_TEAM_SAY", "Team Say"),
            ("EVT_CLIENT_SQUAD_SAY", "Squad Say"),
            ("EVT_CLIENT_PRIVATE_SAY", "Private Message"),
            ("EVT_CLIENT_CONNECT", "Client Connect"),
            ("EVT_CLIENT_AUTH", "Client Authenticated"),
            ("EVT_CLIENT_DISCONNECT", "Client Disconnect"),
            ("EVT_CLIENT_UPDATE", "Client Update"),
            ("EVT_CLIENT_KILL", "Client Kill"),
            ("EVT_CLIENT_GIB", "Client Gib"),
            ("EVT_CLIENT_GIB_TEAM", "Client Gib Team"),
            ("EVT_CLIENT_GIB_SELF", "Client Gib Self"),
            ("EVT_CLIENT_SUICIDE", "Client Suicide"),
            ("EVT_CLIENT_KILL_TEAM", "Client Team Kill"),
            ("EVT_CLIENT_DAMAGE", "Client Damage"),
            ("EVT_CLIENT_DAMAGE_SELF", "Client Damage Self"),
            ("EVT_CLIENT_DAMAGE_TEAM", "Client Team Damage"),
            ("EVT_CLIENT_JOIN", "Client Join Team"),
            ("EVT_CLIENT_NAME_CHANGE", "Client Name Change"),
            ("EVT_CLIENT_TEAM_CHANGE", "Client Team Change"),
            ("EVT_CLIENT_TEAM_CHANGE2", "Client Team Change 2"),
            ("EVT_CLIENT_ITEM_PICKUP", "Client Item Pickup"),
            ("EVT_CLIENT_ACTION", "Client Action"),
            ("EVT_CLIENT_KICK", "Client Kicked"),
            ("EVT_CLIENT_BAN", "Client Banned"),
            ("EVT_CLIENT_BAN_TEMP", "Client Temp Banned"),
            ("EVT_CLIENT_UNBAN", "Client Unbanned"),
            ("EVT_CLIENT_WARN", "Client Warned"),
            ("EVT_CLIENT_NOTICE", "Client given a notice"),
            ("EVT_GAME_ROUND_START", "Game Round Start"),
            ("EVT_GAME_ROUND_END", "Game Round End"),
            ("EVT_GAME_WARMUP", "Game Warmup"),
            ("EVT_GAME_EXIT", "Game Exit"),
            ("EVT_GAME_MAP_CHANGE", "Map Changed"),
            ("EVT_CLIENT_INFO_CHANGE", "Client Info Changed"),
            ("EVT_CLIENT_RADIO", "Client Radio"),
            ("EVT_CLIENT_CALLVOTE", "Client Callvote"),
            ("EVT_CLIENT_VOTE", "Client Vote"),
            ("EVT_CLIENT_FLAG_PICKUP", "Client Flag Pickup"),
            ("EVT_CLIENT_FLAG_DROPPED", "Client Flag Dropped"),
            ("EVT_CLIENT_FLAG_CAPTURED", "Client Flag Captured"),
            ("EVT_CLIENT_FLAG_RETURNED", "Client Flag Returned"),
            ("EVT_CLIENT_BOMB_PLANTED", "Client Bomb Planted"),
            ("EVT_CLIENT_BOMB_DEFUSED", "Client Bomb Defused"),
            ("EVT_CLIENT_BOMB_EXPLODED", "Client Bomb Exploded"),
            ("EVT_CLIENT_ASSIST", "Client Assist"),
            ("EVT_SURVIVOR_WIN", "Survivor Win"),
            ("EVT_CLIENT_SPAWN", "Client Spawn"),
        ];

        for (key, name) in defaults {
            self.create_event(key, name);
        }
    }
}

impl Default for EventRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// A concrete event instance, carrying data from source to handlers.
#[derive(Debug, Clone)]
pub struct Event {
    /// When the event was created (unix timestamp).
    pub time: i64,
    /// The event type ID.
    pub event_type: EventId,
    /// Arbitrary event data (game-specific payload).
    pub data: EventData,
    /// The client (player) who caused this event, if any.
    pub client_id: Option<i64>,
    /// The target client of this event, if any.
    pub target_id: Option<i64>,
}

/// Flexible event payload — different events carry different data shapes.
#[derive(Debug, Clone, serde::Serialize)]
pub enum EventData {
    Empty,
    Text(String),
    Kill {
        weapon: String,
        damage: f32,
        damage_type: String,
        hit_location: String,
    },
    MapChange {
        old: Option<String>,
        new: String,
    },
    Custom(serde_json::Value),
}

impl Event {
    pub fn new(event_type: EventId, data: EventData) -> Self {
        Self {
            time: chrono::Utc::now().timestamp(),
            event_type,
            data,
            client_id: None,
            target_id: None,
        }
    }

    pub fn with_client(mut self, client_id: i64) -> Self {
        self.client_id = Some(client_id);
        self
    }

    pub fn with_target(mut self, target_id: i64) -> Self {
        self.target_id = Some(target_id);
        self
    }
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Event<{}>({:?}, client={:?}, target={:?})",
            self.event_type.0, self.data, self.client_id, self.target_id
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_registry() {
        let mut reg = EventRegistry::new();
        let id = reg.get_id("EVT_CLIENT_SAY").unwrap();
        assert_eq!(reg.get_key(id), Some("EVT_CLIENT_SAY"));
        assert_eq!(reg.get_name("EVT_CLIENT_SAY"), Some("Say"));

        // Custom events
        let custom_id = reg.create_event("EVT_MY_PLUGIN", "Custom Plugin Event");
        assert_eq!(reg.get_id("EVT_MY_PLUGIN"), Some(custom_id));
    }
}
