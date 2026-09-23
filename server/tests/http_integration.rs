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
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

mod realtime;

async fn state(pool: PgPool) -> AppState {
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
            partner_code: String::new(),
            access_key: String::new(),
            secret_key: String::new(),
            endpoint: String::new(),
        },
        zalopay: ZaloPaySettings {
            app_id: String::new(),
            key1: String::new(),
            key2: String::new(),
            endpoint: String::new(),
        },
        s3: S3Settings {
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
        StatusCode::NOT_IMPLEMENTED
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
async fn revoked_sessions_fail_in_middleware_and_fallback_extractor(pool: PgPool) {
    let app = build_router(state(pool.clone()).await);
    let id = Uuid::new_v4();
    let (status, auth) = call(&app, "POST", "/auth/register", serde_json::json!({
        "phone": "0901234567", "email": format!("{id}@example.com"), "password": "valid-password-123"
    }), None).await;
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
    sqlx::query("DELETE FROM auth_sessions")
        .execute(&pool)
        .await
        .unwrap();
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
    .expect("shutdown must not issue BRPOP with an infinite timeout");
}
