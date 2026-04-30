use axum::{
    extract::{Path, State},
    routing::{get, patch, post},
    Json, Router,
};
use uuid::Uuid;

use crate::error::AppResult;
use crate::extractors::{AuthUser, ValidatedJson, ValidatedQuery};
use crate::models::court::SportType;
use crate::models::game::{
    BillSplit, CreateGameRequest, GameDetail, GameListResponse, GameRow, ListGamesQuery,
    UpdateStatusRequest,
};
use crate::services::{BillSplitService, GameService};
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
    ValidatedQuery(query): ValidatedQuery<ListGamesQuery>,
) -> AppResult<Json<GameListResponse>> {
    let games = GameService::list_games(&state.pool, query).await?;
    Ok(Json(games))
}

/// `POST /games` — create a new game.
async fn create_game(
    State(state): State<AppState>,
    auth: AuthUser,
    ValidatedJson(req): ValidatedJson<CreateGameRequest>,
) -> AppResult<Json<GameDetail>> {
    let game = GameService::create_game(&state.pool, auth.user_id(), req).await?;
    Ok(Json(game))
}

/// `GET /games/:id` — get game detail.
async fn get_game(
    State(state): State<AppState>,
    _auth: AuthUser,
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
    ValidatedJson(req): ValidatedJson<UpdateStatusRequest>,
) -> AppResult<Json<GameRow>> {
    let game = GameService::update_status(&state.pool, game_id, auth.user_id(), req.status).await?;
    Ok(Json(game))
}

/// `GET /games/:id/split` — get current bill split.
async fn get_split(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(game_id): Path<Uuid>,
) -> AppResult<Json<BillSplit>> {
    let split = BillSplitService::calculate(&state.pool, game_id).await?;
    Ok(Json(split))
}

/// `GET /games/:id/share` — generate share deeplink.
async fn get_share_link(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(game_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let game = GameService::get_game(&state.pool, game_id).await?;
    let sport_name = sport_display_name(game.game.sport_type);
    let time_str = format!(
        "{} {}",
        game.slot.start_time.format("%l").to_string().trim(),
        game.slot.start_time.format("%p")
    );
    Ok(Json(serde_json::json!({
        "url": format!("https://pickup.app/game/{game_id}"),
        "message": format!(
            "Join my {} game at {} on {} at {}! {}/{} spots left.",
            sport_name,
            game.court.name,
            game.slot.start_time.format("%b %-d"),
            time_str,
            game.players.len(),
            game.game.max_players,
        ),
    })))
}

fn sport_display_name(sport: SportType) -> &'static str {
    match sport {
        SportType::Pickleball => "pickleball",
        SportType::MiniFootball => "mini football",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── sport_display_name ─────────────────────────────────────────

    #[test]
    fn sport_display_name_pickleball() {
        assert_eq!(sport_display_name(SportType::Pickleball), "pickleball");
    }

    #[test]
    fn sport_display_name_mini_football() {
        assert_eq!(sport_display_name(SportType::MiniFootball), "mini football");
    }

    #[test]
    fn sport_display_name_is_lowercase_no_underscores() {
        // Verify the output has no PascalCase and no underscores (unlike Debug format)
        for sport in [SportType::Pickleball, SportType::MiniFootball] {
            let name = sport_display_name(sport);
            assert_eq!(name, name.to_lowercase());
            assert!(!name.contains('_'), "Should use spaces not underscores");
        }
    }
}
