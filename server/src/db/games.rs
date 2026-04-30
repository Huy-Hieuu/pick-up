use chrono::{DateTime, NaiveDate, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::court::SportType;
use crate::models::game::{
    GameCourtFull, GameListRow, GameRow, GameSlotBrief, GameStatus, PlayerWithProfile,
};

// ── Reads (&PgPool) ───────────────────────────────────────────

pub async fn find_game_by_id(pool: &PgPool, game_id: Uuid) -> sqlx::Result<GameRow> {
    sqlx::query_as::<_, GameRow>(
        r#"SELECT id, court_slot_id, creator_id, sport_type, max_players, description, status, created_at
           FROM games WHERE id = $1"#,
    )
    .bind(game_id)
    .fetch_one(pool)
    .await
}

/// List games with JOIN (no geo) — returns flat `GameListRow`.
pub async fn list_games_all_joined(
    pool: &PgPool,
    sport_type: Option<SportType>,
    status: Option<GameStatus>,
    date: Option<NaiveDate>,
    limit: i64,
    offset: i64,
) -> sqlx::Result<Vec<GameListRow>> {
    sqlx::query_as::<_, GameListRow>(
        r#"
        SELECT
            g.id, g.sport_type, g.max_players, g.status,
            COALESCE(pc.cnt, 0) AS current_players,
            c.id AS court_id, c.name AS court_name, c.address AS court_address,
            NULL::double precision AS distance_km,
            cs.start_time AS slot_start_time, cs.end_time AS slot_end_time,
            u.display_name AS creator_display_name, u.avatar_url AS creator_avatar_url
        FROM games g
        JOIN court_slots cs ON cs.id = g.court_slot_id
        JOIN courts c ON c.id = cs.court_id
        JOIN users u ON u.id = g.creator_id
        LEFT JOIN (SELECT game_id, COUNT(*)::int8 AS cnt FROM game_players GROUP BY game_id) pc ON pc.game_id = g.id
        WHERE (
            ($1::game_status IS NULL AND g.status IN ('open', 'full'))
            OR ($1::game_status IS NOT NULL AND g.status = $1)
        )
          AND ($2::sport_type IS NULL OR g.sport_type = $2)
          AND ($3::date IS NULL OR (cs.start_time >= $3 AND cs.start_time < $3 + INTERVAL '1 day'))
        ORDER BY g.created_at DESC
        LIMIT $4 OFFSET $5
        "#,
    )
    .bind(status)
    .bind(sport_type)
    .bind(date)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
}

/// List games with JOIN + Haversine geo filter.
#[allow(clippy::too_many_arguments)]
pub async fn list_games_near_joined(
    pool: &PgPool,
    sport_type: Option<SportType>,
    status: Option<GameStatus>,
    date: Option<NaiveDate>,
    lat: f64,
    lng: f64,
    radius_km: f64,
    limit: i64,
    offset: i64,
) -> sqlx::Result<Vec<GameListRow>> {
    sqlx::query_as::<_, GameListRow>(
        r#"
        SELECT * FROM (
            SELECT
                g.id, g.sport_type, g.max_players, g.status,
                COALESCE(pc.cnt, 0) AS current_players,
                c.id AS court_id, c.name AS court_name, c.address AS court_address,
                (6371 * acos(LEAST(1.0,
                    cos(radians($1)) * cos(radians(c.lat)) *
                    cos(radians(c.lng) - radians($2)) +
                    sin(radians($1)) * sin(radians(c.lat))
                ))) AS distance_km,
                cs.start_time AS slot_start_time, cs.end_time AS slot_end_time,
                u.display_name AS creator_display_name, u.avatar_url AS creator_avatar_url
            FROM games g
            JOIN court_slots cs ON cs.id = g.court_slot_id
            JOIN courts c ON c.id = cs.court_id
            JOIN users u ON u.id = g.creator_id
            LEFT JOIN (SELECT game_id, COUNT(*)::int8 AS cnt FROM game_players GROUP BY game_id) pc ON pc.game_id = g.id
            WHERE (
                ($3::game_status IS NULL AND g.status IN ('open', 'full'))
                OR ($3::game_status IS NOT NULL AND g.status = $3)
            )
              AND ($4::sport_type IS NULL OR g.sport_type = $4)
              AND ($5::date IS NULL OR (cs.start_time >= $5 AND cs.start_time < $5 + INTERVAL '1 day'))
        ) sub
        WHERE sub.distance_km <= $6
        ORDER BY sub.distance_km ASC
        LIMIT $7 OFFSET $8
        "#,
    )
    .bind(lat)
    .bind(lng)
    .bind(status)
    .bind(sport_type)
    .bind(date)
    .bind(radius_km)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
}

/// Count games without geo filter.
pub async fn count_games_all_filtered(
    pool: &PgPool,
    sport_type: Option<SportType>,
    status: Option<GameStatus>,
    date: Option<NaiveDate>,
) -> sqlx::Result<i64> {
    sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*) FROM games g
        JOIN court_slots cs ON cs.id = g.court_slot_id
        WHERE (
            ($1::game_status IS NULL AND g.status IN ('open', 'full'))
            OR ($1::game_status IS NOT NULL AND g.status = $1)
        )
          AND ($2::sport_type IS NULL OR g.sport_type = $2)
          AND ($3::date IS NULL OR (cs.start_time >= $3 AND cs.start_time < $3 + INTERVAL '1 day'))
        "#,
    )
    .bind(status)
    .bind(sport_type)
    .bind(date)
    .fetch_one(pool)
    .await
}

