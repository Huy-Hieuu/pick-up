use axum::{
    extract::{Path, Query, State, WebSocketUpgrade},
    response::{IntoResponse, Response},
    http::StatusCode,
};
use axum::extract::ws::{Message, WebSocket};
use serde::Deserialize;
use tokio::time::Instant;
use uuid::Uuid;

use crate::extractors::auth::{Claims, AUDIENCE};
use crate::state::AppState;

/// Maximum allowed WebSocket message size (64 KB).
const MAX_MESSAGE_SIZE: usize = 64 * 1024;

/// Interval for sending ping frames.
const PING_INTERVAL: std::time::Duration = std::time::Duration::from_secs(30);

/// Timeout for receiving a pong response.
const PONG_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// Query params for WebSocket authentication.
#[derive(Debug, Deserialize)]
pub struct WsAuth {
    pub token: String,
}

/// Messages broadcast from server → clients in a game lobby.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
    Query(auth): Query<WsAuth>,
) -> Response {
    // Validate JWT before accepting the upgrade.
    let claims = match verify_ws_token(&auth.token, &state.settings.jwt.secret) {
        Some(c) => c,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                axum::Json(serde_json::json!({
                    "error": { "code": "UNAUTHORIZED", "message": "Invalid or missing token" }
                })),
            ).into_response();
        }
    };

    // Verify user is a member of the game.
    let user_id = match uuid::Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                axum::Json(serde_json::json!({
                    "error": { "code": "UNAUTHORIZED", "message": "Invalid token" }
                })),
            ).into_response();
        }
    };

    let is_member = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM game_players WHERE game_id = $1 AND user_id = $2)"
    )
    .bind(game_id)
    .bind(user_id)
    .fetch_one(&state.pool)
    .await
    .unwrap_or(false);

    if !is_member {
        return (
            StatusCode::FORBIDDEN,
            axum::Json(serde_json::json!({
                "error": { "code": "FORBIDDEN", "message": "You are not a member of this game" }
            })),
        ).into_response();
    }

    ws.max_frame_size(MAX_MESSAGE_SIZE)
      .on_upgrade(move |socket| handle_socket(socket, game_id, state))
      .into_response()
}

/// Verify JWT for WebSocket connections. Returns Claims if valid.
fn verify_ws_token(token: &str, secret: &str) -> Option<Claims> {
    use jsonwebtoken::{decode, DecodingKey, Validation};

    let mut validation = Validation::default();
    validation.set_issuer(&["pickup-server"]);
    validation.set_audience(&[AUDIENCE]);

    decode::<Claims>(token, &DecodingKey::from_secret(secret.as_bytes()), &validation)
        .ok()
        .filter(|data| data.claims.typ == "access")
        .map(|data| data.claims)
}

async fn handle_socket(mut socket: WebSocket, game_id: Uuid, _state: AppState) {
    tracing::info!(%game_id, "WebSocket client connected to game lobby");

    let mut last_pong = Instant::now();
    let mut ping_interval = tokio::time::interval(PING_INTERVAL);

    // TODO:
    // 1. Subscribe to game_id channel (e.g., tokio broadcast)
    // 2. Broadcast outgoing LobbyEvent messages to this socket
    // 3. Handle disconnect (remove from subscriber list)

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
                        // Respond to client pings — but don't reset last_pong
                        // (only our own pong responses should reset the timeout).
                        let _ = socket.send(Message::Pong(data)).await;
                    }
                    Some(Ok(Message::Pong(_))) => {
                        // Only reset timeout on pong responses to OUR pings.
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
