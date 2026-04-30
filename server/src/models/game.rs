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
    #[validate(length(max = 500, message = "Description must be under 500 characters"))]
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

#[cfg(test)]
mod tests {
    use super::*;
    use validator::Validate;

    // ── CreateGameRequest ──────────────────────────────────────────

    #[test]
    fn create_game_valid() {
        let req = CreateGameRequest {
            court_slot_id: Uuid::new_v4(),
            sport_type: SportType::Pickleball,
            max_players: 10,
            description: Some("Friendly match".into()),
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn create_game_min_players() {
        let req = CreateGameRequest {
            court_slot_id: Uuid::new_v4(),
            sport_type: SportType::MiniFootball,
            max_players: 2, // minimum
            description: None,
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn create_game_below_min_players() {
        let req = CreateGameRequest {
            court_slot_id: Uuid::new_v4(),
            sport_type: SportType::Pickleball,
            max_players: 1, // below min of 2
            description: None,
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn create_game_max_players() {
        let req = CreateGameRequest {
            court_slot_id: Uuid::new_v4(),
            sport_type: SportType::Pickleball,
            max_players: 50, // maximum
            description: None,
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn create_game_above_max_players() {
        let req = CreateGameRequest {
            court_slot_id: Uuid::new_v4(),
            sport_type: SportType::Pickleball,
            max_players: 51, // above max of 50
            description: None,
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn create_game_description_max_length() {
        let req = CreateGameRequest {
            court_slot_id: Uuid::new_v4(),
            sport_type: SportType::Pickleball,
            max_players: 4,
            description: Some("x".repeat(500)), // exactly 500
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn create_game_description_too_long() {
        let req = CreateGameRequest {
            court_slot_id: Uuid::new_v4(),
            sport_type: SportType::Pickleball,
            max_players: 4,
            description: Some("x".repeat(501)), // over limit
        };
        assert!(req.validate().is_err());
    }

    // ── GameStatus ─────────────────────────────────────────────────

    #[test]
    fn game_status_serde_roundtrip() {
        let statuses = vec![GameStatus::Open, GameStatus::Full, GameStatus::InProgress, GameStatus::Completed, GameStatus::Cancelled];
        for status in statuses {
            let json = serde_json::to_string(&status).unwrap();
            let parsed: GameStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, parsed);
        }
    }

    #[test]
    fn game_status_serializes_to_snake_case() {
        assert_eq!(serde_json::to_string(&GameStatus::InProgress).unwrap(), "\"in_progress\"");
        assert_eq!(serde_json::to_string(&GameStatus::Open).unwrap(), "\"open\"");
    }

    // ── SportType ──────────────────────────────────────────────────

    #[test]
    fn sport_type_serializes_to_snake_case() {
        assert_eq!(serde_json::to_string(&SportType::MiniFootball).unwrap(), "\"mini_football\"");
        assert_eq!(serde_json::to_string(&SportType::Pickleball).unwrap(), "\"pickleball\"");
    }
}
