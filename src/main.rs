use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

use clap::Parser;
use tokio::sync::{broadcast, mpsc, RwLock};
use tracing::{error, info, warn};

use rusty_rules_referee::config::{RefereeConfig, RunMode};
use rusty_rules_referee::core::context::BotContext;
use rusty_rules_referee::core::log_tailer::LogTailer;
use rusty_rules_referee::core::{Client, Clients, Game, Team};
use rusty_rules_referee::events::{Event, EventRegistry};
use rusty_rules_referee::parsers::{GameParser, LogLine, ParsedAction};
use rusty_rules_referee::parsers::urbanterror::UrbanTerrorParser;
use rusty_rules_referee::plugins::admin::AdminPlugin;
use rusty_rules_referee::plugins::adv::AdvPlugin;
use rusty_rules_referee::plugins::afk::AfkPlugin;
use rusty_rules_referee::plugins::callvote::CallvotePlugin;
use rusty_rules_referee::plugins::censor::CensorPlugin;
use rusty_rules_referee::plugins::censorurt::CensorurtPlugin;
use rusty_rules_referee::plugins::chatlogger::ChatLogPlugin;
use rusty_rules_referee::plugins::countryfilter::CountryFilterPlugin;
use rusty_rules_referee::plugins::customcommands::CustomcommandsPlugin;
use rusty_rules_referee::plugins::discord::DiscordPlugin;
use rusty_rules_referee::plugins::firstkill::FirstkillPlugin;
use rusty_rules_referee::plugins::flagannounce::FlagannouncePlugin;
use rusty_rules_referee::plugins::follow::FollowPlugin;
use rusty_rules_referee::plugins::geowelcome::GeowelcomePlugin;
use rusty_rules_referee::plugins::headshotcounter::HeadshotCounterPlugin;
use rusty_rules_referee::plugins::login::LoginPlugin;
use rusty_rules_referee::plugins::makeroom::MakeroomPlugin;
use rusty_rules_referee::plugins::mapconfig::MapconfigPlugin;
use rusty_rules_referee::plugins::namechecker::NameCheckerPlugin;
use rusty_rules_referee::plugins::nickreg::NickregPlugin;
use rusty_rules_referee::plugins::pingwatch::PingWatchPlugin;
use rusty_rules_referee::plugins::poweradminurt::PowerAdminUrtPlugin;
use rusty_rules_referee::plugins::scheduler::SchedulerPlugin;
use rusty_rules_referee::plugins::spamcontrol::SpamControlPlugin;
use rusty_rules_referee::plugins::spawnkill::SpawnkillPlugin;
use rusty_rules_referee::plugins::specchecker::SpecCheckerPlugin;
use rusty_rules_referee::plugins::spree::SpreePlugin;
use rusty_rules_referee::plugins::stats::StatsPlugin;
use rusty_rules_referee::plugins::tk::TkPlugin;
use rusty_rules_referee::plugins::vpncheck::VpncheckPlugin;
use rusty_rules_referee::plugins::welcome::WelcomePlugin;
use rusty_rules_referee::plugins::xlrstats::XlrstatsPlugin;
use rusty_rules_referee::plugins::PluginRegistry;
use rusty_rules_referee::rcon::RconClient;
use rusty_rules_referee::storage;
use rusty_rules_referee::sync;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const BUILD_HASH: &str = env!("BUILD_HASH");

#[derive(Parser)]
#[command(name = "rusty-rules-referee", about = "Game server administration bot")]
struct Cli {
    /// Path to the configuration file.
    #[arg(default_value = "referee.toml")]
    config: String,

    /// Run mode: standalone (default), master, or client.
    #[arg(long, default_value = "standalone")]
    mode: RunMode,

    /// Print the build hash and exit.
    #[arg(long)]
    build_hash: bool,
}

/// Create the Urban Terror 4.3 game parser.
fn create_parser(
    rcon: Arc<RconClient>,
    event_registry: Arc<EventRegistry>,
) -> Arc<dyn GameParser> {
    Arc::new(UrbanTerrorParser::new(rcon, event_registry))
}

