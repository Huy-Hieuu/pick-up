use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppResult;
use crate::models::court::{
    BookSlotRequest, CourtDetail, CourtRow, CourtSlotRow, ListCourtsQuery, Paginated,
};

/// Court & slot service.
///
/// All business logic related to:
/// - Court listing with geo filters
/// - Slot availability checking
/// - Slot booking with `SELECT FOR UPDATE` concurrency safety
pub struct CourtService;

impl CourtService {
    /// List courts with optional filters (sport type, geo radius, pagination).
    pub async fn list_courts(_pool: &PgPool, _query: ListCourtsQuery) -> AppResult<Paginated<CourtRow>> {
        // TODO: SELECT with filters, geo distance calc, pagination
        tracing::info!("List courts requested (stub)");
        Err(crate::error::AppError::BadRequest("Not implemented".into()))
    }

    /// Get a single court with its available slots.
    pub async fn get_court(_pool: &PgPool, court_id: Uuid) -> AppResult<CourtDetail> {
        // TODO: SELECT court + JOIN slots
        tracing::info!(%court_id, "Get court requested (stub)");
        Err(crate::error::AppError::NotFound("Court not found".into()))
    }

    /// List available slots for a specific court.
    pub async fn list_slots(_pool: &PgPool, court_id: Uuid) -> AppResult<Vec<CourtSlotRow>> {
        // TODO: SELECT FROM court_slots WHERE court_id = $1 AND status = 'available'
        tracing::info!(%court_id, "List slots requested (stub)");
        Err(crate::error::AppError::BadRequest("Not implemented".into()))
    }

    /// Book a specific slot using `SELECT FOR UPDATE` for concurrency safety.
    pub async fn book_slot(
        _pool: &PgPool,
        slot_id: Uuid,
        user_id: Uuid,
        _req: BookSlotRequest,
    ) -> AppResult<CourtSlotRow> {
        // TODO:
        // 1. BEGIN
        // 2. SELECT * FROM court_slots WHERE id = $1 FOR UPDATE
        // 3. Check status = 'available'
        // 4. UPDATE status = 'booked', booked_by = $2
        // 5. COMMIT
        tracing::info!(%slot_id, %user_id, "Slot booking requested (stub)");
        Err(crate::error::AppError::BadRequest("Not implemented".into()))
    }
}