//! Real TCP/WebSocket tests using the same app router as production.
use std::time::Duration;

use axum::http::StatusCode;
use futures_util::{SinkExt, StreamExt};
use pickup_server::{
    extractors::auth::{create_access_token, create_refresh_token},
    models::{
        court::{BookSlotRequest, SportType},
        game::{CreateGameRequest, GameStatus},
    },
    services::{CourtService, GameService},
    state::AppState,
};
use serde_json::{json, Value};
use sqlx::PgPool;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{
        client::IntoClientRequest,
        protocol::{frame::coding::CloseCode, Message},
    },
    MaybeTlsStream, WebSocketStream,
};
use uuid::Uuid;

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

struct Api {
    state: AppState,
    address: std::net::SocketAddr,
}

impl Api {
    async fn start(pool: PgPool) -> Self {
        let state = super::state(pool).await;
        state
            .lobby
            .start_listener(&state.pool, state.shutdown.clone())
            .await
            .unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let app = pickup_server::routes::build_router(state.clone());
        let shutdown = state.shutdown.clone();
        tokio::spawn(async move {
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
            )
            .with_graceful_shutdown(shutdown.cancelled_owned())
            .await
            .unwrap();
        });
        Self { state, address }
    }

    async fn connect(&self, game: Uuid, token: &str) -> Socket {
        connect_async(format!(
            "ws://{}/ws/games/{game}?token={token}",
            self.address
        ))
        .await
        .unwrap()
        .0
    }
}

impl Drop for Api {
    fn drop(&mut self) {
        self.state.shutdown.cancel();
    }
}

async fn player(api: &Api) -> (Uuid, String, Uuid) {
    let user: Uuid = sqlx::query_scalar(
        "INSERT INTO users (phone, display_name) VALUES ($1, 'Player') RETURNING id",
    )
    .bind(Uuid::new_v4().to_string()[..20].to_owned())
    .fetch_one(&api.state.pool)
    .await
    .unwrap();
    let sid = Uuid::new_v4();
    sqlx::query("INSERT INTO auth_sessions (id, user_id, refresh_jti, expires_at) VALUES ($1, $2, $3, now() + interval '1 hour')")
        .bind(sid).bind(user).bind(Uuid::new_v4()).execute(&api.state.pool).await.unwrap();
    let token = create_access_token(user, sid, &api.state.settings.jwt.secret, 15).unwrap();
    (user, token, sid)
}

async fn game(api: &Api, creator: Uuid) -> Uuid {
    let pool = &api.state.pool;
    let court: Uuid = sqlx::query_scalar("INSERT INTO courts (name, sport_type, lat, lng, address, price_per_slot, owner_id) VALUES ('Test', 'pickleball', 10, 106, 'Test', 600001, $1) RETURNING id")
        .bind(creator).fetch_one(pool).await.unwrap();
    let slot: Uuid = sqlx::query_scalar("INSERT INTO court_slots (court_id, start_time, end_time) VALUES ($1, now() + interval '1 day', now() + interval '25 hours') RETURNING id")
        .bind(court).fetch_one(pool).await.unwrap();
    CourtService::book_slot(pool, court, slot, creator, BookSlotRequest {})
        .await
        .unwrap();
    GameService::create_game(
        pool,
        creator,
        CreateGameRequest {
            court_slot_id: slot,
            sport_type: SportType::Pickleball,
            max_players: 3,
            description: None,
        },
    )
    .await
    .unwrap()
    .game
    .id
}

async fn event(socket: &mut Socket) -> Value {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match socket.next().await.unwrap().unwrap() {
                Message::Text(text) => return serde_json::from_str(&text).unwrap(),
                Message::Ping(data) => socket.send(Message::Pong(data)).await.unwrap(),
                other => panic!("Unexpected message: {other:?}"),
            }
        }
    })
    .await
    .expect("committed change should reach websocket")
}

