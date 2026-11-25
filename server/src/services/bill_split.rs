use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::game::{BillSplit, GameStatus, PlayerPayment};

#[derive(sqlx::FromRow)]
struct SplitPlayer {
    user_id: Uuid,
    display_name: Option<String>,
    active: bool,
    paid_amount: i64,
}

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
    pub async fn calculate(pool: &PgPool, game_id: Uuid) -> AppResult<BillSplit> {
        let mut tx = pool.begin().await?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
            .execute(&mut *tx)
            .await?;
        let split = Self::calculate_on(&mut tx, game_id).await?;
        tx.commit().await?;
        Ok(split)
    }

    /// Reuse the caller's consistent transaction for a full lobby snapshot.
    pub(crate) async fn calculate_on(
        conn: &mut sqlx::PgConnection,
        game_id: Uuid,
    ) -> AppResult<BillSplit> {
        let (creator_id, total_amount, status): (Uuid, i32, GameStatus) =
            sqlx::query_as("SELECT creator_id, total_amount, status FROM games WHERE id = $1")
                .bind(game_id)
                .fetch_one(&mut *conn)
                .await?;
        if status == GameStatus::Cancelled {
            return Err(AppError::Conflict("Cancelled games are not payable".into()));
        }
        let disputed: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM payments WHERE game_id = $1 AND (status = 'disputed' OR dispute_reason IS NOT NULL))",
        )
        .bind(game_id)
        .fetch_one(&mut *conn)
        .await?;
        if disputed {
            return Err(AppError::Conflict(
                "Payment requires manual reconciliation".into(),
            ));
        }
        // Include paid players who left: their contribution is still part of the bill.
        let players: Vec<SplitPlayer> = sqlx::query_as(r#"
            SELECT u.id AS user_id, u.display_name,
                EXISTS(SELECT 1 FROM game_players gp WHERE gp.game_id = $1 AND gp.user_id = u.id) AS active,
                COALESCE((SELECT SUM(p.amount) FROM payments p
                    WHERE p.game_id = $1 AND p.user_id = u.id AND p.status = 'paid'), 0)::bigint AS paid_amount
            FROM users u WHERE u.id IN (
                SELECT user_id FROM game_players WHERE game_id = $1
                UNION SELECT user_id FROM payments WHERE game_id = $1 AND status = 'paid'
            ) ORDER BY u.id
        "#).bind(game_id).fetch_all(&mut *conn).await?;
        split_players(total_amount, creator_id, players)
    }
}

/// Only participants (including former paid participants) may view billing records.
pub async fn authorize_billing(pool: &PgPool, game_id: Uuid, user_id: Uuid) -> AppResult<()> {
    let allowed: bool = sqlx::query_scalar(
        r#"
        SELECT creator_id = $2
            OR EXISTS(SELECT 1 FROM game_players WHERE game_id = $1 AND user_id = $2)
            OR EXISTS(SELECT 1 FROM payments WHERE game_id = $1 AND user_id = $2)
        FROM games WHERE id = $1
    "#,
    )
    .bind(game_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    if !allowed {
        return Err(AppError::Forbidden(
            "Only game participants may view billing".into(),
        ));
    }
    Ok(())
}

fn split_players(total: i32, creator: Uuid, players: Vec<SplitPlayer>) -> AppResult<BillSplit> {
    let paid_total: i64 = players.iter().map(|p| p.paid_amount).sum();
    if paid_total > i64::from(total) || total < 0 {
        return Err(AppError::Conflict(
            "Payments exceed the bill; reconciliation required".into(),
        ));
    }
    let remaining = total - paid_total as i32;
    let unpaid: Vec<_> = players
        .iter()
        .filter(|p| p.active && p.paid_amount == 0)
        .collect();
    let split = if unpaid.is_empty() {
        SplitResult {
            per_player: 0,
            remainder: 0,
            creator_pays_extra: false,
        }
    } else {
        calculate_split(remaining, unpaid.len() as i32).map_err(|e| AppError::Conflict(e.into()))?
    };
    // Paid amounts never change. If the creator has paid, assign rounding to
    // the first unpaid UUID in stable order, not to an already settled payment.
    let remainder_user = unpaid
        .iter()
        .find(|p| p.user_id == creator)
        .or_else(|| unpaid.first())
        .map(|p| p.user_id);
    let player_count = players.iter().filter(|p| p.active).count() as i32;
    let payments = players
        .into_iter()
        .map(|p| PlayerPayment {
            user_id: p.user_id,
            display_name: p.display_name,
            amount: if p.paid_amount > 0 {
                p.paid_amount as i32
            } else {
                split.per_player
                    + if Some(p.user_id) == remainder_user {
                        split.remainder
                    } else {
                        0
                    }
            },
            paid: p.paid_amount > 0,
        })
        .collect();
    Ok(BillSplit {
        total_amount: total,
        per_player: split.per_player,
        remainder: split.remainder,
        creator_pays_extra: split.remainder > 0 && remainder_user == Some(creator),
        player_count,
        paid_total,
        remaining_total: i64::from(remaining),
        payments,
    })
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
    fn splits_conserve_every_dong_across_remainders() {
        let creator = Uuid::from_u128(1);
        for total in [0, 1, 29, 600_001, i32::MAX] {
            for count in 1..=30 {
                let players = (1..=count)
                    .map(|id| SplitPlayer {
                        user_id: Uuid::from_u128(id),
                        display_name: None,
                        active: true,
                        paid_amount: 0,
                    })
                    .collect();
                let result = split_players(total, creator, players).unwrap();
                assert_eq!(
                    result
                        .payments
                        .iter()
                        .map(|p| i64::from(p.amount))
                        .sum::<i64>(),
                    i64::from(total)
                );
                assert!(result.payments.iter().all(|p| p.amount >= 0));
            }
        }
    }

    #[test]
    fn overpayment_fails_closed_without_integer_overflow() {
        let players = (1..=2)
            .map(|id| SplitPlayer {
                user_id: Uuid::from_u128(id),
                display_name: None,
                active: true,
                paid_amount: i64::from(i32::MAX),
            })
            .collect();
        assert!(split_players(i32::MAX, Uuid::from_u128(1), players).is_err());
    }

    #[test]
    fn fully_paid_members_are_never_recharged_for_a_deficit() {
        let creator = Uuid::new_v4();
        let result = split_players(
            100,
            creator,
            vec![SplitPlayer {
                user_id: creator,
                display_name: None,
                active: true,
                paid_amount: 50,
            }],
        )
        .unwrap();
        assert_eq!(result.remaining_total, 50);
        assert_eq!(result.payments[0].amount, 50);
        assert_eq!(result.per_player, 0);
    }

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
