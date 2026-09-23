use rand::Rng;
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use sqlx::PgPool;
use uuid::Uuid;

use crate::db;
use crate::error::{AppError, AppResult};
use crate::models::otp::{
    LoginRequest, RegisterRequest, SetNewPasswordRequest, SetNewPasswordResponse, VerifyOtpRequest,
    VerifyResetOtpRequest, VerifyResetOtpResponse,
};
use crate::models::user::{AuthResponse, PatchValue, UpdateProfileRequest, UserProfile};

pub struct AuthService;

impl AuthService {
    pub async fn send_otp(
        redis: &ConnectionManager,
        email: &str,
        ttl_seconds: i64,
    ) -> AppResult<()> {
        super::auth_security::send_otp(redis, email, "login", ttl_seconds).await
    }

    pub async fn verify_otp(
        pool: &PgPool,
        redis: &ConnectionManager,
        req: VerifyOtpRequest,
        max_attempts: i16,
        _otp_ttl_seconds: i64,
        jwt_secret: &str,
        access_ttl_minutes: i64,
        refresh_ttl_days: i64,
    ) -> AppResult<AuthResponse> {
        let user = db::find_user_by_email(pool, &req.email)
            .await?
            .ok_or_else(|| AppError::Unauthorized("Invalid or expired OTP".into()))?;
        super::auth_security::consume_otp(redis, &req.email, "login", &req.code, max_attempts)
            .await?;
        super::auth_security::issue_tokens(
            pool,
            user,
            jwt_secret,
            access_ttl_minutes,
            refresh_ttl_days,
        )
        .await
    }

    pub async fn refresh_token(
        pool: &PgPool,
        _redis: &ConnectionManager,
        refresh_token: &str,
        jwt_secret: &str,
        access_ttl_minutes: i64,
        refresh_ttl_days: i64,
    ) -> AppResult<AuthResponse> {
        super::auth_security::rotate_tokens(
            pool,
            refresh_token,
            jwt_secret,
            access_ttl_minutes,
            refresh_ttl_days,
        )
        .await
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
        if db::find_user_by_email(pool, &req.email).await?.is_some() {
            return Err(AppError::Conflict("Email already registered".into()));
        }
        let password_hash = super::auth_security::hash_password(req.password).await?;
        let phone = if let Some(local) = req.phone.strip_prefix('0') {
            format!("+84{local}")
        } else {
            req.phone.clone()
        };
        let user = db::create_user(
            pool,
            &phone,
            &req.email,
            Some(&password_hash),
            req.display_name.as_deref(),
            req.avatar_url.as_deref(),
        )
        .await?;
        super::auth_security::issue_tokens(
            pool,
            user,
            jwt_secret,
            access_ttl_minutes,
            refresh_ttl_days,
        )
        .await
    }

    pub async fn login(
        pool: &PgPool,
        redis: &ConnectionManager,
        req: LoginRequest,
        jwt_secret: &str,
        access_ttl_minutes: i64,
        refresh_ttl_days: i64,
    ) -> AppResult<AuthResponse> {
        super::auth_security::limit_attempts(
            redis,
            &format!("login:attempts:{}", req.email),
            5,
            300,
        )
        .await?;
        let user = db::find_user_by_email_and_password(pool, &req.email).await?;
        let Some(user) = user else {
            // Equal-cost password work for unknown accounts; no cheap email oracle.
            super::auth_security::hash_password(req.password).await?;
            return Err(AppError::Unauthorized("Invalid email or password".into()));
        };
        let hash = user
            .password_hash
            .clone()
            .ok_or_else(|| AppError::Unauthorized("Invalid email or password".into()))?;
        if !super::auth_security::verify_password(req.password, hash).await? {
            return Err(AppError::Unauthorized("Invalid email or password".into()));
        }
        super::auth_security::issue_tokens(
            pool,
            user,
            jwt_secret,
            access_ttl_minutes,
            refresh_ttl_days,
        )
        .await
    }

    pub async fn forgot_password(
        pool: &PgPool,
        redis: &ConnectionManager,
        email: &str,
        ttl_seconds: i64,
    ) -> AppResult<()> {
        // Apply the same response and cooldown to known and unknown addresses.
        super::auth_security::limit_attempts(redis, &format!("reset:request:{email}"), 1, 60)
            .await?;
        if db::find_user_by_email(pool, email).await?.is_some() {
            super::auth_security::send_otp(redis, email, "reset", ttl_seconds).await?;
        }
        Ok(())
    }

    pub async fn verify_reset_otp(
        pool: &PgPool,
        redis: &ConnectionManager,
        req: VerifyResetOtpRequest,
        max_attempts: i16,
        reset_token_ttl_seconds: i64,
    ) -> AppResult<VerifyResetOtpResponse> {
        super::auth_security::consume_otp(redis, &req.email, "reset", &req.code, max_attempts)
            .await?;
        let user = db::find_user_by_email(pool, &req.email)
            .await?
            .ok_or_else(|| AppError::Unauthorized("Invalid or expired OTP".into()))?;
        let reset_token = hex::encode(rand::thread_rng().gen::<[u8; 32]>());
        // Bind reset grants to the current password hash: an older grant must not
        // overwrite a password changed by another reset.
        let grant = serde_json::to_string(&(user.id, user.password_hash))
            .map_err(|e| AppError::Internal(e.into()))?;
        let _: () = redis
            .clone()
            .set_ex(
                format!("reset_token:{reset_token}"),
                grant,
                reset_token_ttl_seconds as u64,
            )
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
        Ok(VerifyResetOtpResponse { reset_token })
    }

    pub async fn set_new_password(
        pool: &PgPool,
        redis: &ConnectionManager,
        req: SetNewPasswordRequest,
    ) -> AppResult<SetNewPasswordResponse> {
        let grant: Option<String> = redis::cmd("GETDEL")
            .arg(format!("reset_token:{}", req.reset_token))
            .query_async(&mut redis.clone())
            .await
            .map_err(|e| AppError::Internal(e.into()))?;
        let grant =
            grant.ok_or_else(|| AppError::Unauthorized("Invalid or expired reset token".into()))?;
        let (user_id, previous_hash): (Uuid, Option<String>) = serde_json::from_str(&grant)
            .map_err(|_| AppError::Unauthorized("Invalid reset token".into()))?;
        let password_hash = super::auth_security::hash_password(req.new_password).await?;
        let mut tx = pool.begin().await?;
        let changed = sqlx::query("UPDATE users SET password_hash = $2 WHERE id = $1 AND password_hash IS NOT DISTINCT FROM $3")
            .bind(user_id).bind(password_hash).bind(previous_hash).execute(&mut *tx).await?.rows_affected();
        if changed != 1 {
            return Err(AppError::Unauthorized(
                "Reset token is no longer valid".into(),
            ));
        }
        // Password change and revocation commit together; no Redis/DB split-brain.
        sqlx::query("DELETE FROM auth_sessions WHERE user_id = $1")
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(SetNewPasswordResponse {
            message: "Password reset successfully".into(),
        })
    }
}
