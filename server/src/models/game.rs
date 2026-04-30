use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use validator::Validate;

use super::court::{SportType, Pagination};

// ── Enums ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "game_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum GameStatus {
    Open,
    Full,
    InProgress,
    Completed,
    Cancelled,
}

// ── Database row types ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct GameRow {
    pub id: Uuid,
    pub court_slot_id: Uuid,
    pub creator_id: Uuid,
    pub sport_type: SportType,
    pub max_players: i16,
    pub description: Option<String>,
    pub status: GameStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct GamePlayerRow {
    pub game_id: Uuid,
    pub user_id: Uuid,
    pub joined_at: DateTime<Utc>,
}

// ── Request DTOs ───────────────────────────────────────────────

#[derive(Debug, Deserialize, Validate)]
pub struct CreateGameRequest {
    pub court_slot_id: Uuid,
    pub sport_type: SportType,
    #[validate(range(min = 2, max = 50))]
    pub max_players: i32,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListGamesQuery {
    pub sport_type: Option<SportType>,
    pub status: Option<GameStatus>,
    #[serde(flatten)]
    pub pagination: Pagination,
}

#[derive(Debug, Deserialize)]
pub struct UpdateStatusRequest {
    pub status: GameStatus,
}

// ── Response DTOs ──────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct GameDetail {
    #[serde(flatten)]
    pub game: GameRow,
    pub players: Vec<GamePlayerRow>,
    pub split: Option<BillSplit>,
}

#[derive(Debug, Serialize)]
pub struct BillSplit {
    pub total_amount: i32,
    pub per_player: i32,
    pub remainder: i32,
    pub creator_pays_extra: bool,
    pub player_count: i32,
    pub payments: Vec<PlayerPayment>,
}

#[derive(Debug, Serialize)]
pub struct PlayerPayment {
    pub user_id: Uuid,
    pub display_name: Option<String>,
    pub amount: i32,
    pub paid: bool,
}
