use axum::{body::Bytes, extract::State, routing::post, Json, Router};

use crate::error::AppResult;
use crate::services::PaymentService;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/momo", post(momo_webhook))
        .route("/zalopay", post(zalopay_webhook))
}

/// `POST /webhooks/momo` — Momo payment callback.
///
/// **No auth** — identity is verified via HMAC signature.
async fn momo_webhook(
    State(state): State<AppState>,
    body: Bytes,
) -> AppResult<Json<serde_json::Value>> {
    let body_str = String::from_utf8(body.to_vec())
        .map_err(|e| crate::error::AppError::BadRequest(format!("Invalid UTF-8 body: {e}")))?;
    PaymentService::handle_momo_webhook(&state.pool, &body_str).await?;
    Ok(Json(serde_json::json!({ "resultCode": 0 }))) // reached only after implementation
}

/// `POST /webhooks/zalopay` — ZaloPay payment callback.
///
/// **No auth** — identity is verified via MAC checksum.
async fn zalopay_webhook(
    State(state): State<AppState>,
    body: Bytes,
) -> AppResult<Json<serde_json::Value>> {
    let body_str = String::from_utf8(body.to_vec())
        .map_err(|e| crate::error::AppError::BadRequest(format!("Invalid UTF-8 body: {e}")))?;
    PaymentService::handle_zalopay_webhook(&state.pool, &body_str).await?;
    Ok(Json(serde_json::json!({ "return_code": 1 })))
}
