use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppResult;
use crate::models::payment::{InitiatePaymentRequest, PaymentResponse, PaymentRow};

/// Payment service.
///
/// Handles payment initiation and webhook processing for Momo & ZaloPay.
pub struct PaymentService;

impl PaymentService {
    /// List all payments for a game.
    pub async fn list_payments(_pool: &PgPool, game_id: Uuid) -> AppResult<Vec<PaymentRow>> {
        // TODO: SELECT FROM payments WHERE game_id = $1
        tracing::info!(%game_id, "List payments requested (stub)");
        Err(crate::error::AppError::BadRequest("Not implemented".into()))
    }

    /// Initiate a payment for a player's share of the bill.
    pub async fn initiate(
        _pool: &PgPool,
        game_id: Uuid,
        user_id: Uuid,
        req: InitiatePaymentRequest,
    ) -> AppResult<PaymentResponse> {
        // TODO:
        // 1. Calculate player's share via BillSplitService
        // 2. INSERT payment record (status = pending)
        // 3. Call Momo/ZaloPay API to get pay_url
        // 4. Return payment response with pay_url
        tracing::info!(%game_id, %user_id, ?req.provider, "Payment initiate requested (stub)");
        Err(crate::error::AppError::BadRequest("Not implemented".into()))
    }

    /// Process Momo webhook callback.
    ///
    /// **MUST** verify the signature before processing.
    #[allow(dead_code)]
    pub async fn handle_momo_webhook(_pool: &PgPool, _body: &str) -> AppResult<()> {
        // TODO:
        // 1. Verify signature using MOMO_SECRET_KEY
        // 2. Extract orderId, transId, resultCode
        // 3. Update payment status
        // 4. Broadcast payment_updated via WebSocket
        tracing::info!("Momo webhook received (stub)");
        Ok(())
    }

    /// Process ZaloPay webhook callback.
    ///
    /// **MUST** verify the MAC using ZALOPAY_KEY2.
    #[allow(dead_code)]
    pub async fn handle_zalopay_webhook(_pool: &PgPool, _body: &str) -> AppResult<()> {
        // TODO:
        // 1. Verify MAC using key2
        // 2. Extract app_trans_id, zp_trans_id, status
        // 3. Update payment status
        // 4. Broadcast payment_updated via WebSocket
        tracing::info!("ZaloPay webhook received (stub)");
        Ok(())
    }
}