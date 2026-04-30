use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::game::BillSplit;

/// Result of the pure bill-split calculation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SplitResult {
    pub per_player: i32,
    pub remainder: i32,
    pub creator_pays_extra: bool,
}

/// Bill splitting service.
///
/// Core viral feature — calculates equal split with remainder handling.
/// The creator pays the extra VND when amounts don't divide evenly.
///
/// VND has no decimal places, so rounding is straightforward.
pub struct BillSplitService;

impl BillSplitService {
    /// Calculate the current bill split for a game.
    ///
    /// Split algorithm:
    /// 1. Get court slot price (total_amount)
    /// 2. Divide by number of joined players
    /// 3. Remainder goes to the creator
    /// 4. Check payment status for each player
    pub async fn calculate(_pool: &PgPool, game_id: Uuid) -> AppResult<BillSplit> {
        // TODO:
        // 1. SELECT game with slot price
        // 2. Count players
        // 3. calculate_split(total, count)
        // 4. creator pays per_player + remainder
        // 5. JOIN payments for status
        tracing::info!(%game_id, "Bill split calculation requested (stub)");
        Err(AppError::Unimplemented("Bill split calculation not yet implemented".into()))
    }
}

/// Pure calculation helper — no DB access, easy to unit test.
///
/// Returns an error for invalid inputs (zero/negative player count or negative total).
pub fn calculate_split(total_amount: i32, player_count: i32) -> Result<SplitResult, &'static str> {
    if player_count <= 0 {
        return Err("Player count must be positive");
    }
    if total_amount < 0 {
        return Err("Total amount cannot be negative");
    }
    let per_player = total_amount / player_count;
    let remainder = total_amount % player_count;
    Ok(SplitResult {
        per_player,
        remainder,
        creator_pays_extra: remainder > 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_equal_split() {
        let result = calculate_split(200_000, 4).unwrap();
        assert_eq!(result.per_player, 50_000);
        assert_eq!(result.remainder, 0);
        assert!(!result.creator_pays_extra);
    }

    #[test]
    fn test_uneven_split() {
        let result = calculate_split(200_000, 3).unwrap();
        assert_eq!(result.per_player, 66_666);
        assert_eq!(result.remainder, 2);
        assert!(result.creator_pays_extra);
    }

    #[test]
    fn test_zero_players() {
        assert!(calculate_split(200_000, 0).is_err());
    }

    #[test]
    fn test_negative_players() {
        assert!(calculate_split(200_000, -3).is_err());
    }

    #[test]
    fn test_negative_total() {
        assert!(calculate_split(-5000, 3).is_err());
    }
}
