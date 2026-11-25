//! Caller-scoped schedule recovery, independent of public discovery filters.
use crate::{
    error::AppResult,
    models::{
        court::{Pagination, SlotStatus, SportType},
        game::{GameListItem, GameListResponse, GameListRow},
    },
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, Serialize, FromRow)]
pub struct BookingItem {
    pub slot_id: Uuid,
    pub court_id: Uuid,
    pub court_name: String,
    pub sport_type: SportType,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub status: SlotStatus,
    pub booked_by: Uuid,
    pub game_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct BookingPage {
    pub bookings: Vec<BookingItem>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

pub async fn bookings(pool: &PgPool, user: Uuid, pagination: Pagination) -> AppResult<BookingPage> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx)
        .await?;
    let bookings = sqlx::query_as::<_, BookingItem>(
        "SELECT s.id AS slot_id, s.court_id, c.name AS court_name, c.sport_type, s.start_time, s.end_time, s.status, s.booked_by, g.id AS game_id
         FROM court_slots s JOIN courts c ON c.id = s.court_id
         LEFT JOIN games g ON g.court_slot_id = s.id AND g.status != 'cancelled'
         WHERE s.booked_by = $1
         ORDER BY (s.end_time >= now()) DESC, CASE WHEN s.end_time >= now() THEN s.start_time END ASC, s.start_time DESC, s.id
         LIMIT $2 OFFSET $3")
        .bind(user).bind(pagination.effective_limit()).bind(pagination.offset()).fetch_all(&mut *tx).await?;
    let total = sqlx::query_scalar("SELECT count(*) FROM court_slots WHERE booked_by = $1")
        .bind(user)
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(BookingPage {
        bookings,
        total,
        page: pagination.page(),
        per_page: pagination.effective_limit(),
    })
}

pub async fn games(
    pool: &PgPool,
    user: Uuid,
    pagination: Pagination,
) -> AppResult<GameListResponse> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx)
        .await?;
    let scope = "g.creator_id = $1 OR EXISTS (SELECT 1 FROM game_players WHERE game_id = g.id AND user_id = $1) OR EXISTS (SELECT 1 FROM payments WHERE game_id = g.id AND user_id = $1 AND status = 'paid')";
    let query = format!("SELECT g.id, g.sport_type, g.max_players, g.status,
        (SELECT count(*) FROM game_players WHERE game_id = g.id) AS current_players,
        c.id AS court_id, c.name AS court_name, c.address AS court_address, NULL::double precision AS distance_km,
        s.start_time AS slot_start_time, s.end_time AS slot_end_time, u.display_name AS creator_display_name, u.avatar_url AS creator_avatar_url
        FROM games g JOIN court_slots s ON s.id = g.court_slot_id JOIN courts c ON c.id = s.court_id JOIN users u ON u.id = g.creator_id
        WHERE {scope} ORDER BY s.start_time DESC, g.id LIMIT $2 OFFSET $3");
    let rows = sqlx::query_as::<_, GameListRow>(&query)
        .bind(user)
        .bind(pagination.effective_limit())
        .bind(pagination.offset())
        .fetch_all(&mut *tx)
        .await?;
    let total = sqlx::query_scalar(&format!("SELECT count(*) FROM games g WHERE {scope}"))
        .bind(user)
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(GameListResponse {
        games: rows.into_iter().map(GameListItem::from).collect(),
        total,
        page: pagination.page(),
        per_page: pagination.effective_limit(),
    })
}
