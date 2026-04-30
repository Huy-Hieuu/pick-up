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

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::decode;
    use uuid::Uuid;

    const TEST_SECRET: &str = "test-secret-key-for-testing-only";

    fn decode_token(token: &str, secret: &str) -> jsonwebtoken::TokenData<Claims> {
        let mut validation = Validation::default();
        validation.set_issuer(&[ISSUER]);
        decode::<Claims>(token, &DecodingKey::from_secret(secret.as_bytes()), &validation)
            .expect("Token should decode successfully")
    }

    // ── Access token ──────────────────────────────────────────────

    #[test]
    fn access_token_contains_correct_claims() {
        let user_id = Uuid::new_v4();
        let token = create_access_token(user_id, TEST_SECRET, 15).unwrap();
        let data = decode_token(&token, TEST_SECRET);

        assert_eq!(data.claims.sub, user_id.to_string());
        assert_eq!(data.claims.typ, "access");
        assert_eq!(data.claims.iss, ISSUER);
    }

    #[test]
    fn access_token_has_valid_expiry() {
        let user_id = Uuid::new_v4();
        let before = Utc::now().timestamp();
        let token = create_access_token(user_id, TEST_SECRET, 15).unwrap();
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
        let token = create_access_token(user_id, TEST_SECRET, 15).unwrap();

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
        let token = create_refresh_token(user_id, TEST_SECRET, 30).unwrap();
        let data = decode_token(&token, TEST_SECRET);

        assert_eq!(data.claims.typ, "refresh");
        assert_eq!(data.claims.sub, user_id.to_string());
    }

    #[test]
    fn refresh_token_expiry_is_in_days() {
        let user_id = Uuid::new_v4();
        let before = Utc::now().timestamp();
        let token = create_refresh_token(user_id, TEST_SECRET, 30).unwrap();
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
        let access = create_access_token(user_id, TEST_SECRET, 15).unwrap();
        let refresh = create_refresh_token(user_id, TEST_SECRET, 30).unwrap();

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
            sub: user_id.to_string(),
            typ: "access".into(),
            iss: ISSUER.into(),
            iat: Utc::now().timestamp(),
            exp: (Utc::now() + Duration::hours(1)).timestamp(),
        };
        let auth_user = AuthUser { claims };
        assert_eq!(auth_user.user_id(), user_id);
    }

    #[test]
    fn auth_user_claims_accessor() {
        let claims = Claims {
            sub: Uuid::new_v4().to_string(),
            typ: "access".into(),
            iss: ISSUER.into(),
            iat: 1000,
            exp: 2000,
        };
        let auth_user = AuthUser { claims };
        assert_eq!(auth_user.claims().iat, 1000);
        assert_eq!(auth_user.claims().exp, 2000);
    }
}
