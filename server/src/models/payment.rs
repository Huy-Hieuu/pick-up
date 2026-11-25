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
    Disputed,
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
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MomoWebhook {
    pub partner_code: String,
    pub order_id: String,
    pub request_id: String,
    pub amount: i64,
    pub order_info: String,
    pub order_type: String,
    #[serde(default)]
    pub trans_id: i64,
    pub result_code: i32,
    pub message: String,
    pub response_time: i64,
    #[serde(default)]
    pub extra_data: String,
    #[serde(default)]
    pub pay_type: String,
    pub signature: String,
}

#[derive(Debug, Deserialize)]
pub struct ZaloPayWebhook {
    pub data: String,
    pub mac: String,
    #[serde(rename = "type")]
    pub callback_type: i32,
}

#[derive(Debug, Deserialize)]
pub struct ZaloPayCallbackData {
    pub app_id: i64,
    pub app_trans_id: String,
    pub app_time: i64,
    pub amount: i64,
    pub zp_trans_id: i64,
    pub server_time: i64,
}
