use axum::{
    extract::{Path, State, WebSocketUpgrade},
    response::IntoResponse,
};
use axum::extract::ws::{Message, WebSocket};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::state::AppState;

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
    ws.on_upgrade(move |socket| handle_socket(socket, game_id, state))
}

async fn handle_socket(mut socket: WebSocket, game_id: Uuid, _state: AppState) {
    tracing::info!(%game_id, "WebSocket client connected to game lobby");

    // TODO:
    // 1. Authenticate via JWT from query param
    // 2. Subscribe to game_id channel (e.g., tokio broadcast)
    // 3. Forward incoming messages (if any client→server types are needed)
    // 4. Broadcast outgoing LobbyEvent messages to this socket
    // 5. Handle disconnect (remove from subscriber list)

    // Placeholder: echo messages back
    while let Some(msg) = socket.recv().await {
        match msg {
            Ok(Message::Text(text)) => {
                tracing::debug!(%game_id, %text, "Received WebSocket message");
                if socket.send(Message::Text(text)).await.is_err() {
                    break;
                }
            }
            Ok(Message::Close(_)) => break,
            Err(e) => {
                tracing::warn!(%game_id, "WebSocket error: {e}");
                break;
            }
            _ => {}
        }
    }

    tracing::info!(%game_id, "WebSocket client disconnected from game lobby");
}