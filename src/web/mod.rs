pub mod api;
pub mod auth;
pub mod state;
pub mod ws;

use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    http::{header, Request, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{delete, get, post, put},
};
use rust_embed::Embed;
use tokio::sync::broadcast;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::info;

use crate::config::RefereeConfig;
use crate::core::context::BotContext;
use crate::core::AdminUser;
use crate::events::Event;
use crate::storage::Storage;

use self::state::AppState;

#[derive(Embed)]
#[folder = "ui/build"]
struct UiAssets;

/// Build the Axum router with all API routes and static file serving.
pub fn build_router(state: AppState) -> Router {
    let api = Router::new()
        // Auth
        .route("/auth/login", post(auth::login))
        .route("/auth/me", get(auth::me))
        // Config
        .route("/config", get(api::config::get_config))
        .route("/config", put(api::config::update_config))
        .route("/config/migrate-to-mysql", post(api::config::migrate_to_mysql))
        .route("/config/server-cfg", post(api::config::analyze_server_cfg))
        .route("/config/server-cfg/save", post(api::config::save_server_cfg))
        .route("/config/browse", post(api::config::browse_files))
        .route("/config/check-game-log", post(api::config::check_game_log))
        // Plugins
        .route("/plugins", get(api::plugins::list_plugins))
        // Players
        .route("/players", get(api::players::list_players))
        .route("/players/:id", get(api::players::get_player))
        .route("/players/:cid/kick", post(api::players::kick_player))
        .route("/players/:cid/ban", post(api::players::ban_player))
        .route("/players/:cid/message", post(api::players::message_player))
        .route("/players/:cid/mute", post(api::players::mute_player))
        .route("/players/:cid/unmute", post(api::players::unmute_player))
        .route("/players/:id/group", put(api::players::update_player_group))
        // Client search
        .route("/clients", get(api::players::list_all_clients))
        .route("/clients/search", get(api::players::search_clients))
        // Penalties
        .route("/penalties", get(api::penalties::list_penalties))
        .route("/penalties/:id/disable", post(api::penalties::disable_penalty))
        // Groups
        .route("/groups", get(api::groups::list_groups))
        // Aliases
        .route("/aliases", get(api::aliases::list_aliases))
        // Server
        .route("/server/status", get(api::server::server_status))
        .route("/server/rcon", post(api::server::rcon_command))
        .route("/server/say", post(api::server::server_say))
        .route("/server/maps", get(api::server::list_maps))
        .route("/server/maps/refresh", post(api::server::refresh_maps))
        .route("/server/maps/import", post(api::server::import_map))
        .route("/server/maps/missing", post(api::server::missing_maps))
        .route("/server/map", post(api::server::change_map))
        .route("/server/mapcycle", get(api::mapcycle::get_mapcycle))
        .route("/server/mapcycle", put(api::mapcycle::update_mapcycle))
        .route("/server/restart", post(api::server::restart_bot))
        // Stats
        .route("/stats/leaderboard", get(api::stats::leaderboard))
        .route("/stats/player/:id", get(api::stats::player_stats))
        .route("/stats/weapons", get(api::stats::weapon_stats))
        .route("/stats/maps", get(api::stats::map_stats))
        .route("/stats/summary", get(api::stats::summary))
        // Chat
        .route("/chat", get(api::chat::list_chat))
        // Commands documentation
        .route("/commands", get(api::commands::list_commands))
        // Votes
        .route("/votes", get(api::votes::list_votes))
        // Notes
        .route("/notes", get(api::notes::get_note))
        .route("/notes", put(api::notes::save_note))
        // Audit log
        .route("/audit-log", get(api::audit::list_audit_log))
        // Map configs (per-map settings)
        .route("/map-configs", get(api::mapconfigs::list_map_configs))
        .route("/map-configs", post(api::mapconfigs::create_map_config))
        .route("/map-configs/:id", get(api::mapconfigs::get_map_config))
        .route("/map-configs/:id", put(api::mapconfigs::update_map_config))
        .route("/map-configs/:id", delete(api::mapconfigs::delete_map_config))
        // Map repository (external .pk3 browser)
        .route("/map-repo", get(api::maprepo::search_map_repo))
        .route("/map-repo/refresh", post(api::maprepo::refresh_map_repo))
        .route("/map-repo/status", get(api::maprepo::map_repo_status))
        // Admin users
        .route("/users", get(api::users::list_users))
        .route("/users", post(api::users::create_user))
        .route("/users/me/password", put(api::users::change_password))
        .route("/users/:id", put(api::users::update_user))
        .route("/users/:id", delete(api::users::delete_user))
        // Quick-connect pairing (master mode)
        .route("/pairing/enable", post(api::pairing::enable_pairing))
        .route("/pairing/disable", post(api::pairing::disable_pairing))
        .route("/pairing/pair", post(api::pairing::pair_client))
        // Multi-server management (master mode)
        .route("/servers", get(api::servers::list_servers))
        .route("/servers/:id", get(api::servers::get_server))
        .route("/servers/:id", delete(api::servers::delete_server))
        .route("/servers/:id/rcon", post(api::servers::server_rcon))
        .route("/servers/:id/kick", post(api::servers::server_kick))
        .route("/servers/:id/ban", post(api::servers::server_ban))
        .route("/servers/:id/say", post(api::servers::server_say))
        .route("/servers/:id/message", post(api::servers::server_message))
        .route("/servers/:id/config", get(api::servers::get_server_config))
        .route("/servers/:id/config", put(api::servers::update_server_config))
        // Server setup (config scan, install, browse)
        .route("/servers/:id/scan-configs", post(api::servers::scan_server_configs))
        .route("/servers/:id/parse-config", post(api::servers::parse_server_config))
        .route("/servers/:id/browse", post(api::servers::browse_server_files))
        .route("/servers/:id/install-server", post(api::servers::install_game_server))
        .route("/servers/:id/install-status", get(api::servers::install_status))
        // Client version & forced update
        .route("/servers/:id/version", get(api::servers::get_server_version))
        .route("/servers/:id/force-update", post(api::servers::force_server_update))
        .route("/servers/:id/restart", post(api::servers::restart_server))
        .route("/servers/:id/update-channel", put(api::servers::set_server_update_channel))
        .route("/servers/:id/check-game-log", post(api::servers::check_server_game_log))
        // Map repo: per-server import + missing-map diff
        .route("/servers/:id/maps/import", post(api::servers::import_map))
        .route("/servers/:id/maps/missing", post(api::servers::missing_maps))
        // Phase 3 — per-server live control (standalone parity)
        .route("/servers/:id/live", get(api::server_control::server_live_status))
        .route("/servers/:id/players", get(api::server_control::server_players))
        .route("/servers/:id/players/:cid/mute", post(api::server_control::server_player_mute))
        .route("/servers/:id/players/:cid/unmute", post(api::server_control::server_player_unmute))
        .route("/servers/:id/maps", get(api::server_control::server_maps))
        .route("/servers/:id/maps/refresh", post(api::server_control::server_maps_refresh))
        .route("/servers/:id/map", post(api::server_control::server_change_map))
        .route("/servers/:id/mapcycle", get(api::server_control::server_get_mapcycle))
        .route("/servers/:id/mapcycle", put(api::server_control::server_set_mapcycle))
        .route("/servers/:id/server-cfg", get(api::server_control::server_get_server_cfg))
        .route("/servers/:id/server-cfg", put(api::server_control::server_save_server_cfg))
        .route("/servers/:id/map-configs", get(api::server_control::server_list_map_configs))
        .route("/servers/:id/map-configs", post(api::server_control::server_save_map_config))
        .route("/servers/:id/map-configs", put(api::server_control::server_save_map_config))
        .route("/servers/:id/map-configs/:map_config_id", delete(api::server_control::server_delete_map_config))
        .route("/servers/:id/penalties", get(api::server_control::server_penalties_history))
        .route("/servers/:id/chat", get(api::server_control::server_chat_history))
        .route("/servers/:id/audit-log", get(api::server_control::server_audit_log))
        .route("/servers/:id/plugins", get(api::server_control::server_list_plugins))
        .route("/servers/:id/plugins/:plugin_name", put(api::server_control::server_update_plugin))
        // First-run setup wizard
        .route("/setup/status", get(api::setup::setup_status))
        .route("/setup/complete", post(api::setup::complete_setup))
        .route("/setup/browse", post(api::setup::setup_browse))
        .route("/setup/scan-configs", post(api::setup::setup_scan_configs))
        .route("/setup/analyze-cfg", post(api::setup::setup_analyze_cfg))
        // Version & updates
        .route("/version", get(api::version::get_version))
        .route("/version/check", post(api::version::check_update))
        .route("/version/update", post(api::version::apply_latest_update));

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .nest("/api/v1", api)
        .route("/ws", get(ws::ws_handler))
        .fallback(static_handler)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Serve embedded static files, with SPA fallback to index.html.
async fn static_handler(req: Request<Body>) -> impl IntoResponse {
    let path = req.uri().path().trim_start_matches('/');

    // Try to serve the exact file
    if let Some(content) = UiAssets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        // Only immutable assets (content-hashed filenames) get long cache; everything else no-cache
        let cache = if path.contains("/immutable/") {
            "public, max-age=31536000, immutable"
        } else {
            "no-cache, no-store, must-revalidate"
        };
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime.as_ref())
            .header(header::CACHE_CONTROL, cache)
            .body(Body::from(content.data.to_vec()))
            .unwrap();
    }

    // SPA fallback: return index.html for any non-file route
    if let Some(content) = UiAssets::get("index.html") {
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .header(header::CACHE_CONTROL, "no-cache")
            .body(Body::from(content.data.to_vec()))
            .unwrap();
    }

    // No UI built yet — show a helpful message
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(Body::from(
            r#"<!DOCTYPE html>
<html>
<head><title>R3 Admin</title>
<style>
body{font-family:system-ui;display:flex;justify-content:center;align-items:center;height:100vh;margin:0;background:#0a0a0a;color:#e5e5e5}
.card{text-align:center;padding:3rem;border:1px solid #333;border-radius:12px;background:#111}
h1{font-size:2rem;margin-bottom:0.5rem}
code{background:#222;padding:2px 8px;border-radius:4px;font-size:0.9rem}
</style></head>
<body><div class="card">
<h1>R3 Admin API</h1>
<p>The API is running. Build the UI to enable the dashboard:</p>
<p><code>cd ui && npm install && npm run build</code></p>
<p>API base: <code>/api/v1/</code></p>
</div></body></html>"#,
        ))
        .unwrap()
}

/// Start the web admin server.
pub async fn start_server(
    ctx: Option<Arc<BotContext>>,
    config: RefereeConfig,
    config_path: String,
    storage: Arc<dyn Storage>,
    event_tx: broadcast::Sender<Event>,
    connected_clients: Option<Arc<tokio::sync::RwLock<std::collections::HashMap<i64, crate::sync::master::ConnectedClient>>>>,
    pending_responses: Option<Arc<tokio::sync::RwLock<std::collections::HashMap<String, tokio::sync::oneshot::Sender<crate::sync::protocol::ClientResponse>>>>>,
    pending_client_requests: Option<Arc<tokio::sync::RwLock<std::collections::HashMap<i64, Vec<(String, crate::sync::protocol::ClientRequest)>>>>>,
    client_versions: Option<Arc<tokio::sync::RwLock<std::collections::HashMap<i64, crate::sync::master::ClientVersionInfo>>>>,
) -> anyhow::Result<()> {
    let jwt_secret = config
        .web
        .jwt_secret
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    // Seed default admin user if none exist
    let users = storage.get_admin_users().await.unwrap_or_default();
    if users.is_empty() {
        let hash = bcrypt::hash("changeme", bcrypt::DEFAULT_COST)?;
        let default_user = AdminUser {
            id: 0,
            username: "admin".to_string(),
            password_hash: hash,
            role: "admin".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        storage.save_admin_user(&default_user).await?;
        info!("Created default admin user (username: admin, password: changeme)");
    }

    let state = AppState {
        ctx,
        config: config.clone(),
        config_path,
        jwt_secret,
        event_tx,
        storage,
        connected_clients,
        pending_responses,
        pending_client_requests,
        client_versions,
    };

    let app = build_router(state);
    let addr = format!("{}:{}", config.web.bind_address, config.web.port);
    let listener = crate::bind_reuse(&addr)?;
    info!(addr = %addr, "Web admin UI started");
    info!("Open http://{} in your browser", addr);

    axum::serve(listener, app).await?;
    Ok(())
}
