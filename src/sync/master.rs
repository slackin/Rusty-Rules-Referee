//! Master server internal API for game server bot clients.
//!
//! This runs on a separate mTLS-protected port (default 9443) and provides:
//! - Registration for new game server bots
//! - Event batch ingestion
//! - Penalty sync (bidirectional)
//! - Config distribution
//! - Heartbeat / health monitoring
//! - WebSocket for real-time bidirectional communication

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{State, WebSocketUpgrade, ws},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post, put},
};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, oneshot, RwLock};
use tracing::{error, info, warn};

use crate::config::MasterSection;
use crate::core::GameServer;
use crate::storage::Storage;
use crate::sync::protocol::*;
use crate::sync::tls;

/// Shared state for the master internal API.
#[derive(Clone)]
pub struct MasterState {
    pub storage: Arc<dyn Storage>,
    /// Connected client bots by server_id, with their WS sender.
    pub connected_clients: Arc<RwLock<HashMap<i64, ConnectedClient>>>,
    /// Broadcast channel for forwarding events to the web UI.
    pub event_tx: broadcast::Sender<EventPayload>,
    /// Pending request/response correlations: request_id → oneshot sender.
    pub pending_responses: Arc<RwLock<HashMap<String, oneshot::Sender<ClientResponse>>>>,
    /// Pending requests queued for client bots to pick up via polling.
    /// Key: server_id → Vec of (request_id, request).
    pub pending_client_requests: Arc<RwLock<HashMap<i64, Vec<(String, ClientRequest)>>>>,
    /// Last-known version info reported by each client via heartbeat.
    /// Key: server_id → (build_hash, version, last_reported_at)
    pub client_versions:
        Arc<RwLock<HashMap<i64, ClientVersionInfo>>>,
}

/// Client-reported build/version info, refreshed on every heartbeat.
#[derive(Debug, Clone)]
pub struct ClientVersionInfo {
    pub build_hash: Option<String>,
    pub version: Option<String>,
    pub reported_at: chrono::DateTime<chrono::Utc>,
}

/// Represents a connected client bot.
pub struct ConnectedClient {
    pub server_id: i64,
    pub server_name: String,
    pub tx: tokio::sync::mpsc::Sender<SyncMessage>,
}

