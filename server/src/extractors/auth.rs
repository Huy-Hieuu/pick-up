use std::future::Future;

use axum::{
    extract::FromRequestParts,
    http::request::Parts,
    response::IntoResponse,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

use crate::error::AppError;

const ISSUER: &str = "pickup-server";

/// JWT claims embedded in access and refresh tokens.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject — the user's UUID as a string.
    pub sub: String,
    /// Token type: "access" or "refresh".
    pub typ: String,
    /// Issuer.
    pub iss: String,
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
        uuid::Uuid::parse_str(&self.claims.sub)
            .expect("sub claim must be a valid UUID — tokens are issued by us")
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
        let secret = state.settings.jwt.secret.clone();
        async move {
            let auth_header = parts
                .headers
                .get("Authorization")
                .and_then(|v| v.to_str().ok())
                .ok_or_else(|| {
                    AuthRejection(AppError::Unauthorized("Missing Authorization header".into()))
                })?;

            let token = auth_header.strip_prefix("Bearer ").ok_or_else(|| {
                AuthRejection(AppError::Unauthorized(
                    "Invalid Authorization format".into(),
                ))
            })?;

            let mut validation = Validation::default();
            validation.set_issuer(&[ISSUER]);

            let claims = decode::<Claims>(
                token,
                &DecodingKey::from_secret(secret.as_bytes()),
                &validation,
            )
            .map_err(|e| {
                tracing::warn!("JWT decode failed: {e}");
                AuthRejection(AppError::Unauthorized("Invalid or expired token".into()))
            })?
            .claims;

            // Ensure this is an access token, not a refresh token.
            if claims.typ != "access" {
                return Err(AuthRejection(AppError::Unauthorized(
                    "Invalid token type".into(),
                )));
            }

            Ok(Self { claims })
        }
    }
}

// ── JWT helpers ─────────────────────────────────────────────────

/// Create a signed access token.
pub fn create_access_token(
    user_id: uuid::Uuid,
    secret: &str,
    ttl_minutes: i64,
) -> anyhow::Result<String> {
    let now = Utc::now();
    let claims = Claims {
        sub: user_id.to_string(),
        typ: "access".into(),
        iss: ISSUER.into(),
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
    secret: &str,
    ttl_days: i64,
) -> anyhow::Result<String> {
    let now = Utc::now();
    let claims = Claims {
        sub: user_id.to_string(),
        typ: "refresh".into(),
        iss: ISSUER.into(),
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
