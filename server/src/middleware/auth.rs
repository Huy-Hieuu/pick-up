use axum::{
    extract::Request,
    middleware::Next,
    response::{IntoResponse, Response},
    http::StatusCode,
};

use crate::extractors::auth::Claims;
use crate::state::AppState;

/// Auth middleware that verifies JWT on protected routes.
///
/// Short-circuits with 401 if no valid token is present.
/// Handlers should still use `AuthUser` extractor for user ID access.
/// This middleware exists as a safety net in case a handler forgets the extractor.
pub async fn require_auth(
    axum::extract::State(state): axum::extract::State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let secret = &state.settings.jwt.secret;

    let auth_header = request.headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok());

    let token = match auth_header {
        Some(h) => match h.strip_prefix("Bearer ") {
            Some(t) => t,
            None => return unauthorized("Invalid Authorization format"),
        },
        None => return unauthorized("Missing Authorization header"),
    };

    if let Err(response) = verify_token(token, secret) {
        return response;
    }

    next.run(request).await
}

fn verify_token(token: &str, secret: &str) -> Result<(), Response> {
    use jsonwebtoken::{decode, DecodingKey, Validation};

    let mut validation = Validation::default();
    validation.set_issuer(&["pickup-server"]);

    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|_| unauthorized("Invalid or expired token"))?;

    if token_data.claims.typ != "access" {
        return Err(unauthorized("Invalid token type"));
    }

    Ok(())
}

fn unauthorized(msg: &str) -> Response {
    let body = serde_json::json!({
        "error": {
            "code": "UNAUTHORIZED",
            "message": msg,
        }
    });
    (StatusCode::UNAUTHORIZED, axum::Json(body)).into_response()
}
