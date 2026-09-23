//! Atomic OTP operations and durable, revocable login sessions.
use redis::aio::ConnectionManager;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    db,
    error::{AppError, AppResult},
    extractors::auth::{create_access_token, create_refresh_token, decode_token},
    models::user::{AuthResponse, UserRow},
};

// Bound Argon2 memory/CPU and keep it off Tokio's I/O workers.
static PASSWORD_WORK: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(4);

pub(super) async fn hash_password(password: String) -> AppResult<String> {
    let permit = PASSWORD_WORK
        .try_acquire()
        .map_err(|_| AppError::TooManyRequests("Authentication busy, please retry".into()))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        db::hash_password(&password)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Password hash error: {e}")))
    })
    .await
    .map_err(|e| AppError::Internal(e.into()))?
}

pub(super) async fn verify_password(password: String, hash: String) -> AppResult<bool> {
    let permit = PASSWORD_WORK
        .try_acquire()
        .map_err(|_| AppError::TooManyRequests("Authentication busy, please retry".into()))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        db::verify_password(&password, &hash)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Password verify error: {e}")))
    })
    .await
    .map_err(|e| AppError::Internal(e.into()))?
}

pub(super) async fn limit_attempts(
    redis: &ConnectionManager,
    key: &str,
    limit: i32,
    ttl: i32,
) -> AppResult<()> {
    let count: i32 = redis::Script::new(
        r#"
        local n = redis.call('INCR', KEYS[1])
        if n == 1 then redis.call('EXPIRE', KEYS[1], ARGV[1]) end
        return n
    "#,
    )
    .key(key)
    .arg(ttl)
    .invoke_async(&mut redis.clone())
    .await
    .map_err(|e| AppError::Internal(e.into()))?;
    if count > limit {
        return Err(AppError::TooManyRequests(
            "Too many attempts. Please try again later".into(),
        ));
    }
    Ok(())
}

pub(super) async fn send_otp(
    redis: &ConnectionManager,
    email: &str,
    purpose: &str,
    ttl: i64,
) -> AppResult<()> {
    use rand::Rng;
    let code = format!("{:06}", rand::thread_rng().gen_range(0..1_000_000));
    let job = crate::jobs::email::EmailJob {
        id: Uuid::new_v4(),
        to: email.to_owned(),
        subject: format!("Your PickUp {purpose} code"),
        body: format!(
            "Your PickUp {purpose} code is: {code}\n\nThis code expires in {ttl} seconds."
        ),
        retry_count: 0,
    };
    let payload = serde_json::to_string(&job).map_err(|e| AppError::Internal(e.into()))?;
    // OTP, its cooldown and the email become visible together. Concurrent sends
    // cannot overwrite a valid code or queue conflicting codes.
    let sent: i32 = redis::Script::new(
        r#"
        if redis.call('EXISTS', KEYS[1]) == 1 then return 0 end
        local length = redis.call('LLEN', KEYS[4]) + redis.call('HLEN', KEYS[5]) + redis.call('ZCARD', KEYS[6]) + redis.call('HLEN', KEYS[7])
        if length >= tonumber(ARGV[4]) then return -1 end
        redis.call('SET', KEYS[1], '1', 'EX', 60)
        redis.call('SET', KEYS[2], ARGV[1], 'EX', ARGV[2])
        redis.call('DEL', KEYS[3])
        redis.call('LPUSH', KEYS[4], ARGV[3])
        if redis.call('LLEN', KEYS[8]) == 0 then redis.call('LPUSH', KEYS[8], 'ready') end
        return 1
    "#,
    )
    .key(format!("otp:{purpose}:rate:{email}"))
    .key(format!("otp:{purpose}:{email}"))
    .key(format!("otp:{purpose}:attempts:{email}"))
    .key("email:queue")
    .key("email:processing")
    .key("email:retry")
    .key("email:retry-payloads")
    .key("email:wake")
    .arg(code)
    .arg(ttl)
    .arg(payload)
    .arg(10_000_i64)
    .invoke_async(&mut redis.clone())
    .await
    .map_err(|e| AppError::Internal(e.into()))?;
    if sent == 0 {
        return Err(AppError::TooManyRequests(
            "Please wait before requesting another OTP".into(),
        ));
    }
    if sent < 0 {
        return Err(AppError::TooManyRequests(
            "Email queue is full; retry later".into(),
        ));
    }
    Ok(())
}

pub(super) async fn send_registration_otp(
    pool: &PgPool,
    redis: &ConnectionManager,
    email: &str,
    ttl: i64,
) -> AppResult<()> {
    if db::email_is_verified(pool, email).await? {
        return Err(AppError::Conflict("This email is already verified".into()));
    }
    if db::find_user_by_email(pool, email).await?.is_none() {
        return Err(AppError::Unauthorized(
            "Invalid or expired verification request".into(),
        ));
    }
    send_otp(redis, email, "registration", ttl).await
}