/// Build the internal API router.
pub fn build_internal_router(state: MasterState) -> Router {
    Router::new()
        .route("/internal/register", post(handle_register))
        .route("/internal/heartbeat", post(handle_heartbeat))
        .route("/internal/events", post(handle_events))
        .route("/internal/penalties", post(handle_penalty_sync))
        .route("/internal/players", post(handle_player_sync))
        .route("/internal/config/:server_id", get(handle_get_config))
        .route("/internal/config/:server_id", put(handle_put_config))
        .route("/internal/bans", get(handle_get_global_bans))
        .route("/internal/ws", get(handle_ws))
        .route("/internal/requests/:server_id", get(handle_poll_requests))
        .route("/internal/responses", post(handle_client_response))
        .with_state(state)
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn handle_register(
    State(state): State<MasterState>,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<RegisterResponse>, StatusCode> {
    info!(
        name = %req.server_name,
        address = %req.address,
        port = req.port,
        "Client bot registering"
    );

    // Check if already registered by cert fingerprint
    let existing = state
        .storage
        .get_server_by_fingerprint(&req.cert_fingerprint)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to look up server by fingerprint");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let server_id = if let Some(existing) = existing {
        // Update existing registration.
        // NOTE: client-mode bots register with empty address / port 0 because the
        // game server config is managed by the master and pushed down. Only
        // overwrite address/port when the client reports real values, otherwise
        // a reconnect would wipe a config saved via the master UI.
        let mut server = existing;
        server.name = req.server_name;
        if !req.address.is_empty() && req.address != "0.0.0.0" {
            server.address = req.address;
        }
        if req.port != 0 {
            server.port = req.port;
        }
        server.status = "online".to_string();
        server.last_seen = Some(chrono::Utc::now());
        state.storage.save_server(&server).await.map_err(|e| {
            error!(error = %e, "Failed to update server");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        server.id
    } else {
        // New registration
        let server = GameServer {
            id: 0,
            name: req.server_name,
            address: req.address,
            port: req.port,
            status: "online".to_string(),
            current_map: None,
            player_count: 0,
            max_clients: 0,
            last_seen: Some(chrono::Utc::now()),
            config_json: None,
            config_version: 0,
            cert_fingerprint: Some(req.cert_fingerprint),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        state.storage.save_server(&server).await.map_err(|e| {
            error!(error = %e, "Failed to save new server");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
    };

    let config_version = state
        .storage
        .get_server(server_id)
        .await
        .map(|s| s.config_version)
        .unwrap_or(0);

    info!(server_id, "Client bot registered");

    Ok(Json(RegisterResponse {
        server_id,
        config_version,
    }))
}

async fn handle_heartbeat(
    State(state): State<MasterState>,
    Json(req): Json<HeartbeatRequest>,
) -> Result<Json<HeartbeatResponse>, StatusCode> {
    state
        .storage
        .update_server_status(
            req.server_id,
            &req.status,
            req.current_map.as_deref(),
            req.player_count,
            req.max_clients,
        )
        .await
        .map_err(|e| {
            error!(error = %e, server_id = req.server_id, "Failed to update server status");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Record client-reported version info if present in this heartbeat.
    if req.build_hash.is_some() || req.version.is_some() {
        state.client_versions.write().await.insert(
            req.server_id,
            ClientVersionInfo {
                build_hash: req.build_hash.clone(),
                version: req.version.clone(),
                reported_at: chrono::Utc::now(),
            },
        );
    }

    let config_version = state
        .storage
        .get_server(req.server_id)
        .await
        .map(|s| s.config_version)
        .unwrap_or(0);

    Ok(Json(HeartbeatResponse {
        ok: true,
        config_version,
        pending_global_bans: Vec::new(), // TODO: track pending bans since last heartbeat
    }))
}

async fn handle_events(
    State(state): State<MasterState>,
    Json(batch): Json<EventBatch>,
) -> Result<StatusCode, StatusCode> {
    info!(
        server_id = batch.server_id,
        count = batch.events.len(),
        "Received event batch"
    );

    // Forward events to the web UI broadcast channel
    for event in &batch.events {
        let _ = state.event_tx.send(event.clone());
    }

    // TODO: persist events to database for historical querying

    Ok(StatusCode::OK)
}

async fn handle_penalty_sync(
    State(state): State<MasterState>,
    Json(penalty): Json<PenaltySync>,
) -> Result<StatusCode, StatusCode> {
    info!(
        server_id = penalty.server_id,
        penalty_type = %penalty.penalty_type,
        client = %penalty.client_name,
        scope = ?penalty.scope,
        "Received penalty sync"
    );

    // If global, broadcast to all OTHER connected clients
    if penalty.scope == PenaltyScope::Global {
        let clients = state.connected_clients.read().await;
        for (sid, client) in clients.iter() {
            if *sid != penalty.server_id {
                let msg = SyncMessage::GlobalPenalty(penalty.clone());
                if let Err(e) = client.tx.send(msg).await {
                    warn!(
                        server_id = sid,
                        error = %e,
                        "Failed to forward global penalty to client"
                    );
                }
            }
        }
    }

    // TODO: persist penalty to master database

    Ok(StatusCode::OK)
}

async fn handle_player_sync(
    State(_state): State<MasterState>,
    Json(batch): Json<PlayerSyncBatch>,
) -> Result<StatusCode, StatusCode> {
    info!(
        server_id = batch.server_id,
        count = batch.players.len(),
        "Received player sync"
    );

    // TODO: merge player data into master database

    Ok(StatusCode::OK)
}

async fn handle_get_config(
    State(state): State<MasterState>,
    axum::extract::Path(server_id): axum::extract::Path<i64>,
) -> Result<Json<ConfigSync>, StatusCode> {
    let server = state
        .storage
        .get_server(server_id)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(Json(ConfigSync {
        server_id,
        config_json: server.config_json.unwrap_or_default(),
        config_version: server.config_version,
    }))
}

async fn handle_put_config(
    State(state): State<MasterState>,
    axum::extract::Path(server_id): axum::extract::Path<i64>,
    Json(config): Json<ConfigSync>,
) -> Result<StatusCode, StatusCode> {
    let mut server = state
        .storage
        .get_server(server_id)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    server.config_json = Some(config.config_json.clone());
    server.config_version = config.config_version;
    state.storage.save_server(&server).await.map_err(|e| {
        error!(error = %e, "Failed to save server config");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Push to connected client if online
    let clients = state.connected_clients.read().await;
    if let Some(client) = clients.get(&server_id) {
        let msg = SyncMessage::ConfigUpdate(config);
        let _ = client.tx.send(msg).await;
    }

    Ok(StatusCode::OK)
}

async fn handle_get_global_bans(
    State(_state): State<MasterState>,
) -> Result<Json<Vec<PenaltySync>>, StatusCode> {
    // TODO: query global bans from database
    Ok(Json(Vec::new()))
}

async fn handle_ws(
    State(state): State<MasterState>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws_connection(socket, state))
}

async fn handle_ws_connection(mut socket: ws::WebSocket, state: MasterState) {
    info!("Internal WebSocket connection established");

    // The client should send a Heartbeat as the first message to identify itself
    let server_id = match socket.recv().await {
        Some(Ok(ws::Message::Text(text))) => {
            match serde_json::from_str::<SyncMessage>(&text) {
                Ok(SyncMessage::Heartbeat(hb)) => {
                    info!(server_id = hb.server_id, "Client identified via WS");
                    hb.server_id
                }
                _ => {
                    warn!("First WS message was not a Heartbeat, closing");
                    return;
                }
            }
        }
        _ => {
            warn!("Failed to receive identification message on WS");
            return;
        }
    };

    // Create a channel for sending messages to this client
    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel::<SyncMessage>(256);

    // Register this client
    {
        let server_name = state
            .storage
            .get_server(server_id)
            .await
            .map(|s| s.name)
            .unwrap_or_else(|_| format!("server-{}", server_id));

        state.connected_clients.write().await.insert(
            server_id,
            ConnectedClient {
                server_id,
                server_name,
                tx: cmd_tx,
            },
        );
    }

    // Use the socket directly — axum's WebSocket has recv()/send() methods
    loop {
        tokio::select! {
            // Messages from the client bot
            msg = socket.recv() => {
                match msg {
                    Some(Ok(ws::Message::Text(text))) => {
                        match serde_json::from_str::<SyncMessage>(&text) {
                            Ok(SyncMessage::Event(event)) => {
                                let _ = state.event_tx.send(event);
                            }
                            Ok(SyncMessage::EventBatch(batch)) => {
                                for event in batch.events {
                                    let _ = state.event_tx.send(event);
                                }
                            }
                            Ok(SyncMessage::Penalty(penalty)) => {
                                if penalty.scope == PenaltyScope::Global {
                                    let clients = state.connected_clients.read().await;
                                    for (sid, client) in clients.iter() {
                                        if *sid != server_id {
                                            let _ = client.tx.send(
                                                SyncMessage::GlobalPenalty(penalty.clone())
                                            ).await;
                                        }
                                    }
                                }
                            }
                            Ok(SyncMessage::Heartbeat(hb)) => {
                                let _ = state.storage.update_server_status(
                                    hb.server_id,
                                    &hb.status,
                                    hb.current_map.as_deref(),
                                    hb.player_count,
                                    hb.max_clients,
                                ).await;
                            }
                            Ok(SyncMessage::Response { request_id, response }) => {
                                let mut pending = state.pending_responses.write().await;
                                if let Some(tx) = pending.remove(&request_id) {
                                    let _ = tx.send(response);
                                } else {
                                    warn!(request_id, "Received response for unknown request");
                                }
                            }
                            Ok(other) => {
                                warn!(?other, "Unhandled WS message from client");
                            }
                            Err(e) => {
                                warn!(error = %e, "Failed to parse WS message from client");
                            }
                        }
                    }
                    Some(Ok(ws::Message::Close(_))) | None => {
                        info!(server_id, "Client WS disconnected");
                        break;
                    }
                    _ => {}
                }
            }
            // Commands from master to send to this client
            cmd = cmd_rx.recv() => {
                if let Some(msg) = cmd {
                    let text = serde_json::to_string(&msg).unwrap_or_default();
                    if socket.send(ws::Message::Text(text.into())).await.is_err() {
                        warn!(server_id, "Failed to send WS message to client");
                        break;
                    }
                }
            }
        }
    }

    // Cleanup
    state.connected_clients.write().await.remove(&server_id);
    let _ = state.storage.update_server_status(server_id, "offline", None, 0, 0).await;
    info!(server_id, "Client bot disconnected, marked offline");
}

/// Start the master internal API server with mTLS.
/// Returns the shared `connected_clients` map so the web API can also use it.
pub async fn start_master_api(
    config: &MasterSection,
    storage: Arc<dyn Storage>,
    event_tx: broadcast::Sender<EventPayload>,
    connected_clients: Arc<RwLock<HashMap<i64, ConnectedClient>>>,
    pending_responses: Arc<RwLock<HashMap<String, oneshot::Sender<ClientResponse>>>>,
    pending_client_requests: Arc<RwLock<HashMap<i64, Vec<(String, ClientRequest)>>>>,
    client_versions: Arc<RwLock<HashMap<i64, ClientVersionInfo>>>,
) -> anyhow::Result<()> {
    let tls_acceptor = tls::build_master_tls_acceptor(
        Path::new(&config.tls_cert),
        Path::new(&config.tls_key),
        Path::new(&config.ca_cert),
    )?;

    let state = MasterState {
        storage,
        connected_clients,
        event_tx,
        pending_responses,
        pending_client_requests,
        client_versions,
    };

    let app = build_internal_router(state);
    let addr: SocketAddr = format!("{}:{}", config.bind_address, config.port).parse()?;

    info!(addr = %addr, "Master internal API starting (mTLS)");

    let listener = crate::bind_reuse(&addr.to_string())?;

    loop {
        let (stream, peer_addr) = listener.accept().await?;
        let acceptor = tls_acceptor.clone();
        let app = app.clone();

        tokio::spawn(async move {
            match acceptor.accept(stream).await {
                Ok(tls_stream) => {
                    let io = hyper_util::rt::TokioIo::new(tls_stream);
                    let tower_service = app;

                    let hyper_service = hyper::service::service_fn(
                        move |request: hyper::Request<hyper::body::Incoming>| {
                            let svc = tower_service.clone();
                            async move {
                                use tower::ServiceExt;
                                svc.oneshot(request.map(axum::body::Body::new)).await
                            }
                        },
                    );

                    let builder = hyper_util::server::conn::auto::Builder::new(
                        hyper_util::rt::TokioExecutor::new(),
                    );
                    // Use serve_connection_with_upgrades for WebSocket support
                    if let Err(e) = builder
                        .serve_connection_with_upgrades(io, hyper_service)
                        .await
                    {
                        error!(peer = %peer_addr, error = %e, "Connection error");
                    }
                }
                Err(e) => {
                    warn!(peer = %peer_addr, error = %e, "TLS handshake failed");
                }
            }
        });
    }
}

// ---------------------------------------------------------------------------
// Internal REST endpoints for client request/response polling
// ---------------------------------------------------------------------------

/// GET /internal/requests/:server_id — client polls for pending requests.
async fn handle_poll_requests(
    State(state): State<MasterState>,
    axum::extract::Path(server_id): axum::extract::Path<i64>,
) -> Json<PendingRequestsResponse> {
    let mut pending = state.pending_client_requests.write().await;
    let items = pending.remove(&server_id).unwrap_or_default();

    let requests: Vec<PendingRequestItem> = items
        .into_iter()
        .map(|(request_id, request)| PendingRequestItem { request_id, request })
        .collect();

    Json(PendingRequestsResponse { requests })
}

/// POST /internal/responses — client sends back a response to a request.
async fn handle_client_response(
    State(state): State<MasterState>,
    Json(body): Json<ClientResponseSubmission>,
) -> StatusCode {
    let mut pending = state.pending_responses.write().await;
    if let Some(tx) = pending.remove(&body.request_id) {
        let _ = tx.send(body.response);
        StatusCode::OK
    } else {
        warn!(request_id = %body.request_id, "Response for unknown or expired request");
        StatusCode::NOT_FOUND
    }
}

// ---------------------------------------------------------------------------
// Public helper: send a request to a connected client and await the response
// ---------------------------------------------------------------------------

/// Queue a request for a client bot and wait for its response.
///
/// The request is placed in `pending_client_requests` for the given server_id.
/// The client picks it up during its next poll cycle (every ~2s). A oneshot
/// channel correlates the eventual response. Times out after `timeout`.
pub async fn send_request_to_server(
    pending_responses: &Arc<RwLock<HashMap<String, oneshot::Sender<ClientResponse>>>>,
    pending_client_requests: &Arc<RwLock<HashMap<i64, Vec<(String, ClientRequest)>>>>,
    server_id: i64,
    request: ClientRequest,
    timeout: std::time::Duration,
) -> Result<ClientResponse, String> {
    let request_id = uuid::Uuid::new_v4().to_string();

    // Create oneshot for the response
    let (resp_tx, resp_rx) = oneshot::channel();

    // Store the oneshot sender
    pending_responses.write().await.insert(request_id.clone(), resp_tx);

    // Queue the request for the client to pick up
    {
        let mut pending = pending_client_requests.write().await;
        pending
            .entry(server_id)
            .or_insert_with(Vec::new)
            .push((request_id.clone(), request));
    }

    // Await the response with timeout
    match tokio::time::timeout(timeout, resp_rx).await {
        Ok(Ok(response)) => Ok(response),
        Ok(Err(_)) => {
            pending_responses.write().await.remove(&request_id);
            Err("Client disconnected before responding".to_string())
        }
        Err(_) => {
            pending_responses.write().await.remove(&request_id);
            // Also clean up the queued request if it hasn't been picked up
            {
                let mut pending = pending_client_requests.write().await;
                if let Some(reqs) = pending.get_mut(&server_id) {
                    reqs.retain(|(id, _)| id != &request_id);
                }
            }
            Err("Request timed out waiting for client response".to_string())
        }
    }
}
