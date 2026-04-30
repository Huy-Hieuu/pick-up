use axum::{
    extract::Request,
    middleware::Next,
    response::{IntoResponse, Response},
    http::StatusCode,
};

use crate::extractors::auth::{Claims, AUDIENCE};
use crate::state::AppState;

/// Auth middleware that verifies JWT on protected routes.
///
/// Decodes the JWT and injects `Claims` into request extensions so that
/// the `AuthUser` extractor can read them without re-decoding.
pub async fn require_auth(
    axum::extract::State(state): axum::extract::State<AppState>,
    mut request: Request,
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

    match verify_token(token, secret) {
        Ok(claims) => {
            // Inject decoded claims into request extensions — avoids double decode.
            request.extensions_mut().insert(claims);
        }
        Err(response) => return response,
    }

    next.run(request).await
}

fn verify_token(token: &str, secret: &str) -> Result<Claims, Response> {
    use jsonwebtoken::{decode, DecodingKey, Validation};

    let mut validation = Validation::default();
    validation.set_issuer(&["pickup-server"]);
    validation.set_audience(&[AUDIENCE]);

    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|_| unauthorized("Invalid or expired token"))?;

    if token_data.claims.typ != "access" {
        return Err(unauthorized("Invalid token type"));
    }

    Ok(token_data.claims)
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