/// Resolve a host (IP or DNS name) and port into a SocketAddr.
///
/// Accepts numeric IPs directly and uses DNS resolution for hostnames.
/// Returns a descriptive error if resolution fails.
async fn resolve_host_port(host: &str, port: u16) -> anyhow::Result<SocketAddr> {
    // Fast path: already a numeric IP + port
    if let Ok(addr) = format!("{}:{}", host, port).parse::<SocketAddr>() {
        return Ok(addr);
    }
    // DNS lookup
    let target = format!("{}:{}", host, port);
    match tokio::net::lookup_host(target.clone()).await {
        Ok(mut iter) => iter
            .next()
            .ok_or_else(|| anyhow::anyhow!("DNS lookup for '{}' returned no addresses", target)),
        Err(e) => Err(anyhow::anyhow!(
            "Failed to resolve address '{}': {}",
            target,
            e
        )),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Install the ring crypto provider for rustls before anything else
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!("Rusty Rules Referee v{VERSION} (build {BUILD_HASH})");
    info!("====================================================");

    // Parse CLI arguments
    let cli = Cli::parse();

    // --build-hash flag: print build hash and exit (used by push-update scripts)
    if cli.build_hash {
        println!("{}", BUILD_HASH);
        return Ok(());
    }

    let config_path = cli.config;
    let mode = cli.mode;

    info!(mode = %mode, "Run mode");

    let config = match RefereeConfig::from_file(Path::new(&config_path)) {
        Ok(c) => {
            info!(bot = %c.referee.bot_name, "Configuration loaded");
            c
        }
        Err(e) => {
            error!(path = %config_path, error = %e, "Failed to load configuration");
            eprintln!("ERROR: Could not load config file '{}': {}", config_path, e);
            eprintln!("Usage: rusty-rules-referee [--mode standalone|master|client] [config.toml]");
            std::process::exit(1);
        }
    };

    // Validate config for the selected mode
    if let Err(e) = config.validate_for_mode(mode) {
        error!(mode = %mode, error = %e, "Configuration validation failed");
        eprintln!("ERROR: {}", e);
        std::process::exit(1);
    }

    match mode {
        RunMode::Standalone => run_standalone(config, config_path).await,
        RunMode::Master => run_master(config, config_path).await,
        RunMode::Client => run_client(config, config_path).await,
    }
}

// ============================================================================
// Standalone mode — self-contained bot (original behavior)
// ============================================================================

async fn run_standalone(config: RefereeConfig, config_path: String) -> anyhow::Result<()> {
    info!("Starting in STANDALONE mode");


    // Set up event registry
    let event_registry = Arc::new(EventRegistry::new());

    // Set up RCON client
    let rcon_addr = resolve_host_port(config.rcon_ip(), config.rcon_port()).await?;
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
    plugins.register(Box::new(PowerAdminUrtPlugin::new()))?;
    plugins.register(Box::new(CensorPlugin::new()))?;
    plugins.register(Box::new(CensorurtPlugin::new()))?;
    plugins.register(Box::new(SpamControlPlugin::new()))?;
    plugins.register(Box::new(TkPlugin::new()))?;
    plugins.register(Box::new(WelcomePlugin::new()))?;
    plugins.register(Box::new(GeowelcomePlugin::new()))?;
    plugins.register(Box::new(ChatLogPlugin::new()))?;
    plugins.register(Box::new(PingWatchPlugin::new()))?;
    plugins.register(Box::new(CountryFilterPlugin::new()))?;
    plugins.register(Box::new(StatsPlugin::new()))?;
    plugins.register(Box::new(XlrstatsPlugin::new()))?;
    plugins.register(Box::new(NameCheckerPlugin::new()))?;
    plugins.register(Box::new(SpecCheckerPlugin::new()))?;
    plugins.register(Box::new(HeadshotCounterPlugin::new()))?;
    plugins.register(Box::new(AdvPlugin::new()))?;
    plugins.register(Box::new(AfkPlugin::new()))?;
    plugins.register(Box::new(CallvotePlugin::new()))?;
    plugins.register(Box::new(CustomcommandsPlugin::new()))?;
    plugins.register(Box::new(DiscordPlugin::new()))?;
    plugins.register(Box::new(FirstkillPlugin::new()))?;
    plugins.register(Box::new(FlagannouncePlugin::new()))?;
    plugins.register(Box::new(FollowPlugin::new()))?;
    plugins.register(Box::new(LoginPlugin::new()))?;
    plugins.register(Box::new(MakeroomPlugin::new()))?;
    plugins.register(Box::new(MapconfigPlugin::new()))?;
    plugins.register(Box::new(NickregPlugin::new()))?;
    plugins.register(Box::new(SchedulerPlugin::new()))?;
    plugins.register(Box::new(SpawnkillPlugin::new()))?;
    plugins.register(Box::new(SpreePlugin::new()))?;
    plugins.register(Box::new(VpncheckPlugin::new()))?;

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
                Some(web_ctx),
                web_config,
                web_config_path,
                web_storage,
                web_event_tx,
                None, // No connected_clients in standalone mode
                None, // No pending_responses in standalone mode
                None, // No pending_client_requests in standalone mode
                None, // No client_versions in standalone mode
            ).await {
                error!(error = %e, "Web admin server failed");
            }
        });
    }

    // Start auto-update checker if enabled
    if config.update.enabled {
        let update_config = config.update.clone();
        tokio::spawn(async move {
            rusty_rules_referee::update::run_update_loop(update_config, BUILD_HASH).await;
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
                            client.current_name = Some(info.name.clone());
                            client.name = info.name.clone();
                            if !info.auth.is_empty() {
                                client.auth_name = Some(info.auth.clone());
                                client.auth = info.auth.clone();
                            }
                            if !info.cg_rgb.is_empty() {
                                client.armband = Some(info.cg_rgb.clone());
                            }

                            // Save alias (track name history)
                            if client.id > 0 && !info.name.is_empty() {
                                if let Err(e) = handler_storage.save_alias(client.id, &info.name).await {
                                    error!(error = %e, "Failed to save alias");
                                }
                            }

                            // Save updated client (including auth) to DB
                            if client.id > 0 {
                                if let Err(e) = handler_storage.save_client(&client).await {
                                    error!(error = %e, "Failed to save client on connect");
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

            // --- Client auth from AccountValidated ---
            if evt_key == Some("EVT_CLIENT_AUTH") {
                if let Some(cid) = &event.client_id {
                    if let rusty_rules_referee::events::EventData::Text(ref auth_val) = event.data {
                        let cid_str = cid.to_string();
                        let auth_clone = auth_val.clone();
                        handler_clients.update(&cid_str, |c| {
                            c.auth = auth_clone.clone();
                            c.auth_name = Some(auth_clone);
                        }).await;
                        // Persist auth to DB
                        if let Some(c) = handler_clients.get_by_cid(&cid.to_string()).await {
                            if c.id > 0 {
                                if let Err(e) = handler_storage.save_client(&c).await {
                                    error!(error = %e, "Failed to save client auth");
                                }
                            }
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

            // --- Client info change (team updates) ---
            if evt_key == Some("EVT_CLIENT_INFO_CHANGE") {
                if let Some(cid) = &event.client_id {
                    if let rusty_rules_referee::events::EventData::Text(ref json) = event.data {
                        if let Ok(pairs) = serde_json::from_str::<Vec<(String, String)>>(json) {
                            for (k, v) in &pairs {
                                if k == "t" {
                                    let team = Team::from_str_urt(v);
                                    handler_clients.update(&cid.to_string(), |c| {
                                        c.team = team;
                                    }).await;
                                    break;
                                }
                            }
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
        let poller_storage = db.clone();
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
                                let mut cg_rgb: Option<String> = None;
                                let mut current_name: Option<String> = None;
                                for line in raw.lines() {
                                    let trimmed = line.trim();
                                    let mut parts = trimmed.splitn(2, char::is_whitespace);
                                    if let (Some(key), Some(val)) = (parts.next(), parts.next()) {
                                        let val = val.trim();
                                        match key {
                                            "gear" => gear = Some(val.to_string()),
                                            "authl" => auth = Some(val.to_string()),
                                            "cg_rgb" => cg_rgb = Some(val.to_string()),
                                            "name" | "n" => current_name = Some(val.to_string()),
                                            _ => {}
                                        }
                                    }
                                }
                                let g = gear;
                                let a = auth;
                                let rgb = cg_rgb;
                                let cn = current_name;
                                let need_auth_save = a.as_ref().map_or(false, |v| !v.is_empty());
                                poller_clients.update(cid, |c| {
                                    if let Some(ref v) = g { c.gear = Some(v.clone()); }
                                    if let Some(ref v) = a {
                                        c.auth_name = Some(v.clone());
                                        if !v.is_empty() && c.auth != *v {
                                            c.auth = v.clone();
                                        }
                                    }
                                    if let Some(ref v) = rgb { c.armband = Some(v.clone()); }
                                    if let Some(ref v) = cn { c.current_name = Some(v.clone()); }
                                }).await;
                                // Persist auth if it was updated
                                if need_auth_save {
                                    if let Some(c) = poller_clients.get_by_cid(cid).await {
                                        if c.id > 0 {
                                            let _ = poller_storage.save_client(&c).await;
                                        }
                                    }
                                }
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
                                client.auth = info.auth.clone();
                            }
                            if !info.cg_rgb.is_empty() {
                                client.armband = Some(info.cg_rgb.clone());
                            }
                            if let Ok(ip) = info.ip.parse() {
                                client.ip = Some(ip);
                            }
                            client.current_name = Some(info.name.clone());
                            client.name = info.name.clone();

                            if client.id > 0 && !info.name.is_empty() {
                                let _ = db.save_alias(client.id, &info.name).await;
                            }

                            // Save updated client (including auth) to DB
                            if client.id > 0 {
                                if let Err(e) = db.save_client(&client).await {
                                    error!(error = %e, "Startup sync: failed to save client auth");
                                }
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

        // Fetch initial teams via RCON `players` command
        if let Ok(raw) = rcon.send("players").await {
            for line in raw.lines() {
                let line = line.trim();
                // Format: "0:PlayerName TEAM:BLUE KILLS:0 ..."
                if let Some(colon_pos) = line.find(':') {
                    if let Ok(_slot_num) = line[..colon_pos].parse::<u32>() {
                        let slot = line[..colon_pos].to_string();
                        if let Some(team_start) = line.find("TEAM:") {
                            let team_str = &line[team_start + 5..];
                            let team_val = team_str.split_whitespace().next().unwrap_or("");
                            let team = Team::from_str_urt(team_val);
                            clients.update(&slot, |c| { c.team = team; }).await;
                        }
                    }
                }
            }
            info!("Startup sync: player teams loaded");
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
        let delay = std::time::Duration::from_secs_f64(config.server.delay);
        // Retry opening the log forever so the service doesn't exit (and
        // systemd-restart-loop) just because the game server isn't running
        // yet or the path is wrong. When the file appears we pick it up.
        loop {
            info!(path = %log_path, "Starting log tailer");
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
                    // Tailer returned None — the file ended / rotated. Retry.
                    warn!(path = %log_path, "Log tailer stopped, retrying in 30s");
                    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                }
                Err(e) => {
                    error!(path = %log_path, error = %e, "Cannot open game log — staying alive, will retry in 60s");
                    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                }
            }
        }
    } else {
        info!("No game log configured — running in RCON-only mode");
        info!("Press Ctrl+C to stop");
        tokio::signal::ctrl_c().await?;
    }

    // Shutdown (unreachable unless ctrl_c path above)
    #[allow(unreachable_code)]
    {
        drop(event_tx);
        handler_task.await?;
        info!("Rusty Rules Referee shutdown complete");
        Ok(())
    }
}

// ============================================================================
// Master mode — central server hosting database, web UI, sync API
// ============================================================================

async fn run_master(config: RefereeConfig, config_path: String) -> anyhow::Result<()> {
    info!("Starting in MASTER mode");

    let master_config = config.master.as_ref().expect("master config validated");

    // Connect to database
    let db: Arc<dyn storage::Storage> =
        Arc::from(storage::create_storage(&config.referee.database).await?);
    info!("Database connected");

    // Broadcast channel for event streaming to web UI
    let (ws_event_tx, _ws_event_rx) = broadcast::channel::<Event>(256);

    // Broadcast channel for internal events from client bots
    let (internal_event_tx, _) = broadcast::channel::<sync::protocol::EventPayload>(1024);

    // Shared connected_clients map — used by both sync API and web API
    let connected_clients = Arc::new(RwLock::new(std::collections::HashMap::<i64, sync::master::ConnectedClient>::new()));

    // Shared pending request maps — used by both sync API and web API
    let pending_responses = Arc::new(RwLock::new(std::collections::HashMap::<String, tokio::sync::oneshot::Sender<sync::protocol::ClientResponse>>::new()));
    let pending_client_requests = Arc::new(RwLock::new(std::collections::HashMap::<i64, Vec<(String, sync::protocol::ClientRequest)>>::new()));

    // Shared client version map — updated by heartbeat, read by web API
    let client_versions = Arc::new(RwLock::new(std::collections::HashMap::<i64, sync::master::ClientVersionInfo>::new()));

    // Start the web admin server if enabled
    if config.web.enabled {
        let web_config = config.clone();
        let web_config_path = config_path.clone();
        let web_storage = db.clone();
        let web_event_tx = ws_event_tx.clone();
        let web_connected = connected_clients.clone();
        let web_pending_responses = pending_responses.clone();
        let web_pending_client_requests = pending_client_requests.clone();
        let web_client_versions = client_versions.clone();
        tokio::spawn(async move {
            if let Err(e) = rusty_rules_referee::web::start_server(
                None, // No local BotContext in master mode
                web_config,
                web_config_path,
                web_storage,
                web_event_tx,
                Some(web_connected),
                Some(web_pending_responses),
                Some(web_pending_client_requests),
                Some(web_client_versions),
            ).await {
                error!(error = %e, "Web admin server failed");
            }
        });
    }

    // Start the internal sync API (mTLS)
    let sync_storage = db.clone();
    let sync_config = master_config.clone();
    let sync_connected = connected_clients.clone();
    let sync_pending_responses = pending_responses.clone();
    let sync_pending_client_requests = pending_client_requests.clone();
    let sync_client_versions = client_versions.clone();
    tokio::spawn(async move {
        if let Err(e) = sync::master::start_master_api(
            &sync_config,
            sync_storage,
            internal_event_tx,
            sync_connected,
            sync_pending_responses,
            sync_pending_client_requests,
            sync_client_versions,
        ).await {
            error!(error = %e, "Master internal API failed");
        }
    });

    // Map repository background refresher (scrapes external .pk3 autoindexes).
    if config.map_repo.enabled {
        rusty_rules_referee::maprepo::spawn_refresher(
            db.clone(),
            config.map_repo.sources.clone(),
            config.map_repo.refresh_interval_hours,
        );
    }

    // Health monitor: periodically check for offline servers
    let health_storage = db.clone();
    tokio::spawn(async move {
        let timeout = chrono::Duration::seconds(60);
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            if let Ok(servers) = health_storage.get_servers().await {
                let now = chrono::Utc::now();
                for server in servers {
                    if server.status == "online" {
                        if let Some(last_seen) = server.last_seen {
                            if now - last_seen > timeout {
                                warn!(
                                    server_id = server.id,
                                    name = %server.name,
                                    "Server heartbeat timeout, marking offline"
                                );
                                let _ = health_storage.update_server_status(
                                    server.id, "offline", None, 0, 0,
                                ).await;
                            }
                        }
                    }
                }
            }
        }
    });

    // Start auto-update checker if enabled
    if config.update.enabled {
        let update_config = config.update.clone();
        tokio::spawn(async move {
            rusty_rules_referee::update::run_update_loop(update_config, BUILD_HASH).await;
        });
    }

    info!("Master server running. Press Ctrl+C to stop.");
    tokio::signal::ctrl_c().await?;
    info!("Master server shutdown complete");
    Ok(())
}

// ============================================================================
// Client mode — game server bot that syncs with master
// ============================================================================

async fn run_client(config: RefereeConfig, config_path: String) -> anyhow::Result<()> {
    info!("Starting in CLIENT mode");

    let client_config = config.client.as_ref().expect("client config validated").clone();

    // Check if game server is configured yet
    let server_configured = config.server.is_configured();

    // Connect to local SQLite cache database
    let db: Arc<dyn storage::Storage> =
        Arc::from(storage::create_storage(&config.referee.database).await?);
    info!("Local cache database connected");

    // Shared release-channel watch. Populated from local config at startup;
    // the sync manager updates this when the master pushes a new channel.
    let update_channel = std::sync::Arc::new(tokio::sync::RwLock::new(config.update.channel.clone()));

    // Set up the sync manager (always needed)
    let (sync_manager, mut sync_handle) = sync::client::ClientSyncManager::new(
        client_config.clone(),
        db.clone(),
        config_path.clone(),
        update_channel.clone(),
    );

    // Spawn the sync manager
    tokio::spawn(async move {
        if let Err(e) = sync_manager.run().await {
            error!(error = %e, "Client sync manager failed");
        }
    });

    if !server_configured {
        info!("Game server not configured — waiting for configuration from master...");
        info!("Configure this server from the master's web dashboard.");

        // Start auto-update checker if enabled
        if config.update.enabled {
            let update_config = config.update.clone();
            let channel_watch = update_channel.clone();
            tokio::spawn(async move {
                rusty_rules_referee::update::run_update_loop_with_channel(
                    update_config,
                    BUILD_HASH,
                    Some(channel_watch),
                ).await;
            });
        }

        // Wait for config push from master — sync manager will write config and signal restart
        loop {
            if sync_handle.config_updated.changed().await.is_ok() {
                info!("Game server configuration received from master — restarting...");
                // Re-read the config file that the sync manager updated
                let new_config = RefereeConfig::from_file(std::path::Path::new(&config_path))?;
                if new_config.server.is_configured() {
                    info!("Configuration is valid, restarting client with game server connection");
                    // Recursive call with the new config — clean restart of the full client
                    return Box::pin(run_client(new_config, config_path)).await;
                }
                warn!("Received config update but game server still not configured, continuing to wait...");
            }
        }
    }

    // --- Game server is configured — proceed with full client startup ---

    // Set up event registry
    let event_registry = Arc::new(EventRegistry::new());

    // Set up RCON client — resolve address (supports hostnames).
    // On failure, log and keep the service alive rather than crash-looping via systemd.
    let rcon_addr = match resolve_host_port(config.rcon_ip(), config.rcon_port()).await {
        Ok(addr) => addr,
        Err(e) => {
            error!(
                host = %config.rcon_ip(),
                port = config.rcon_port(),
                error = %e,
                "Failed to resolve RCON address — fix the 'address' setting in the master UI. Staying alive and will retry on next config update."
            );
            // Park the task indefinitely so the service doesn't crash-loop.
            // A config update (via master) will trigger a restart of run_client.
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(300)).await;
                if let Ok(addr) = resolve_host_port(config.rcon_ip(), config.rcon_port()).await {
                    info!(%addr, "RCON address now resolvable, continuing startup");
                    break addr;
                }
            }
        }
    };
    let rcon = Arc::new(RconClient::new(rcon_addr, &config.server.rcon_password));
    info!(addr = %rcon_addr, "RCON client configured");

    // Create game state
    let game = Arc::new(RwLock::new(Game::new("iourt43")));

    // Create the game parser
    let parser = create_parser(rcon.clone(), event_registry.clone());
    info!(game = parser.game_name(), "Game parser initialized");

    // Create the connected-clients manager
    let clients = Arc::new(Clients::new());

    // Create BotContext
    let ctx = Arc::new(BotContext::new(
        rcon.clone(),
        db.clone(),
        game.clone(),
        event_registry.clone(),
        parser.clone(),
        clients.clone(),
    ));

    // Give the sync manager live access to game state so heartbeats can
    // report current map, player count, and max clients to the master.
    sync_handle.attach_game_state(game.clone(), clients.clone()).await;
    // Also attach the full BotContext so master-initiated handlers have
    // RCON/storage/game state access.
    sync_handle.attach_bot_context(ctx.clone()).await;

    // Watch for config pushes from the master and exit the process cleanly
    // so systemd (or the supervisor) restarts us with the freshly-written
    // referee.toml. This is the simplest way to fully rebuild the plugin
    // registry with new settings without a complex live-reload path.
    {
        let mut config_watch = sync_handle.config_updated.clone();
        tokio::spawn(async move {
            loop {
                if config_watch.changed().await.is_err() {
                    break;
                }
                warn!("Configuration updated by master — exiting for restart");
                // Give the sync manager a moment to flush writes, then exit.
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                std::process::exit(0);
            }
        });
    }

    // Set up plugins (all execute locally)
    let mut plugins = PluginRegistry::new();
    plugins.register(Box::new(AdminPlugin::new()))?;
    plugins.register(Box::new(PowerAdminUrtPlugin::new()))?;
    plugins.register(Box::new(CensorPlugin::new()))?;
    plugins.register(Box::new(CensorurtPlugin::new()))?;
    plugins.register(Box::new(SpamControlPlugin::new()))?;
    plugins.register(Box::new(TkPlugin::new()))?;
    plugins.register(Box::new(WelcomePlugin::new()))?;
    plugins.register(Box::new(GeowelcomePlugin::new()))?;
    plugins.register(Box::new(ChatLogPlugin::new()))?;
    plugins.register(Box::new(PingWatchPlugin::new()))?;
    plugins.register(Box::new(CountryFilterPlugin::new()))?;
    plugins.register(Box::new(StatsPlugin::new()))?;
    plugins.register(Box::new(XlrstatsPlugin::new()))?;
    plugins.register(Box::new(NameCheckerPlugin::new()))?;
    plugins.register(Box::new(SpecCheckerPlugin::new()))?;
    plugins.register(Box::new(HeadshotCounterPlugin::new()))?;
    plugins.register(Box::new(AdvPlugin::new()))?;
    plugins.register(Box::new(AfkPlugin::new()))?;
    plugins.register(Box::new(CallvotePlugin::new()))?;
    plugins.register(Box::new(CustomcommandsPlugin::new()))?;
    plugins.register(Box::new(DiscordPlugin::new()))?;
    plugins.register(Box::new(FirstkillPlugin::new()))?;
    plugins.register(Box::new(FlagannouncePlugin::new()))?;
    plugins.register(Box::new(FollowPlugin::new()))?;
    plugins.register(Box::new(LoginPlugin::new()))?;
    plugins.register(Box::new(MakeroomPlugin::new()))?;
    plugins.register(Box::new(MapconfigPlugin::new()))?;
    plugins.register(Box::new(NickregPlugin::new()))?;
    plugins.register(Box::new(SchedulerPlugin::new()))?;
    plugins.register(Box::new(SpawnkillPlugin::new()))?;
    plugins.register(Box::new(SpreePlugin::new()))?;
    plugins.register(Box::new(VpncheckPlugin::new()))?;

    plugins.startup_all(&config.plugins).await?;
    info!("All plugins started");

    // Spawn a task to handle commands from master
    let cmd_ctx = ctx.clone();
    tokio::spawn(async move {
        while let Some(msg) = sync_handle.command_rx.recv().await {
            match msg {
                sync::protocol::SyncMessage::Command(cmd) => {
                    info!(id = %cmd.command_id, "Received remote command from master");
                    match cmd.action {
                        sync::protocol::RemoteAction::Rcon { command } => {
                            let _ = cmd_ctx.write(&command).await;
                        }
                        sync::protocol::RemoteAction::Kick { cid, reason } => {
                            let _ = cmd_ctx.kick(&cid, &reason).await;
                        }
                        sync::protocol::RemoteAction::Ban { cid, reason } => {
                            let _ = cmd_ctx.ban(&cid, &reason).await;
                        }
                        sync::protocol::RemoteAction::TempBan { cid, reason, duration_minutes } => {
                            let _ = cmd_ctx.temp_ban(&cid, &reason, duration_minutes as u32).await;
                        }
                        sync::protocol::RemoteAction::Say { message } => {
                            let _ = cmd_ctx.say(&message).await;
                        }
                        sync::protocol::RemoteAction::Message { cid, message } => {
                            let _ = cmd_ctx.message(&cid, &message).await;
                        }
                        sync::protocol::RemoteAction::Unban { .. } => {
                            // TODO: implement unban by client_id
                        }
                    }
                }
                sync::protocol::SyncMessage::GlobalPenalty(penalty) => {
                    info!(
                        client = %penalty.client_name,
                        penalty_type = %penalty.penalty_type,
                        "Received global penalty from master"
                    );
                    // TODO: enforce global penalty locally
                }
                sync::protocol::SyncMessage::ConfigUpdate(config_sync) => {
                    info!(
                        version = config_sync.config_version,
                        "Received config update from master"
                    );
                    // TODO: apply config update
                }
                _ => {}
            }
        }
    });

    // Start auto-update checker if enabled
    if config.update.enabled {
        let update_config = config.update.clone();
        let channel_watch = update_channel.clone();
        tokio::spawn(async move {
            rusty_rules_referee::update::run_update_loop_with_channel(
                update_config,
                BUILD_HASH,
                Some(channel_watch),
            ).await;
        });
    }

    // Event queue (channel between log reader and event handler)
    let (event_tx, mut event_rx) = mpsc::channel::<Event>(1024);

    // Spawn the event handler task (same as standalone, plus sync forwarding)
    let handler_plugins = plugins;
    let handler_ctx = ctx.clone();
    let handler_event_registry = event_registry.clone();
    let handler_parser_raw = Arc::new(UrbanTerrorParser::new(rcon.clone(), event_registry.clone()));
    let handler_clients = clients.clone();
    let handler_storage = db.clone();
    let handler_game = game.clone();
    let sync_event_tx = sync_handle.event_tx.clone();
    let handler_task = tokio::spawn(async move {
        info!("Event handler started (client mode)");
        while let Some(event) = event_rx.recv().await {
            let evt_key = handler_event_registry.get_key(event.event_type);

            // --- Client auth flow (same as standalone) ---
            if evt_key == Some("EVT_CLIENT_CONNECT") {
                if let Some(cid) = &event.client_id {
                    let cid_str = cid.to_string();
                    match handler_parser_raw.dumpuser(&cid_str).await {
                        Ok(info) => {
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
                                    match handler_storage.save_client(&c).await {
                                        Ok(new_id) => c.id = new_id,
                                        Err(e) => error!(error = %e, "Failed to save new client"),
                                    }
                                    c
                                }
                            };

                            client.cid = Some(cid_str.clone());
                            client.connected = true;
                            if let Ok(ip) = info.ip.parse() {
                                client.ip = Some(ip);
                            }
                            client.current_name = Some(info.name.clone());
                            client.name = info.name.clone();
                            if !info.auth.is_empty() {
                                client.auth_name = Some(info.auth.clone());
                                client.auth = info.auth.clone();
                            }
                            if !info.cg_rgb.is_empty() {
                                client.armband = Some(info.cg_rgb.clone());
                            }

                            if client.id > 0 && !info.name.is_empty() {
                                if let Err(e) = handler_storage.save_alias(client.id, &info.name).await {
                                    error!(error = %e, "Failed to save alias");
                                }
                            }

                            // Save updated client (including auth) to DB
                            if client.id > 0 {
                                if let Err(e) = handler_storage.save_client(&client).await {
                                    error!(error = %e, "Failed to save client on connect");
                                }
                            }

                            handler_clients.connect(&cid_str, client).await;

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

            // --- Client auth from AccountValidated ---
            if evt_key == Some("EVT_CLIENT_AUTH") {
                if let Some(cid) = &event.client_id {
                    if let rusty_rules_referee::events::EventData::Text(ref auth_val) = event.data {
                        let cid_str = cid.to_string();
                        let auth_clone = auth_val.clone();
                        handler_clients.update(&cid_str, |c| {
                            c.auth = auth_clone.clone();
                            c.auth_name = Some(auth_clone);
                        }).await;
                        // Persist auth to DB
                        if let Some(c) = handler_clients.get_by_cid(&cid.to_string()).await {
                            if c.id > 0 {
                                if let Err(e) = handler_storage.save_client(&c).await {
                                    error!(error = %e, "Failed to save client auth");
                                }
                            }
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

            // --- Client info change (team updates) ---
            if evt_key == Some("EVT_CLIENT_INFO_CHANGE") {
                if let Some(cid) = &event.client_id {
                    if let rusty_rules_referee::events::EventData::Text(ref json) = event.data {
                        if let Ok(pairs) = serde_json::from_str::<Vec<(String, String)>>(json) {
                            for (k, v) in &pairs {
                                if k == "t" {
                                    let team = Team::from_str_urt(v);
                                    handler_clients.update(&cid.to_string(), |c| {
                                        c.team = team;
                                    }).await;
                                    break;
                                }
                            }
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

            // Forward event to sync manager for master relay
            let _ = sync_event_tx.send(event).await;
        }
        info!("Event handler stopped");
    });

    // Spawn background RCON poller (same as standalone)
    {
        let poller_rcon = rcon.clone();
        let poller_clients = clients.clone();
        let poller_game = game.clone();
        let poller_storage = db.clone();
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

                if tick % 3 == 0 {
                    let all = poller_clients.get_all().await;
                    for client in &all {
                        if let Some(ref cid) = client.cid {
                            if let Ok(raw) = poller_rcon.send(&format!("dumpuser {}", cid)).await {
                                let mut gear: Option<String> = None;
                                let mut auth: Option<String> = None;
                                let mut cg_rgb: Option<String> = None;
                                let mut current_name: Option<String> = None;
                                for line in raw.lines() {
                                    let trimmed = line.trim();
                                    let mut parts = trimmed.splitn(2, char::is_whitespace);
                                    if let (Some(key), Some(val)) = (parts.next(), parts.next()) {
                                        let val = val.trim();
                                        match key {
                                            "gear" => gear = Some(val.to_string()),
                                            "authl" => auth = Some(val.to_string()),
                                            "cg_rgb" => cg_rgb = Some(val.to_string()),
                                            "name" | "n" => current_name = Some(val.to_string()),
                                            _ => {}
                                        }
                                    }
                                }
                                let g = gear;
                                let a = auth;
                                let rgb = cg_rgb;
                                let cn = current_name;
                                let need_auth_save = a.as_ref().map_or(false, |v| !v.is_empty());
                                poller_clients.update(cid, |c| {
                                    if let Some(ref v) = g { c.gear = Some(v.clone()); }
                                    if let Some(ref v) = a {
                                        c.auth_name = Some(v.clone());
                                        if !v.is_empty() && c.auth != *v {
                                            c.auth = v.clone();
                                        }
                                    }
                                    if let Some(ref v) = rgb { c.armband = Some(v.clone()); }
                                    if let Some(ref v) = cn { c.current_name = Some(v.clone()); }
                                }).await;
                                // Persist auth if it was updated
                                if need_auth_save {
                                    if let Some(c) = poller_clients.get_by_cid(cid).await {
                                        if c.id > 0 {
                                            let _ = poller_storage.save_client(&c).await;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

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
        info!("Background RCON poller started");
    }

    // Startup sync: discover already-connected players
    {
        let sync_parser = UrbanTerrorParser::new(rcon.clone(), event_registry.clone());
        match sync_parser.get_status_players().await {
            Ok(status_players) => {
                info!(count = status_players.len(), "Startup sync: found players from RCON status");
                for sp in &status_players {
                    match sync_parser.dumpuser(&sp.slot).await {
                        Ok(info) => {
                            if info.guid.is_empty() { continue; }
                            let db_client = db.get_client_by_guid(&info.guid).await.ok();
                            let mut client = match db_client {
                                Some(existing) => existing,
                                None => {
                                    let mut c = Client::new(&info.guid, &info.name);
                                    c.cid = Some(sp.slot.clone());
                                    if let Ok(ip) = info.ip.parse() { c.ip = Some(ip); }
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
                                client.auth = info.auth.clone();
                            }
                            if !info.cg_rgb.is_empty() {
                                client.armband = Some(info.cg_rgb.clone());
                            }
                            if let Ok(ip) = info.ip.parse() { client.ip = Some(ip); }
                            client.current_name = Some(info.name.clone());
                            client.name = info.name.clone();

                            if client.id > 0 && !info.name.is_empty() {
                                let _ = db.save_alias(client.id, &info.name).await;
                            }

                            // Save updated client (including auth) to DB
                            if client.id > 0 {
                                if let Err(e) = db.save_client(&client).await {
                                    error!(error = %e, "Startup sync: failed to save client auth");
                                }
                            }

                            clients.connect(&sp.slot, client).await;
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

        // Fetch initial teams via RCON `players` command
        if let Ok(raw) = rcon.send("players").await {
            for line in raw.lines() {
                let line = line.trim();
                // Format: "0:PlayerName TEAM:BLUE KILLS:0 ..."
                if let Some(colon_pos) = line.find(':') {
                    if let Ok(_slot_num) = line[..colon_pos].parse::<u32>() {
                        let slot = line[..colon_pos].to_string();
                        if let Some(team_start) = line.find("TEAM:") {
                            let team_str = &line[team_start + 5..];
                            let team_val = team_str.split_whitespace().next().unwrap_or("");
                            let team = Team::from_str_urt(team_val);
                            clients.update(&slot, |c| { c.team = team; }).await;
                        }
                    }
                }
            }
            info!("Startup sync: player teams loaded");
        }
    }

    // Main loop: tail game log
    if let Some(ref log_path) = config.server.game_log {
        let delay = std::time::Duration::from_secs_f64(config.server.delay);
        // Retry opening the log forever so the service doesn't exit (and
        // systemd-restart-loop) just because the game server isn't running
        // yet or the path is wrong. When the file appears we pick it up.
        loop {
            info!(path = %log_path, "Starting log tailer");
            let mut tailer = LogTailer::new(Path::new(log_path), delay);
            match tailer.start().await {
                Ok(()) => {
                    info!(path = %log_path, "Log tailer active");
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
                    warn!(path = %log_path, "Log tailer stopped, retrying in 30s");
                    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                }
                Err(e) => {
                    error!(path = %log_path, error = %e, "Cannot open game log — staying alive, will retry in 60s");
                    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                }
            }
        }
    } else {
        info!("No game log configured — running in RCON-only mode");
        tokio::signal::ctrl_c().await?;
    }

    // Shutdown (unreachable unless ctrl_c path above)
    #[allow(unreachable_code)]
    {
        drop(event_tx);
        handler_task.await?;
        info!("Client bot shutdown complete");
        Ok(())
    }
}
