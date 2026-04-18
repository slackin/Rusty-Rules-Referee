use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

use tokio::sync::{mpsc, RwLock};
use tracing::{error, info};

use rusty_rules_referee::config::RefereeConfig;
use rusty_rules_referee::core::context::BotContext;
use rusty_rules_referee::core::log_tailer::LogTailer;
use rusty_rules_referee::core::{Client, Clients, Game};
use rusty_rules_referee::events::{Event, EventRegistry};
use rusty_rules_referee::parsers::{GameParser, LogLine, ParsedAction};
use rusty_rules_referee::parsers::urbanterror::UrbanTerrorParser;
use rusty_rules_referee::plugins::admin::AdminPlugin;
use rusty_rules_referee::plugins::censor::CensorPlugin;
use rusty_rules_referee::plugins::chatlogger::ChatLogPlugin;
use rusty_rules_referee::plugins::countryfilter::CountryFilterPlugin;
use rusty_rules_referee::plugins::pingwatch::PingWatchPlugin;
use rusty_rules_referee::plugins::spamcontrol::SpamControlPlugin;
use rusty_rules_referee::plugins::stats::StatsPlugin;
use rusty_rules_referee::plugins::tk::TkPlugin;
use rusty_rules_referee::plugins::welcome::WelcomePlugin;
use rusty_rules_referee::plugins::PluginRegistry;
use rusty_rules_referee::rcon::RconClient;
use rusty_rules_referee::storage;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Create the Urban Terror 4.3 game parser.
fn create_parser(
    rcon: Arc<RconClient>,
    event_registry: Arc<EventRegistry>,
) -> Arc<dyn GameParser> {
    Arc::new(UrbanTerrorParser::new(rcon, event_registry))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!("Rusty Rules Referee v{VERSION}");
    info!("====================================================");

    // Parse command-line args
    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "referee.toml".to_string());

    let config = match RefereeConfig::from_file(Path::new(&config_path)) {
        Ok(c) => {
            info!(bot = %c.referee.bot_name, "Configuration loaded");
            c
        }
        Err(e) => {
            error!(path = %config_path, error = %e, "Failed to load configuration");
            eprintln!("ERROR: Could not load config file '{}': {}", config_path, e);
            eprintln!("Usage: rusty-rules-referee [config.toml]");
            std::process::exit(1);
        }
    };

    // Set up event registry
    let event_registry = Arc::new(EventRegistry::new());

    // Set up RCON client
    let rcon_addr: SocketAddr = format!("{}:{}", config.rcon_ip(), config.rcon_port()).parse()?;
    let rcon = Arc::new(RconClient::new(rcon_addr, &config.server.rcon_password));
    info!(addr = %rcon_addr, "RCON client configured");

    // Connect to database
    let db: Arc<dyn storage::Storage> =
        Arc::from(storage::create_storage(&config.referee.database).await?);
    info!("Database connected");

    // Create game state
    let game = Arc::new(RwLock::new(Game::new("iourt43")));

    // Create the game parser
    let parser = create_parser(rcon.clone(), event_registry.clone());
    info!(game = parser.game_name(), "Game parser initialized");

    // Create the connected-clients manager
    let clients = Arc::new(Clients::new());

    // Create BotContext — shared across all plugins
    let ctx = Arc::new(BotContext::new(
        rcon.clone(),
        db.clone(),
        game.clone(),
        event_registry.clone(),
        parser.clone(),
        clients.clone(),
    ));

    // Set up the plugin registry and load plugins
    let mut plugins = PluginRegistry::new();

    // Register core plugins (order matters — dependencies first)
    plugins.register(Box::new(AdminPlugin::new()))?;
    plugins.register(Box::new(CensorPlugin::new()))?;
    plugins.register(Box::new(SpamControlPlugin::new()))?;
    plugins.register(Box::new(TkPlugin::new()))?;
    plugins.register(Box::new(WelcomePlugin::new()))?;
    plugins.register(Box::new(ChatLogPlugin::new()))?;
    plugins.register(Box::new(PingWatchPlugin::new()))?;
    plugins.register(Box::new(CountryFilterPlugin::new()))?;
    plugins.register(Box::new(StatsPlugin::new()))?;

    // Start all plugins
    plugins.startup_all().await?;
    info!("All plugins started");

    // Event queue (channel between log reader and event handler)
    let (event_tx, mut event_rx) = mpsc::channel::<Event>(1024);

    // Spawn the event handler task
    let handler_plugins = plugins;
    let handler_ctx = ctx.clone();
    let handler_event_registry = event_registry.clone();
    let handler_parser_raw = Arc::new(UrbanTerrorParser::new(rcon.clone(), event_registry.clone()));
    let handler_clients = clients.clone();
    let handler_storage = db.clone();
    let _handler_event_tx = event_tx.clone();
    let handler_task = tokio::spawn(async move {
        info!("Event handler started");
        while let Some(event) = event_rx.recv().await {
            let evt_key = handler_event_registry.get_key(event.event_type);

            // --- Client auth flow ---
            if evt_key == Some("EVT_CLIENT_CONNECT") {
                if let Some(cid) = &event.client_id {
                    let cid_str = cid.to_string();
                    match handler_parser_raw.dumpuser(&cid_str).await {
                        Ok(info) => {
                            // Look up or create client in DB by GUID
                            let db_client = if !info.guid.is_empty() {
                                handler_storage.get_client_by_guid(&info.guid).await.ok()
                            } else {
                                None
                            };

                            let mut client = match db_client {
                                Some(existing) => existing,
                                None => {
                                    let mut c = Client::new(&info.guid, &info.name);
                                    c.cid = Some(cid_str.clone());
                                    if let Ok(ip) = info.ip.parse() {
                                        c.ip = Some(ip);
                                    }
                                    // Save to DB
                                    if let Err(e) = handler_storage.save_client(&c).await {
                                        error!(error = %e, "Failed to save new client");
                                    }
                                    c
                                }
                            };

                            // Update runtime fields
                            client.cid = Some(cid_str.clone());
                            client.connected = true;
                            if let Ok(ip) = info.ip.parse() {
                                client.ip = Some(ip);
                            }
                            client.name = info.name.clone();

                            // Save alias (track name history)
                            if client.id > 0 && !info.name.is_empty() {
                                if let Err(e) = handler_storage.save_alias(client.id, &info.name).await {
                                    error!(error = %e, "Failed to save alias");
                                }
                            }

                            handler_clients.connect(&cid_str, client).await;

                            // Fire EVT_CLIENT_AUTH
                            if let Some(auth_id) = handler_event_registry.get_id("EVT_CLIENT_AUTH") {
                                let auth_event = Event::new(
                                    auth_id,
                                    rusty_rules_referee::events::EventData::Empty,
                                ).with_client(*cid);
                                handler_plugins.dispatch(&auth_event, &handler_ctx).await;
                            }
                        }
                        Err(e) => {
                            error!(cid = %cid, error = %e, "Failed to dumpuser on connect");
                        }
                    }
                }
            }

            // --- Client disconnect ---
            if evt_key == Some("EVT_CLIENT_DISCONNECT") {
                if let Some(cid) = &event.client_id {
                    handler_clients.disconnect(&cid.to_string()).await;
                }
            }

            // Dispatch event to plugins
            handler_plugins.dispatch(&event, &handler_ctx).await;
        }
        info!("Event handler stopped");
    });

    // Main loop: tail game log continuously and parse into events
    if let Some(ref log_path) = config.server.game_log {
        info!(path = %log_path, "Starting log tailer");

        let delay = std::time::Duration::from_secs_f64(config.server.delay);
        let mut tailer = LogTailer::new(Path::new(log_path), delay);

        match tailer.start().await {
            Ok(()) => {
                info!(path = %log_path, "Log tailer active — processing new lines");
                while let Some(line) = tailer.next_line().await {
                    let log_line = LogLine {
                        raw: line.clone(),
                        timestamp: None,
                        clean: line,
                    };

                    match parser.parse_line(&log_line) {
                        ParsedAction::Event(event) => {
                            if event_tx.send(event).await.is_err() {
                                error!("Event channel closed");
                                break;
                            }
                        }
                        ParsedAction::NoOp => {}
                        ParsedAction::Unknown(_) => {}
                    }
                }
            }
            Err(e) => {
                error!(path = %log_path, error = %e, "Cannot open game log");
            }
        }
    } else {
        info!("No game log configured — running in RCON-only mode");
        info!("Press Ctrl+C to stop");
        tokio::signal::ctrl_c().await?;
    }

    // Shutdown
    drop(event_tx);
    handler_task.await?;
    info!("Rusty Rules Referee shutdown complete");

    Ok(())
}
