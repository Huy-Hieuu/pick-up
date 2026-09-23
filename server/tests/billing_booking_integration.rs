use chrono::{Duration, Utc};
use pickup_server::{
    db,
    error::AppError,
    models::{
        court::{BookSlotRequest, SportType},
        game::CreateGameRequest,
    },
    services::{BillSplitService, CourtService, GameService, PaymentService},
};
use sqlx::PgPool;
use uuid::Uuid;

async fn user(pool: &PgPool) -> Uuid {
    sqlx::query_scalar("INSERT INTO users (phone, display_name) VALUES ($1, 'Player') RETURNING id")
        .bind(Uuid::new_v4().to_string()[..20].to_owned())
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn slot(pool: &PgPool, owner: Uuid) -> (Uuid, Uuid) {
    let court: Uuid = sqlx::query_scalar("INSERT INTO courts (name, sport_type, lat, lng, address, price_per_slot, owner_id) VALUES ('Test', 'pickleball', 10, 106, 'Test', 600001, $1) RETURNING id")
        .bind(owner).fetch_one(pool).await.unwrap();
    let slot: Uuid = sqlx::query_scalar(
        "INSERT INTO court_slots (court_id, start_time, end_time) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(court)
    .bind(Utc::now() + Duration::days(1))
    .bind(Utc::now() + Duration::days(1) + Duration::hours(1))
    .fetch_one(pool)
    .await
    .unwrap();
    (court, slot)
}

async fn book(pool: &PgPool, court: Uuid, slot: Uuid, player: Uuid) -> bool {
    CourtService::book_slot(pool, court, slot, player, BookSlotRequest {})
        .await
        .is_ok()
}

async fn game(pool: &PgPool, creator: Uuid, slot: Uuid, max: i32) -> Uuid {
    GameService::create_game(
        pool,
        creator,
        CreateGameRequest {
            court_slot_id: slot,
            sport_type: SportType::Pickleball,
            max_players: max,
            description: None,
        },
    )
    .await
    .unwrap()
    .game
    .id
}

async fn paid(pool: &PgPool, game: Uuid, user: Uuid, amount: i32) {
    sqlx::query("INSERT INTO payments (game_id, user_id, amount, provider, status, paid_at) VALUES ($1, $2, $3, 'momo', 'paid', now())")
        .bind(game).bind(user).bind(amount).execute(pool).await.unwrap();
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL"]
async fn split_preserves_money_paid_contributions_and_price_snapshot(pool: PgPool) {
    let creator = user(&pool).await;
    let (court, slot) = slot(&pool, creator).await;
    assert!(book(&pool, court, slot, creator).await);
    let game = game(&pool, creator, slot, 6).await;
    let mut players = vec![creator];
    for _ in 0..5 {
        let player = user(&pool).await;
        GameService::join_game(&pool, game, player).await.unwrap();
        players.push(player);
    }
    let split = BillSplitService::calculate(&pool, game).await.unwrap();
    assert_eq!(split.per_player, 100000);
    assert_eq!(split.payments.iter().map(|p| p.amount).sum::<i32>(), 600001);
    assert_eq!(
        split
            .payments
            .iter()
            .find(|p| p.user_id == creator)
            .unwrap()
            .amount,
        100001
    );
    paid(&pool, game, creator, 100001).await;
    paid(&pool, game, players[1], 100000).await;
    GameService::leave_game(&pool, game, players[2])
        .await
        .unwrap();
    // Paid players may leave, but their money must still count.
    GameService::leave_game(&pool, game, players[1])
        .await
        .unwrap();
    sqlx::query("UPDATE courts SET price_per_slot = 900000 WHERE id = $1")
        .bind(court)
        .execute(&pool)
        .await
        .unwrap();
    let split = BillSplitService::calculate(&pool, game).await.unwrap();
    assert_eq!(split.total_amount, 600001);
    assert_eq!(split.player_count, 4);
    assert_eq!(split.paid_total, 200001);
    assert_eq!(split.remaining_total, 400000);
    assert_eq!(split.payments.iter().map(|p| p.amount).sum::<i32>(), 600001);
    assert!(!split.creator_pays_extra);
    assert_eq!(split.payments.iter().filter(|p| p.paid).count(), 2);
    assert!(GameService::cancel_game(&pool, game, creator)
        .await
        .is_err());
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL"]
async fn cancelled_slot_can_be_booked_and_used_for_a_new_game(pool: PgPool) {
    let creator = user(&pool).await;
    let (court, slot) = slot(&pool, creator).await;
    assert!(book(&pool, court, slot, creator).await);
    let old = game(&pool, creator, slot, 4).await;
    GameService::cancel_game(&pool, old, creator).await.unwrap();
    assert!(matches!(
        BillSplitService::calculate(&pool, old).await,
        Err(AppError::Conflict(_))
    ));
    assert!(book(&pool, court, slot, creator).await);
    let new = game(&pool, creator, slot, 4).await;
    assert_ne!(old, new);
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL"]
async fn concurrent_bookings_cannot_double_book_one_slot(pool: PgPool) {
    let a = user(&pool).await;
    let b = user(&pool).await;
    let (court, slot) = slot(&pool, a).await;
    let (a, b) = tokio::join!(book(&pool, court, slot, a), book(&pool, court, slot, b));
    assert_ne!(a, b);
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL"]
async fn concurrent_bookings_cannot_overlap_for_one_player(pool: PgPool) {
    let player = user(&pool).await;
    let (c1, s1) = slot(&pool, player).await;
    let (c2, s2) = slot(&pool, player).await;
    let (a, b) = tokio::join!(book(&pool, c1, s1, player), book(&pool, c2, s2, player));
    assert_ne!(a, b);
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL"]
async fn concurrent_joins_cannot_overfill_game(pool: PgPool) {
    let creator = user(&pool).await;
    let (court, slot) = slot(&pool, creator).await;
    assert!(book(&pool, court, slot, creator).await);
    let game = game(&pool, creator, slot, 2).await;
    let a = user(&pool).await;
    let b = user(&pool).await;
    let (a, b) = tokio::join!(
        GameService::join_game(&pool, game, a),
        GameService::join_game(&pool, game, b)
    );
    assert_ne!(a.is_ok(), b.is_ok());
    assert_eq!(
        db::games::list_players_with_profile(&pool, game)
            .await
            .unwrap()
            .len(),
        2
    );
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL"]
async fn concurrent_joins_cannot_overlap_for_one_player(pool: PgPool) {
    let a = user(&pool).await;
    let b = user(&pool).await;
    let joining = user(&pool).await;
    let (c1, s1) = slot(&pool, a).await;
    let (c2, s2) = slot(&pool, b).await;
    assert!(book(&pool, c1, s1, a).await);
    assert!(book(&pool, c2, s2, b).await);
    let g1 = game(&pool, a, s1, 6).await;
    let g2 = game(&pool, b, s2, 6).await;
    let (a, b) = tokio::join!(
        GameService::join_game(&pool, g1, joining),
        GameService::join_game(&pool, g2, joining)
    );
    assert_ne!(a.is_ok(), b.is_ok());
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL"]
async fn billing_is_private_and_active_payments_are_unique(pool: PgPool) {
    let creator = user(&pool).await;
    let stranger = user(&pool).await;
    let (court, slot) = slot(&pool, creator).await;
    assert!(book(&pool, court, slot, creator).await);
    let game = game(&pool, creator, slot, 4).await;
    assert!(
        pickup_server::services::bill_split::authorize_billing(&pool, game, creator)
            .await
            .is_ok()
    );
    assert!(matches!(
        pickup_server::services::bill_split::authorize_billing(&pool, game, stranger).await,
        Err(AppError::Forbidden(_))
    ));
    paid(&pool, game, creator, 600001).await;
    let duplicate = sqlx::query("INSERT INTO payments (game_id, user_id, amount, provider) VALUES ($1, $2, 600001, 'zalopay')")
        .bind(game).bind(creator).execute(&pool).await;
    assert_eq!(
        duplicate
            .unwrap_err()
            .as_database_error()
            .unwrap()
            .code()
            .as_deref(),
        Some("23505")
    );
    assert_eq!(
        PaymentService::list_payments(&pool, game)
            .await
            .unwrap()
            .len(),
        1
    );
    let split = BillSplitService::calculate(&pool, game).await.unwrap();
    assert_eq!(split.remaining_total, 0);
    assert_eq!(split.per_player, 0);
}
