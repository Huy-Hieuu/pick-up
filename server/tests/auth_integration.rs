//! Run with DATABASE_URL and TEST_REDIS_URL pointing at disposable services:
//! cargo test --test auth_integration -- --include-ignored
use pickup_server::{
    db,
    error::AppError,
    extractors::auth::{authenticate_access_token, decode_token},
    models::otp::{
        EmailRequest, LoginRequest, RegisterRequest, SetNewPasswordRequest, VerifyOtpRequest,
        VerifyRegistrationRequest, VerifyResetOtpRequest,
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
    let redis = redis().await;
    let pending = AuthService::register(
        pool,
        &redis,
        RegisterRequest {
            email: format!("{id}@example.com"),
        },
        300,
    )
    .await
    .unwrap();
    let code: String = redis
        .clone()
        .get(format!("otp:registration:{}", pending.email))
        .await
        .unwrap();
    AuthService::verify_registration_otp(
        pool,
        &redis,
        VerifyRegistrationRequest {
            email: pending.email.clone(),
            code,
            password: "valid-password-123".into(),
            phone: format!("+849{:08}", id.as_u128() % 100_000_000),
            display_name: None,
            avatar_url: None,
        },
        3,
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
    AuthService::send_otp(&pool, &redis, &email, 300)
        .await
        .unwrap();
    let key = format!("otp:login:{email}");
    let code: String = redis.clone().get(&key).await.unwrap();
    let _: () = redis.clone().pexpire(&key, 20_000).await.unwrap();
    for _ in 0..3 {
        assert!(AuthService::verify_otp(
            &pool,
            &redis,
            VerifyOtpRequest {
                email: email.clone(),
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
        AuthService::send_otp(&pool, &redis, &email, 300),
        AuthService::send_otp(&pool, &redis, &email, 300)
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
    let redis = redis().await;
    let pending = AuthService::register(
        &pool,
        &redis,
        RegisterRequest {
            email: "local@example.com".into(),
        },
        300,
    )
    .await
    .unwrap();
    assert!(
        db::find_user_by_email(&pool, &pending.email)
            .await
            .unwrap()
            .is_none(),
        "registration reserves no account or phone before email proof"
    );
    let sessions: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM auth_sessions s JOIN users u ON u.id = s.user_id WHERE u.email = $1",
    )
    .bind(&pending.email)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(sessions, 0, "no session is issued before OTP verification");
    let code: String = redis
        .clone()
        .get(format!("otp:registration:{}", pending.email))
        .await
        .unwrap();
    let auth = AuthService::verify_registration_otp(
        &pool,
        &redis,
        VerifyRegistrationRequest {
            email: pending.email.clone(),
            code,
            password: "verified-owner-password".into(),
            phone: "0901234567".into(),
            display_name: None,
            avatar_url: None,
        },
        3,
        SECRET,
        15,
        30,
    )
    .await
    .unwrap();
    assert_eq!(auth.user.phone, "+84901234567");
    let verified: bool = sqlx::query_scalar("SELECT email_verified FROM users WHERE email = $1")
        .bind(&pending.email)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(verified);
    assert!(AuthService::verify_registration_otp(
        &pool,
        &redis,
        VerifyRegistrationRequest {
            email: pending.email.clone(),
            code: "000000".into(),
            password: "another-owner-password".into(),
            phone: "0901234567".into(),
            display_name: None,
            avatar_url: None,
        },
        3,
        SECRET,
        15,
        30
    )
    .await
    .is_err());

    let duplicate_email = AuthService::register(
        &pool,
        &redis,
        RegisterRequest {
            email: pending.email.clone(),
        },
        300,
    )
    .await
    .unwrap();
    assert_eq!(duplicate_email.message, pending.message);
    assert_eq!(duplicate_email.email, pending.email);
    let another_pending = AuthService::register(
        &pool,
        &redis,
        RegisterRequest {
            email: "different-address@example.com".into(),
        },
        300,
    )
    .await
    .unwrap();
    assert_eq!(another_pending.message, pending.message);
    assert!(
        db::find_user_by_email(&pool, "different-address@example.com")
            .await
            .unwrap()
            .is_none()
    );
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL and Redis"]
async fn email_owner_sets_password_when_verifying_registration(pool: PgPool) {
    let redis = redis().await;
    let email = format!("{}@example.com", Uuid::new_v4());
    // Older clients may still submit a password at registration. It is ignored
    // and never stored before control of the email address is proven.
    let attacker_request: RegisterRequest = serde_json::from_value(serde_json::json!({
        "phone": "0905555666",
        "email": email,
        "password": "attacker-chosen-password",
    }))
    .unwrap();
    let pending = AuthService::register(&pool, &redis, attacker_request, 300)
        .await
        .unwrap();
    assert!(db::find_user_by_email(&pool, &pending.email)
        .await
        .unwrap()
        .is_none());

    // A second registration cannot set credentials either. The email owner
    // chooses the password only while consuming the delivered OTP.
    AuthService::register(
        &pool,
        &redis,
        RegisterRequest {
            email: pending.email.clone(),
        },
        300,
    )
    .await
    .unwrap();
    let code: String = redis
        .clone()
        .get(format!("otp:registration:{}", pending.email))
        .await
        .unwrap();
    let verified = AuthService::verify_registration_otp(
        &pool,
        &redis,
        VerifyRegistrationRequest {
            email: pending.email.clone(),
            code,
            password: "email-owner-chosen-password".into(),
            phone: "0901111222".into(),
            display_name: Some("Email owner".into()),
            avatar_url: None,
        },
        3,
        SECRET,
        15,
        30,
    )
    .await
    .unwrap();
    assert_eq!(verified.user.phone, "+84901111222");
    assert_eq!(verified.user.display_name.as_deref(), Some("Email owner"));

    assert!(matches!(
        AuthService::login(
            &pool,
            &redis,
            LoginRequest {
                email: pending.email.clone(),
                password: "attacker-chosen-password".into(),
            },
            SECRET,
            15,
            30,
        )
        .await,
        Err(AppError::Unauthorized(_))
    ));
    assert!(AuthService::login(
        &pool,
        &redis,
        LoginRequest {
            email: pending.email,
            password: "email-owner-chosen-password".into(),
        },
        SECRET,
        15,
        30,
    )
    .await
    .is_ok());
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL and Redis"]
async fn new_account_has_no_session_until_email_is_verified(pool: PgPool) {
    let redis = redis().await;
    let email = format!("{}@example.com", Uuid::new_v4());
    let pending = AuthService::register(
        &pool,
        &redis,
        RegisterRequest {
            email: email.clone(),
        },
        300,
    )
    .await
    .unwrap();
    assert_eq!(pending.email, email);
    assert!(matches!(
        AuthService::login(
            &pool,
            &redis,
            LoginRequest {
                email: email.clone(),
                password: "verified-pass-123".into(),
            },
            SECRET,
            15,
            30
        )
        .await,
        Err(AppError::Unauthorized(_))
    ));
    AuthService::send_otp(&pool, &redis, &email, 300)
        .await
        .unwrap();
    let unverified_otp: Option<String> = redis
        .clone()
        .get(format!("otp:login:{email}"))
        .await
        .unwrap();
    assert!(
        unverified_otp.is_none(),
        "unverified addresses receive no login OTP"
    );
    let session_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM auth_sessions s JOIN users u ON u.id = s.user_id WHERE u.email = $1",
    )
    .bind(&email)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(session_count, 0);
    AuthService::resend_registration_otp(
        &pool,
        &redis,
        EmailRequest {
            email: email.clone(),
        },
        300,
    )
    .await
    .unwrap();
    assert!(matches!(
        AuthService::resend_registration_otp(
            &pool,
            &redis,
            EmailRequest {
                email: email.clone(),
            },
            300
        )
        .await,
        Err(AppError::TooManyRequests(_))
    ));
    let code: String = redis
        .clone()
        .get(format!("otp:registration:{email}"))
        .await
        .unwrap();
    let auth = AuthService::verify_registration_otp(
        &pool,
        &redis,
        VerifyRegistrationRequest {
            email: email.clone(),
            code: code.clone(),
            password: "verified-pass-123".into(),
            phone: "0907777666".into(),
            display_name: None,
            avatar_url: None,
        },
        3,
        SECRET,
        15,
        30,
    )
    .await
    .unwrap();
    assert!(authenticate_access_token(&pool, &auth.access_token, SECRET)
        .await
        .is_ok());
    assert!(AuthService::verify_registration_otp(
        &pool,
        &redis,
        VerifyRegistrationRequest {
            email: email.clone(),
            code,
            password: "another-valid-password".into(),
            phone: "0907777666".into(),
            display_name: None,
            avatar_url: None,
        },
        3,
        SECRET,
        15,
        30
    )
    .await
    .is_err());
    assert!(AuthService::login(
        &pool,
        &redis,
        LoginRequest {
            email,
            password: "verified-pass-123".into(),
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
async fn legacy_registration_reclaims_identity_and_revokes_old_sessions(pool: PgPool) {
    let redis = redis().await;
    let email = format!("{}@example.test", Uuid::new_v4());
    let old_hash = db::hash_password("old-unproved-password").unwrap();
    let old_user = db::create_user(
        &pool,
        "+84903334444",
        &email,
        Some(&old_hash),
        Some("Unproved"),
        None,
    )
    .await
    .unwrap();
    let old_sid = Uuid::new_v4();
    sqlx::query("INSERT INTO auth_sessions (id, user_id, refresh_jti, expires_at) VALUES ($1, $2, $3, now() + interval '1 day')")
        .bind(old_sid).bind(old_user.id).bind(Uuid::new_v4()).execute(&pool).await.unwrap();
    AuthService::resend_registration_otp(
        &pool,
        &redis,
        EmailRequest {
            email: email.clone(),
        },
        300,
    )
    .await
    .unwrap();
    let code: String = redis
        .clone()
        .get(format!("otp:registration:{email}"))
        .await
        .unwrap();
    let verified = AuthService::verify_registration_otp(
        &pool,
        &redis,
        VerifyRegistrationRequest {
            email: email.clone(),
            code,
            phone: "0902222333".into(),
            password: "owner-chosen-password".into(),
            display_name: Some("Verified owner".into()),
            avatar_url: None,
        },
        3,
        SECRET,
        15,
        30,
    )
    .await
    .unwrap();
    assert_eq!(verified.user.id, old_user.id);
    assert_eq!(verified.user.phone, "+84902222333");
    assert_eq!(
        verified.user.display_name.as_deref(),
        Some("Verified owner")
    );
    let stale_session: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM auth_sessions WHERE id = $1)")
            .bind(old_sid)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!stale_session);
    let user = db::find_user_by_id(&pool, old_user.id)
        .await
        .unwrap()
        .unwrap();
    assert!(db::verify_password(
        "owner-chosen-password",
        user.password_hash.as_deref().unwrap()
    )
    .unwrap());
    assert!(!db::verify_password(
        "old-unproved-password",
        user.password_hash.as_deref().unwrap()
    )
    .unwrap());
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL and Redis"]
async fn registration_code_creates_at_most_one_account_under_concurrency(pool: PgPool) {
    let redis = redis().await;
    let email = format!("{}@example.test", Uuid::new_v4());
    AuthService::register(
        &pool,
        &redis,
        RegisterRequest {
            email: email.clone(),
        },
        300,
    )
    .await
    .unwrap();
    let code: String = redis
        .clone()
        .get(format!("otp:registration:{email}"))
        .await
        .unwrap();
    let request = || VerifyRegistrationRequest {
        email: email.clone(),
        code: code.clone(),
        phone: "0908888777".into(),
        password: "owner-chosen-password".into(),
        display_name: None,
        avatar_url: None,
    };
    let (left, right) = tokio::join!(
        AuthService::verify_registration_otp(&pool, &redis, request(), 3, SECRET, 15, 30),
        AuthService::verify_registration_otp(&pool, &redis, request(), 3, SECRET, 15, 30)
    );
    assert_ne!(left.is_ok(), right.is_ok());
    let accounts: i64 = sqlx::query_scalar("SELECT count(*) FROM users WHERE email = $1")
        .bind(&email)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(accounts, 1);
    let sessions: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM auth_sessions s JOIN users u ON u.id = s.user_id WHERE u.email = $1",
    )
    .bind(&email)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(sessions, 1);
}
