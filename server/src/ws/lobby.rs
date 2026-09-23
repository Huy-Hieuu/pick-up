use std::time::Duration;

use axum::extract::ws::{CloseFrame, Message, WebSocket};
use axum::{
    extract::{Path, Query, State, WebSocketUpgrade},
    http::HeaderMap,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use tokio::time::Instant;
use uuid::Uuid;

use super::hub::LobbySubscription;
use crate::error::{AppError, AppResult};
use crate::extractors::auth::{authenticate_access_token, Claims};
use crate::models::game::{BillSplit, GameStatus, PlayerWithProfile};
use crate::services::BillSplitService;
use crate::state::AppState;

const MAX_MESSAGE_SIZE: usize = 64 * 1024;
const MAX_SNAPSHOT_SIZE: usize = 256 * 1024;
const PING_INTERVAL: Duration = Duration::from_secs(30);
const PONG_TIMEOUT: Duration = Duration::from_secs(60);
const WRITE_TIMEOUT: Duration = Duration::from_secs(5);
const SNAPSHOT_TIMEOUT: Duration = Duration::from_secs(5);
const RECHECK_INTERVAL: Duration = Duration::from_secs(15);
const MIN_REFRESH_INTERVAL: Duration = Duration::from_millis(100);

/// Browser clients can use a query token; native clients should use a header.
/// Deliberately not Debug: bearer tokens must never enter tracing output.
#[derive(Deserialize)]
pub struct WsAuth {
    pub token: Option<String>,
}

/// Full replacements, never financial deltas. Bursts may coalesce, and every
/// connection starts with an authoritative snapshot. See docs/backend-realtime.md.
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LobbyEvent {
    LobbySnapshot {
        protocol_version: u8,
        game_id: Uuid,
        status: GameStatus,
        max_players: i16,
        players: Vec<PlayerWithProfile>,
        split: Option<BillSplit>,
        split_error: Option<&'static str>,
    },
    GameCancelled {
        game_id: Uuid,
    },
}

/// Authenticate and reserve a bounded subscription before upgrading.
pub async fn ws_game_lobby(
    ws: WebSocketUpgrade,
    Path(game_id): Path<Uuid>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(auth): Query<WsAuth>,
) -> Response {
    let prepared = async {
        if state.shutdown.is_cancelled() {
            return Err(AppError::Conflict("Server is shutting down".into()));
        }
        check_origin(&headers, &state.settings.app.cors_origins)?;
        let token = match headers.get(axum::http::header::AUTHORIZATION) {
            Some(header) => header.to_str().ok().and_then(|h| h.strip_prefix("Bearer ")),
            None => auth.token.as_deref(),
        }
        .ok_or_else(|| AppError::Unauthorized("Missing or invalid access token".into()))?;
        let claims =
            authenticate_access_token(&state.pool, token, &state.settings.jwt.secret).await?;
        let user_id = Uuid::parse_str(&claims.sub)
            .map_err(|_| AppError::Unauthorized("Invalid token".into()))?;
        let is_member: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM game_players WHERE game_id = $1 AND user_id = $2)",
        )
        .bind(game_id)
        .bind(user_id)
        .fetch_one(&state.pool)
        .await?;
        if !is_member {
            return Err(AppError::Forbidden(
                "You are not a member of this game".into(),
            ));
        }
        let subscription = state.lobby.subscribe(game_id, user_id)?;
        Ok((claims, subscription))
    }
    .await;
    let (claims, subscription) = match prepared {
        Ok(value) => value,
        Err(error) => return error.into_response(),
    };
    ws.max_frame_size(MAX_MESSAGE_SIZE)
        .max_message_size(MAX_MESSAGE_SIZE)
        .max_write_buffer_size(MAX_SNAPSHOT_SIZE * 2)
        .on_upgrade(move |socket| handle_socket(socket, game_id, state, claims, subscription))
        .into_response()
}

