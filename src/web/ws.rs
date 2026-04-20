use axum::{
    extract::{Query, State, WebSocketUpgrade, ws::{Message, WebSocket}},
    response::IntoResponse,
};
use serde::Deserialize;
use tokio::sync::broadcast;
use tracing::warn;

use super::auth::decode_token;
use super::state::AppState;

#[derive(Debug, Deserialize)]
pub struct WsQuery {
    pub token: String,
}

/// GET /ws?token=<jwt>
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(query): Query<WsQuery>,
) -> impl IntoResponse {
    // Validate JWT from query param
    match decode_token(&state.jwt_secret, &query.token) {
        Ok(_claims) => {
            let rx = state.event_tx.subscribe();
            ws.on_upgrade(move |socket| handle_socket(socket, rx, state))
        }
        Err(_) => {
            axum::http::Response::builder()
                .status(401)
                .body(axum::body::Body::from("Unauthorized"))
                .unwrap()
                .into_response()
        }
    }
}

async fn handle_socket(
    mut socket: WebSocket,
    mut rx: broadcast::Receiver<crate::events::Event>,
    state: AppState,
) {
    loop {
        tokio::select! {
            // Forward events from broadcast channel to WebSocket
            result = rx.recv() => {
                match result {
                    Ok(event) => {
                        let evt_key = state.ctx.as_ref()
                            .and_then(|ctx| ctx.event_registry.get_key(event.event_type))
                            .unwrap_or("UNKNOWN");

                        // Serialize EventData as structured JSON
                        let data = match &event.data {
                            crate::events::EventData::Empty => serde_json::json!(null),
                            crate::events::EventData::Text(t) => serde_json::json!({"text": t}),
                            crate::events::EventData::Kill { weapon, damage, damage_type, hit_location } => {
                                serde_json::json!({
                                    "weapon": weapon,
                                    "damage": damage,
                                    "damage_type": damage_type,
                                    "hit_location": hit_location,
                                })
                            }
                            crate::events::EventData::MapChange { old, new } => {
                                serde_json::json!({ "old_map": old, "new_map": new })
                            }
                            crate::events::EventData::Custom(v) => v.clone(),
                        };

                        // Resolve client/target names by slot CID
                        let (client_db_id, client_name) = if let Some(cid) = event.client_id {
                            match state.ctx.as_ref() {
                                Some(ctx) => match ctx.clients.get_by_cid(&cid.to_string()).await {
                                    Some(c) => (Some(c.id), Some(c.name.clone())),
                                    None => (None, None),
                                },
                                None => (None, None),
                            }
                        } else { (None, None) };
                        let (target_db_id, target_name) = if let Some(tid) = event.target_id {
                            match state.ctx.as_ref() {
                                Some(ctx) => match ctx.clients.get_by_cid(&tid.to_string()).await {
                                    Some(c) => (Some(c.id), Some(c.name.clone())),
                                    None => (None, None),
                                },
                                None => (None, None),
                            }
                        } else { (None, None) };

                        let payload = serde_json::json!({
                            "type": evt_key,
                            "time": event.time,
                            "client_id": client_db_id.or(event.client_id),
                            "target_id": target_db_id.or(event.target_id),
                            "client_name": client_name,
                            "target_name": target_name,
                            "data": data,
                        });

                        if socket.send(Message::Text(payload.to_string().into())).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!(skipped = n, "WebSocket client lagged, skipping events");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            // Handle incoming messages from client (ping/pong, close)
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(data))) => {
                        if socket.send(Message::Pong(data)).await.is_err() {
                            break;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}
