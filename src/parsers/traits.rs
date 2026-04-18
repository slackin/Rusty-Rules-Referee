use async_trait::async_trait;
use std::collections::HashMap;

use crate::events::Event;

/// A parsed log line from a game server log file.
#[derive(Debug, Clone)]
pub struct LogLine {
    /// Raw text of the line.
    pub raw: String,
    /// The timestamp extracted from the line, if any.
    pub timestamp: Option<String>,
    /// The clean line (with timestamp prefix stripped).
    pub clean: String,
}

/// Data extracted from parsing a single log line.
#[derive(Debug, Clone)]
pub enum ParsedAction {
    /// The line produced an R3 event.
    Event(Event),
    /// The line was recognized but produced no event (e.g., server info).
    NoOp,
    /// The line could not be parsed.
    Unknown(String),
}

/// RCON commands that a parser knows how to format for its game engine.
/// Each game has different RCON syntax for the same action.
#[derive(Debug, Clone)]
pub struct GameCommands {
    pub say: String,
    pub message: String,
    pub kick: String,
    pub ban: String,
    pub unban: String,
    pub tempban: String,
    pub set_cvar: String,
}

impl Default for GameCommands {
    fn default() -> Self {
        Self {
            say: "say %(message)s".to_string(),
            message: "tell %(cid)s %(message)s".to_string(),
            kick: "clientkick %(cid)s".to_string(),
            ban: "banclient %(cid)s".to_string(),
            unban: "unbanuser %(name)s".to_string(),
            tempban: "clientkick %(cid)s".to_string(),
            set_cvar: "set %(name)s \"%(value)s\"".to_string(),
        }
    }
}

impl GameCommands {
    /// Format a command template with the given parameters.
    pub fn format(&self, template: &str, params: &HashMap<String, String>) -> String {
        let mut result = template.to_string();
        for (key, value) in params {
            result = result.replace(&format!("%({})s", key), value);
        }
        result
    }
}

/// The GameParser trait — the game engine parser interface.
///
/// A parser is responsible for:
///   1. Parsing game log lines into R3 events
///   2. Formatting RCON commands for the game engine
///   3. Querying game server state (players, map, etc.)
#[async_trait]
pub trait GameParser: Send + Sync {
    /// The game name (e.g., "iourt43").
    fn game_name(&self) -> &str;

    /// The RCON command templates for this game.
    fn commands(&self) -> &GameCommands;

    /// Parse a single log line into an R3 action.
    fn parse_line(&self, line: &LogLine) -> ParsedAction;

    /// Get the current map name from the server.
    async fn get_map(&self) -> anyhow::Result<String>;

    /// Get the list of currently connected player slot IDs.
    async fn get_player_list(&self) -> anyhow::Result<Vec<String>>;

    /// Send a public message to the game server.
    async fn say(&self, message: &str) -> anyhow::Result<()>;

    /// Send a private message to a specific player.
    async fn message(&self, client_id: &str, message: &str) -> anyhow::Result<()>;

    /// Kick a player from the server.
    async fn kick(&self, client_id: &str, reason: &str) -> anyhow::Result<()>;

    /// Ban a player from the server.
    async fn ban(&self, client_id: &str, reason: &str) -> anyhow::Result<()>;

    /// Temporarily ban a player.
    async fn temp_ban(&self, client_id: &str, reason: &str, duration_mins: u32) -> anyhow::Result<()>;

    /// Unban a player.
    async fn unban(&self, name: &str) -> anyhow::Result<()>;

    /// Get a server cvar value.
    async fn get_cvar(&self, name: &str) -> anyhow::Result<String>;

    /// Set a server cvar value.
    async fn set_cvar(&self, name: &str, value: &str) -> anyhow::Result<()>;
}
