use constant_time_eq::constant_time_eq;
use rand::Rng;
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use sqlx::PgPool;
use uuid::Uuid;

use crate::db;
use crate::error::{AppError, AppResult};
use crate::extractors::auth::{create_access_token, create_refresh_token};
use crate::models::otp::{
    LoginRequest, RegisterRequest, SetNewPasswordRequest, SetNewPasswordResponse,
    VerifyOtpRequest, VerifyResetOtpRequest, VerifyResetOtpResponse,
};
use crate::models::user::{AuthResponse, PatchValue, UpdateProfileRequest, UserProfile};

pub struct AuthService;

impl AuthService {
    pub async fn send_otp(
        redis: &ConnectionManager,
        email: &str,
        ttl_seconds: i64,
    ) -> AppResult<()> {
        let code: String = {
            let mut rng = rand::thread_rng();
            format!("{:06}", rng.gen_range(100_000..1_000_000))
        };

        let mut conn = redis.clone();
        let email_key = format!("otp:{email}");
        let attempts_key = format!("otp:attempts:{email}");

        let _: () = conn.set_ex(&email_key, &code, ttl_seconds as u64)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Redis error: {e}")))?;
        let _: () = conn.set_ex(&attempts_key, 0_i32, ttl_seconds as u64)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Redis error: {e}")))?;

        let subject = "Your PickUp verification code";
        let body = format!("Your PickUp verification code is: {}\n\nThis code expires in 5 minutes.", code);
        crate::jobs::email::enqueue_email(redis, email, subject, &body).await?;

        tracing::info!(email, "OTP sent (queued)");
        Ok(())
    }