/// Count games with Haversine geo filter.
#[allow(clippy::too_many_arguments)]
pub async fn count_games_near_filtered(
    pool: &PgPool,
    sport_type: Option<SportType>,
    status: Option<GameStatus>,
    date: Option<NaiveDate>,
    lat: f64,
    lng: f64,
    radius_km: f64,
) -> sqlx::Result<i64> {
    sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*) FROM (
            SELECT g.id,
                (6371 * acos(LEAST(1.0,
                    cos(radians($1)) * cos(radians(c.lat)) *
                    cos(radians(c.lng) - radians($2)) +
                    sin(radians($1)) * sin(radians(c.lat))
                ))) AS distance_km
            FROM games g
            JOIN court_slots cs ON cs.id = g.court_slot_id
            JOIN courts c ON c.id = cs.court_id
            WHERE (
                ($3::game_status IS NULL AND g.status IN ('open', 'full'))
                OR ($3::game_status IS NOT NULL AND g.status = $3)
            )
              AND ($4::sport_type IS NULL OR g.sport_type = $4)
              AND ($5::date IS NULL OR (cs.start_time >= $5 AND cs.start_time < $5 + INTERVAL '1 day'))
        ) sub
        WHERE sub.distance_km <= $6
        "#,
    )
    .bind(lat)
    .bind(lng)
    .bind(status)
    .bind(sport_type)
    .bind(date)
    .bind(radius_km)
    .fetch_one(pool)
    .await
}

/// Get court info for a specific game.
pub async fn find_game_court(pool: &PgPool, game_id: Uuid) -> sqlx::Result<GameCourtFull> {
    sqlx::query_as::<_, GameCourtFull>(
        r#"
        SELECT c.id, c.name, c.address, c.price_per_slot
        FROM courts c
        JOIN court_slots cs ON cs.court_id = c.id
        JOIN games g ON g.court_slot_id = cs.id
        WHERE g.id = $1
        "#,
    )
    .bind(game_id)
    .fetch_one(pool)
    .await
}

/// Get slot info for a specific game.
pub async fn find_game_slot(pool: &PgPool, game_id: Uuid) -> sqlx::Result<GameSlotBrief> {
    sqlx::query_as::<_, GameSlotBrief>(
        r#"
        SELECT cs.start_time, cs.end_time
        FROM court_slots cs
        JOIN games g ON g.court_slot_id = cs.id
        WHERE g.id = $1
        "#,
    )
    .bind(game_id)
    .fetch_one(pool)
    .await
}

/// List players with user profile and latest payment status.
pub async fn list_players_with_profile(
    pool: &PgPool,
    game_id: Uuid,
) -> sqlx::Result<Vec<PlayerWithProfile>> {
    sqlx::query_as::<_, PlayerWithProfile>(
        r#"
        SELECT
            gp.user_id,
            u.display_name,
            u.avatar_url,
            gp.joined_at,
            p.status AS payment_status
        FROM game_players gp
        JOIN users u ON u.id = gp.user_id
        LEFT JOIN LATERAL (
            SELECT status FROM payments
            WHERE game_id = gp.game_id AND user_id = gp.user_id
            ORDER BY created_at DESC LIMIT 1
        ) p ON true
        WHERE gp.game_id = $1
        ORDER BY gp.joined_at
        "#,
    )
    .bind(game_id)
    .fetch_all(pool)
    .await
}

// ── Transactional reads + writes (&mut PgConnection) ──────────

/// Lock a game row for update. Re-reads fresh status within the tx.
pub async fn find_game_for_update(
    conn: &mut sqlx::PgConnection,
    game_id: Uuid,
) -> sqlx::Result<GameRow> {
    sqlx::query("SET LOCAL statement_timeout = '5s'")
        .execute(&mut *conn)
        .await?;

    sqlx::query_as::<_, GameRow>(
        r#"SELECT id, court_slot_id, creator_id, sport_type, max_players, description, status, created_at
           FROM games WHERE id = $1 FOR UPDATE"#,
    )
    .bind(game_id)
    .fetch_one(&mut *conn)
    .await
}

