use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
// use validator::Validate;

// ── Enums ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "sport_type", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum SportType {
    Pickleball,
    MiniFootball,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "slot_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum SlotStatus {
    Available,
    Booked,
    Locked,
}

// ── Database row types ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct CourtRow {
    pub id: Uuid,
    pub name: String,
    pub sport_type: SportType,
    pub lat: f64,
    pub lng: f64,
    pub address: String,
    pub price_per_slot: i32,
    pub photo_urls: Vec<String>,
    pub owner_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct CourtSlotRow {
    pub id: Uuid,
    pub court_id: Uuid,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub status: SlotStatus,
    pub booked_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

// ── Request DTOs ───────────────────────────────────────────────

/// Shared pagination parameters with safe bounds.
#[derive(Debug, Clone, Deserialize)]
pub struct Pagination {
    pub page: Option<i64>,
    pub limit: Option<i64>,
}

impl Pagination {
    pub fn offset(&self) -> i64 {
        (self.page().saturating_sub(1)) * self.effective_limit()
    }

    pub fn page(&self) -> i64 {
        self.page.unwrap_or(1).max(1)
    }

    pub fn effective_limit(&self) -> i64 {
        self.limit.unwrap_or(20).clamp(1, 100)
    }
}

#[derive(Debug, Deserialize)]
pub struct ListCourtsQuery {
    pub sport_type: Option<SportType>,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    pub radius_km: Option<f64>,
    #[serde(flatten)]
    pub pagination: Pagination,
}

#[derive(Debug, Deserialize)]
pub struct BookSlotRequest {
    // Phase 2: add payment method selection here
}

// ── Response DTOs ──────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct CourtDetail {
    #[serde(flatten)]
    pub court: CourtRow,
    pub slots: Vec<CourtSlotRow>,
}

#[derive(Debug, Serialize)]
pub struct Paginated<T> {
    pub data: Vec<T>,
    pub total: i64,
    pub page: i64,
    pub limit: i64,
}
