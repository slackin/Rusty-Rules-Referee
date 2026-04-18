use std::sync::Arc;
use tokio::sync::RwLock;

use crate::core::Clients;
use crate::core::Game;
use crate::events::EventRegistry;
use crate::parsers::GameParser;
use crate::rcon::RconClient;
use crate::storage::Storage;

/// Shared context passed to plugins, giving them access to the bot's
/// core services: RCON, storage, game state, parser, and event registry.
///
/// This is the Rust equivalent of Python B3's `self.console` reference
/// that plugins use to interact with the game server and database.
pub struct BotContext {
    pub rcon: Arc<RconClient>,
    pub storage: Arc<dyn Storage>,
    pub game: Arc<RwLock<Game>>,
    pub event_registry: Arc<EventRegistry>,
    pub parser: Arc<dyn GameParser>,
    pub clients: Arc<Clients>,
}

impl BotContext {
    pub fn new(
        rcon: Arc<RconClient>,
        storage: Arc<dyn Storage>,
        game: Arc<RwLock<Game>>,
        event_registry: Arc<EventRegistry>,
        parser: Arc<dyn GameParser>,
        clients: Arc<Clients>,
    ) -> Self {
        Self {
            rcon,
            storage,
            game,
            event_registry,
            parser,
            clients,
        }
    }

    /// Send a public message to the game server.
    pub async fn say(&self, message: &str) -> anyhow::Result<()> {
        self.parser.say(message).await
    }

    /// Send a private message to a player.
    pub async fn message(&self, client_id: &str, message: &str) -> anyhow::Result<()> {
        self.parser.message(client_id, message).await
    }

    /// Kick a player from the server.
    pub async fn kick(&self, client_id: &str, reason: &str) -> anyhow::Result<()> {
        self.parser.kick(client_id, reason).await
    }

    /// Ban a player.
    pub async fn ban(&self, client_id: &str, reason: &str) -> anyhow::Result<()> {
        self.parser.ban(client_id, reason).await
    }

    /// Temporarily ban a player.
    pub async fn temp_ban(&self, client_id: &str, reason: &str, duration_mins: u32) -> anyhow::Result<()> {
        self.parser.temp_ban(client_id, reason, duration_mins).await
    }

    /// Send a big text message (displayed large on screen).
    pub async fn bigtext(&self, message: &str) -> anyhow::Result<()> {
        self.rcon.send(&format!("bigtext \"{}\"", message)).await?;
        Ok(())
    }

    /// Write a raw RCON command.
    pub async fn write(&self, command: &str) -> anyhow::Result<String> {
        self.rcon.send(command).await
    }

    /// Set a server cvar.
    pub async fn set_cvar(&self, name: &str, value: &str) -> anyhow::Result<()> {
        self.parser.set_cvar(name, value).await
    }

    /// Get a server cvar.
    pub async fn get_cvar(&self, name: &str) -> anyhow::Result<String> {
        self.parser.get_cvar(name).await
    }
}
