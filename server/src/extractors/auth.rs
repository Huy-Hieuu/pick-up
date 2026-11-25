use std::future::Future;

use axum::{extract::FromRequestParts, http::request::Parts, response::IntoResponse};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

use crate::error::AppError;

const ISSUER: &str = "pickup-server";

pub const AUDIENCE: &str = "pickup-app";

/// JWT claims embedded in access and refresh tokens.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Persisted login session and unique token identity. Legacy tokens fail closed.
    pub sid: uuid::Uuid,
    pub jti: uuid::Uuid,
    /// Subject — the user's UUID as a string.
    pub sub: String,
    /// Token type: "access" or "refresh".
    pub typ: String,
    /// Issuer.
    pub iss: String,
    /// Audience.
    pub aud: String,
    /// Issued at (Unix timestamp).
    pub iat: i64,
    /// Expiration (Unix timestamp).
    pub exp: i64,
}

/// The authenticated user, extracted from the `Authorization: Bearer <token>` header.
#[derive(Debug, Clone)]
pub struct AuthUser {
    claims: Claims,
}

impl AuthUser {
    pub fn user_id(&self) -> uuid::Uuid {
        uuid::Uuid::parse_str(&self.claims.sub).unwrap_or_else(|_| {
            tracing::error!(sub = %self.claims.sub, "Invalid UUID in JWT sub claim");
            uuid::Uuid::nil()
        })
    }

    pub fn claims(&self) -> &Claims {
        &self.claims
    }
}

/// Custom rejection used when the auth extractor fails.
#[derive(Debug)]
pub struct AuthRejection(AppError);

impl IntoResponse for AuthRejection {
    fn into_response(self) -> axum::response::Response {
        self.0.into_response()
    }
}

impl FromRequestParts<crate::state::AppState> for AuthUser {
    type Rejection = AuthRejection;

    fn from_request_parts(
        parts: &mut Parts,
        state: &crate::state::AppState,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        // Extract from extensions and headers BEFORE the async block.
        let claims_from_ext = parts.extensions.get::<Claims>().cloned();
        let auth_header = parts
            .headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .map(|h| h.to_string());
        let secret = state.settings.jwt.secret.clone();
        let pool = state.pool.clone();

        async move {
            // Fast path: read Claims injected by require_auth middleware.
            if let Some(claims) = claims_from_ext {
                return Ok(Self { claims });
            }

            // Fallback: decode JWT from header (for routes without auth middleware).
            let auth_header = auth_header.ok_or_else(|| {
                AuthRejection(AppError::Unauthorized(
                    "Missing Authorization header".into(),
                ))
            })?;

            let token = auth_header.strip_prefix("Bearer ").ok_or_else(|| {
                AuthRejection(AppError::Unauthorized(
                    "Invalid Authorization format".into(),
                ))
            })?;

            let claims = authenticate_access_token(&pool, token, &secret)
                .await
                .map_err(AuthRejection)?;

            Ok(Self { claims })
        }
    }
}

// ── JWT helpers ─────────────────────────────────────────────────

