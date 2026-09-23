use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::extractors::auth::authenticate_access_token;
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

    let auth_header = request
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok());

    let token = match auth_header {
        Some(h) => match h.strip_prefix("Bearer ") {
            Some(t) => t,
            None => return unauthorized("Invalid Authorization format"),
        },
        None => return unauthorized("Missing Authorization header"),
    };

    match authenticate_access_token(&state.pool, token, secret).await {
        Ok(claims) => {
            // Inject decoded claims into request extensions — avoids double decode.
            request.extensions_mut().insert(claims);
        }
        Err(error) => return error.into_response(),
    }

    next.run(request).await
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
