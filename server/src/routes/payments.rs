use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use uuid::Uuid;

use crate::error::AppResult;
use crate::extractors::AuthUser;
use crate::models::payment::{InitiatePaymentRequest, PaymentResponse, PaymentRow};
use crate::services::PaymentService;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_payments))
        .route("/initiate", post(initiate_payment))
}

/// `GET /games/:game_id/payments` — list all payments for a game.
async fn list_payments(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(game_id): Path<Uuid>,
) -> AppResult<Json<Vec<PaymentRow>>> {
    let payments = PaymentService::list_payments(&state.pool, game_id).await?;
    Ok(Json(payments))
}

/// `POST /games/:game_id/payments/initiate` — start a payment.
async fn initiate_payment(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(game_id): Path<Uuid>,
    Json(req): Json<InitiatePaymentRequest>,
) -> AppResult<Json<PaymentResponse>> {
    let payment = PaymentService::initiate(&state.pool, game_id, auth.user_id(), req).await?;
    Ok(Json(payment))
}
