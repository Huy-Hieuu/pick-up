//! Run with DATABASE_URL and TEST_REDIS_URL pointing at disposable services:
//! cargo test --test auth_integration -- --include-ignored
use pickup_server::{
    db,
    error::AppError,
    extractors::auth::{authenticate_access_token, decode_token},
    models::otp::{
        LoginRequest, RegisterRequest, SetNewPasswordRequest, VerifyOtpRequest,
        VerifyResetOtpRequest,
    },
    services::AuthService,
};
use redis::{aio::ConnectionManager, AsyncCommands};
use sqlx::PgPool;
use uuid::Uuid;

const SECRET: &str = "integration-test-secret-at-least-32-bytes";

async fn redis() -> ConnectionManager {
    redis::Client::open(std::env::var("TEST_REDIS_URL").expect("TEST_REDIS_URL required"))
        .unwrap()
        .get_connection_manager()
        .await
        .unwrap()
}

async fn register(pool: &PgPool) -> pickup_server::models::user::AuthResponse {
    let id = Uuid::new_v4();
    AuthService::register(
        pool,
        RegisterRequest {
            phone: format!("+849{:08}", id.as_u128() % 100_000_000),
            email: format!("{id}@example.com"),
            password: "valid-password-123".into(),
            display_name: None,
            avatar_url: None,
        },
        SECRET,
        15,
        30,
    )
    .await
    .unwrap()
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL and Redis"]
async fn refresh_is_single_use_under_concurrency(pool: PgPool) {
    let auth = register(&pool).await;
    let redis = redis().await;
    let (a, b) = tokio::join!(
        AuthService::refresh_token(&pool, &redis, &auth.refresh_token, SECRET, 15, 30),
        AuthService::refresh_token(&pool, &redis, &auth.refresh_token, SECRET, 15, 30),
    );
    assert_ne!(a.is_ok(), b.is_ok(), "exactly one rotation must succeed");
    let rotated = a.or(b).unwrap();
    assert_ne!(auth.refresh_token, rotated.refresh_token);
    assert!(
        AuthService::refresh_token(&pool, &redis, &auth.refresh_token, SECRET, 15, 30)
            .await
            .is_err()
    );
    assert!(
        AuthService::refresh_token(&pool, &redis, &rotated.refresh_token, SECRET, 15, 30)
            .await
            .is_ok()
    );
    assert!(authenticate_access_token(&pool, &auth.access_token, SECRET)
        .await
        .is_ok());
    assert!(
        authenticate_access_token(&pool, &auth.refresh_token, SECRET)
            .await
            .is_err()
    );
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL and Redis"]
async fn password_reset_revokes_access_refresh_and_older_reset_grants(pool: PgPool) {
    let auth = register(&pool).await;
    let redis = redis().await;
    let claims = decode_token(&auth.access_token, SECRET, "access").unwrap();
    let id = Uuid::parse_str(&claims.sub).unwrap();
    let user = db::find_user_by_id(&pool, id).await.unwrap().unwrap();
    let email = user.email.as_ref().unwrap();
    AuthService::forgot_password(&pool, &redis, email, 300)
        .await
        .unwrap();
    let code: String = redis
        .clone()
        .get(format!("otp:reset:{email}"))
        .await
        .unwrap();
    let grant = AuthService::verify_reset_otp(
        &pool,
        &redis,
        VerifyResetOtpRequest {
            email: email.clone(),
            code,
        },
        3,
        300,
    )
    .await
    .unwrap();
    // A second outstanding grant must be invalidated by the first reset.
    let stale_token = hex::encode(rand::random::<[u8; 32]>());
    let stale_grant = serde_json::to_string(&(id, user.password_hash)).unwrap();
    let _: () = redis
        .clone()
        .set_ex(format!("reset_token:{stale_token}"), stale_grant, 300)
        .await
        .unwrap();
    AuthService::set_new_password(
        &pool,
        &redis,
        SetNewPasswordRequest {
            reset_token: grant.reset_token.clone(),
            new_password: "new-password-456".into(),
        },
    )
    .await
    .unwrap();
    assert!(authenticate_access_token(&pool, &auth.access_token, SECRET)
        .await
        .is_err());
    assert!(
        AuthService::refresh_token(&pool, &redis, &auth.refresh_token, SECRET, 15, 30)
            .await
            .is_err()
    );
    for token in [grant.reset_token, stale_token] {
        assert!(AuthService::set_new_password(
            &pool,
            &redis,
            SetNewPasswordRequest {
                reset_token: token,
                new_password: "should-not-be-used".into(),
            }
        )
        .await
        .is_err());
    }
    assert!(AuthService::login(
        &pool,
        &redis,
        LoginRequest {
            email: email.clone(),
            password: "new-password-456".into(),
        },
        SECRET,
        15,
        30
    )
    .await
    .is_ok());
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL and Redis"]
async fn otp_attempts_are_bounded_and_do_not_extend_expiry(pool: PgPool) {
    let auth = register(&pool).await;
    let redis = redis().await;
    let id = Uuid::parse_str(
        &decode_token(&auth.access_token, SECRET, "access")
            .unwrap()
            .sub,
    )
    .unwrap();
    let email = db::find_user_by_id(&pool, id)
        .await
        .unwrap()
        .unwrap()
        .email
        .unwrap();
    AuthService::send_otp(&redis, &email, 300).await.unwrap();
    let key = format!("otp:login:{email}");
    let code: String = redis.clone().get(&key).await.unwrap();
    let _: () = redis.clone().pexpire(&key, 20_000).await.unwrap();
    for _ in 0..3 {
        assert!(AuthService::verify_otp(
            &pool,
            &redis,
            VerifyOtpRequest {
                email: email.clone(),
                phone: "+84901234567".into(),
                code: "wrong!".into(),
            },
            3,
            300,
            SECRET,
            15,
            30
        )
        .await
        .is_err());
        let ttl: i64 = redis.clone().pttl(&key).await.unwrap();
        assert!(ttl <= 20_000, "failed attempts must not renew expiry");
    }
    assert!(AuthService::verify_otp(
        &pool,
        &redis,
        VerifyOtpRequest {
            email: email.clone(),
            phone: "+84901234567".into(),
            code,
        },
        3,
        300,
        SECRET,
        15,
        30
    )
    .await
    .is_err());
    let exists: bool = redis
        .clone()
        .exists(format!("otp:login:attempts:{email}"))
        .await
        .unwrap();
    assert!(
        !exists,
        "exhausted/missing codes must not leak permanent attempt keys"
    );
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL and Redis"]
async fn otp_is_single_use_and_purpose_bound(pool: PgPool) {
    let auth = register(&pool).await;
    let redis = redis().await;
    let id = Uuid::parse_str(
        &decode_token(&auth.access_token, SECRET, "access")
            .unwrap()
            .sub,
    )
    .unwrap();
    let email = db::find_user_by_id(&pool, id)
        .await
        .unwrap()
        .unwrap()
        .email
        .unwrap();
    let (a, b) = tokio::join!(
        AuthService::send_otp(&redis, &email, 300),
        AuthService::send_otp(&redis, &email, 300)
    );
    assert_ne!(a.is_ok(), b.is_ok());
    let code: String = redis
        .clone()
        .get(format!("otp:login:{email}"))
        .await
        .unwrap();
    assert!(AuthService::verify_reset_otp(
        &pool,
        &redis,
        VerifyResetOtpRequest {
            email: email.clone(),
            code: code.clone(),
        },
        3,
        300
    )
    .await
    .is_err());
    let request = || VerifyOtpRequest {
        email: email.clone(),
        phone: "+84901234567".into(),
        code: code.clone(),
    };
    let (a, b) = tokio::join!(
        AuthService::verify_otp(&pool, &redis, request(), 3, 300, SECRET, 15, 30),
        AuthService::verify_otp(&pool, &redis, request(), 3, 300, SECRET, 15, 30),
    );
    assert_ne!(a.is_ok(), b.is_ok());
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL and Redis"]
async fn unknown_accounts_also_have_a_login_attempt_budget(pool: PgPool) {
    let redis = redis().await;
    let email = format!("{}@example.com", Uuid::new_v4());
    for _ in 0..5 {
        assert!(matches!(
            AuthService::login(
                &pool,
                &redis,
                LoginRequest {
                    email: email.clone(),
                    password: "bad-password".into(),
                },
                SECRET,
                15,
                30
            )
            .await,
            Err(AppError::Unauthorized(_))
        ));
    }
    assert!(matches!(
        AuthService::login(
            &pool,
            &redis,
            LoginRequest {
                email,
                password: "bad-password".into(),
            },
            SECRET,
            15,
            30
        )
        .await,
        Err(AppError::TooManyRequests(_))
    ));
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL"]
async fn registration_preserves_international_phone_and_normalizes_local_phone(pool: PgPool) {
    let a = register(&pool).await;
    assert!(a.user.phone.starts_with("+849"));
    assert!(!a.user.phone.contains("+84+84"));
    let b = AuthService::register(
        &pool,
        RegisterRequest {
            phone: "0901234567".into(),
            email: "local@example.com".into(),
            password: "valid-password".into(),
            display_name: None,
            avatar_url: None,
        },
        SECRET,
        15,
        30,
    )
    .await
    .unwrap();
    assert_eq!(b.user.phone, "+84901234567");
}