fn check_origin(headers: &HeaderMap, origins: &[String]) -> AppResult<()> {
    if let Some(origin) = headers.get(axum::http::header::ORIGIN) {
        let origin = origin
            .to_str()
            .map_err(|_| AppError::Forbidden("Invalid origin".into()))?;
        if !origins
            .iter()
            .any(|allowed| allowed == origin || allowed == "*")
        {
            return Err(AppError::Forbidden(
                "WebSocket origin is not allowed".into(),
            ));
        }
    }
    Ok(())
}

/// Membership, session validity, roster and bill all use the same MVCC snapshot.
async fn load_snapshot(
    pool: &sqlx::PgPool,
    game_id: Uuid,
    claims: &Claims,
) -> AppResult<LobbyEvent> {
    if claims.exp <= chrono::Utc::now().timestamp() {
        return Err(AppError::Unauthorized("Token expired".into()));
    }
    let user_id =
        Uuid::parse_str(&claims.sub).map_err(|_| AppError::Unauthorized("Invalid token".into()))?;
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx)
        .await?;
    let active: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM auth_sessions WHERE id = $1 AND user_id = $2 AND expires_at > now())"
    ).bind(claims.sid).bind(user_id).fetch_one(&mut *tx).await?;
    if !active {
        return Err(AppError::Unauthorized("Session revoked".into()));
    }
    let (status, max_players, member): (GameStatus, i16, bool) = sqlx::query_as(
        "SELECT status, max_players, EXISTS(SELECT 1 FROM game_players WHERE game_id = $1 AND user_id = $2) FROM games WHERE id = $1"
    ).bind(game_id).bind(user_id).fetch_one(&mut *tx).await?;
    // The handshake already verified membership. Cancellation deletes players,
    // so only this non-sensitive terminal message is allowed after that deletion.
    if status == GameStatus::Cancelled {
        return Ok(LobbyEvent::GameCancelled { game_id });
    }
    if !member {
        return Err(AppError::Forbidden("Lobby membership ended".into()));
    }
    let players = sqlx::query_as::<_, PlayerWithProfile>(
        r#"
        SELECT gp.user_id, u.display_name, u.avatar_url, gp.joined_at, p.status AS payment_status
        FROM game_players gp JOIN users u ON u.id = gp.user_id
        LEFT JOIN LATERAL (
            SELECT status FROM payments WHERE game_id = gp.game_id AND user_id = gp.user_id
            ORDER BY (status = 'paid') DESC, created_at DESC, id DESC LIMIT 1
        ) p ON true
        WHERE gp.game_id = $1 ORDER BY gp.joined_at, gp.user_id
    "#,
    )
    .bind(game_id)
    .fetch_all(&mut *tx)
    .await?;
    let (split, split_error) = match BillSplitService::calculate_on(&mut tx, game_id).await {
        Ok(split) => (Some(split), None),
        Err(AppError::Conflict(_)) => (None, Some("RECONCILIATION_REQUIRED")),
        Err(error) => return Err(error),
    };
    tx.commit().await?;
    Ok(LobbyEvent::LobbySnapshot {
        protocol_version: 1,
        game_id,
        status,
        max_players,
        players,
        split,
        split_error,
    })
}

async fn send(socket: &mut WebSocket, message: Message) -> bool {
    matches!(
        tokio::time::timeout(WRITE_TIMEOUT, socket.send(message)).await,
        Ok(Ok(()))
    )
}

async fn close(socket: &mut WebSocket, code: u16, reason: &'static str) {
    send(
        socket,
        Message::Close(Some(CloseFrame {
            code,
            reason: reason.into(),
        })),
    )
    .await;
}

