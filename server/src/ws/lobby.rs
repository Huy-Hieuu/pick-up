use axum::{
    extract::{Path, State, WebSocketUpgrade},
    response::IntoResponse,
};
use axum::extract::ws::{Message, WebSocket};
use serde::{Deserialize, Serialize};
use tokio::time::Instant;
use uuid::Uuid;

use crate::state::AppState;

/// Maximum allowed WebSocket message size (64 KB).
const MAX_MESSAGE_SIZE: usize = 64 * 1024;

/// Interval for sending ping frames.
const PING_INTERVAL: std::time::Duration = std::time::Duration::from_secs(30);

/// Timeout for receiving a pong response.
const PONG_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// Messages broadcast from server → clients in a game lobby.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LobbyEvent {
    PlayerJoined { user_id: Uuid, display_name: Option<String> },
    PlayerLeft { user_id: Uuid },
    GameStatusChanged { status: String },
    PaymentUpdated { user_id: Uuid, paid: bool },
    SplitRecalculated { per_player: i32, remainder: i32 },
    GameCancelled,
}

/// `GET /ws/games/:game_id` — WebSocket upgrade for game lobby.
///
/// JWT is passed as a query parameter: `?token=<jwt>`
/// The connection stays open and receives `LobbyEvent` broadcasts.
pub async fn ws_game_lobby(
    ws: WebSocketUpgrade,
    Path(game_id): Path<Uuid>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    // TODO: Extract and validate JWT from query string before accepting.
    ws.max_frame_size(MAX_MESSAGE_SIZE)
      .on_upgrade(move |socket| handle_socket(socket, game_id, state))
}

async fn handle_socket(mut socket: WebSocket, game_id: Uuid, _state: AppState) {
    tracing::info!(%game_id, "WebSocket client connected to game lobby");

    let mut last_pong = Instant::now();
    let mut ping_interval = tokio::time::interval(PING_INTERVAL);

    // TODO:
    // 1. Authenticate via JWT from query param
    // 2. Subscribe to game_id channel (e.g., tokio broadcast)
    // 3. Broadcast outgoing LobbyEvent messages to this socket
    // 4. Handle disconnect (remove from subscriber list)

    loop {
        tokio::select! {
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        // Reject oversized messages.
                        if text.len() > MAX_MESSAGE_SIZE {
                            tracing::warn!(%game_id, "WebSocket text message too large, closing");
                            let _ = socket.send(Message::Close(None)).await;
                            break;
                        }
                        // TODO: Parse as client→server message type and process.
                        tracing::debug!(%game_id, %text, "Received WebSocket message (ignored — no client→server protocol yet)");
                    }
                    Some(Ok(Message::Binary(_))) => {
                        // Reject binary frames — only text/JSON is supported.
                        tracing::warn!(%game_id, "Binary WebSocket frame rejected");
                        let _ = socket.send(Message::Text(
                            serde_json::json!({"type": "error", "message": "Binary frames not supported"}).to_string().into()
                        )).await;
                        let _ = socket.send(Message::Close(None)).await;
                        break;
                    }
                    Some(Ok(Message::Ping(data))) => {
                        last_pong = Instant::now();
                        let _ = socket.send(Message::Pong(data)).await;
                    }
                    Some(Ok(Message::Pong(_))) => {
                        last_pong = Instant::now();
                    }
                    Some(Ok(Message::Close(_))) => break,
                    Some(Err(e)) => {
                        tracing::warn!(%game_id, "WebSocket error: {e}");
                        break;
                    }
                    None => break,
                }
            }
            _ = ping_interval.tick() => {
                // Check for pong timeout — close zombie connections.
                if last_pong.elapsed() > PONG_TIMEOUT {
                    tracing::info!(%game_id, "WebSocket pong timeout, closing connection");
                    let _ = socket.send(Message::Close(None)).await;
                    break;
                }
                let _ = socket.send(Message::Ping(vec![].into())).await;
            }
        }
    }

    tracing::info!(%game_id, "WebSocket client disconnected from game lobby");
}
