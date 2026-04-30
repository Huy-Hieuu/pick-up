use sqlx::PgPool;
use uuid::Uuid;

use crate::db;
use crate::error::{AppError, AppResult};
use crate::models::court::SlotStatus;
use crate::models::game::{
    GameDetail, GameListResponse, GameListItem, GameRow, GameStatus, ListGamesQuery,
};

/// Game service.
///
/// All business logic related to:
/// - Game creation, joining, leaving
/// - Game status transitions (state machine)
/// - Player management with concurrency safety
pub struct GameService;

impl GameService {
    /// List games with optional filters (sport type, status, date, geo radius).
    pub async fn list_games(
        pool: &PgPool,
        query: ListGamesQuery,
    ) -> AppResult<GameListResponse> {
        // Require both lat and lng together, or neither.
        if query.lat.is_some() != query.lng.is_some() {
            return Err(AppError::BadRequest(
                "Both lat and lng are required for geo search".into(),
            ));
        }

        let limit = query.pagination.effective_limit();
        let offset = query.pagination.offset();

        let (rows, total) = match (query.lat, query.lng) {
            (Some(lat), Some(lng)) => {
                let radius = query.radius_km.unwrap_or(10.0);
                let rows = db::games::list_games_near_joined(
                    pool, query.sport_type, query.status, query.date,
                    lat, lng, radius, limit, offset,
                )
                .await?;
                let total = db::games::count_games_near_filtered(
                    pool, query.sport_type, query.status, query.date,
                    lat, lng, radius,
                )
                .await?;
                (rows, total)
            }
            _ => {
                let rows = db::games::list_games_all_joined(
                    pool, query.sport_type, query.status, query.date,
                    limit, offset,
                )
                .await?;
                let total = db::games::count_games_all_filtered(
                    pool, query.sport_type, query.status, query.date,
                )
                .await?;
                (rows, total)
            }
        };

        let games: Vec<GameListItem> = rows.into_iter().map(GameListItem::from).collect();

        Ok(GameListResponse {
            games,
            total,
            page: query.pagination.page(),
            per_page: limit,
        })
    }

    /// Create a new game. The creator is automatically added as the first player.
    ///
    /// Uses SELECT FOR UPDATE on the slot + UNIQUE index on court_slot_id
    /// to prevent duplicate games under concurrent requests.
    pub async fn create_game(
        pool: &PgPool,
        creator_id: Uuid,
        req: crate::models::game::CreateGameRequest,
    ) -> AppResult<GameDetail> {
        // 1. Verify the court slot exists and is booked by the creator.
        let slot = db::courts::find_slot_by_id(pool, req.court_slot_id).await?;

        if slot.status != SlotStatus::Booked {
            return Err(AppError::Conflict("Court slot is not booked".into()));
        }
        if slot.booked_by != Some(creator_id) {
            return Err(AppError::Forbidden(
                "Only the slot booker can create a game".into(),
            ));
        }

        // 2. Begin tx: lock slot, re-validate, check uniqueness, insert game + player.
        let mut tx = pool.begin().await.map_err(AppError::Database)?;

        let locked_slot =
            db::courts::find_slot_for_update(&mut tx, req.court_slot_id, slot.court_id).await?;

        // Re-validate on locked row to prevent race.
        if locked_slot.status != SlotStatus::Booked {
            return Err(AppError::Conflict("Court slot is not booked".into()));
        }
        if locked_slot.booked_by != Some(creator_id) {
            return Err(AppError::Forbidden(
                "Only the slot booker can create a game".into(),
            ));
        }

        // Check uniqueness inside the tx.
        let exists = db::games::game_exists_for_slot_tx(&mut tx, req.court_slot_id).await?;
        if exists {
            return Err(AppError::Conflict(
                "A game already exists for this court slot".into(),
            ));
        }

        let game = db::games::insert_game(
            &mut tx,
            req.court_slot_id,
            creator_id,
            req.sport_type,
            req.max_players as i16,
            req.description.as_deref(),
        )
        .await?;

        let _player = db::games::insert_game_player(&mut tx, game.id, creator_id).await?;

        tx.commit().await.map_err(AppError::Database)?;

        // Fetch court, slot, and players for the response.
        let court = db::games::find_game_court(pool, game.id).await?;
        let slot_brief = db::games::find_game_slot(pool, game.id).await?;
        let players = db::games::list_players_with_profile(pool, game.id).await?;

        Ok(GameDetail {
            game,
            court,
            slot: slot_brief,
            players,
            split: None,
        })
    }

    /// Get game detail with court, slot, and player profiles.
    pub async fn get_game(pool: &PgPool, game_id: Uuid) -> AppResult<GameDetail> {
        let game = db::games::find_game_by_id(pool, game_id).await?;
        let court = db::games::find_game_court(pool, game_id).await?;
        let slot = db::games::find_game_slot(pool, game_id).await?;
        let players = db::games::list_players_with_profile(pool, game_id).await?;
        Ok(GameDetail {
            game,
            court,
            slot,
            players,
            split: None,
        })
    }