async fn refresh_snapshot(
    socket: &mut WebSocket,
    state: &AppState,
    game_id: Uuid,
    claims: &Claims,
    previous: &mut String,
) -> bool {
    let event = tokio::select! {
        biased;
        _ = state.shutdown.cancelled() => { close(socket, 1001, "Server shutting down").await; return false; }
        result = tokio::time::timeout(SNAPSHOT_TIMEOUT, load_snapshot(&state.pool, game_id, claims)) => {
            match result {
                Ok(Ok(event)) => event,
                Ok(Err(AppError::Unauthorized(_) | AppError::Forbidden(_) | AppError::NotFound(_))) => {
                    close(socket, 1008, "Access expired or membership ended").await;
                    return false;
                }
                Ok(Err(error)) => {
                    // Do not log claims, URLs, tokens or application message contents.
                    tracing::warn!(%game_id, %error, "Lobby snapshot unavailable");
                    close(socket, 1013, "Snapshot unavailable; reconnect").await;
                    return false;
                }
                Err(_) => {
                    close(socket, 1013, "Snapshot timed out; reconnect").await;
                    return false;
                }
            }
        }
    };
    let terminal = matches!(event, LobbyEvent::GameCancelled { .. });
    let payload = match serde_json::to_string(&event) {
        Ok(payload) if payload.len() <= MAX_SNAPSHOT_SIZE => payload,
        _ => {
            close(socket, 1011, "Snapshot unavailable").await;
            return false;
        }
    };
    if payload != *previous {
        if !send(socket, Message::Text(payload.clone().into())).await {
            return false;
        }
        *previous = payload;
    }
    if terminal {
        close(socket, 1000, "Game cancelled").await;
    }
    !terminal
}

async fn handle_socket(
    mut socket: WebSocket,
    game_id: Uuid,
    state: AppState,
    claims: Claims,
    mut subscription: LobbySubscription,
) {
    let mut previous = String::new();
    if !refresh_snapshot(&mut socket, &state, game_id, &claims, &mut previous).await {
        return;
    }
    let mut last_refresh = Instant::now();
    let mut last_pong = Instant::now();
    let mut pending_ping: Option<Vec<u8>> = None;
    let mut ping = tokio::time::interval_at(Instant::now() + PING_INTERVAL, PING_INTERVAL);
    let mut recheck = tokio::time::interval_at(Instant::now() + RECHECK_INTERVAL, RECHECK_INTERVAL);
    ping.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    recheck.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let expires_in = claims
        .exp
        .saturating_sub(chrono::Utc::now().timestamp())
        .max(0) as u64;
    let expires = tokio::time::sleep(Duration::from_secs(expires_in));
    tokio::pin!(expires);

    loop {
        let refresh = tokio::select! {
            biased;
            _ = state.shutdown.cancelled() => { close(&mut socket, 1001, "Server shutting down").await; break; }
            _ = &mut expires => { close(&mut socket, 1008, "Token expired").await; break; }
            _ = ping.tick() => {
                if last_pong.elapsed() >= PONG_TIMEOUT {
                    close(&mut socket, 1001, "Heartbeat timeout").await; break;
                }
                if pending_ping.is_none() { pending_ping = Some(Uuid::new_v4().as_bytes().to_vec()); }
                if !send(&mut socket, Message::Ping(pending_ping.clone().unwrap_or_default().into())).await { break; }
                false
            }
            _ = recheck.tick() => true,
            changed = subscription.changed() => {
                if changed.is_err() { break; }
                true
            }
            message = socket.recv() => {
                match message {
                    Some(Ok(Message::Pong(data))) => {
                        if pending_ping.as_deref() == Some(data.as_ref()) {
                            last_pong = Instant::now();
                            pending_ping = None;
                        }
                    }
                    Some(Ok(Message::Ping(data))) => {
                        if !send(&mut socket, Message::Pong(data)).await { break; }
                    }
                    Some(Ok(Message::Text(_) | Message::Binary(_))) => {
                        close(&mut socket, 1008, "Lobby is server-push only").await; break;
                    }
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                }
                false
            }
        };
        if refresh {
            // Coalesce transaction bursts without unbounded per-client queues.
            tokio::select! {
                biased;
                _ = state.shutdown.cancelled() => { close(&mut socket, 1001, "Server shutting down").await; break; }
                _ = &mut expires => { close(&mut socket, 1008, "Token expired").await; break; }
                _ = tokio::time::sleep_until(last_refresh + MIN_REFRESH_INTERVAL) => {}
            }
            if !refresh_snapshot(&mut socket, &state, game_id, &claims, &mut previous).await {
                break;
            }
            last_refresh = Instant::now();
        }
    }
    // Dropping the subscription releases connection limits and empty rooms.
}
