use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use uuid::Uuid;

use crate::error::AppResult;
use crate::extractors::AuthUser;
use crate::models::court::{
    BookSlotRequest, CourtDetail, CourtRow, CourtSlotRow, ListCourtsQuery, Paginated,
};
use crate::services::CourtService;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_courts))
        .route("/{court_id}", get(get_court))
        .route("/{court_id}/slots", get(list_slots))
        .route("/{court_id}/slots/{slot_id}/book", post(book_slot))
}

/// `GET /courts` — list courts with optional filters.
async fn list_courts(
    State(state): State<AppState>,
    Query(query): Query<ListCourtsQuery>,
) -> AppResult<Json<Paginated<CourtRow>>> {
    let result = CourtService::list_courts(&state.pool, query).await?;
    Ok(Json(result))
}

/// `GET /courts/:id` — get court detail with available slots.
async fn get_court(
    State(state): State<AppState>,
    Path(court_id): Path<Uuid>,
) -> AppResult<Json<CourtDetail>> {
    let court = CourtService::get_court(&state.pool, court_id).await?;
    Ok(Json(court))
}

/// `GET /courts/:id/slots` — list available slots for a court.
async fn list_slots(
    State(state): State<AppState>,
    Path(court_id): Path<Uuid>,
) -> AppResult<Json<Vec<CourtSlotRow>>> {
    let slots = CourtService::list_slots(&state.pool, court_id).await?;
    Ok(Json(slots))
}

/// `POST /courts/:id/slots/:slot_id/book` — book a slot.
async fn book_slot(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((_court_id, slot_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<BookSlotRequest>,
) -> AppResult<Json<CourtSlotRow>> {
    let slot = CourtService::book_slot(&state.pool, slot_id, auth.user_id(), req).await?;
    Ok(Json(slot))
}
