use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppResult;
use crate::models::game::{CreateGameRequest, GameDetail, GameRow, GameStatus, ListGamesQuery};

/// Game service.
///
/// All business logic related to:
/// - Game creation, joining, leaving
/// - Game status transitions
/// - Player management
pub struct GameService;

impl GameService {
    /// List open games with optional filters.
    pub async fn list_games(_pool: &PgPool, _query: ListGamesQuery) -> AppResult<Vec<GameRow>> {
        // TODO: SELECT with filters, use query.pagination.offset()/effective_limit()
        tracing::info!("List games requested (stub)");
        Err(crate::error::AppError::BadRequest("Not implemented".into()))
    }

    /// Create a new game. The creator is automatically added as the first player.
    pub async fn create_game(
        _pool: &PgPool,
        creator_id: Uuid,
        _req: CreateGameRequest,
    ) -> AppResult<GameDetail> {
        // TODO:
        // 1. Verify court_slot is booked by creator
        // 2. INSERT into games
        // 3. INSERT creator into game_players
        // 4. Broadcast via WebSocket
        tracing::info!(%creator_id, "Create game requested (stub)");
        Err(crate::error::AppError::BadRequest("Not implemented".into()))
    }

    /// Get game detail with players and bill split.
    pub async fn get_game(_pool: &PgPool, game_id: Uuid) -> AppResult<GameDetail> {
        // TODO: SELECT game + JOIN players
        tracing::info!(%game_id, "Get game requested (stub)");
        Err(crate::error::AppError::NotFound("Game not found".into()))
    }

    /// Join a game. Fails if game is full or already joined.
    pub async fn join_game(_pool: &PgPool, game_id: Uuid, user_id: Uuid) -> AppResult<GameDetail> {
        // TODO:
        // 1. Check game status & player count
        // 2. INSERT into game_players
        // 3. Update game status if now full
        // 4. Broadcast player_joined via WebSocket
        tracing::info!(%game_id, %user_id, "Join game requested (stub)");
        Err(crate::error::AppError::BadRequest("Not implemented".into()))
    }

    /// Leave a game. Creator leaving cancels the game.
    pub async fn leave_game(_pool: &PgPool, game_id: Uuid, user_id: Uuid) -> AppResult<()> {
        // TODO:
        // 1. Check if player is in game
        // 2. If creator → cancel game, refund payments
        // 3. Else → remove from game_players, recalculate split
        // 4. Broadcast player_left via WebSocket
        tracing::info!(%game_id, %user_id, "Leave game requested (stub)");
        Err(crate::error::AppError::BadRequest("Not implemented".into()))
    }

    /// Cancel a game (creator only).
    pub async fn cancel_game(_pool: &PgPool, game_id: Uuid, creator_id: Uuid) -> AppResult<()> {
        // TODO: Set status = cancelled, broadcast game_cancelled
        tracing::info!(%game_id, %creator_id, "Cancel game requested (stub)");
        Err(crate::error::AppError::BadRequest("Not implemented".into()))
    }

    /// Update game status (for state machine transitions).
    /// Caller is responsible for verifying the user is authorized.
    pub async fn update_status(
        _pool: &PgPool,
        game_id: Uuid,
        _caller_id: Uuid,
        status: GameStatus,
    ) -> AppResult<GameRow> {
        // TODO: Validate state transition, UPDATE, broadcast
        tracing::info!(%game_id, ?status, "Update game status requested (stub)");
        Err(crate::error::AppError::BadRequest("Not implemented".into()))
    }
}