pub(super) async fn consume_otp(
    redis: &ConnectionManager,
    email: &str,
    purpose: &str,
    code: &str,
    max_attempts: i16,
) -> AppResult<()> {
    // Never restore a consumed code or extend its expiry. The comparison,
    // attempt budget and single-use deletion happen in one Redis operation.
    let valid: i32 = redis::Script::new(
        r#"
        local stored = redis.call('GET', KEYS[1])
        local ttl = redis.call('PTTL', KEYS[1])
        if not stored or ttl <= 0 then return 0 end
        local n = redis.call('INCR', KEYS[2])
        redis.call('PEXPIRE', KEYS[2], ttl)
        local diff = 0
        if string.len(ARGV[1]) ~= string.len(stored) then diff = 1 else
            for i = 1, string.len(stored) do
                diff = bit.bor(diff, bit.bxor(string.byte(stored, i), string.byte(ARGV[1], i)))
            end
        end
        if n <= tonumber(ARGV[2]) and diff == 0 then
            redis.call('DEL', KEYS[1], KEYS[2])
            return 1
        end
        if n >= tonumber(ARGV[2]) then redis.call('DEL', KEYS[1], KEYS[2]) end
        return 0
    "#,
    )
    .key(format!("otp:{purpose}:{email}"))
    .key(format!("otp:{purpose}:attempts:{email}"))
    .arg(code)
    .arg(max_attempts)
    .invoke_async(&mut redis.clone())
    .await
    .map_err(|e| AppError::Internal(e.into()))?;
    if valid != 1 {
        return Err(AppError::Unauthorized("Invalid or expired OTP".into()));
    }
    Ok(())
}

pub(super) async fn issue_tokens(
    pool: &PgPool,
    user: UserRow,
    secret: &str,
    access_ttl: i64,
    refresh_ttl: i64,
) -> AppResult<AuthResponse> {
    let mut tx = pool.begin().await?;
    // Serialize issuance with password resets; stale password checks cannot
    // create a new session after a reset has revoked existing sessions.
    let current: UserRow = sqlx::query_as("SELECT * FROM users WHERE id = $1 FOR UPDATE")
        .bind(user.id)
        .fetch_one(&mut *tx)
        .await?;
    if current.password_hash != user.password_hash {
        return Err(AppError::Unauthorized(
            "Credentials changed, please sign in again".into(),
        ));
    }
    let sid = Uuid::new_v4();
    let access_token = create_access_token(user.id, sid, secret, access_ttl)?;
    let refresh_token = create_refresh_token(user.id, sid, secret, refresh_ttl)?;
    let refresh = decode_token(&refresh_token, secret, "refresh")?;
    sqlx::query("DELETE FROM auth_sessions WHERE user_id = $1 AND expires_at <= now()")
        .bind(user.id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO auth_sessions (id, user_id, refresh_jti, expires_at) VALUES ($1, $2, $3, to_timestamp($4))")
        .bind(sid).bind(user.id).bind(refresh.jti).bind(refresh.exp as f64).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(AuthResponse {
        access_token,
        refresh_token,
        user: current.into(),
    })
}

pub(super) async fn rotate_tokens(
    pool: &PgPool,
    token: &str,
    secret: &str,
    access_ttl: i64,
    refresh_ttl: i64,
) -> AppResult<AuthResponse> {
    let claims = decode_token(token, secret, "refresh")?;
    let user_id =
        Uuid::parse_str(&claims.sub).map_err(|_| AppError::Unauthorized("Invalid token".into()))?;
    let mut tx = pool.begin().await?;
    let user: UserRow = sqlx::query_as("SELECT * FROM users WHERE id = $1 FOR UPDATE")
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Invalid session".into()))?;
    let access_token = create_access_token(user_id, claims.sid, secret, access_ttl)?;
    let refresh_token = create_refresh_token(user_id, claims.sid, secret, refresh_ttl)?;
    let refresh = decode_token(&refresh_token, secret, "refresh")?;
    let changed = sqlx::query("UPDATE auth_sessions SET refresh_jti = $1, expires_at = to_timestamp($2) WHERE id = $3 AND user_id = $4 AND refresh_jti = $5 AND expires_at > now()")
        .bind(refresh.jti).bind(refresh.exp as f64).bind(claims.sid).bind(user_id).bind(claims.jti)
        .execute(&mut *tx).await?.rows_affected();
    if changed != 1 {
        return Err(AppError::Unauthorized(
            "Refresh token expired, revoked or already used".into(),
        ));
    }
    tx.commit().await?;
    Ok(AuthResponse {
        access_token,
        refresh_token,
        user: user.into(),
    })
}
