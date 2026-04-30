use chrono::{NaiveDate, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::db;
use crate::error::{AppError, AppResult};
use crate::models::court::{
    BookSlotRequest, BookedSlotResponse, CourtDetail, CourtWithDistance, ListCourtsQuery, Paginated,
    SlotListResponse, SlotItem, SlotStatus,
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
    pub async fn list_courts(
        pool: &PgPool,
        query: ListCourtsQuery,
    ) -> AppResult<Paginated<CourtWithDistance>> {
        // Require both lat and lng together, or neither.
        if query.lat.is_some() != query.lng.is_some() {
            return Err(AppError::BadRequest(
                "Both lat and lng are required for geo search".into(),
            ));
        }

        let limit = query.pagination.effective_limit();
        let offset = query.pagination.offset();

        let (courts, total) = match (query.lat, query.lng) {
            (Some(lat), Some(lng)) => {
                let radius = query.radius_km.unwrap_or(10.0);
                let courts = db::courts::list_courts_near(
                    pool, query.sport_type, lat, lng, radius, limit, offset,
                )
                .await?;
                let total = db::courts::count_courts_near(pool, query.sport_type, lat, lng, radius)
                    .await?;
                (courts, total)
            }
            _ => {
                let courts =
                    db::courts::list_courts_all(pool, query.sport_type, limit, offset).await?;
                let total = db::courts::count_courts_all(pool, query.sport_type).await?;
                (courts, total)
            }
        };

        Ok(Paginated {
            data: courts,
            total,
            page: query.pagination.page(),
            per_page: limit,
        })
    }

    /// Get a single court (no slots — fetch those via GET /courts/:id/slots).
    pub async fn get_court(pool: &PgPool, court_id: Uuid) -> AppResult<CourtDetail> {
        let court = db::courts::find_court_by_id(pool, court_id).await?;
        Ok(CourtDetail { court })
    }

    /// List slots for a specific court on a given date.
    pub async fn list_slots(
        pool: &PgPool,
        court_id: Uuid,
        date: NaiveDate,
    ) -> AppResult<SlotListResponse> {
        let _court = db::courts::find_court_by_id(pool, court_id).await?;
        let slots = db::courts::list_slots_by_court_and_date(pool, court_id, date).await?;
        Ok(SlotListResponse {
            court_id,
            date,
            slots: slots.into_iter().map(SlotItem::from).collect(),
        })
    }

    /// Book a specific slot using `SELECT FOR UPDATE` for concurrency safety.
    pub async fn book_slot(
        pool: &PgPool,
        court_id: Uuid,
        slot_id: Uuid,
        user_id: Uuid,
        _req: BookSlotRequest,
    ) -> AppResult<BookedSlotResponse> {
        let mut tx = pool.begin().await.map_err(AppError::Database)?;

        let slot = db::courts::find_slot_for_update(&mut tx, slot_id, court_id).await?;

        if slot.status != SlotStatus::Available {
            return Err(AppError::Conflict("This time slot is already booked".into()));
        }

        if slot.start_time <= Utc::now() {
            return Err(AppError::BadRequest("Cannot book past time slots".into()));
        }

        let updated =
            db::courts::update_slot_status(&mut tx, slot_id, SlotStatus::Booked, Some(user_id))
                .await?;

        tx.commit().await.map_err(AppError::Database)?;
        Ok(BookedSlotResponse::from(updated))
    }

    /// Cancel a booking. Only the booker can cancel, and only if no active game exists.
    pub async fn cancel_booking(
        pool: &PgPool,
        court_id: Uuid,
        slot_id: Uuid,
        user_id: Uuid,
    ) -> AppResult<BookedSlotResponse> {
        let mut tx = pool.begin().await.map_err(AppError::Database)?;

        let slot = db::courts::find_slot_for_update(&mut tx, slot_id, court_id).await?;

        if slot.status != SlotStatus::Booked {
            return Err(AppError::Conflict("Slot is not currently booked".into()));
        }

        if slot.booked_by != Some(user_id) {
            return Err(AppError::Forbidden(
                "Only the booker can cancel this booking".into(),
            ));
        }

        if slot.start_time <= Utc::now() {
            return Err(AppError::BadRequest("Cannot cancel past bookings".into()));
        }

        // Check no active game exists for this slot.
        let has_game = db::games::has_active_game_for_slot_tx(&mut tx, slot_id).await?;
        if has_game {
            return Err(AppError::Conflict(
                "Cannot cancel booking: an active game exists for this slot".into(),
            ));
        }

        let updated =
            db::courts::update_slot_status(&mut tx, slot_id, SlotStatus::Available, None).await?;

        tx.commit().await.map_err(AppError::Database)?;
        Ok(BookedSlotResponse::from(updated))
    }
}
