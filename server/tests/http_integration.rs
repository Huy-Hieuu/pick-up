use std::sync::Arc;

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use pickup_server::{
    config::{settings::*, Settings},
    routes::build_router,
    state::AppState,
};
use redis::AsyncCommands;
use sqlx::PgPool;
use std::time::Duration;
use tower::ServiceExt;
use uuid::Uuid;

mod media;
mod payment;
mod realtime;

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL and Redis"]
async fn invalid_geo_queries_fail_before_database_functions(pool: PgPool) {
    let state = state(pool.clone()).await;
    let user: Uuid = sqlx::query_scalar("INSERT INTO users (phone) VALUES ($1) RETURNING id")
        .bind(Uuid::new_v4().to_string()[..20].to_owned())
        .fetch_one(&pool)
        .await
        .unwrap();
    let sid = Uuid::new_v4();
    sqlx::query("INSERT INTO auth_sessions (id, user_id, refresh_jti, expires_at) VALUES ($1, $2, $3, now() + interval '1 hour')").bind(sid).bind(user).bind(Uuid::new_v4()).execute(&pool).await.unwrap();
    let token = pickup_server::extractors::auth::create_access_token(
        user,
        sid,
        &state.settings.jwt.secret,
        15,
    )
    .unwrap();
    let app = build_router(state);
    for route in ["/courts", "/games"] {
        for query in [
            "lat=NaN&lng=106",
            "lat=inf&lng=106",
            "lat=91&lng=106",
            "lat=10&lng=-181",
            "lat=10",
            "lng=106",
            "radius_km=NaN",
            "lat=10&lng=106&radius_km=inf",
        ] {
            let (status, response) = call(
                &app,
                "GET",
                &format!("{route}?{query}"),
                serde_json::Value::Null,
                Some(&token),
            )
            .await;
            assert!(
                matches!(
                    status,
                    StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY
                ),
                "{route}?{query}: {status} {response}"
            );
        }
        for query in ["lat=90&lng=180", "lat=-90&lng=-180"] {
            assert_eq!(
                call(
                    &app,
                    "GET",
                    &format!("{route}?{query}"),
                    serde_json::Value::Null,
                    Some(&token)
                )
                .await
                .0,
                StatusCode::OK
            );
        }
    }
}

async fn state(pool: PgPool) -> AppState {
    state_with_payment_endpoints(
        pool,
        "https://test-payment.momo.vn/v2/gateway/api/create".into(),
        "https://sb-openapi.zalopay.vn/v2/create".into(),
    )
    .await
}

async fn state_with_payment_endpoints(
    pool: PgPool,
    momo_endpoint: String,
    zalopay_endpoint: String,
) -> AppState {
    let redis_url = std::env::var("TEST_REDIS_URL").expect("TEST_REDIS_URL required");
    let redis = redis::Client::open(redis_url.as_str())
        .unwrap()
        .get_connection_manager()
        .await
        .unwrap();
    let settings = Settings {
        app: AppSettings {
            host: "127.0.0.1".into(),
            port: 8080,
            cors_origins: vec!["https://app.example.com".into()],
            public_app_url: "https://app.example.com".into(),
            public_api_url: "https://api.example.com".into(),
            trusted_proxy_cidrs: Vec::new(),
            requests_per_minute: 600,
        },
        database: DatabaseSettings {
            url: String::new(),
            max_connections: 5,
            run_migrations: false,
        },
        redis: RedisSettings {
            url: redis_url,
            otp_ttl_seconds: 300,
            otp_max_attempts: 3,
        },
        email: EmailSettings {
            smtp_host: "localhost".into(),
            smtp_port: 1025,
            smtp_username: String::new(),
            smtp_password: String::new(),
            mail_from: "test@example.com".into(),
            mail_from_name: "Test".into(),
            use_tls: false,
        },
        jwt: JwtSettings {
            secret: "http-integration-secret-at-least-32-bytes".into(),
            access_ttl_minutes: 15,
            refresh_ttl_days: 30,
        },
        sms: SmsSettings {
            api_key: String::new(),
            secret_key: String::new(),
            sender: String::new(),
        },
        momo: MomoSettings {
            partner_code: "TEST_PARTNER".into(),
            access_key: "test_access_key".into(),
            secret_key: "test_secret_key".into(),
            endpoint: momo_endpoint,
        },
        zalopay: ZaloPaySettings {
            app_id: "2553".into(),
            key1: "test_key1".into(),
            key2: "test_key2".into(),
            endpoint: zalopay_endpoint,
        },
        s3: S3Settings {
            internal_endpoint: "http://localhost:9000".into(),
            region: "us-east-1".into(),
            endpoint: "http://localhost:9000".into(),
            access_key: "test".into(),
            secret_key: "test".into(),
            bucket: "test".into(),
            public_url: "http://localhost:9000/test".into(),
        },
    };
    let s3 = aws_sdk_s3::Client::from_conf(
        aws_sdk_s3::config::Builder::new()
            .behavior_version_latest()
            .region(aws_types::region::Region::new("us-east-1"))
            .credentials_provider(aws_credential_types::Credentials::new(
                "test", "test", None, None, "test",
            ))
            .endpoint_url("http://localhost:9000")
            .force_path_style(true)
            .build(),
    );
    AppState::new(pool, redis, settings, s3)
}