    pub async fn verify_otp(
        pool: &PgPool,
        redis: &ConnectionManager,
        req: VerifyOtpRequest,
        max_attempts: i16,
        otp_ttl_seconds: i64,
        jwt_secret: &str,
        access_ttl_minutes: i64,
        refresh_ttl_days: i64,
    ) -> AppResult<AuthResponse> {
        let mut conn = redis.clone();
        let otp_key = format!("otp:{}", req.email);
        let attempts_key = format!("otp:attempts:{}", req.email);

        // Atomically fetch and delete the OTP (prevents concurrent use).
        let stored: Option<String> = redis::cmd("GETDEL")
            .arg(&otp_key)
            .query_async(&mut conn)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Redis error: {e}")))?;

        let stored = match stored {
            Some(code) => code,
            None => {
                // No OTP found — it either expired or was never issued.
                // Still increment attempts to prevent brute-force probing.
                let _: i32 = conn.incr(&attempts_key, 1i32)
                    .await
                    .map_err(|e| AppError::Internal(anyhow::anyhow!("Redis error: {e}")))?;
                return Err(AppError::Unauthorized("Invalid or expired OTP".into()));
            }
        };

        // Check attempts AFTER confirming OTP exists.
        let new_count: i32 = conn.incr(&attempts_key, 1i32)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Redis error: {e}")))?;

        if new_count > max_attempts as i32 {
            // Re-store the OTP so the user can retry (they haven't used it yet).
            // Use original TTL instead of hardcoded 60s to avoid shortening.
            let restore_ttl = std::cmp::max(otp_ttl_seconds, 30) as u64;
            let _: () = conn.set_ex(&otp_key, &stored, restore_ttl)
                .await
                .map_err(|e| AppError::Internal(anyhow::anyhow!("Redis error: {e}")))?;
            return Err(AppError::BadRequest("Too many attempts".into()));
        }

        // Constant-time comparison to prevent timing attacks.
        if !constant_time_eq(stored.as_bytes(), req.code.as_bytes()) {
            // OTP was consumed via GETDEL but didn't match — restore it so the
            // user can retry with remaining attempts. Preserve original TTL.
            let restore_ttl = std::cmp::max(otp_ttl_seconds, 30) as u64;
            let _: () = conn.set_ex(&otp_key, &stored, restore_ttl)
                .await
                .map_err(|e| AppError::Internal(anyhow::anyhow!("Redis error: {e}")))?;
            tracing::warn!(email = %req.email, attempts = %new_count, "Invalid OTP");
            return Err(AppError::Unauthorized("Invalid or expired OTP".into()));
        }

        // OTP matched — clean up attempts key.
        let _: () = conn.del(&attempts_key)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Redis error: {e}")))?;

        let user = match db::find_user_by_email(pool, &req.email).await.map_err(AppError::Database)? {
            Some(u) => u,
            None => {
                db::create_user(pool, &req.phone, &req.email, None, None, None)
                    .await
                    .map_err(|e: sqlx::Error| {
                        // Handle UNIQUE constraint violation on email (concurrent registration).
                        if let Some(db_err) = e.as_database_error() {
                            if db_err.code().as_deref() == Some("23505") {
                                return AppError::Conflict("Email already registered".into());
                            }
                        }
                        AppError::Database(e)
                    })?
            }
        };

        let access_token = create_access_token(user.id, jwt_secret, access_ttl_minutes)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("JWT encode error: {e}")))?;
        let refresh_token = create_refresh_token(user.id, jwt_secret, refresh_ttl_days)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("JWT encode error: {e}")))?;

        Ok(AuthResponse {
            access_token,
            refresh_token,
            user: UserProfile::from(user),
        })
    }

    pub async fn refresh_token(
        pool: &PgPool,
        refresh_token: &str,
        jwt_secret: &str,
        access_ttl_minutes: i64,
        refresh_ttl_days: i64,
    ) -> AppResult<AuthResponse> {
        use jsonwebtoken::{decode, DecodingKey, Validation};
        use crate::extractors::auth::Claims;

        let mut validation = Validation::default();
        validation.set_issuer(&["pickup-server"]);

        let claims = decode::<Claims>(
            refresh_token,
            &DecodingKey::from_secret(jwt_secret.as_bytes()),
            &validation,
        )
        .map_err(|e| {
            tracing::warn!("Refresh token decode failed: {e}");
            AppError::Unauthorized("Invalid or expired token".into())
        })?
        .claims;

        if claims.typ != "refresh" {
            return Err(AppError::Unauthorized("Invalid token type".into()));
        }

        let user_id = Uuid::parse_str(&claims.sub)
            .map_err(|_| AppError::Unauthorized("Invalid token".into()))?;

        let user = db::find_user_by_id(pool, user_id)
            .await
            .map_err(AppError::Database)?
            .ok_or_else(|| AppError::NotFound("User not found".into()))?;

        let new_access_token = create_access_token(user.id, jwt_secret, access_ttl_minutes)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("JWT encode error: {e}")))?;
        let new_refresh_token = create_refresh_token(user.id, jwt_secret, refresh_ttl_days)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("JWT encode error: {e}")))?;

        Ok(AuthResponse {
            access_token: new_access_token,
            refresh_token: new_refresh_token,
            user: UserProfile::from(user),
        })
    }

    pub async fn get_profile(pool: &PgPool, user_id: Uuid) -> AppResult<UserProfile> {
        db::find_user_by_id(pool, user_id)
            .await
            .map_err(AppError::Database)?
            .map(UserProfile::from)
            .ok_or_else(|| AppError::NotFound("User not found".into()))
    }

    pub async fn update_profile(
        pool: &PgPool,
        user_id: Uuid,
        req: UpdateProfileRequest,
    ) -> AppResult<UserProfile> {
        // Trim display_name; convert empty string to Null.
        let display_name = match &req.display_name {
            PatchValue::Value(s) => {
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    PatchValue::Null
                } else if trimmed.len() > 50 {
                    return Err(AppError::BadRequest(
                        "Display name must be 1–50 characters".into(),
                    ));
                } else {
                    PatchValue::Value(trimmed.to_string())
                }
            }
            other => other.clone(),
        };

        let row = db::update_user_profile_patch(pool, user_id, &display_name, &req.avatar_url)
            .await
            .map_err(AppError::Database)?;
        Ok(row.into())
    }

    pub async fn register(
        pool: &PgPool,
        req: RegisterRequest,
        jwt_secret: &str,
        access_ttl_minutes: i64,
        refresh_ttl_days: i64,
    ) -> AppResult<AuthResponse> {
        if db::find_user_by_email(pool, &req.email).await.map_err(AppError::Database)?.is_some() {
            return Err(AppError::Conflict("Email already registered".into()));
        }

        let password_hash = db::hash_password(&req.password)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Password hash error: {e}")))?;

        let phone = format!("+84{}", req.phone.trim_start_matches("0"));
        let user = db::create_user(
            pool,
            &phone,
            &req.email,
            Some(&password_hash),
            req.display_name.as_deref(),
            req.avatar_url.as_deref(),
        )
        .await
        .map_err(|e: sqlx::Error| {
            // Handle UNIQUE constraint violation (concurrent registration).
            if let Some(db_err) = e.as_database_error() {
                if db_err.code().as_deref() == Some("23505") {
                    return AppError::Conflict("Email already registered".into());
                }
            }
            AppError::Database(e)
        })?;

        let access_token = create_access_token(user.id, jwt_secret, access_ttl_minutes)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("JWT encode error: {e}")))?;
        let refresh_token = create_refresh_token(user.id, jwt_secret, refresh_ttl_days)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("JWT encode error: {e}")))?;

        Ok(AuthResponse {
            access_token,
            refresh_token,
            user: UserProfile::from(user),
        })
    }

    pub async fn login(
        pool: &PgPool,
        req: LoginRequest,
        jwt_secret: &str,
        access_ttl_minutes: i64,
        refresh_ttl_days: i64,
    ) -> AppResult<AuthResponse> {
        let user = db::find_user_by_email_and_password(pool, &req.email)
            .await
            .map_err(AppError::Database)?
            .ok_or_else(|| AppError::Unauthorized("Invalid email or password".into()))?;

        // Explicitly check for missing password hash instead of falling back to empty string.
        let hash = user.password_hash.as_ref()
            .ok_or_else(|| AppError::Unauthorized("Invalid email or password".into()))?;

        let valid = db::verify_password(&req.password, hash)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Password verify error: {e}")))?;

        if !valid {
            return Err(AppError::Unauthorized("Invalid email or password".into()));
        }

        let access_token = create_access_token(user.id, jwt_secret, access_ttl_minutes)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("JWT encode error: {e}")))?;
        let refresh_token = create_refresh_token(user.id, jwt_secret, refresh_ttl_days)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("JWT encode error: {e}")))?;

        Ok(AuthResponse {
            access_token,
            refresh_token,
            user: UserProfile::from(user),
        })
    }

    pub async fn forgot_password(
        pool: &PgPool,
        redis: &ConnectionManager,
        email: &str,
        ttl_seconds: i64,
    ) -> AppResult<()> {
        // Check if user exists before sending OTP to avoid wasting email quota.
        let user_exists = db::find_user_by_email(pool, email)
            .await
            .map_err(AppError::Database)?
            .is_some();

        if !user_exists {
            // Return the same response to avoid user enumeration.
            tracing::warn!(email, "Forgot-password requested for unregistered email");
            return Ok(());
        }

        Self::send_otp(redis, email, ttl_seconds).await
    }

    pub async fn verify_reset_otp(
        pool: &PgPool,
        redis: &ConnectionManager,
        req: VerifyResetOtpRequest,
        max_attempts: i16,
        reset_token_ttl_seconds: i64,
    ) -> AppResult<VerifyResetOtpResponse> {
        let mut conn = redis.clone();
        let otp_key = format!("otp:{}", req.email);
        let attempts_key = format!("otp:attempts:{}", req.email);

        // Atomically fetch and delete the OTP.
        let stored: Option<String> = redis::cmd("GETDEL")
            .arg(&otp_key)
            .query_async(&mut conn)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Redis error: {e}")))?;

        let stored = match stored {
            Some(code) => code,
            None => {
                let _: i32 = conn.incr(&attempts_key, 1i32)
                    .await
                    .map_err(|e| AppError::Internal(anyhow::anyhow!("Redis error: {e}")))?;
                return Err(AppError::Unauthorized("Invalid or expired OTP".into()));
            }
        };

        let new_count: i32 = conn.incr(&attempts_key, 1i32)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Redis error: {e}")))?;

        if new_count > max_attempts as i32 {
            let _: () = conn.set_ex(&otp_key, &stored, 60)
                .await
                .map_err(|e| AppError::Internal(anyhow::anyhow!("Redis error: {e}")))?;
            return Err(AppError::BadRequest("Too many attempts".into()));
        }

        if !constant_time_eq(stored.as_bytes(), req.code.as_bytes()) {
            let _: () = conn.set_ex(&otp_key, &stored, 60)
                .await
                .map_err(|e| AppError::Internal(anyhow::anyhow!("Redis error: {e}")))?;
            tracing::warn!(email = %req.email, attempts = %new_count, "Invalid OTP for password reset");
            return Err(AppError::Unauthorized("Invalid or expired OTP".into()));
        }

        let _: () = conn.del(&attempts_key)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Redis error: {e}")))?;

        let user = db::find_user_by_email(pool, &req.email)
            .await
            .map_err(AppError::Database)?
            .ok_or_else(|| AppError::NotFound("User not found".into()))?;

        let reset_token: String = {
            let mut rng = rand::thread_rng();
            hex::encode(rng.gen::<[u8; 32]>())
        };

        let _: () = conn.set_ex(format!("reset_token:{}", reset_token), user.id.to_string(), reset_token_ttl_seconds as u64)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Redis error: {e}")))?;

        tracing::info!(email = %req.email, "Reset OTP verified, token issued");

        Ok(VerifyResetOtpResponse { reset_token })
    }

    pub async fn set_new_password(
        pool: &PgPool,
        redis: &ConnectionManager,
        req: SetNewPasswordRequest,
    ) -> AppResult<SetNewPasswordResponse> {
        // Hash the password BEFORE consuming the token.
        let password_hash = db::hash_password(&req.new_password)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Password hash error: {e}")))?;

        let mut conn = redis.clone();
        let token_key = format!("reset_token:{}", req.reset_token);

        // Atomically consume the reset token with GETDEL.
        let user_id_str: Option<String> = redis::cmd("GETDEL")
            .arg(&token_key)
            .query_async(&mut conn)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Redis error: {e}")))?;

        let user_id_str = user_id_str
            .ok_or_else(|| AppError::Unauthorized("Invalid or expired reset token".into()))?;

        let user_id = Uuid::parse_str(&user_id_str)
            .map_err(|_| AppError::Unauthorized("Invalid reset token".into()))?;

        db::update_password_hash(pool, user_id, &password_hash)
            .await
            .map_err(AppError::Database)?;

        tracing::info!(user_id = %user_id, "Password reset completed");

        Ok(SetNewPasswordResponse {
            message: "Password reset successfully".to_string(),
        })
    }
}
