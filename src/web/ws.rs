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
                        let evt_key = state.ctx.event_registry.get_key(event.event_type)
                            .unwrap_or("UNKNOWN");

                        let payload = serde_json::json!({
                            "type": evt_key,
                            "time": event.time,
                            "client_id": event.client_id,
                            "target_id": event.target_id,
                            "data": format!("{:?}", event.data),
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
