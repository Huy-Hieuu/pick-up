use axum::{
    extract::{Path, Query, State},
    routing::{get, patch, post},
    Json, Router,
};
use uuid::Uuid;

use crate::error::AppResult;
use crate::extractors::AuthUser;
use crate::models::game::{CreateGameRequest, GameDetail, GameRow, ListGamesQuery, UpdateStatusRequest};
use crate::services::{GameService, BillSplitService};
use crate::models::game::BillSplit;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_games).post(create_game))
        .route("/{game_id}", get(get_game))
        .route("/{game_id}/join", post(join_game))
        .route("/{game_id}/leave", post(leave_game))
        .route("/{game_id}/cancel", post(cancel_game))
        .route("/{game_id}/status", patch(update_status))
        .route("/{game_id}/split", get(get_split))
        .route("/{game_id}/share", get(get_share_link))
}

/// `GET /games` — list open games.
async fn list_games(
    State(state): State<AppState>,
    Query(query): Query<ListGamesQuery>,
) -> AppResult<Json<Vec<GameRow>>> {
    let games = GameService::list_games(&state.pool, query).await?;
    Ok(Json(games))
}

/// `POST /games` — create a new game.
async fn create_game(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<CreateGameRequest>,
) -> AppResult<Json<GameDetail>> {
    let game = GameService::create_game(&state.pool, auth.user_id(), req).await?;
    Ok(Json(game))
}

/// `GET /games/:id` — get game detail.
async fn get_game(
    State(state): State<AppState>,
    Path(game_id): Path<Uuid>,
) -> AppResult<Json<GameDetail>> {
    let game = GameService::get_game(&state.pool, game_id).await?;
    Ok(Json(game))
}

/// `POST /games/:id/join` — join a game.
async fn join_game(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(game_id): Path<Uuid>,
) -> AppResult<Json<GameDetail>> {
    let game = GameService::join_game(&state.pool, game_id, auth.user_id()).await?;
    Ok(Json(game))
}

/// `POST /games/:id/leave` — leave a game.
async fn leave_game(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(game_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    GameService::leave_game(&state.pool, game_id, auth.user_id()).await?;
    Ok(Json(serde_json::json!({ "message": "Left game" })))
}

/// `POST /games/:id/cancel` — cancel a game (creator only).
async fn cancel_game(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(game_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    GameService::cancel_game(&state.pool, game_id, auth.user_id()).await?;
    Ok(Json(serde_json::json!({ "message": "Game cancelled" })))
}

/// `PATCH /games/:id/status` — update game status.
async fn update_status(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(game_id): Path<Uuid>,
    Json(req): Json<UpdateStatusRequest>,
) -> AppResult<Json<GameRow>> {
    let game = GameService::update_status(&state.pool, game_id, auth.user_id(), req.status).await?;
    Ok(Json(game))
}

/// `GET /games/:id/split` — get current bill split.
async fn get_split(
    State(state): State<AppState>,
    Path(game_id): Path<Uuid>,
) -> AppResult<Json<BillSplit>> {
    let split = BillSplitService::calculate(&state.pool, game_id).await?;
    Ok(Json(split))
}

/// `GET /games/:id/share` — generate share deeplink.
async fn get_share_link(
    State(_state): State<AppState>,
    Path(game_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    // TODO: Generate Zalo deeplink for game sharing
    Ok(Json(serde_json::json!({
        "deeplink": format!("pickup://games/{game_id}"),
        "game_id": game_id,
    })))
}