pub async fn find_game_player_tx(
    conn: &mut sqlx::PgConnection,
    game_id: Uuid,
    user_id: Uuid,
) -> sqlx::Result<Option<crate::models::game::GamePlayerRow>> {
    sqlx::query_as::<_, crate::models::game::GamePlayerRow>(
        "SELECT game_id, user_id, joined_at FROM game_players WHERE game_id = $1 AND user_id = $2",
    )
    .bind(game_id)
    .bind(user_id)
    .fetch_optional(&mut *conn)
    .await
}

pub async fn count_game_players_tx(
    conn: &mut sqlx::PgConnection,
    game_id: Uuid,
) -> sqlx::Result<i64> {
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM game_players WHERE game_id = $1",
    )
    .bind(game_id)
    .fetch_one(&mut *conn)
    .await
}

pub async fn game_exists_for_slot_tx(
    conn: &mut sqlx::PgConnection,
    court_slot_id: Uuid,
) -> sqlx::Result<bool> {
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM games WHERE court_slot_id = $1 AND status != 'cancelled')",
    )
    .bind(court_slot_id)
    .fetch_one(&mut *conn)
    .await
}

/// Check if user has a game whose slot overlaps with the given time range.
/// Excludes the specified game_id from the check.
pub async fn has_overlapping_game_tx(
    conn: &mut sqlx::PgConnection,
    user_id: Uuid,
    exclude_game_id: Uuid,
    slot_start: DateTime<Utc>,
    slot_end: DateTime<Utc>,
) -> sqlx::Result<bool> {
    sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM game_players gp
            JOIN games g ON g.id = gp.game_id
            JOIN court_slots cs ON cs.id = g.court_slot_id
            WHERE gp.user_id = $1
              AND g.id != $2
              AND g.status IN ('open', 'full', 'in_progress')
              AND cs.start_time < $4
              AND cs.end_time > $3
        )
        "#,
    )
    .bind(user_id)
    .bind(exclude_game_id)
    .bind(slot_start)
    .bind(slot_end)
    .fetch_one(&mut *conn)
    .await
}

/// Check if an active game exists for a slot (used by cancel_booking).
pub async fn has_active_game_for_slot_tx(
    conn: &mut sqlx::PgConnection,
    slot_id: Uuid,
) -> sqlx::Result<bool> {
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM games WHERE court_slot_id = $1 AND status IN ('open', 'full', 'in_progress'))",
    )
    .bind(slot_id)
    .fetch_one(&mut *conn)
    .await
}

pub async fn insert_game(
    conn: &mut sqlx::PgConnection,
    court_slot_id: Uuid,
    creator_id: Uuid,
    sport_type: SportType,
    max_players: i16,
    description: Option<&str>,
) -> sqlx::Result<GameRow> {
    sqlx::query_as::<_, GameRow>(
        r#"INSERT INTO games (court_slot_id, creator_id, sport_type, max_players, description)
           VALUES ($1, $2, $3, $4, $5)
           RETURNING id, court_slot_id, creator_id, sport_type, max_players, description, status, created_at"#,
    )
    .bind(court_slot_id)
    .bind(creator_id)
    .bind(sport_type)
    .bind(max_players)
    .bind(description)
    .fetch_one(&mut *conn)
    .await
}

pub async fn insert_game_player(
    conn: &mut sqlx::PgConnection,
    game_id: Uuid,
    user_id: Uuid,
) -> sqlx::Result<crate::models::game::GamePlayerRow> {
    sqlx::query_as::<_, crate::models::game::GamePlayerRow>(
        r#"INSERT INTO game_players (game_id, user_id)
           VALUES ($1, $2)
           RETURNING game_id, user_id, joined_at"#,
    )
    .bind(game_id)
    .bind(user_id)
    .fetch_one(&mut *conn)
    .await
}

/// Remove a player from a game. Returns error if no row was deleted.
pub async fn remove_game_player(
    conn: &mut sqlx::PgConnection,
    game_id: Uuid,
    user_id: Uuid,
) -> sqlx::Result<()> {
    let result = sqlx::query("DELETE FROM game_players WHERE game_id = $1 AND user_id = $2")
        .bind(game_id)
        .bind(user_id)
        .execute(&mut *conn)
        .await?;

    if result.rows_affected() == 0 {
        return Err(sqlx::Error::RowNotFound);
    }
    Ok(())
}

pub async fn set_game_status(
    conn: &mut sqlx::PgConnection,
    game_id: Uuid,
    status: GameStatus,
) -> sqlx::Result<GameRow> {
    sqlx::query_as::<_, GameRow>(
        r#"UPDATE games SET status = $2 WHERE id = $1
           RETURNING id, court_slot_id, creator_id, sport_type, max_players, description, status, created_at"#,
    )
    .bind(game_id)
    .bind(status)
    .fetch_one(&mut *conn)
    .await
}
