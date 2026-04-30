use chrono::NaiveDate;
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::court::{CourtRow, CourtSlotRow, CourtWithDistance, SlotStatus, SportType};

// ── List courts with geo (Haversine) ───────────────────────────

pub async fn list_courts_near(
    pool: &PgPool,
    sport_type: Option<SportType>,
    lat: f64,
    lng: f64,
    radius_km: f64,
    limit: i64,
    offset: i64,
) -> sqlx::Result<Vec<CourtWithDistance>> {
    sqlx::query_as::<_, CourtWithDistance>(
        r#"
        SELECT * FROM (
            SELECT
                id, name, sport_type, lat, lng, address,
                price_per_slot, photo_urls, owner_id, created_at,
                (6371 * acos(LEAST(1.0,
                    cos(radians($1)) * cos(radians(lat)) *
                    cos(radians(lng) - radians($2)) +
                    sin(radians($1)) * sin(radians(lat))
                ))) AS distance_km
            FROM courts
            WHERE ($3::sport_type IS NULL OR sport_type = $3)
        ) sub
        WHERE sub.distance_km <= $4
        ORDER BY sub.distance_km ASC
        LIMIT $5 OFFSET $6
        "#,
    )
    .bind(lat)
    .bind(lng)
    .bind(sport_type)
    .bind(radius_km)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
}

pub async fn count_courts_near(
    pool: &PgPool,
    sport_type: Option<SportType>,
    lat: f64,
    lng: f64,
    radius_km: f64,
) -> sqlx::Result<i64> {
    sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*) FROM (
            SELECT
                (6371 * acos(LEAST(1.0,
                    cos(radians($1)) * cos(radians(lat)) *
                    cos(radians(lng) - radians($2)) +
                    sin(radians($1)) * sin(radians(lat))
                ))) AS distance_km
            FROM courts
            WHERE ($3::sport_type IS NULL OR sport_type = $3)
        ) sub
        WHERE sub.distance_km <= $4
        "#,
    )
    .bind(lat)
    .bind(lng)
    .bind(sport_type)
    .bind(radius_km)
    .fetch_one(pool)
    .await
}

// ── List courts without geo ───────────────────────────────────

pub async fn list_courts_all(
    pool: &PgPool,
    sport_type: Option<SportType>,
    limit: i64,
    offset: i64,
) -> sqlx::Result<Vec<CourtWithDistance>> {
    sqlx::query_as::<_, CourtWithDistance>(
        r#"
        SELECT
            id, name, sport_type, lat, lng, address,
            price_per_slot, photo_urls, owner_id, created_at,
            NULL::double precision AS distance_km
        FROM courts
        WHERE ($1::sport_type IS NULL OR sport_type = $1)
        ORDER BY created_at DESC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(sport_type)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
}

pub async fn count_courts_all(
    pool: &PgPool,
    sport_type: Option<SportType>,
) -> sqlx::Result<i64> {
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM courts WHERE ($1::sport_type IS NULL OR sport_type = $1)",
    )
    .bind(sport_type)
    .fetch_one(pool)
    .await
}

// ── Single court ──────────────────────────────────────────────

pub async fn find_court_by_id(pool: &PgPool, court_id: Uuid) -> sqlx::Result<CourtRow> {
    sqlx::query_as::<_, CourtRow>(
        "SELECT id, name, sport_type, lat, lng, address, price_per_slot, photo_urls, owner_id, created_at FROM courts WHERE id = $1",
    )
    .bind(court_id)
    .fetch_one(pool)
    .await
}

// ── Slots ─────────────────────────────────────────────────────

pub async fn find_slot_by_id(pool: &PgPool, slot_id: Uuid) -> sqlx::Result<CourtSlotRow> {
    sqlx::query_as::<_, CourtSlotRow>(
        "SELECT id, court_id, start_time, end_time, status, booked_by, created_at FROM court_slots WHERE id = $1",
    )
    .bind(slot_id)
    .fetch_one(pool)
    .await
}

pub async fn list_slots_by_court(pool: &PgPool, court_id: Uuid) -> sqlx::Result<Vec<CourtSlotRow>> {
    sqlx::query_as::<_, CourtSlotRow>(
        "SELECT id, court_id, start_time, end_time, status, booked_by, created_at FROM court_slots WHERE court_id = $1 ORDER BY start_time",
    )
    .bind(court_id)
    .fetch_all(pool)
    .await
}

pub async fn list_slots_by_court_and_date(
    pool: &PgPool,
    court_id: Uuid,
    date: NaiveDate,
) -> sqlx::Result<Vec<CourtSlotRow>> {
    sqlx::query_as::<_, CourtSlotRow>(
        r#"
        SELECT id, court_id, start_time, end_time, status, booked_by, created_at
        FROM court_slots
        WHERE court_id = $1
          AND start_time >= $2
          AND start_time < $2 + INTERVAL '1 day'
        ORDER BY start_time
        "#,
    )
    .bind(court_id)
    .bind(date)
    .fetch_all(pool)
    .await
}

// ── Booking (transaction helpers — call within a transaction) ─

pub async fn find_slot_for_update(
    conn: &mut sqlx::PgConnection,
    slot_id: Uuid,
    court_id: Uuid,
) -> sqlx::Result<CourtSlotRow> {
    // Set per-transaction timeout so SELECT FOR UPDATE doesn't block forever.
    sqlx::query("SET LOCAL statement_timeout = '5s'")
        .execute(&mut *conn)
        .await?;

    sqlx::query_as::<_, CourtSlotRow>(
        "SELECT id, court_id, start_time, end_time, status, booked_by, created_at FROM court_slots WHERE id = $1 AND court_id = $2 FOR UPDATE",
    )
    .bind(slot_id)
    .bind(court_id)
    .fetch_one(&mut *conn)
    .await
}

pub async fn update_slot_status(
    conn: &mut sqlx::PgConnection,
    slot_id: Uuid,
    status: SlotStatus,
    booked_by: Option<Uuid>,
) -> sqlx::Result<CourtSlotRow> {
    sqlx::query_as::<_, CourtSlotRow>(
        r#"UPDATE court_slots
           SET status = $2, booked_by = $3
           WHERE id = $1
           RETURNING id, court_id, start_time, end_time, status, booked_by, created_at"#,
    )
    .bind(slot_id)
    .bind(status)
    .bind(booked_by)
    .fetch_one(&mut *conn)
    .await
}