async fn call(
    app: &Router,
    method: &str,
    path: &str,
    body: serde_json::Value,
    token: Option<&str>,
) -> (StatusCode, serde_json::Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(token) = token {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1_000_000).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    )
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL and Redis"]
async fn personal_schedule_is_private_and_recoverable(pool: PgPool) {
    use pickup_server::models::court::Pagination;
    let state = state(pool.clone()).await;
    let mut identities = Vec::new();
    for _ in 0..2 {
        let user: Uuid = sqlx::query_scalar("INSERT INTO users (phone) VALUES ($1) RETURNING id")
            .bind(Uuid::new_v4().to_string()[..20].to_owned())
            .fetch_one(&pool)
            .await
            .unwrap();
        let sid = Uuid::new_v4();
        sqlx::query("INSERT INTO auth_sessions (id, user_id, refresh_jti, expires_at) VALUES ($1, $2, $3, now() + interval '1 hour')").bind(sid).bind(user).bind(Uuid::new_v4()).execute(&pool).await.unwrap();
        let token = pickup_server::extractors::auth::create_access_token(
            user,
            sid,
            &state.settings.jwt.secret,
            15,
        )
        .unwrap();
        identities.push((user, token));
    }
    let (creator, token) = &identities[0];
    let (stranger, stranger_token) = &identities[1];
    let creator = *creator;
    let stranger = *stranger;
    let court: Uuid = sqlx::query_scalar("INSERT INTO courts (name, sport_type, lat, lng, address, price_per_slot, owner_id) VALUES ('Schedule', 'pickleball', 10, 106, 'Test', 600001, $1) RETURNING id").bind(creator).fetch_one(&pool).await.unwrap();
    let slot: Uuid = sqlx::query_scalar("INSERT INTO court_slots (court_id, start_time, end_time) VALUES ($1, now() + interval '1 day', now() + interval '25 hours') RETURNING id").bind(court).fetch_one(&pool).await.unwrap();
    pickup_server::services::CourtService::book_slot(
        &pool,
        court,
        slot,
        creator,
        pickup_server::models::court::BookSlotRequest {},
    )
    .await
    .unwrap();
    let game = pickup_server::services::GameService::create_game(
        &pool,
        creator,
        pickup_server::models::game::CreateGameRequest {
            court_slot_id: slot,
            sport_type: pickup_server::models::court::SportType::Pickleball,
            max_players: 4,
            description: None,
        },
    )
    .await
    .unwrap()
    .game
    .id;
    let app = build_router(state);
    sqlx::query("UPDATE court_slots SET start_time = '2030-01-01T12:00:00Z', end_time = '2030-01-01T13:00:00Z' WHERE id = $1").bind(slot).execute(&pool).await.unwrap();
    let (status, share) = call(
        &app,
        "GET",
        &format!("/games/{game}/share"),
        serde_json::Value::Null,
        Some(token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(share["url"], format!("https://app.example.com/game/{game}"));
    assert!(share["message"]
        .as_str()
        .unwrap()
        .contains("01/01 at 19:00"));
    let (status, bookings) = call(
        &app,
        "GET",
        "/auth/me/bookings",
        serde_json::Value::Null,
        Some(token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(bookings["total"], 1);
    assert_eq!(bookings["bookings"][0]["game_id"], game.to_string());
    assert_eq!(bookings["bookings"][0]["booked_by"], creator.to_string());
    let (_, games) = call(
        &app,
        "GET",
        "/auth/me/games",
        serde_json::Value::Null,
        Some(token),
    )
    .await;
    assert_eq!(games["total"], 1);
    assert_eq!(games["games"][0]["id"], game.to_string());
    for path in ["/auth/me/bookings", "/auth/me/games"] {
        assert_eq!(
            call(&app, "GET", path, serde_json::Value::Null, None)
                .await
                .0,
            StatusCode::UNAUTHORIZED
        );
        let (_, private) = call(
            &app,
            "GET",
            path,
            serde_json::Value::Null,
            Some(stranger_token),
        )
        .await;
        assert_eq!(private["total"], 0);
    }
    pickup_server::services::GameService::join_game(&pool, game, stranger)
        .await
        .unwrap();
    let joined = pickup_server::services::schedule::games(
        &pool,
        stranger,
        Pagination {
            page: None,
            per_page: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(joined.total, 1);
    pickup_server::services::GameService::leave_game(&pool, game, stranger)
        .await
        .unwrap();
    assert_eq!(
        pickup_server::services::schedule::games(
            &pool,
            stranger,
            Pagination {
                page: None,
                per_page: None
            }
        )
        .await
        .unwrap()
        .total,
        0
    );
    pickup_server::services::GameService::cancel_game(&pool, game, creator)
        .await
        .unwrap();
    let history = pickup_server::services::schedule::games(
        &pool,
        creator,
        Pagination {
            page: None,
            per_page: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(history.total, 1);
    assert_eq!(
        history.games[0].status,
        pickup_server::models::game::GameStatus::Cancelled
    );
    let bookings = pickup_server::services::schedule::bookings(
        &pool,
        creator,
        Pagination {
            page: None,
            per_page: None,
        },
    )
    .await
    .unwrap();
    assert!(bookings.bookings.is_empty());
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL and Redis"]
async fn router_enforces_auth_and_responds_to_health_and_body_limits(pool: PgPool) {
    let state = state(pool.clone()).await;
    let app = build_router(state.clone());
    for path in ["/health", "/health/ready", "/courts"] {
        assert_eq!(
            call(&app, "GET", path, serde_json::Value::Null, None)
                .await
                .0,
            StatusCode::OK,
            "{path}"
        );
    }
    for path in ["/games", "/auth/me"] {
        assert_eq!(
            call(&app, "GET", path, serde_json::Value::Null, None)
                .await
                .0,
            StatusCode::UNAUTHORIZED
        );
    }
    assert_eq!(
        call(&app, "POST", "/webhooks/momo", serde_json::json!({}), None)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/login")
                .header("content-type", "application/json")
                .header("content-length", "70000")
                .body(Body::from("x".repeat(70000)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    state.shutdown.cancel();
    assert_eq!(
        call(&app, "GET", "/health/ready", serde_json::Value::Null, None)
            .await
            .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        call(&app, "GET", "/health", serde_json::Value::Null, None)
            .await
            .0,
        StatusCode::OK
    );
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL and Redis"]
async fn login_otp_request_does_not_reveal_account_state(pool: PgPool) {
    let state = state(pool.clone()).await;
    let redis = state.redis.clone();
    let app = build_router(state);
    let verified_email = format!("{}@example.test", Uuid::new_v4());
    sqlx::query("INSERT INTO users (phone, email, email_verified) VALUES ($1, $2, TRUE)")
        .bind(Uuid::new_v4().to_string()[..20].to_owned())
        .bind(&verified_email)
        .execute(&pool)
        .await
        .unwrap();
    let unknown_email = format!("{}@example.test", Uuid::new_v4());

    let (unknown_status, unknown_body) = call(
        &app,
        "POST",
        "/auth/otp",
        serde_json::json!({"email": unknown_email}),
        None,
    )
    .await;
    let (verified_status, verified_body) = call(
        &app,
        "POST",
        "/auth/otp",
        serde_json::json!({"email": verified_email}),
        None,
    )
    .await;
    assert_eq!(unknown_status, StatusCode::OK);
    assert_eq!(verified_status, StatusCode::OK);
    assert_eq!(unknown_body, verified_body);

    let (unknown_reset_status, unknown_reset_body) = call(
        &app,
        "POST",
        "/auth/forgot-password",
        serde_json::json!({"email": unknown_email}),
        None,
    )
    .await;
    let (verified_reset_status, verified_reset_body) = call(
        &app,
        "POST",
        "/auth/forgot-password",
        serde_json::json!({"email": verified_email}),
        None,
    )
    .await;
    assert_eq!(unknown_reset_status, StatusCode::OK);
    assert_eq!(verified_reset_status, StatusCode::OK);
    assert_eq!(unknown_reset_body, verified_reset_body);

    let (unknown_resend_status, unknown_resend_body) = call(
        &app,
        "POST",
        "/auth/resend-registration-otp",
        serde_json::json!({"email": unknown_email}),
        None,
    )
    .await;
    let (verified_resend_status, verified_resend_body) = call(
        &app,
        "POST",
        "/auth/resend-registration-otp",
        serde_json::json!({"email": verified_email}),
        None,
    )
    .await;
    assert_eq!(unknown_resend_status, StatusCode::OK);
    assert_eq!(verified_resend_status, StatusCode::OK);
    assert_eq!(unknown_resend_body, verified_resend_body);

    let issued_registration_code: String = redis
        .clone()
        .get(format!("otp:registration:{unknown_email}"))
        .await
        .unwrap();
    let wrong_code = if issued_registration_code == "000000" {
        "999999"
    } else {
        "000000"
    };
    let verify_body = |email: &str| {
        serde_json::json!({
            "email": email,
            "code": wrong_code,
            "password": "registration-owner-password",
            "phone": "0907777888",
        })
    };
    let (unknown_verify_status, unknown_verify_body) = call(
        &app,
        "POST",
        "/auth/verify-registration",
        verify_body(&unknown_email),
        None,
    )
    .await;
    let (verified_verify_status, verified_verify_body) = call(
        &app,
        "POST",
        "/auth/verify-registration",
        verify_body(&verified_email),
        None,
    )
    .await;
    assert_eq!(unknown_verify_status, StatusCode::UNAUTHORIZED);
    assert_eq!(verified_verify_status, StatusCode::UNAUTHORIZED);
    assert_eq!(unknown_verify_body, verified_verify_body);

    let unknown_code: Option<String> = redis
        .clone()
        .get(format!("otp:login:{unknown_email}"))
        .await
        .unwrap();
    let verified_code: Option<String> = redis
        .clone()
        .get(format!("otp:login:{verified_email}"))
        .await
        .unwrap();
    assert!(unknown_code.is_none());
    assert!(verified_code.is_some());
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL and Redis"]
async fn per_ip_rate_limit_is_shared_and_health_probes_are_exempt(pool: PgPool) {
    let mut state = state(pool).await;
    Arc::make_mut(&mut state.settings).app.requests_per_minute = 2;
    let app = build_router(state);
    let bytes = Uuid::new_v4().as_bytes()[0];
    let peer =
        axum::extract::ConnectInfo(std::net::SocketAddr::from(([198, 51, 100, bytes], 12345)));
    for _ in 0..2 {
        let mut request = Request::builder()
            .method("POST")
            .uri("/auth/login")
            .header("content-type", "application/json")
            .body(Body::from("{}"))
            .unwrap();
        request.extensions_mut().insert(peer);
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    // A second socket peer has an independent window even when the request
    // handler cannot extract authentication because the body is malformed.
    let other_peer = axum::extract::ConnectInfo(std::net::SocketAddr::from((
        [198, 51, 100, bytes.wrapping_add(1)],
        12345,
    )));
    let mut other_request = Request::builder()
        .method("POST")
        .uri("/auth/login")
        .header("content-type", "application/json")
        .body(Body::from("{}"))
        .unwrap();
    other_request.extensions_mut().insert(other_peer);
    assert_eq!(
        app.clone().oneshot(other_request).await.unwrap().status(),
        StatusCode::BAD_REQUEST
    );
    let mut request = Request::builder()
        .uri("/another-route")
        .body(Body::empty())
        .unwrap();
    request.extensions_mut().insert(peer);
    let limited = app.clone().oneshot(request).await.unwrap();
    assert_eq!(limited.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(limited.headers()[axum::http::header::RETRY_AFTER], "60");
    assert_eq!(
        call(&app, "GET", "/health", serde_json::Value::Null, None)
            .await
            .0,
        StatusCode::OK
    );
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL and Redis"]
async fn avatar_upload_signature_binds_exact_length_and_rejects_oversize(pool: PgPool) {
    use pickup_server::extractors::auth::create_access_token;
    let state = state(pool.clone()).await;
    let user: Uuid = sqlx::query_scalar(
        "INSERT INTO users (phone, email, email_verified, display_name) VALUES ($1, $2, TRUE, 'Player') RETURNING id",
    ).bind(Uuid::new_v4().to_string()[..20].to_owned())
        .bind(format!("{}@example.test", Uuid::new_v4()))
        .fetch_one(&pool).await.unwrap();
    let session = Uuid::new_v4();
    sqlx::query("INSERT INTO auth_sessions (id, user_id, refresh_jti, expires_at) VALUES ($1, $2, $3, now() + interval '15 minutes')")
        .bind(session).bind(user).bind(Uuid::new_v4()).execute(&pool).await.unwrap();
    let token = create_access_token(user, session, &state.settings.jwt.secret, 15).unwrap();
    let app = build_router(state);
    let (status, _) = call(
        &app,
        "POST",
        "/upload/avatar",
        serde_json::json!({
            "content_type": "image/png", "content_length": 8 * 1024 * 1024 + 1
        }),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, response) = call(
        &app,
        "POST",
        "/upload/avatar",
        serde_json::json!({
            "content_type": "image/jpeg", "content_length": 245760
        }),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["content_length"], 245760);
    assert_eq!(response["max_upload_bytes"], 8 * 1024 * 1024);
    assert!(response["upload_id"].as_str().is_some());
    assert!(response.get("public_url").is_none());
    assert!(response["object_key"]
        .as_str()
        .unwrap()
        .starts_with("quarantine/"));
    let signed = url::Url::parse(response["upload_url"].as_str().unwrap()).unwrap();
    let headers = signed
        .query_pairs()
        .find(|(key, _)| key == "X-Amz-SignedHeaders")
        .unwrap()
        .1;
    assert!(headers.split(';').any(|header| header == "content-length"));
    assert!(headers.split(';').any(|header| header == "content-type"));
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL and Redis"]
async fn revoked_sessions_fail_in_middleware_and_fallback_extractor(pool: PgPool) {
    let state = state(pool.clone()).await;
    let redis = state.redis.clone();
    let app = build_router(state);
    let id = Uuid::new_v4();
    let email = format!("{id}@example.com");
    let (status, pending) = call(
        &app,
        "POST",
        "/auth/register",
        serde_json::json!({ "email": email }),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{pending}");
    assert!(pending["message"]
        .as_str()
        .unwrap()
        .contains("verification instructions"));
    let code: String = redis
        .clone()
        .get(format!("otp:registration:{email}"))
        .await
        .unwrap();
    let (status, auth) = call(
        &app,
        "POST",
        "/auth/verify-registration",
        serde_json::json!({
            "email": email, "code": code, "password": "verified-test-password-123",
            "phone": "0901234567",
        }),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{auth}");
    let token = auth["access_token"].as_str().unwrap();
    assert_eq!(
        call(
            &app,
            "GET",
            "/auth/me",
            serde_json::Value::Null,
            Some(token)
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &app,
            "POST",
            "/auth/logout",
            serde_json::json!({}),
            Some(token)
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(
            &app,
            "POST",
            "/auth/refresh",
            serde_json::json!({"refresh_token": auth["refresh_token"]}),
            None
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(
            &app,
            "GET",
            "/auth/me",
            serde_json::Value::Null,
            Some(token)
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let path = format!("/courts/{}/slots/{}/book", Uuid::new_v4(), Uuid::new_v4());
    assert_eq!(
        call(&app, "POST", &path, serde_json::json!({}), Some(token))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL and Redis"]
async fn readiness_fails_when_database_is_unavailable(pool: PgPool) {
    let app = build_router(state(pool.clone()).await);
    pool.close().await;
    assert_eq!(
        call(&app, "GET", "/health/ready", serde_json::Value::Null, None)
            .await
            .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        call(&app, "GET", "/health", serde_json::Value::Null, None)
            .await
            .0,
        StatusCode::OK
    );
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL and Redis"]
async fn cancelled_email_worker_does_not_block_on_an_empty_queue(pool: PgPool) {
    let state = state(pool).await;
    state.shutdown.cancel();
    tokio::time::timeout(
        std::time::Duration::from_secs(1),
        pickup_server::jobs::email::run_email_worker(
            state.redis,
            state.settings.email.clone(),
            state.shutdown,
        ),
    )
    .await
    .expect("shutdown releases email worker leases promptly");
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL and Redis"]
async fn email_worker_delivers_queued_otp_through_smtp(pool: PgPool) {
    use pickup_server::jobs::email::{enqueue_email, run_email_worker, EmailJob};
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let state = state(pool).await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let smtp_address = listener.local_addr().unwrap();
    let expected = format!("otp-worker-test-{}", Uuid::new_v4());
    let (delivered, delivered_rx) = tokio::sync::oneshot::channel::<String>();
    let (diagnostics, diagnostic_rx) = tokio::sync::oneshot::channel::<String>();
    let smtp_cancel = tokio_util::sync::CancellationToken::new();
    let server_cancel = smtp_cancel.clone();
    let target = expected.clone();
    let diagnostic_tx = Arc::new(std::sync::Mutex::new(Some(diagnostics)));
    let diagnostic_server = diagnostic_tx.clone();
    let mut delivered = Some(delivered);
    let smtp = tokio::spawn(async move {
        loop {
            let accepted = tokio::select! {
                _ = server_cancel.cancelled() => return,
                connection = listener.accept() => connection.unwrap().0,
            };
            let mut client = BufReader::new(accepted);
            client
                .get_mut()
                .write_all(b"220 local.test ESMTP\r\n")
                .await
                .unwrap();
            let mut captured = String::new();
            loop {
                let mut command = String::new();
                if client.read_line(&mut command).await.unwrap() == 0 {
                    break;
                }
                let command = command.trim_end();
                if !(command.starts_with("EHLO")
                    || command.starts_with("HELO")
                    || command.starts_with("MAIL FROM")
                    || command.starts_with("RCPT TO")
                    || command == "NOOP"
                    || command == "RSET"
                    || command == "DATA"
                    || command == "QUIT")
                {
                    if let Some(diagnostic) = diagnostic_server.lock().unwrap().take() {
                        let _ = diagnostic.send(command.to_owned());
                    }
                    client
                        .get_mut()
                        .write_all(b"500 unsupported\r\n")
                        .await
                        .unwrap();
                    continue;
                }
                let response = if command.starts_with("EHLO") || command.starts_with("HELO") {
                    "250-local.test\r\n250 SIZE 1048576\r\n"
                } else if command.starts_with("MAIL FROM") || command.starts_with("RCPT TO") {
                    "250 accepted\r\n"
                } else if command == "DATA" {
                    client
                        .get_mut()
                        .write_all(b"354 continue\r\n")
                        .await
                        .unwrap();
                    loop {
                        let mut line = String::new();
                        if client.read_line(&mut line).await.unwrap() == 0 {
                            return;
                        }
                        if line == ".\r\n" {
                            break;
                        }
                        captured.push_str(&line);
                    }
                    if captured.contains(&target) {
                        if let Some(delivered) = delivered.take() {
                            let _ = delivered.send(captured.clone());
                        }
                    }
                    "250 queued\r\n"
                } else if command == "QUIT" {
                    client.get_mut().write_all(b"221 bye\r\n").await.unwrap();
                    break;
                } else {
                    "250 ok\r\n"
                };
                client
                    .get_mut()
                    .write_all(response.as_bytes())
                    .await
                    .unwrap();
            }
        }
    });

    let redis = state.redis.clone();
    let mut email = state.settings.email.clone();
    email.smtp_host = "127.0.0.1".into();
    email.smtp_port = smtp_address.port();
    email.use_tls = false;
    let job = EmailJob {
        id: Uuid::new_v4(),
        to: "delivery@example.test".into(),
        subject: expected.clone(),
        body: "Your OTP is 654321".into(),
        retry_count: 0,
    };
    enqueue_email(&redis, &job).await.unwrap();
    let shutdown = state.shutdown.clone();
    let worker = tokio::spawn(run_email_worker(redis, email, shutdown.clone()));
    let message = tokio::select! {
        result = tokio::time::timeout(Duration::from_secs(20), delivered_rx) => result.expect("SMTP worker should deliver the queued job").unwrap(),
        error = diagnostic_rx => panic!("Unexpected SMTP command: {error:?}"),
    };
    assert!(message.contains("To: delivery@example.test"));
    assert!(message.contains("Your OTP is 654321"));
    shutdown.cancel();
    tokio::time::timeout(Duration::from_secs(2), worker)
        .await
        .unwrap()
        .unwrap();
    smtp_cancel.cancel();
    tokio::time::timeout(Duration::from_secs(2), smtp)
        .await
        .unwrap()
        .unwrap();
}
