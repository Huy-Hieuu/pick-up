use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use validator::Validate;

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
    #[serde(skip_serializing)]
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

// ── Query DTOs ──────────────────────────────────────────────────

/// Query params for `GET /courts/:id/slots`. Date is required per spec.
#[derive(Debug, Deserialize, Validate)]
pub struct SlotListQuery {
    pub date: NaiveDate,
}

// ── Request DTOs ───────────────────────────────────────────────

/// Shared pagination parameters with safe bounds.
#[derive(Debug, Clone, Deserialize)]
pub struct Pagination {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

impl Pagination {
    pub fn offset(&self) -> i64 {
        (self.page().saturating_sub(1))
            .saturating_mul(self.effective_limit())
    }

    pub fn page(&self) -> i64 {
        self.page.unwrap_or(1).max(1)
    }

    pub fn effective_limit(&self) -> i64 {
        self.per_page.unwrap_or(20).clamp(1, 50)
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct ListCourtsQuery {
    pub sport_type: Option<SportType>,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    #[validate(range(min = 0.1, max = 50.0, message = "radius_km must be between 0.1 and 50"))]
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
}

#[derive(Debug, Serialize)]
pub struct Paginated<T> {
    #[serde(rename = "items")]
    pub data: Vec<T>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

/// Court row with optional geo distance — used in list responses.
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct CourtWithDistance {
    #[sqlx(flatten)]
    #[serde(flatten)]
    pub court: CourtRow,
    pub distance_km: Option<f64>,
}

// ── Slot response types ─────────────────────────────────────────

/// Slot item in list response — excludes `court_id`, `booked_by`, `created_at`.
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct SlotItem {
    pub id: Uuid,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub status: SlotStatus,
}

/// `GET /courts/:id/slots` response wrapper.
#[derive(Debug, Serialize)]
pub struct SlotListResponse {
    pub court_id: Uuid,
    pub date: NaiveDate,
    pub slots: Vec<SlotItem>,
}

/// `POST /courts/:id/slots/:slot_id/book` response — renames `id` → `slot_id`.
#[derive(Debug, Serialize)]
pub struct BookedSlotResponse {
    #[serde(rename = "slot_id")]
    pub id: Uuid,
    pub court_id: Uuid,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub status: SlotStatus,
    pub booked_by: Option<Uuid>,
}

impl From<CourtSlotRow> for SlotItem {
    fn from(s: CourtSlotRow) -> Self {
        Self {
            id: s.id,
            start_time: s.start_time,
            end_time: s.end_time,
            status: s.status,
        }
    }
}

impl From<CourtSlotRow> for BookedSlotResponse {
    fn from(s: CourtSlotRow) -> Self {
        Self {
            id: s.id,
            court_id: s.court_id,
            start_time: s.start_time,
            end_time: s.end_time,
            status: s.status,
            booked_by: s.booked_by,
        }
    }
}
