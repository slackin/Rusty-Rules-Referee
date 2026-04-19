use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

use tokio::sync::{broadcast, mpsc, RwLock};
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
use rusty_rules_referee::plugins::headshotcounter::HeadshotCounterPlugin;
use rusty_rules_referee::plugins::namechecker::NameCheckerPlugin;
use rusty_rules_referee::plugins::pingwatch::PingWatchPlugin;
use rusty_rules_referee::plugins::spamcontrol::SpamControlPlugin;
use rusty_rules_referee::plugins::specchecker::SpecCheckerPlugin;
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
    plugins.register(Box::new(NameCheckerPlugin::new()))?;
    plugins.register(Box::new(SpecCheckerPlugin::new()))?;
    plugins.register(Box::new(HeadshotCounterPlugin::new()))?;

    // Start all plugins
    plugins.startup_all(&config.plugins).await?;
    info!("All plugins started");

    // Broadcast channel for WebSocket event streaming
    let (ws_event_tx, _ws_event_rx) = broadcast::channel::<Event>(256);

    // Start the web admin server if enabled
    if config.web.enabled {
        let web_ctx = ctx.clone();
        let web_config = config.clone();
        let web_config_path = config_path.clone();
        let web_storage = db.clone();
        let web_event_tx = ws_event_tx.clone();
        tokio::spawn(async move {
            if let Err(e) = rusty_rules_referee::web::start_server(
                web_ctx,
                web_config,
                web_config_path,
                web_storage,
                web_event_tx,
            ).await {
                error!(error = %e, "Web admin server failed");
            }
        });
    }

    // Event queue (channel between log reader and event handler)
    let (event_tx, mut event_rx) = mpsc::channel::<Event>(1024);

    // Spawn the event handler task
    let handler_plugins = plugins;
    let handler_ctx = ctx.clone();
    let handler_event_registry = event_registry.clone();
    let handler_parser_raw = Arc::new(UrbanTerrorParser::new(rcon.clone(), event_registry.clone()));
    let handler_clients = clients.clone();
    let handler_storage = db.clone();
    let handler_game = game.clone();
    let _handler_event_tx = event_tx.clone();
    let handler_ws_tx = ws_event_tx.clone();
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
                                    // Save to DB and assign the new ID
                                    match handler_storage.save_client(&c).await {
                                        Ok(new_id) => c.id = new_id,
                                        Err(e) => error!(error = %e, "Failed to save new client"),
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

            // --- Map change ---
            if evt_key == Some("EVT_GAME_MAP_CHANGE") {
                if let rusty_rules_referee::events::EventData::MapChange { new, .. } = &event.data {
                    let mut g = handler_game.write().await;
                    g.start_map(new);
                    info!(map = %new, "Game state updated: map change");
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

            // Forward event to WebSocket clients
            let _ = handler_ws_tx.send(event);
        }
        info!("Event handler stopped");
    });

    // --- Spawn background RCON poller: keeps game/client state fresh ---
    {
        let poller_rcon = rcon.clone();
        let poller_clients = clients.clone();
        let poller_game = game.clone();
        tokio::spawn(async move {
            use regex::Regex;
            use std::collections::HashMap;
            let re_status = Regex::new(
                r"^\s*(?P<slot>\d+)\s+(?P<score>-?\d+)\s+(?P<ping>\d+)\s+(?P<name>.*?)\s+(?P<lastmsg>\d+)\s+(?P<address>\S+)\s+(?P<qport>\d+)\s+(?P<rate>\d+)$"
            ).unwrap();

            let mut tick: u64 = 0;
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                tick += 1;

                // Poll status every tick (score + ping for all players)
                if let Ok(raw) = poller_rcon.send("status").await {
                    for line in raw.lines() {
                        if let Some(caps) = re_status.captures(line) {
                            let slot = caps.name("slot").unwrap().as_str().to_string();
                            let score: i32 = caps.name("score").unwrap().as_str().parse().unwrap_or(0);
                            let ping: u32 = caps.name("ping").unwrap().as_str().parse().unwrap_or(0);
                            poller_clients.update(&slot, |c| {
                                c.score = score;
                                c.ping = ping;
                            }).await;
                        }
                    }
                }

                // Poll dumpuser for each client every 3rd tick (~15s) to get gear
                if tick % 3 == 0 {
                    let all = poller_clients.get_all().await;
                    for client in &all {
                        if let Some(ref cid) = client.cid {
                            if let Ok(raw) = poller_rcon.send(&format!("dumpuser {}", cid)).await {
                                let mut gear: Option<String> = None;
                                let mut auth: Option<String> = None;
                                for line in raw.lines() {
                                    let trimmed = line.trim();
                                    let mut parts = trimmed.splitn(2, char::is_whitespace);
                                    if let (Some(key), Some(val)) = (parts.next(), parts.next()) {
                                        let val = val.trim();
                                        match key {
                                            "gear" => gear = Some(val.to_string()),
                                            "auth" => auth = Some(val.to_string()),
                                            _ => {}
                                        }
                                    }
                                }
                                let g = gear;
                                let a = auth;
                                poller_clients.update(cid, |c| {
                                    if let Some(ref v) = g { c.gear = Some(v.clone()); }
                                    if let Some(ref v) = a { c.auth_name = Some(v.clone()); }
                                }).await;
                            }
                        }
                    }
                }

                // Poll serverinfo every 6th tick (~30s)
                if tick % 6 == 0 {
                    if let Ok(raw) = poller_rcon.send("serverinfo").await {
                        let mut info = HashMap::new();
                        for line in raw.lines() {
                            let line = line.trim();
                            if line.is_empty() || line.starts_with("Server info") { continue; }
                            let mut parts = line.splitn(2, char::is_whitespace);
                            if let (Some(key), Some(val)) = (parts.next(), parts.next()) {
                                info.insert(key.trim().to_string(), val.trim().to_string());
                            }
                        }
                        let mut g = poller_game.write().await;
                        g.hostname = info.get("sv_hostname").cloned();
                        g.max_clients = info.get("sv_maxclients").and_then(|v| v.parse().ok());
                        if let Some(gt) = info.get("g_gametype") { g.game_type = Some(gt.clone()); }
                        if let Some(mn) = info.get("mapname") { g.map_name = Some(mn.clone()); }
                        g.server_info = info;
                    }
                }
            }
        });
        info!("Background RCON poller started (status every 5s, dumpuser every 15s, serverinfo every 30s)");
    }

    // --- Startup sync: discover already-connected players via RCON ---
    {
        let sync_parser = UrbanTerrorParser::new(rcon.clone(), event_registry.clone());
        match sync_parser.get_status_players().await {
            Ok(status_players) => {
                info!(count = status_players.len(), "Startup sync: found players from RCON status");
                for sp in &status_players {
                    match sync_parser.dumpuser(&sp.slot).await {
                        Ok(info) => {
                            if info.guid.is_empty() {
                                continue;
                            }
                            let db_client = db.get_client_by_guid(&info.guid).await.ok();
                            let mut client = match db_client {
                                Some(existing) => existing,
                                None => {
                                    let mut c = Client::new(&info.guid, &info.name);
                                    c.cid = Some(sp.slot.clone());
                                    if let Ok(ip) = info.ip.parse() {
                                        c.ip = Some(ip);
                                    }
                                    match db.save_client(&c).await {
                                        Ok(new_id) => c.id = new_id,
                                        Err(e) => error!(error = %e, "Startup sync: failed to save client"),
                                    }
                                    c
                                }
                            };
                            client.cid = Some(sp.slot.clone());
                            client.connected = true;
                            client.score = sp.score.parse().unwrap_or(0);
                            client.ping = sp.ping.parse().unwrap_or(0);
                            if !info.auth.is_empty() {
                                client.auth_name = Some(info.auth.clone());
                            }
                            if let Ok(ip) = info.ip.parse() {
                                client.ip = Some(ip);
                            }
                            client.name = info.name.clone();

                            if client.id > 0 && !info.name.is_empty() {
                                let _ = db.save_alias(client.id, &info.name).await;
                            }

                            clients.connect(&sp.slot, client).await;
                            info!(slot = %sp.slot, name = %info.name, guid = %info.guid, "Startup sync: registered player");
                        }
                        Err(e) => {
                            error!(slot = %sp.slot, error = %e, "Startup sync: dumpuser failed");
                        }
                    }
                }
            }
            Err(e) => {
                error!(error = %e, "Startup sync: RCON status query failed");
            }
        }

        // Also fetch initial serverinfo
        if let Ok(raw) = rcon.send("serverinfo").await {
            let mut g = game.write().await;
            for line in raw.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with("Server info") { continue; }
                let mut parts = line.splitn(2, char::is_whitespace);
                if let (Some(key), Some(val)) = (parts.next(), parts.next()) {
                    let (key, val) = (key.trim().to_string(), val.trim().to_string());
                    match key.as_str() {
                        "sv_hostname" => g.hostname = Some(val.clone()),
                        "sv_maxclients" => g.max_clients = val.parse().ok(),
                        "g_gametype" => g.game_type = Some(val.clone()),
                        "mapname" => g.map_name = Some(val.clone()),
                        _ => {}
                    }
                    g.server_info.insert(key, val);
                }
            }
            info!("Startup sync: serverinfo loaded");
        }
    }

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