    /// Join a game. Uses SELECT FOR UPDATE on the game row to prevent
    /// race conditions (over-capacity, stale status).
    /// Also checks for overlapping game times.
    pub async fn join_game(
        pool: &PgPool,
        game_id: Uuid,
        user_id: Uuid,
    ) -> AppResult<GameDetail> {
        // Look up slot times before tx — slot times are immutable.
        let slot = db::games::find_game_slot(pool, game_id).await?;

        let mut tx = pool.begin().await.map_err(AppError::Database)?;

        // 1. Lock game row + re-read fresh status.
        let game = db::games::find_game_for_update(&mut tx, game_id).await?;
        if game.status != GameStatus::Open {
            return Err(AppError::Conflict("Game is not open for joining".into()));
        }

        // 2. Check user is not already a player (inside tx).
        let existing = db::games::find_game_player_tx(&mut tx, game_id, user_id).await?;
        if existing.is_some() {
            return Err(AppError::Conflict("Already joined this game".into()));
        }

        // 3. Check game is not full (inside tx).
        let player_count = db::games::count_game_players_tx(&mut tx, game_id).await?;
        if player_count >= i64::from(game.max_players) {
            return Err(AppError::Conflict("Game is full".into()));
        }

        // 4. Check for overlapping game times.
        let overlap =
            db::games::has_overlapping_game_tx(&mut tx, user_id, game_id, slot.start_time, slot.end_time)
                .await?;
        if overlap {
            return Err(AppError::Conflict(
                "You have another game at the same time".into(),
            ));
        }

        // 5. Add player.
        db::games::insert_game_player(&mut tx, game_id, user_id).await?;

        // 6. If now full, update status.
        if player_count + 1 >= i64::from(game.max_players) {
            db::games::set_game_status(&mut tx, game_id, GameStatus::Full).await?;
        }

        tx.commit().await.map_err(AppError::Database)?;

        // Fetch fresh state for response.
        Self::get_game(pool, game_id).await
    }

    /// Leave a game. Creator cannot leave (must cancel instead).
    /// All checks happen inside the transaction for consistency.
    pub async fn leave_game(pool: &PgPool, game_id: Uuid, user_id: Uuid) -> AppResult<()> {
        // Quick check outside tx: creator cannot leave.
        let game = db::games::find_game_by_id(pool, game_id).await?;
        if game.creator_id == user_id {
            return Err(AppError::Conflict(
                "Creator cannot leave — cancel the game instead".into(),
            ));
        }

        let mut tx = pool.begin().await.map_err(AppError::Database)?;

        // Lock game row + re-read fresh status inside tx.
        let game = db::games::find_game_for_update(&mut tx, game_id).await?;

        if matches!(
            game.status,
            GameStatus::Cancelled | GameStatus::Completed
        ) {
            return Err(AppError::Conflict(format!(
                "Cannot leave a {:?} game",
                game.status
            )));
        }

        // Check user is a member (inside tx). If not, remove_game_player
        // will return RowNotFound.
        db::games::remove_game_player(&mut tx, game_id, user_id).await?;

        // If game was full, reopen.
        if game.status == GameStatus::Full {
            db::games::set_game_status(&mut tx, game_id, GameStatus::Open).await?;
        }

        tx.commit().await.map_err(AppError::Database)?;
        Ok(())
    }

    /// Cancel a game (creator only).
    pub async fn cancel_game(pool: &PgPool, game_id: Uuid, creator_id: Uuid) -> AppResult<()> {
        let mut tx = pool.begin().await.map_err(AppError::Database)?;

        let game = db::games::find_game_for_update(&mut tx, game_id).await?;

        if game.creator_id != creator_id {
            return Err(AppError::Forbidden(
                "Only the creator can cancel this game".into(),
            ));
        }

        match game.status {
            GameStatus::Cancelled => {
                return Err(AppError::Conflict("Game is already cancelled".into()));
            }
            GameStatus::Completed => {
                return Err(AppError::Conflict("Cannot cancel a completed game".into()));
            }
            _ => {}
        }

        db::games::set_game_status(&mut tx, game_id, GameStatus::Cancelled).await?;
        tx.commit().await.map_err(AppError::Database)?;

        Ok(())
    }

    /// Update game status (state machine transitions). Creator-only.
    pub async fn update_status(
        pool: &PgPool,
        game_id: Uuid,
        caller_id: Uuid,
        new_status: GameStatus,
    ) -> AppResult<GameRow> {
        let mut tx = pool.begin().await.map_err(AppError::Database)?;

        let game = db::games::find_game_for_update(&mut tx, game_id).await?;

        if game.creator_id != caller_id {
            return Err(AppError::Forbidden(
                "Only the creator can change game status".into(),
            ));
        }

        let valid = matches!(
            (&game.status, &new_status),
            (GameStatus::Open | GameStatus::Full, GameStatus::InProgress)
                | (GameStatus::InProgress, GameStatus::Completed)
        );
        if !valid {
            return Err(AppError::Conflict(format!(
                "Cannot transition from {:?} to {:?}",
                game.status, new_status
            )));
        }

        let updated = db::games::set_game_status(&mut tx, game_id, new_status).await?;
        tx.commit().await.map_err(AppError::Database)?;

        Ok(updated)
    }
}
