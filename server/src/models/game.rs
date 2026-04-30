use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use validator::Validate;

use super::court::{Pagination, SportType};
use super::query;
use super::payment::PaymentStatus;

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
    #[validate(range(min = 2, max = 30))]
    pub max_players: i32,
    #[validate(length(max = 500, message = "Description must be under 500 characters"))]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ListGamesQuery {
    pub sport_type: Option<SportType>,
    pub status: Option<GameStatus>,
    pub date: Option<NaiveDate>,
    #[serde(default, deserialize_with = "query::deserialize_f64_from_str")]
    pub lat: Option<f64>,
    #[serde(default, deserialize_with = "query::deserialize_f64_from_str")]
    pub lng: Option<f64>,
    #[serde(default, deserialize_with = "query::deserialize_f64_from_str")]
    #[validate(range(min = 0.1, max = 50.0, message = "radius_km must be between 0.1 and 50"))]
    pub radius_km: Option<f64>,
    #[serde(flatten)]
    pub pagination: Pagination,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateStatusRequest {
    pub status: GameStatus,
}

// ── Game list response types ───────────────────────────────────

/// Flat row from JOIN query for game listing.
#[derive(Debug, FromRow)]
pub struct GameListRow {
    pub id: Uuid,
    pub sport_type: SportType,
    pub max_players: i16,
    pub status: GameStatus,
    pub current_players: i64,
    pub court_id: Uuid,
    pub court_name: String,
    pub court_address: String,
    pub distance_km: Option<f64>,
    pub slot_start_time: DateTime<Utc>,
    pub slot_end_time: DateTime<Utc>,
    pub creator_display_name: Option<String>,
    pub creator_avatar_url: Option<String>,
}

/// Nested game item in list response.
#[derive(Debug, Serialize)]
pub struct GameListItem {
    pub id: Uuid,
    pub sport_type: SportType,
    pub court: GameCourtBrief,
    pub slot: GameSlotBrief,
    pub max_players: i16,
    pub current_players: i64,
    pub status: GameStatus,
    pub creator: GameCreatorInfo,
}

/// `GET /games` response wrapper.
#[derive(Debug, Serialize)]
pub struct GameListResponse {
    pub games: Vec<GameListItem>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

/// Brief court info nested in game list items.
#[derive(Debug, Serialize)]
pub struct GameCourtBrief {
    pub id: Uuid,
    pub name: String,
    pub address: String,
    pub distance_km: Option<f64>,
}

/// Slot info nested in game responses.
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct GameSlotBrief {
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
}

/// Creator info nested in game list items.
#[derive(Debug, Serialize)]
pub struct GameCreatorInfo {
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}

impl From<GameListRow> for GameListItem {
    fn from(row: GameListRow) -> Self {
        Self {
            id: row.id,
            sport_type: row.sport_type,
            court: GameCourtBrief {
                id: row.court_id,
                name: row.court_name,
                address: row.court_address,
                distance_km: row.distance_km,
            },
            slot: GameSlotBrief {
                start_time: row.slot_start_time,
                end_time: row.slot_end_time,
            },
            max_players: row.max_players,
            current_players: row.current_players,
            status: row.status,
            creator: GameCreatorInfo {
                display_name: row.creator_display_name,
                avatar_url: row.creator_avatar_url,
            },
        }
    }
}

// ── Game detail response types ─────────────────────────────────

/// Full court info nested in game detail.
#[derive(Debug, Serialize, FromRow)]
pub struct GameCourtFull {
    pub id: Uuid,
    pub name: String,
    pub address: String,
    pub price_per_slot: i32,
}

/// Player with profile + payment info nested in game detail.
#[derive(Debug, Serialize, FromRow)]
pub struct PlayerWithProfile {
    pub user_id: Uuid,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub joined_at: DateTime<Utc>,
    pub payment_status: Option<PaymentStatus>,
}

/// `GET /games/:id` response — nested court, slot, players.
#[derive(Debug, Serialize)]
pub struct GameDetail {
    #[serde(flatten)]
    pub game: GameRow,
    pub court: GameCourtFull,
    pub slot: GameSlotBrief,
    pub players: Vec<PlayerWithProfile>,
    pub split: Option<BillSplit>,
}

// ── Bill split types ───────────────────────────────────────────

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