async fn closed(socket: &mut Socket) -> CloseCode {
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            match socket.next().await.unwrap().unwrap() {
                Message::Close(frame) => return frame.unwrap().code,
                Message::Ping(data) => socket.send(Message::Pong(data)).await.unwrap(),
                other => panic!("Unexpected payload after access ended: {other:?}"),
            }
        }
    })
    .await
    .expect("connection must close promptly")
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL and Redis"]
async fn committed_changes_reach_two_instances_with_consistent_splits(pool: PgPool) {
    let a = Api::start(pool.clone()).await;
    let b = Api::start(pool.clone()).await;
    let (creator, creator_token, _) = player(&a).await;
    let (joining, token, _) = player(&a).await;
    let game = game(&a, creator).await;
    let mut first = a.connect(game, &creator_token).await;
    let mut second = b.connect(game, &creator_token).await;
    for ws in [&mut first, &mut second] {
        let snapshot = event(ws).await;
        assert_eq!(snapshot["type"], "lobby_snapshot");
        assert_eq!(snapshot["players"].as_array().unwrap().len(), 1);
        assert_eq!(snapshot["split"]["per_player"], 600001);
    }
    // Exercise the REST handler, not just the underlying SQL trigger.
    let app = pickup_server::routes::build_router(a.state.clone());
    let (status, _) = super::call(
        &app,
        "POST",
        &format!("/games/{game}/join"),
        json!({}),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    for ws in [&mut first, &mut second] {
        let snapshot = event(ws).await;
        assert_eq!(snapshot["players"].as_array().unwrap().len(), 2);
        assert_eq!(snapshot["split"]["player_count"], 2);
        assert_eq!(snapshot["split"]["per_player"], 300000);
        assert_eq!(snapshot["split"]["remainder"], 1);
    }
    let mut leaving = b.connect(game, &token).await;
    event(&mut leaving).await;
    sqlx::query("INSERT INTO payments (game_id, user_id, amount, provider, status) VALUES ($1, $2, 300001, 'momo', 'paid')")
        .bind(game).bind(creator).execute(&pool).await.unwrap();
    for ws in [&mut first, &mut second, &mut leaving] {
        let snapshot = event(ws).await;
        assert_eq!(snapshot["split"]["paid_total"], 300001);
        assert_eq!(snapshot["split"]["remaining_total"], 300000);
    }
    GameService::leave_game(&pool, game, joining).await.unwrap();
    assert_eq!(closed(&mut leaving).await, CloseCode::Policy);
    for ws in [&mut first, &mut second] {
        let snapshot = event(ws).await;
        assert_eq!(snapshot["players"].as_array().unwrap().len(), 1);
        assert_eq!(snapshot["split"]["paid_total"], 300001);
    }
    GameService::update_status(&pool, game, creator, GameStatus::InProgress)
        .await
        .unwrap();
    for ws in [&mut first, &mut second] {
        assert_eq!(event(ws).await["status"], "in_progress");
    }
    sqlx::query("UPDATE payments SET status = 'refunded' WHERE game_id = $1")
        .bind(game)
        .execute(&pool)
        .await
        .unwrap();
    for ws in [&mut first, &mut second] {
        assert_eq!(event(ws).await["split"]["paid_total"], 0);
    }
    GameService::cancel_game(&pool, game, creator)
        .await
        .unwrap();
    for ws in [&mut first, &mut second] {
        assert_eq!(event(ws).await["type"], "game_cancelled");
        assert_eq!(closed(ws).await, CloseCode::Normal);
    }
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL and Redis"]
async fn uncommitted_and_rolled_back_changes_never_leak(pool: PgPool) {
    let api = Api::start(pool.clone()).await;
    let (creator, token, _) = player(&api).await;
    let game = game(&api, creator).await;
    let mut socket = api.connect(game, &token).await;
    event(&mut socket).await;
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("UPDATE games SET status = 'in_progress' WHERE id = $1")
        .bind(game)
        .execute(&mut *tx)
        .await
        .unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(250), socket.next())
            .await
            .is_err()
    );
    tx.rollback().await.unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(250), socket.next())
            .await
            .is_err()
    );
    GameService::update_status(&pool, game, creator, GameStatus::InProgress)
        .await
        .unwrap();
    assert_eq!(event(&mut socket).await["status"], "in_progress");
    socket.close(None).await.unwrap();
    GameService::update_status(&pool, game, creator, GameStatus::Completed)
        .await
        .unwrap();
    let mut reconnected = api.connect(game, &token).await;
    assert_eq!(event(&mut reconnected).await["status"], "completed");
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL and Redis"]
async fn handshake_checks_origin_membership_and_token_type(pool: PgPool) {
    let api = Api::start(pool.clone()).await;
    let (creator, token, sid) = player(&api).await;
    let (_, outsider, _) = player(&api).await;
    let game = game(&api, creator).await;
    let refresh = create_refresh_token(creator, sid, &api.state.settings.jwt.secret, 30).unwrap();
    for (token, status) in [
        ("invalid", 401),
        (refresh.as_str(), 401),
        (outsider.as_str(), 403),
    ] {
        let err = connect_async(format!(
            "ws://{}/ws/games/{game}?token={token}",
            api.address
        ))
        .await
        .unwrap_err();
        match err {
            tokio_tungstenite::tungstenite::Error::Http(response) => {
                assert_eq!(response.status().as_u16(), status)
            }
            other => panic!("{other}"),
        }
    }
    let url = format!("ws://{}/ws/games/{game}", api.address);
    let mut request = url.clone().into_client_request().unwrap();
    request
        .headers_mut()
        .insert("Authorization", format!("Bearer {token}").parse().unwrap());
    request
        .headers_mut()
        .insert("Origin", "https://evil.example".parse().unwrap());
    let error = connect_async(request).await.unwrap_err();
    assert!(
        matches!(error, tokio_tungstenite::tungstenite::Error::Http(response) if response.status() == 403)
    );
    let mut request = url.into_client_request().unwrap();
    request
        .headers_mut()
        .insert("Authorization", format!("Bearer {token}").parse().unwrap());
    request
        .headers_mut()
        .insert("Origin", "https://app.example.com".parse().unwrap());
    let mut socket = connect_async(request).await.unwrap().0;
    assert_eq!(event(&mut socket).await["type"], "lobby_snapshot");
    socket
        .send(Message::Text("do not accept client mutations".into()))
        .await
        .unwrap();
    assert_eq!(closed(&mut socket).await, CloseCode::Policy);
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL and Redis"]
async fn revocation_closes_idle_connections_without_a_game_write(pool: PgPool) {
    let api = Api::start(pool.clone()).await;
    let (creator, token, sid) = player(&api).await;
    let game = game(&api, creator).await;
    let mut socket = api.connect(game, &token).await;
    event(&mut socket).await;
    sqlx::query("DELETE FROM auth_sessions WHERE id = $1")
        .bind(sid)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(closed(&mut socket).await, CloseCode::Policy);
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL and Redis"]
async fn shutdown_closes_connected_lobbies(pool: PgPool) {
    let api = Api::start(pool.clone()).await;
    let (creator, token, _) = player(&api).await;
    let game = game(&api, creator).await;
    let mut socket = api.connect(game, &token).await;
    event(&mut socket).await;
    api.state.shutdown.cancel();
    assert_eq!(closed(&mut socket).await, CloseCode::Away);
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL and Redis"]
async fn listener_reconnect_and_periodic_recheck_recover_a_missed_notification(pool: PgPool) {
    let api = Api::start(pool.clone()).await;
    let (creator, token, _) = player(&api).await;
    let game = game(&api, creator).await;
    let mut socket = api.connect(game, &token).await;
    event(&mut socket).await;
    let original: i32 = sqlx::query_scalar(
        "SELECT pid FROM pg_stat_activity WHERE application_name = 'pickup-lobby-listener' AND datname = current_database()"
    ).fetch_one(&pool).await.unwrap();
    assert!(
        sqlx::query_scalar::<_, bool>("SELECT pg_terminate_backend($1)")
            .bind(original)
            .fetch_one(&pool)
            .await
            .unwrap()
    );

    // Let SQLx restore LISTEN, then change the row after the forced gap. Even
    // if that notification was lost, the live connection's recheck repairs it.
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let current: Option<i32> = sqlx::query_scalar(
                "SELECT pid FROM pg_stat_activity WHERE application_name = 'pickup-lobby-listener' AND datname = current_database()"
            ).fetch_optional(&pool).await.unwrap();
            if current.is_some_and(|pid| pid != original) { break; }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }).await.expect("dedicated listener should reconnect");
    tokio::time::sleep(Duration::from_secs(1)).await;
    GameService::update_status(&pool, game, creator, GameStatus::InProgress)
        .await
        .unwrap();
    let snapshot = tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let message = socket.next().await.unwrap().unwrap();
            if let Message::Text(text) = message {
                let value: Value = serde_json::from_str(&text).unwrap();
                if value["status"] == "in_progress" {
                    return value;
                }
            }
        }
    })
    .await
    .expect("notification or bounded recovery check must refresh the room");
    assert_eq!(snapshot["game_id"], game.to_string());
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL and Redis"]
async fn websocket_connection_limits_are_enforced_and_released(pool: PgPool) {
    let api = Api::start(pool.clone()).await;
    let (creator, token, _) = player(&api).await;
    let game = game(&api, creator).await;
    let mut sockets = Vec::new();
    for _ in 0..5 {
        sockets.push(api.connect(game, &token).await);
    }
    let error = connect_async(format!(
        "ws://{}/ws/games/{game}?token={token}",
        api.address
    ))
    .await
    .unwrap_err();
    assert!(
        matches!(error, tokio_tungstenite::tungstenite::Error::Http(response)
        if response.status() == StatusCode::TOO_MANY_REQUESTS)
    );
    for socket in &mut sockets {
        socket.close(None).await.unwrap();
    }
    drop(sockets);
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if connect_async(format!(
                "ws://{}/ws/games/{game}?token={token}",
                api.address
            ))
            .await
            .is_ok()
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("disconnects must release per-user connection capacity");
}
