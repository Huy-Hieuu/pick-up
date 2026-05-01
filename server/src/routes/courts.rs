use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use uuid::Uuid;

use crate::error::AppResult;
use crate::extractors::{AuthUser, ValidatedQuery};
use crate::models::court::{
    BookSlotRequest, BookedSlotResponse, CourtDetail, CourtWithDistance, ListCourtsQuery,
    Paginated, SlotListQuery, SlotListResponse,
};
use crate::services::CourtService;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_courts))
        .route("/{court_id}", get(get_court))
        .route("/{court_id}/slots", get(list_slots))
        .route("/{court_id}/slots/{slot_id}/book", post(book_slot))
        .route("/{court_id}/slots/{slot_id}/cancel", post(cancel_booking))
}

/// `GET /courts` — list courts with optional filters.
async fn list_courts(
    State(state): State<AppState>,
    ValidatedQuery(query): ValidatedQuery<ListCourtsQuery>,
) -> AppResult<Json<Paginated<CourtWithDistance>>> {
    let result = CourtService::list_courts(&state.pool, query).await?;
    Ok(Json(result))
}

/// `GET /courts/:id` — get court detail (no slots — use /slots endpoint).
async fn get_court(
    State(state): State<AppState>,
    Path(court_id): Path<Uuid>,
) -> AppResult<Json<CourtDetail>> {
    let court = CourtService::get_court(&state.pool, court_id).await?;
    Ok(Json(court))
}

/// `GET /courts/:id/slots` — list slots for a court on a specific date.
async fn list_slots(
    State(state): State<AppState>,
    Path(court_id): Path<Uuid>,
    ValidatedQuery(query): ValidatedQuery<SlotListQuery>,
) -> AppResult<Json<SlotListResponse>> {
    let result = CourtService::list_slots(&state.pool, court_id, query.date).await?;
    Ok(Json(result))
}

/// `POST /courts/:id/slots/:slot_id/book` — book a slot.
async fn book_slot(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((court_id, slot_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<BookSlotRequest>,
) -> AppResult<Json<BookedSlotResponse>> {
    let slot =
        CourtService::book_slot(&state.pool, court_id, slot_id, auth.user_id(), req).await?;
    Ok(Json(slot))
}

/// `POST /courts/:id/slots/:slot_id/cancel` — cancel a booking.
async fn cancel_booking(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((court_id, slot_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<BookedSlotResponse>> {
    let slot =
        CourtService::cancel_booking(&state.pool, court_id, slot_id, auth.user_id()).await?;
    Ok(Json(slot))
}
