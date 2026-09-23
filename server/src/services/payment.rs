use chrono::{DateTime, Duration, Utc};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use super::payment_provider::{self, Reservation};
use crate::{
    config::Settings,
    error::{AppError, AppResult},
    models::payment::{
        InitiatePaymentRequest, PaymentProvider, PaymentResponse, PaymentRow, PaymentStatus,
    },
};

pub struct PaymentService;

impl PaymentService {
    pub async fn list_payments(pool: &PgPool, game_id: Uuid) -> AppResult<Vec<PaymentRow>> {
        Ok(sqlx::query_as::<_, PaymentRow>(
            "SELECT id, game_id, user_id, amount, provider, provider_txn_id, status, paid_at, created_at FROM payments WHERE game_id = $1 ORDER BY created_at, id"
        ).bind(game_id).fetch_all(pool).await?)
    }

    /// A local deadline does not prove non-payment. Freeze stale orders until
    /// the provider confirms their final state; SKIP LOCKED distributes work.
    pub async fn expire_stale_pending(pool: &PgPool) -> AppResult<u64> {
        let result = sqlx::query(r#"
            WITH expired AS (
                SELECT id FROM payments
                WHERE status = 'pending' AND expires_at <= now()
                    AND (provider_create_claim_until IS NULL OR provider_create_claim_until <= now())
                ORDER BY expires_at LIMIT 500 FOR UPDATE SKIP LOCKED
            )
            UPDATE payments p SET status = 'disputed', dispute_reason = 'awaiting_provider_status', provider_create_claim = NULL,
                provider_create_claim_until = NULL
            FROM expired WHERE p.id = expired.id
        "#).execute(pool).await?;
        Ok(result.rows_affected())
    }

    /// Claim a bounded batch without holding database locks during network I/O.
    /// Claims become eligible again after two minutes if the worker crashes.
    pub async fn reconcile_pending(pool: &PgPool, settings: &Settings) -> AppResult<u64> {
        let orders: Vec<Reservation> = sqlx::query_as(r#"
            WITH due AS (
                SELECT id FROM payments
                WHERE (status = 'pending' OR (status = 'disputed' AND dispute_reason IN ('provider_creation_uncertain', 'awaiting_provider_status')))
                    AND reconcile_after <= now()
                    AND (provider_create_claim_until IS NULL OR provider_create_claim_until <= now())
                ORDER BY reconcile_after LIMIT 4 FOR UPDATE SKIP LOCKED
            )
            UPDATE payments p SET reconcile_after = now() + interval '2 minutes'
            FROM due WHERE p.id = due.id
            RETURNING p.id, p.amount, p.provider, p.provider_txn_id, p.status,
                p.provider_order_id, p.provider_payment_url, p.expires_at,
                p.provider_create_claim, p.provider_create_claim_until, p.dispute_reason
        "#).fetch_all(pool).await?;
        let mut resolved = 0;
        for order in orders {
            let result = match payment_provider::query_order(settings, &order).await {
                Ok(result) => result,
                Err(_) => {
                    tracing::warn!(payment_id = %order.id, "Provider reconciliation query failed; reservation retained");
                    continue;
                }
            };
            if result.outcome == "processing" {
                continue;
            }
            let fingerprint = hex::encode(Sha256::digest(
                serde_json::to_vec(&(
                    "query",
                    &order.provider_order_id,
                    result.amount,
                    &result.transaction_id,
                    result.outcome,
                ))
                .map_err(|e| AppError::Internal(e.into()))?,
            ));
            match apply_provider_event(
                pool,
                order.provider,
                VerifiedProviderEvent {
                    order_id: &order.provider_order_id,
                    request_id: None,
                    received_amount: result.amount,
                    provider_txn_id: result.transaction_id,
                    outcome: result.outcome,
                    payload_hash: fingerprint,
                },
            )
            .await
            {
                Ok(()) => resolved += 1,
                Err(_) => {
                    tracing::error!(payment_id = %order.id, "Provider reconciliation settlement failed; retry retained")
                }
            }
        }
        Ok(resolved)
    }

    /// Reserve the current split under a game-row lock. PSP calls happen after
    /// commit; uncertain HTTP errors retain the same provider order ID and an
    /// expiring lease so retries cannot create a second charge.
    pub async fn initiate(
        pool: &PgPool,
        settings: &Settings,
        game_id: Uuid,
        user_id: Uuid,
        req: InitiatePaymentRequest,
    ) -> AppResult<PaymentResponse> {
        payment_provider::credentials(settings, req.provider)?;
        let claim = Uuid::new_v4();
        let mut tx = pool.begin().await?;
        let (total_amount, status): (i32, crate::models::game::GameStatus) =
            sqlx::query_as("SELECT total_amount, status FROM games WHERE id = $1 FOR UPDATE")
                .bind(game_id)
                .fetch_one(&mut *tx)
                .await?;
        if !matches!(
            status,
            crate::models::game::GameStatus::Open
                | crate::models::game::GameStatus::Full
                | crate::models::game::GameStatus::InProgress
        ) {
            return Err(AppError::Conflict("Game is not in a payable state".into()));
        }
        let split = crate::services::BillSplitService::calculate_on(&mut tx, game_id).await?;
        let entry = split
            .payments
            .iter()
            .find(|entry| entry.user_id == user_id)
            .ok_or_else(|| AppError::NotFound("Player is not a member of this game".into()))?;
        if entry.paid {
            return Err(AppError::Conflict(
                "Your share has already been paid".into(),
            ));
        }
        let amount = entry.amount;
        let provider_max = if req.provider == PaymentProvider::Momo {
            50_000_000
        } else {
            i32::MAX
        };
        if amount < 1_000 || amount > total_amount || amount > provider_max {
            return Err(AppError::Unprocessable(
                "Payment amount is outside provider limits".into(),
            ));
        }

        let existing: Option<Reservation> = sqlx::query_as(
            r#"
            SELECT id, amount, provider, provider_txn_id, status, provider_order_id, provider_payment_url,
                   expires_at, provider_create_claim, provider_create_claim_until, dispute_reason
            FROM payments WHERE game_id = $1 AND user_id = $2
                AND status IN ('pending', 'paid', 'disputed')
            ORDER BY created_at DESC LIMIT 1 FOR UPDATE
        "#,
        )
        .bind(game_id)
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?;

        let retrying_existing_order = existing.is_some();
        let reservation = match existing {
            Some(row) if row.status == PaymentStatus::Paid => {
                return Err(AppError::Conflict(
                    "Your share has already been paid".into(),
                ))
            }
            Some(row) if row.status == PaymentStatus::Disputed => {
                return Err(AppError::Conflict(
                    "Payment requires manual reconciliation".into(),
                ))
            }
            Some(row) if row.provider != req.provider => {
                return Err(AppError::Conflict(
                    "A payment is already active with another provider".into(),
                ))
            }
            Some(row) if row.expires_at.is_some_and(|expires| expires <= Utc::now()) => {
                sqlx::query("UPDATE payments SET status = 'disputed', dispute_reason = 'awaiting_provider_status', provider_create_claim = NULL, provider_create_claim_until = NULL WHERE id = $1")
                    .bind(row.id).execute(&mut *tx).await?;
                tx.commit().await?;
                return Err(AppError::PaymentOrderUncertain);
            }
            Some(row) if row.provider_payment_url.is_some() => row,
            Some(row)
                if row
                    .provider_create_claim_until
                    .is_some_and(|until| until > Utc::now()) =>
            {
                return Err(AppError::Conflict(
                    "Payment order is being created; retry shortly".into(),
                ))
            }
            Some(row) => {
                // Reuse the stable PSP order/reference after an ambiguous timeout.
                sqlx::query("UPDATE payments SET provider_create_claim = $2, provider_create_claim_until = now() + interval '30 seconds' WHERE id = $1")
                    .bind(row.id).bind(claim).execute(&mut *tx).await?;
                Reservation {
                    provider_create_claim: Some(claim),
                    ..row
                }
            }
            None => new_reservation(&mut tx, game_id, user_id, req.provider, amount, claim).await?,
        };
        tx.commit().await?;

        if reservation.provider_payment_url.is_some()
            && reservation
                .expires_at
                .is_some_and(|expires| expires > Utc::now())
        {
            return Ok(response(&reservation));
        }
        if reservation.provider_create_claim != Some(claim) {
            return Err(AppError::Conflict(
                "Payment order is being created; retry shortly".into(),
            ));
        }

        let provider_result = match req.provider {
            PaymentProvider::Momo => {
                payment_provider::create_momo_order(settings, &reservation).await
            }
            PaymentProvider::Zalopay => {
                payment_provider::create_zalopay_order(settings, &reservation, user_id).await
            }
        };
        let url = match provider_result {
            Ok(url) => url,
            Err(AppError::PaymentRejected) if retrying_existing_order => {
                // A rejection of this request cannot disprove an earlier
                // request's capture after a lost response or config change.
                sqlx::query("UPDATE payments SET status = 'disputed', dispute_reason = 'provider_creation_uncertain', provider_create_claim = NULL, provider_create_claim_until = NULL WHERE id = $1 AND status = 'pending' AND provider_create_claim = $2")
                    .bind(reservation.id).bind(claim).execute(pool).await?;
                return Err(AppError::PaymentOrderUncertain);
            }
            Err(AppError::PaymentRejected) => {
                // The PSP definitively refused the order; release the user's slot.
                sqlx::query("UPDATE payments SET status = 'expired', provider_create_claim = NULL, provider_create_claim_until = NULL WHERE id = $1 AND status = 'pending' AND provider_create_claim = $2")
                    .bind(reservation.id).bind(claim).execute(pool).await?;
                return Err(AppError::PaymentRejected);
            }
            Err(AppError::PaymentOrderUncertain) => {
                // Without a provider status query, releasing this reservation
                // at a timer could create a second collectible order. Freeze
                // the game until a callback or an operator reconciles it.
                sqlx::query("UPDATE payments SET status = 'disputed', dispute_reason = 'provider_creation_uncertain', provider_create_claim = NULL, provider_create_claim_until = NULL WHERE id = $1 AND status = 'pending' AND provider_create_claim = $2")
                    .bind(reservation.id).bind(claim).execute(pool).await?;
                return Err(AppError::PaymentOrderUncertain);
            }
            Err(error) => return Err(error),
        };
        let expires_at: Option<DateTime<Utc>> = sqlx::query_scalar(
            "UPDATE payments SET provider_payment_url = $2, provider_create_claim = NULL, provider_create_claim_until = NULL WHERE id = $1 AND status = 'pending' AND provider_create_claim = $3 RETURNING expires_at"
        ).bind(reservation.id).bind(&url).bind(claim).fetch_optional(pool).await?;
        let expires_at = match expires_at {
            Some(expires_at) => expires_at,
            None => {
                // An IPN may race the provider's create-order HTTP response.
                // Return the committed settlement to the client instead of
                // turning the already successful charge into a false 409.
                let settled: Option<(PaymentStatus, Option<String>, Option<DateTime<Utc>>)> =
                    sqlx::query_as(
                        "SELECT status, provider_txn_id, expires_at FROM payments WHERE id = $1",
                    )
                    .bind(reservation.id)
                    .fetch_optional(pool)
                    .await?;
                if let Some((PaymentStatus::Paid, Some(_), expires_at)) = settled {
                    return Ok(PaymentResponse {
                        id: reservation.id,
                        amount: reservation.amount,
                        provider: reservation.provider,
                        status: PaymentStatus::Paid,
                        pay_url: Some(url),
                        expires_at,
                    });
                }
                return Err(AppError::Conflict(
                    "Payment order changed while contacting provider".into(),
                ));
            }
        };
        Ok(PaymentResponse {
            id: reservation.id,
            amount: reservation.amount,
            provider: reservation.provider,
            status: PaymentStatus::Pending,
            pay_url: Some(url),
            expires_at: Some(expires_at),
        })
    }

    pub async fn handle_momo_webhook(
        pool: &PgPool,
        settings: &Settings,
        raw: &str,
    ) -> AppResult<()> {
        if settings.momo.access_key.trim().is_empty()
            || settings.momo.secret_key.trim().is_empty()
            || settings.momo.partner_code.trim().is_empty()
        {
            return Err(AppError::Unimplemented(
                "MoMo callback credentials are not configured".into(),
            ));
        }
        let body: crate::models::payment::MomoWebhook = serde_json::from_str(raw)
            .map_err(|_| AppError::BadRequest("Invalid MoMo callback".into()))?;
        let message = payment_provider::momo_signature_message(&body, &settings.momo.access_key);
        if !payment_provider::verify_hmac_hex(&settings.momo.secret_key, &message, &body.signature)
        {
            return Err(AppError::Unauthorized("Invalid payment signature".into()));
        }
        if body.partner_code != settings.momo.partner_code {
            return Err(AppError::Unauthorized("Invalid payment partner".into()));
        }
        let outcome = match body.result_code {
            0 if body.trans_id > 0 => "paid",
            1000 | 7000 | 7002 | 9000 => "processing",
            0 => "review", // successful results must carry a PSP transaction ID
            code if payment_provider::momo_terminal_failure(code) => "failed",
            _ => "processing",
        };
        let fingerprint = Sha256::digest(message.as_bytes());
        apply_provider_event(
            pool,
            PaymentProvider::Momo,
            VerifiedProviderEvent {
                order_id: &body.order_id,
                request_id: Some(&body.request_id),
                received_amount: body.amount,
                provider_txn_id: (body.trans_id > 0).then(|| body.trans_id.to_string()),
                outcome,
                payload_hash: hex::encode(fingerprint),
            },
        )
        .await
    }

    pub async fn handle_zalopay_webhook(
        pool: &PgPool,
        settings: &Settings,
        raw: &str,
    ) -> AppResult<()> {
        if settings.zalopay.key2.trim().is_empty() || settings.zalopay.app_id.trim().is_empty() {
            return Err(AppError::Unimplemented(
                "ZaloPay callback credentials are not configured".into(),
            ));
        }
        let body: crate::models::payment::ZaloPayWebhook = serde_json::from_str(raw)
            .map_err(|_| AppError::BadRequest("Invalid ZaloPay callback".into()))?;
        if body.callback_type != 1 {
            return Err(AppError::BadRequest(
                "Unsupported ZaloPay callback type".into(),
            ));
        }
        let fingerprint = Sha256::digest(body.data.as_bytes());
        if !payment_provider::verify_hmac_hex(&settings.zalopay.key2, &body.data, &body.mac) {
            return Err(AppError::Unauthorized("Invalid payment signature".into()));
        }
        let callback: crate::models::payment::ZaloPayCallbackData =
            serde_json::from_str(&body.data)
                .map_err(|_| AppError::BadRequest("Invalid signed ZaloPay callback data".into()))?;
        let configured_app_id = settings
            .zalopay
            .app_id
            .parse::<i64>()
            .map_err(|_| AppError::PaymentGateway)?;
        if callback.app_id != configured_app_id || callback.zp_trans_id <= 0 {
            return Err(AppError::Unauthorized("Invalid ZaloPay order".into()));
        }
        apply_provider_event(
            pool,
            PaymentProvider::Zalopay,
            VerifiedProviderEvent {
                order_id: &callback.app_trans_id,
                request_id: None,
                received_amount: callback.amount,
                provider_txn_id: Some(callback.zp_trans_id.to_string()),
                outcome: "paid",
                payload_hash: hex::encode(fingerprint),
            },
        )
        .await
    }
}

async fn new_reservation(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    game_id: Uuid,
    user_id: Uuid,
    provider: PaymentProvider,
    amount: i32,
    claim: Uuid,
) -> AppResult<Reservation> {
    let id = Uuid::new_v4();
    let order_id = match provider {
        PaymentProvider::Momo => id.simple().to_string(),
        PaymentProvider::Zalopay => {
            let vietnam_now = Utc::now() + Duration::hours(7);
            format!("{}{}", vietnam_now.format("%y%m%d_"), id.simple())
        }
    };
    Ok(sqlx::query_as(r#"
        INSERT INTO payments
            (id, game_id, user_id, amount, provider, provider_order_id, expires_at, provider_create_claim, provider_create_claim_until)
        VALUES ($1, $2, $3, $4, $5, $6, now() + interval '15 minutes', $7, now() + interval '30 seconds')
        RETURNING id, amount, provider, provider_txn_id, status, provider_order_id, provider_payment_url,
                  expires_at, provider_create_claim, provider_create_claim_until, dispute_reason
    "#).bind(id).bind(game_id).bind(user_id).bind(amount).bind(provider).bind(order_id)
        .bind(claim).fetch_one(&mut **tx).await?)
}

fn response(reservation: &Reservation) -> PaymentResponse {
    PaymentResponse {
        id: reservation.id,
        amount: reservation.amount,
        provider: reservation.provider,
        status: reservation.status,
        pay_url: reservation.provider_payment_url.clone(),
        expires_at: reservation.expires_at,
    }
}

struct VerifiedProviderEvent<'a> {
    order_id: &'a str,
    request_id: Option<&'a str>,
    received_amount: i64,
    provider_txn_id: Option<String>,
    outcome: &'static str,
    payload_hash: String,
}

async fn apply_provider_event(
    pool: &PgPool,
    provider: PaymentProvider,
    event: VerifiedProviderEvent<'_>,
) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    // Same lock order as initiation/membership changes. Late callbacks cannot
    // race a new order or roster update between its billing check and commit.
    let game_id: Uuid = sqlx::query_scalar(
        "SELECT game_id FROM payments WHERE provider = $1 AND provider_order_id = $2",
    )
    .bind(provider)
    .bind(event.order_id)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query("SELECT id FROM games WHERE id = $1 FOR UPDATE")
        .bind(game_id)
        .fetch_one(&mut *tx)
        .await?;
    let reservation: Option<Reservation> = sqlx::query_as(
        r#"
        SELECT id, amount, provider, provider_txn_id, status, provider_order_id, provider_payment_url,
               expires_at, provider_create_claim, provider_create_claim_until, dispute_reason
        FROM payments WHERE provider = $1 AND provider_order_id = $2 FOR UPDATE
    "#,
    )
    .bind(provider)
    .bind(event.order_id)
    .fetch_optional(&mut *tx)
    .await?;
    let payment =
        reservation.ok_or_else(|| AppError::NotFound("Payment order not found".into()))?;

    if let Some(request_id) = event.request_id {
        if request_id != payment.id.simple().to_string() {
            return Err(AppError::Conflict(
                "Payment request ID does not match the stored order".into(),
            ));
        }
    }

    // Replayed signed observations are acknowledged without mutating state.
    let seen: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM payment_provider_events WHERE provider = $1 AND payload_hash = $2 AND payment_id = $3)",
    ).bind(provider).bind(&event.payload_hash).bind(payment.id).fetch_one(&mut *tx).await?;
    if seen {
        tx.commit().await?;
        return Ok(());
    }

    // In-progress events do not claim a transaction or release a reservation.
    // Later final callbacks may legitimately carry the same PSP transaction ID.
    if event.outcome == "processing" {
        record_provider_event(&mut tx, provider, &payment, &event).await?;
        tx.commit().await?;
        return Ok(());
    }

    let mut anomaly = None;
    if let Some(txn_id) = &event.provider_txn_id {
        sqlx::query("INSERT INTO payment_provider_transactions (provider, transaction_id, payment_id) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING")
            .bind(provider).bind(txn_id).bind(payment.id).execute(&mut *tx).await?;
        let owner: Uuid = sqlx::query_scalar("SELECT payment_id FROM payment_provider_transactions WHERE provider = $1 AND transaction_id = $2")
            .bind(provider).bind(txn_id).fetch_one(&mut *tx).await?;
        if owner != payment.id {
            anomaly = Some("provider_transaction_reused");
        }
        let contradictory: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM payment_provider_events WHERE payment_id = $1 AND reported_txn_id = $2 AND outcome <> 'processing' AND (amount_received <> $3 OR outcome <> $4))",
        ).bind(payment.id).bind(txn_id).bind(event.received_amount).bind(event.outcome)
            .fetch_one(&mut *tx).await?;
        if contradictory {
            anomaly = Some("conflicting_provider_callback");
        }
    }
    record_provider_event(&mut tx, provider, &payment, &event).await?;
    if let Some(reason) = anomaly {
        flag_payment_for_reconciliation(&mut tx, &payment, reason).await?;
        tx.commit().await?;
        return Ok(());
    }

    let recoverable = payment.status == PaymentStatus::Disputed
        && matches!(
            payment.dispute_reason.as_deref(),
            Some("provider_creation_uncertain" | "awaiting_provider_status")
        );
    match (payment.status, event.outcome) {
        (PaymentStatus::Pending, "paid" | "failed")
        | (PaymentStatus::Disputed, "paid" | "failed")
            if payment.status == PaymentStatus::Pending || recoverable =>
        {
            if event.received_amount != i64::from(payment.amount) {
                flag_payment_for_reconciliation(&mut tx, &payment, "amount_mismatch").await?;
            } else if event.outcome == "paid" && event.provider_txn_id.is_some() {
                // Recovery is safe only if no newer collectible attempt exists.
                let other: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM payments WHERE game_id = $1 AND user_id = (SELECT user_id FROM payments WHERE id = $2) AND id <> $2 AND (status IN ('pending', 'paid', 'disputed') OR dispute_reason IS NOT NULL))",
                ).bind(game_id).bind(payment.id).fetch_one(&mut *tx).await?;
                if other {
                    flag_payment_for_reconciliation(&mut tx, &payment, "late_capture").await?;
                } else {
                    sqlx::query("UPDATE payments SET status = 'paid', provider_txn_id = $2, paid_at = now(), dispute_reason = NULL, provider_create_claim = NULL, provider_create_claim_until = NULL WHERE id = $1")
                        .bind(payment.id).bind(&event.provider_txn_id).execute(&mut *tx).await?;
                }
            } else if event.outcome == "failed" {
                sqlx::query("UPDATE payments SET status = 'expired', dispute_reason = NULL, provider_create_claim = NULL, provider_create_claim_until = NULL WHERE id = $1")
                    .bind(payment.id).execute(&mut *tx).await?;
            } else {
                flag_payment_for_reconciliation(&mut tx, &payment, "provider_review").await?;
            }
        }
        (PaymentStatus::Paid, "paid")
            if payment.provider_txn_id == event.provider_txn_id
                && event.received_amount == i64::from(payment.amount) => {}
        (PaymentStatus::Paid, "failed") => {}
        (PaymentStatus::Expired | PaymentStatus::Refunded, "paid" | "review") => {
            flag_payment_for_reconciliation(&mut tx, &payment, "late_capture").await?;
        }
        (PaymentStatus::Disputed, _) => {}
        (_, "review") | (PaymentStatus::Paid, "paid") => {
            flag_payment_for_reconciliation(&mut tx, &payment, "provider_review").await?;
        }
        _ => {}
    }
    tx.commit().await?;
    Ok(())
}

async fn record_provider_event(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    provider: PaymentProvider,
    payment: &Reservation,
    event: &VerifiedProviderEvent<'_>,
) -> AppResult<()> {
    sqlx::query("INSERT INTO payment_provider_events (payment_id, provider, provider_txn_id, reported_txn_id, payload_hash, amount_received, outcome) VALUES ($1, $2, $3, $3, $4, $5, $6) ON CONFLICT DO NOTHING")
        .bind(payment.id).bind(provider).bind(&event.provider_txn_id).bind(&event.payload_hash)
        .bind(event.received_amount).bind(event.outcome).execute(&mut **tx).await?;
    Ok(())
}

/// Historical disputes coexist with replacement attempts. Billing checks every
/// dispute for the game, and only pending/paid rows occupy the collectible slot.
async fn flag_payment_for_reconciliation(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    payment: &Reservation,
    reason: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE payments SET status = 'disputed', dispute_reason = $2, provider_create_claim = NULL, provider_create_claim_until = NULL WHERE id = $1")
        .bind(payment.id).bind(reason).execute(&mut **tx).await?;
    Ok(())
}