/// Create a signed access token.
pub fn create_access_token(
    user_id: uuid::Uuid,
    session_id: uuid::Uuid,
    secret: &str,
    ttl_minutes: i64,
) -> anyhow::Result<String> {
    let now = Utc::now();
    anyhow::ensure!(
        (1..=60).contains(&ttl_minutes),
        "Access TTL must be 1–60 minutes"
    );
    let claims = Claims {
        sid: session_id,
        jti: uuid::Uuid::new_v4(),
        sub: user_id.to_string(),
        typ: "access".into(),
        iss: ISSUER.into(),
        aud: AUDIENCE.into(),
        iat: now.timestamp(),
        exp: (now + Duration::minutes(ttl_minutes)).timestamp(),
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(Into::into)
}

/// Create a signed refresh token.
pub fn create_refresh_token(
    user_id: uuid::Uuid,
    session_id: uuid::Uuid,
    secret: &str,
    ttl_days: i64,
) -> anyhow::Result<String> {
    let now = Utc::now();
    anyhow::ensure!(
        (1..=90).contains(&ttl_days),
        "Refresh TTL must be 1–90 days"
    );
    let claims = Claims {
        sid: session_id,
        jti: uuid::Uuid::new_v4(),
        sub: user_id.to_string(),
        typ: "refresh".into(),
        iss: ISSUER.into(),
        aud: AUDIENCE.into(),
        iat: now.timestamp(),
        exp: (now + Duration::days(ttl_days)).timestamp(),
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(Into::into)
}

pub fn decode_token(token: &str, secret: &str, token_type: &str) -> Result<Claims, AppError> {
    let mut validation = Validation::default();
    validation.set_issuer(&[ISSUER]);
    validation.set_audience(&[AUDIENCE]);
    validation.leeway = 0;
    let claims = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|_| AppError::Unauthorized("Invalid or expired token".into()))?
    .claims;
    if claims.typ != token_type
        || uuid::Uuid::parse_str(&claims.sub).is_err()
        || claims.sid.is_nil()
        || claims.jti.is_nil()
        || claims.exp <= Utc::now().timestamp()
    {
        return Err(AppError::Unauthorized("Invalid token".into()));
    }
    Ok(claims)
}

pub async fn authenticate_access_token(
    pool: &sqlx::PgPool,
    token: &str,
    secret: &str,
) -> Result<Claims, AppError> {
    let claims = decode_token(token, secret, "access")?;
    validate_session(pool, &claims).await?;
    Ok(claims)
}

pub async fn validate_session(pool: &sqlx::PgPool, claims: &Claims) -> Result<(), AppError> {
    let user_id = uuid::Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Unauthorized("Invalid token".into()))?;
    let active: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM auth_sessions WHERE id = $1 AND user_id = $2 AND expires_at > now())"
    ).bind(claims.sid).bind(user_id).fetch_one(pool).await?;
    if !active || claims.exp <= Utc::now().timestamp() {
        return Err(AppError::Unauthorized("Session expired or revoked".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::decode;
    use uuid::Uuid;

    const TEST_SECRET: &str = "test-secret-key-for-testing-only";

    fn decode_token(token: &str, secret: &str) -> jsonwebtoken::TokenData<Claims> {
        let mut validation = Validation::default();
        validation.set_issuer(&[ISSUER]);
        validation.set_audience(&[AUDIENCE]);
        decode::<Claims>(
            token,
            &DecodingKey::from_secret(secret.as_bytes()),
            &validation,
        )
        .expect("Token should decode successfully")
    }

    // ── Access token ──────────────────────────────────────────────

    #[test]
    fn access_token_contains_correct_claims() {
        let user_id = Uuid::new_v4();
        let token = create_access_token(user_id, Uuid::new_v4(), TEST_SECRET, 15).unwrap();
        let data = decode_token(&token, TEST_SECRET);

        assert_eq!(data.claims.sub, user_id.to_string());
        assert_eq!(data.claims.typ, "access");
        assert_eq!(data.claims.iss, ISSUER);
    }

    #[test]
    fn access_token_has_valid_expiry() {
        let user_id = Uuid::new_v4();
        let before = Utc::now().timestamp();
        let token = create_access_token(user_id, Uuid::new_v4(), TEST_SECRET, 15).unwrap();
        let after = Utc::now().timestamp();
        let data = decode_token(&token, TEST_SECRET);

        let expected_exp_min = before + (15 * 60);
        let expected_exp_max = after + (15 * 60);
        assert!(
            data.claims.exp >= expected_exp_min && data.claims.exp <= expected_exp_max,
            "Token expiry should be ~15 minutes from creation"
        );
    }

    #[test]
    fn access_token_rejects_wrong_secret() {
        let user_id = Uuid::new_v4();
        let token = create_access_token(user_id, Uuid::new_v4(), TEST_SECRET, 15).unwrap();

        let result = decode::<Claims>(
            &token,
            &DecodingKey::from_secret("wrong-secret".as_bytes()),
            &Validation::default(),
        );
        assert!(result.is_err(), "Token should fail with wrong secret");
    }

    // ── Refresh token ──────────────────────────────────────────────

    #[test]
    fn refresh_token_contains_correct_type() {
        let user_id = Uuid::new_v4();
        let token = create_refresh_token(user_id, Uuid::new_v4(), TEST_SECRET, 30).unwrap();
        let data = decode_token(&token, TEST_SECRET);

        assert_eq!(data.claims.typ, "refresh");
        assert_eq!(data.claims.sub, user_id.to_string());
    }

    #[test]
    fn refresh_token_expiry_is_in_days() {
        let user_id = Uuid::new_v4();
        let before = Utc::now().timestamp();
        let token = create_refresh_token(user_id, Uuid::new_v4(), TEST_SECRET, 30).unwrap();
        let after = Utc::now().timestamp();
        let data = decode_token(&token, TEST_SECRET);

        // 30 days = 30 * 86400 seconds
        let expected_exp_min = before + (30 * 86400);
        let expected_exp_max = after + (30 * 86400);
        assert!(
            data.claims.exp >= expected_exp_min && data.claims.exp <= expected_exp_max,
            "Refresh token expiry should be ~30 days from creation"
        );
    }

    // ── Token differentiation ──────────────────────────────────────

    #[test]
    fn access_and_refresh_tokens_have_different_types() {
        let user_id = Uuid::new_v4();
        let access = create_access_token(user_id, Uuid::new_v4(), TEST_SECRET, 15).unwrap();
        let refresh = create_refresh_token(user_id, Uuid::new_v4(), TEST_SECRET, 30).unwrap();

        let access_data = decode_token(&access, TEST_SECRET);
        let refresh_data = decode_token(&refresh, TEST_SECRET);

        assert_eq!(access_data.claims.typ, "access");
        assert_eq!(refresh_data.claims.typ, "refresh");
    }

    // ── AuthUser extractor ─────────────────────────────────────────

    #[test]
    fn auth_user_extracts_user_id() {
        let user_id = Uuid::new_v4();
        let claims = Claims {
            sid: Uuid::new_v4(),
            jti: Uuid::new_v4(),
            sub: user_id.to_string(),
            typ: "access".into(),
            iss: ISSUER.into(),
            aud: AUDIENCE.into(),
            iat: Utc::now().timestamp(),
            exp: (Utc::now() + Duration::hours(1)).timestamp(),
        };
        let auth_user = AuthUser { claims };
        assert_eq!(auth_user.user_id(), user_id);
    }

    #[test]
    fn auth_user_claims_accessor() {
        let claims = Claims {
            sid: Uuid::new_v4(),
            jti: Uuid::new_v4(),
            sub: Uuid::new_v4().to_string(),
            typ: "access".into(),
            iss: ISSUER.into(),
            aud: AUDIENCE.into(),
            iat: 1000,
            exp: 2000,
        };
        let auth_user = AuthUser { claims };
        assert_eq!(auth_user.claims().iat, 1000);
        assert_eq!(auth_user.claims().exp, 2000);
    }
}
