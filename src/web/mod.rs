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
        // Plugins
        .route("/plugins", get(api::plugins::list_plugins))
        // Players
        .route("/players", get(api::players::list_players))
        .route("/players/{id}", get(api::players::get_player))
        .route("/players/{cid}/kick", post(api::players::kick_player))
        .route("/players/{cid}/ban", post(api::players::ban_player))
        .route("/players/{cid}/message", post(api::players::message_player))
        // Client search
        .route("/clients/search", get(api::players::search_clients))
        // Penalties
        .route("/penalties", get(api::penalties::list_penalties))
        .route("/penalties/{id}/disable", post(api::penalties::disable_penalty))
        // Groups
        .route("/groups", get(api::groups::list_groups))
        // Aliases
        .route("/aliases", get(api::aliases::list_aliases))
        // Server
        .route("/server/status", get(api::server::server_status))
        .route("/server/rcon", post(api::server::rcon_command))
        .route("/server/say", post(api::server::server_say))
        // Stats
        .route("/stats/leaderboard", get(api::stats::leaderboard))
        .route("/stats/player/{id}", get(api::stats::player_stats))
        .route("/stats/weapons", get(api::stats::weapon_stats))
        .route("/stats/maps", get(api::stats::map_stats))
        // Admin users
        .route("/users", get(api::users::list_users))
        .route("/users", post(api::users::create_user))
        .route("/users/me/password", put(api::users::change_password))
        .route("/users/{id}", put(api::users::update_user))
        .route("/users/{id}", delete(api::users::delete_user));

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
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime.as_ref())
            .header(header::CACHE_CONTROL, "public, max-age=3600")
            .body(Body::from(content.data.to_vec()))
            .unwrap();
    }

    // SPA fallback: return index.html for any non-file route
    if let Some(content) = UiAssets::get("index.html") {
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
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
    ctx: Arc<BotContext>,
    config: RefereeConfig,
    config_path: String,
    storage: Arc<dyn Storage>,
    event_tx: broadcast::Sender<Event>,
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
    };

    let app = build_router(state);
    let addr = format!("{}:{}", config.web.bind_address, config.web.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!(addr = %addr, "Web admin UI started");
    info!("Open http://{} in your browser", addr);

    axum::serve(listener, app).await?;
    Ok(())
}
