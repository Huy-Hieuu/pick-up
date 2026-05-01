use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use validator::Validate;

// ── Enums ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "payment_provider", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum PaymentProvider {
    Momo,
    Zalopay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "payment_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum PaymentStatus {
    Pending,
    Paid,
    Expired,
    Refunded,
}

// ── Database row type ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct PaymentRow {
    pub id: Uuid,
    pub game_id: Uuid,
    pub user_id: Uuid,
    pub amount: i32,
    pub provider: PaymentProvider,
    pub provider_txn_id: Option<String>,
    pub status: PaymentStatus,
    pub paid_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

// ── Request DTOs ───────────────────────────────────────────────

#[derive(Debug, Deserialize, Validate)]
pub struct InitiatePaymentRequest {
    pub provider: PaymentProvider,
}

// ── Response DTOs ──────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct PaymentResponse {
    pub id: Uuid,
    pub amount: i32,
    pub provider: PaymentProvider,
    pub status: PaymentStatus,
    pub pay_url: Option<String>,
}
