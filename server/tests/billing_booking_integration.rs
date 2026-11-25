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

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL"]
async fn mixed_booking_and_game_commitments_share_one_schedule(pool: PgPool) {
    let creator = user(&pool).await;
    let player = user(&pool).await;
    let (court_a, slot_a) = slot(&pool, creator).await;
    let (court_b, slot_b) = slot(&pool, creator).await;
    assert!(book(&pool, court_b, slot_b, creator).await);
    let game_b = game(&pool, creator, slot_b, 6).await;
    assert!(book(&pool, court_a, slot_a, player).await);
    assert!(GameService::join_game(&pool, game_b, player).await.is_err());
    CourtService::cancel_booking(&pool, court_a, slot_a, player)
        .await
        .unwrap();
    GameService::join_game(&pool, game_b, player).await.unwrap();
    assert!(!book(&pool, court_a, slot_a, player).await);
    GameService::leave_game(&pool, game_b, player)
        .await
        .unwrap();
    let (booking, joining) = tokio::join!(
        book(&pool, court_a, slot_a, player),
        GameService::join_game(&pool, game_b, player)
    );
    assert_ne!(
        booking,
        joining.is_ok(),
        "only one mixed concurrent commitment may succeed"
    );
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL"]
async fn adjacent_mixed_commitments_are_allowed(pool: PgPool) {
    let creator = user(&pool).await;
    let player = user(&pool).await;
    let (court_a, slot_a) = slot(&pool, creator).await;
    let (court_b, slot_b) = slot(&pool, creator).await;
    sqlx::query("UPDATE court_slots SET start_time = (SELECT end_time FROM court_slots WHERE id = $1), end_time = (SELECT end_time + INTERVAL '1 hour' FROM court_slots WHERE id = $1) WHERE id = $2")
        .bind(slot_b).bind(slot_a).execute(&pool).await.unwrap();
    assert!(book(&pool, court_b, slot_b, creator).await);
    let game_b = game(&pool, creator, slot_b, 4).await;
    assert!(book(&pool, court_a, slot_a, player).await);
    GameService::join_game(&pool, game_b, player).await.unwrap();
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL"]
async fn completed_game_keeps_its_booking_and_can_collect_remaining_balance(pool: PgPool) {
    use pickup_server::models::game::GameStatus;
    let creator = user(&pool).await;
    let player = user(&pool).await;
    let (court, slot) = slot(&pool, creator).await;
    assert!(book(&pool, court, slot, creator).await);
    let game = game(&pool, creator, slot, 4).await;
    GameService::join_game(&pool, game, player).await.unwrap();
    paid(&pool, game, creator, 300001).await;
    GameService::update_status(&pool, game, creator, GameStatus::InProgress)
        .await
        .unwrap();
    GameService::update_status(&pool, game, creator, GameStatus::Completed)
        .await
        .unwrap();
    assert!(CourtService::cancel_booking(&pool, court, slot, creator)
        .await
        .is_err());
    let split = BillSplitService::calculate(&pool, game).await.unwrap();
    assert_eq!(split.remaining_total, 300000);
    paid(&pool, game, player, 300000).await;
    let split = BillSplitService::calculate(&pool, game).await.unwrap();
    assert_eq!(split.paid_total, 600001);
    assert_eq!(split.remaining_total, 0);
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL"]
async fn vietnam_day_boundaries_are_independent_of_database_timezone(pool: PgPool) {
    let owner = user(&pool).await;
    let (court, slot) = slot(&pool, owner).await;
    sqlx::query("UPDATE court_slots SET start_time = '2030-01-01T17:00:00Z', end_time = '2030-01-01T18:00:00Z' WHERE id = $1")
        .bind(slot).execute(&pool).await.unwrap();
    let date = chrono::NaiveDate::from_ymd_opt(2030, 1, 2).unwrap();
    let slots = db::courts::list_slots_by_court_and_date(&pool, court, date)
        .await
        .unwrap();
    assert_eq!(slots.len(), 1);
    let prior = db::courts::list_slots_by_court_and_date(&pool, court, date.pred_opt().unwrap())
        .await
        .unwrap();
    assert!(prior.is_empty());
    assert!(book(&pool, court, slot, owner).await);
    let game_id = game(&pool, owner, slot, 4).await;
    for day in [date.pred_opt().unwrap(), date, date.succ_opt().unwrap()] {
        let expected = i64::from(day == date);
        let all = db::games::list_games_all_joined(&pool, None, None, Some(day), 20, 0)
            .await
            .unwrap();
        assert_eq!(all.len() as i64, expected);
        if expected == 1 {
            assert_eq!(all[0].id, game_id);
        }
        assert_eq!(
            db::games::count_games_all_filtered(&pool, None, None, Some(day))
                .await
                .unwrap(),
            expected
        );
        assert_eq!(
            db::games::list_games_near_joined(
                &pool,
                None,
                None,
                Some(day),
                10.0,
                106.0,
                50.0,
                20,
                0
            )
            .await
            .unwrap()
            .len() as i64,
            expected
        );
        assert_eq!(
            db::games::count_games_near_filtered(&pool, None, None, Some(day), 10.0, 106.0, 50.0)
                .await
                .unwrap(),
            expected
        );
    }
}
