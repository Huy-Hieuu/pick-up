use std::sync::{Arc, Mutex};

use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{extract::State, routing::post, Json, Router};
use chrono::{Duration, Utc};
use hmac::{Hmac, Mac};
use pickup_server::{
    error::AppError,
    models::{
        court::{BookSlotRequest, SportType},
        game::CreateGameRequest,
        payment::{InitiatePaymentRequest, PaymentProvider, PaymentStatus},
    },
    routes::build_router,
    services::{CourtService, GameService, PaymentService},
};
use serde_json::{json, Value};
use sha2::Sha256;
use sqlx::PgPool;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone, Default)]
struct MockPsp {
    requests: Arc<Mutex<Vec<(String, Value)>>>,
    momo_result_code: Arc<std::sync::atomic::AtomicI32>,
    momo_transport_failure: Arc<std::sync::atomic::AtomicBool>,
    zalopay_return_code: Arc<std::sync::atomic::AtomicI32>,
    zalopay_sub_return_code: Arc<std::sync::atomic::AtomicI32>,
    zalopay_transport_failure: Arc<std::sync::atomic::AtomicBool>,
    query_response: Arc<Mutex<Option<Value>>>,
    settle_momo_before_response: Arc<Mutex<Option<pickup_server::state::AppState>>>,
}

impl MockPsp {
    fn requests(&self, provider: &str) -> Vec<Value> {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .filter(|(name, _)| name == provider)
            .map(|(_, body)| body.clone())
            .collect()
    }
}

async fn momo(State(mock): State<MockPsp>, Json(body): Json<Value>) -> (StatusCode, Json<Value>) {
    mock.requests
        .lock()
        .unwrap()
        .push(("momo".into(), body.clone()));
    let result_code = mock
        .momo_result_code
        .load(std::sync::atomic::Ordering::SeqCst);
    let settle_state = mock.settle_momo_before_response.lock().unwrap().clone();
    if let Some(state) = settle_state {
        let callback = momo_callback(
            &state.settings,
            body["orderId"].as_str().unwrap(),
            body["amount"].as_i64().unwrap(),
            7_777_001,
            0,
        );
        if PaymentService::handle_momo_webhook(&state.pool, &state.settings, &callback.to_string())
            .await
            .is_err()
        {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({})));
        }
    }
    let status = if mock
        .momo_transport_failure
        .load(std::sync::atomic::Ordering::SeqCst)
    {
        StatusCode::BAD_GATEWAY
    } else {
        StatusCode::OK
    };
    (
        status,
        Json(json!({
            "partnerCode": body["partnerCode"], "requestId": body["requestId"],
            "orderId": body["orderId"], "amount": body["amount"], "responseTime": 1714560000000_i64,
            "message": if result_code == 0 { "Successful." } else { "Failed." }, "resultCode": result_code,
            "payUrl": "https://test-payment.momo.vn/v2/gateway/pay?token=test"
        })),
    )
}

async fn zalopay(State(mock): State<MockPsp>, Json(body): Json<Value>) -> axum::response::Response {
    mock.requests.lock().unwrap().push(("zalopay".into(), body));
    if mock
        .zalopay_transport_failure
        .load(std::sync::atomic::Ordering::SeqCst)
    {
        return (StatusCode::BAD_GATEWAY, Json(json!({}))).into_response();
    }
    let return_code = mock
        .zalopay_return_code
        .load(std::sync::atomic::Ordering::SeqCst);
    let sub_return_code = mock
        .zalopay_sub_return_code
        .load(std::sync::atomic::Ordering::SeqCst);
    let return_code = if return_code == 0 { 1 } else { return_code };
    Json(json!({
        "return_code": return_code,
        "sub_return_code": sub_return_code,
        "order_url": if return_code == 1 { Some("https://sbgateway.zalopay.vn/order?token=test") } else { None }
    })).into_response()
}

async fn query_status(State(mock): State<MockPsp>, Json(body): Json<Value>) -> Json<Value> {
    mock.requests
        .lock()
        .unwrap()
        .push(("query".into(), body.clone()));
    if let Some(response) = mock.query_response.lock().unwrap().clone() {
        let mut response = response;
        if body.get("partnerCode").is_some() {
            response["partnerCode"] = body["partnerCode"].clone();
            response["requestId"] = body["requestId"].clone();
            response["orderId"] = body["orderId"].clone();
        }
        return Json(response);
    }
    Json(json!({"return_code": 3, "is_processing": true}))
}

struct ProviderServer {
    endpoint_momo: String,
    endpoint_zalopay: String,
    mock: MockPsp,
    shutdown: tokio_util::sync::CancellationToken,
}

impl ProviderServer {
    async fn start() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let mock = MockPsp::default();
        let app = Router::new()
            .route("/momo", post(momo))
            .route("/zalopay", post(zalopay))
            .route("/query", post(query_status))
            .with_state(mock.clone());
        let shutdown = tokio_util::sync::CancellationToken::new();
        let cancel = shutdown.clone();
        tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(cancel.cancelled_owned())
                .await
                .unwrap();
        });
        Self {
            endpoint_momo: format!("http://{address}/momo"),
            endpoint_zalopay: format!("http://{address}/zalopay"),
            mock,
            shutdown,
        }
    }
}

impl Drop for ProviderServer {
    fn drop(&mut self) {
        self.shutdown.cancel();
    }
}

async fn user(pool: &PgPool, display_name: &str) -> Uuid {
    sqlx::query_scalar("INSERT INTO users (phone, display_name) VALUES ($1, $2) RETURNING id")
        .bind(Uuid::new_v4().to_string()[..20].to_owned())
        .bind(display_name)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn game(pool: &PgPool, creator: Uuid, max_players: i32) -> Uuid {
    let court: Uuid = sqlx::query_scalar("INSERT INTO courts (name, sport_type, lat, lng, address, price_per_slot, owner_id) VALUES ('Payment test', 'pickleball', 10, 106, 'Test', 600001, $1) RETURNING id")
        .bind(creator).fetch_one(pool).await.unwrap();
    let slot: Uuid = sqlx::query_scalar(
        "INSERT INTO court_slots (court_id, start_time, end_time) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(court)
    .bind(Utc::now() + Duration::days(1))
    .bind(Utc::now() + Duration::days(1) + Duration::hours(1))
    .fetch_one(pool)
    .await
    .unwrap();
    CourtService::book_slot(pool, court, slot, creator, BookSlotRequest {})
        .await
        .unwrap();
    GameService::create_game(
        pool,
        creator,
        CreateGameRequest {
            court_slot_id: slot,
            sport_type: SportType::Pickleball,
            max_players,
            description: None,
        },
    )
    .await
    .unwrap()
    .game
    .id
}

async fn api(pool: PgPool, psp: &ProviderServer) -> pickup_server::state::AppState {
    super::state_with_payment_endpoints(
        pool,
        psp.endpoint_momo.clone(),
        psp.endpoint_zalopay.clone(),
    )
    .await
}

fn signature(key: &str, message: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(key.as_bytes()).unwrap();
    mac.update(message.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

fn momo_callback(
    settings: &pickup_server::config::Settings,
    order_id: &str,
    amount: i64,
    txn: i64,
    result: i32,
) -> Value {
    let mut callback = json!({
        "partnerCode": settings.momo.partner_code, "orderId": order_id,
        "requestId": order_id, "amount": amount, "orderInfo": "PickUp game bill",
        "orderType": "momo_wallet", "transId": txn, "resultCode": result,
        "message": if result == 0 { "Successful." } else { "Failed." },
        "responseTime": Utc::now().timestamp_millis(), "extraData": "",
        "payType": "qr", "signature": ""
    });
    let raw = format!(
        "accessKey={}&amount={}&extraData={}&message={}&orderId={}&orderInfo={}&orderType={}&partnerCode={}&payType={}&requestId={}&responseTime={}&resultCode={}&transId={}",
        settings.momo.access_key, amount, "", callback["message"].as_str().unwrap(), order_id,
        "PickUp game bill", "momo_wallet", settings.momo.partner_code, "qr", order_id,
        callback["responseTime"], result, txn,
    );
    callback["signature"] = signature(&settings.momo.secret_key, &raw).into();
    callback
}

async fn initiate(
    state: &pickup_server::state::AppState,
    game: Uuid,
    user: Uuid,
    provider: PaymentProvider,
) -> Result<pickup_server::models::payment::PaymentResponse, AppError> {
    PaymentService::initiate(
        &state.pool,
        &state.settings,
        game,
        user,
        InitiatePaymentRequest { provider },
    )
    .await
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL and Redis"]
async fn provider_orders_are_signed_persisted_and_idempotent(pool: PgPool) {
    let psp = ProviderServer::start().await;
    let state = api(pool.clone(), &psp).await;
    let creator = user(&pool, "Creator").await;
    let joiner = user(&pool, "Joiner").await;
    let game = game(&pool, creator, 3).await;
    GameService::join_game(&pool, game, joiner).await.unwrap();

    let momo = initiate(&state, game, creator, PaymentProvider::Momo)
        .await
        .unwrap();
    assert_eq!(momo.amount, 300001);
    assert_eq!(momo.status, PaymentStatus::Pending);
    assert!(momo.expires_at.unwrap().timestamp() > Utc::now().timestamp());
    assert!(momo
        .pay_url
        .as_deref()
        .unwrap()
        .starts_with("https://test-payment.momo.vn/"));
    let request = psp.mock.requests("momo").pop().unwrap();
    let raw = format!(
        "accessKey={}&amount={}&extraData={}&ipnUrl={}&orderId={}&orderInfo={}&partnerCode={}&redirectUrl={}&requestId={}&requestType={}",
        state.settings.momo.access_key, request["amount"], request["extraData"].as_str().unwrap(),
        request["ipnUrl"].as_str().unwrap(), request["orderId"].as_str().unwrap(),
        request["orderInfo"].as_str().unwrap(), request["partnerCode"].as_str().unwrap(),
        request["redirectUrl"].as_str().unwrap(), request["requestId"].as_str().unwrap(), request["requestType"].as_str().unwrap(),
    );
    assert_eq!(
        request["signature"],
        signature(&state.settings.momo.secret_key, &raw)
    );
    assert_eq!(request["orderId"], momo.id.simple().to_string());
    let retry = initiate(&state, game, creator, PaymentProvider::Momo)
        .await
        .unwrap();
    assert_eq!(retry.id, momo.id);
    assert_eq!(retry.pay_url, momo.pay_url);
    assert_eq!(psp.mock.requests("momo").len(), 1);
    let outsider = user(&pool, "Outsider").await;
    assert!(matches!(
        initiate(&state, game, outsider, PaymentProvider::Momo).await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        GameService::join_game(&pool, game, outsider).await,
        Err(AppError::Conflict(_))
    ));

    let app = build_router(state.clone());
    let bad = momo_callback(
        &state.settings,
        &momo.id.simple().to_string(),
        i64::from(momo.amount),
        123,
        0,
    );
    let mut bad = bad;
    bad["signature"] = "00".into();
    assert_eq!(
        super::call(&app, "POST", "/webhooks/momo", bad, None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let callback = momo_callback(
        &state.settings,
        &momo.id.simple().to_string(),
        i64::from(momo.amount),
        123,
        0,
    );
    assert_eq!(
        super::call(&app, "POST", "/webhooks/momo", callback.clone(), None)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        super::call(&app, "POST", "/webhooks/momo", callback, None)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    let row: PaymentStatus = sqlx::query_scalar("SELECT status FROM payments WHERE id = $1")
        .bind(momo.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row, PaymentStatus::Paid);
    assert_eq!(
        GameService::join_game(&pool, game, outsider)
            .await
            .unwrap()
            .players
            .len(),
        3
    );
    assert!(matches!(
        initiate(&state, game, creator, PaymentProvider::Momo).await,
        Err(AppError::Conflict(_))
    ));

    let zalo = initiate(&state, game, joiner, PaymentProvider::Zalopay)
        .await
        .unwrap();
    assert_eq!(zalo.amount, 150000);
    let request = psp.mock.requests("zalopay").pop().unwrap();
    let raw = format!(
        "{}|{}|{}|{}|{}|{}|{}",
        request["app_id"],
        request["app_trans_id"].as_str().unwrap(),
        request["app_user"].as_str().unwrap(),
        request["amount"],
        request["app_time"],
        request["embed_data"].as_str().unwrap(),
        request["item"].as_str().unwrap()
    );
    assert_eq!(
        request["mac"],
        signature(&state.settings.zalopay.key1, &raw)
    );
    assert!(request["expire_duration_seconds"].as_i64().unwrap() >= 300);
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL and Redis"]
async fn amount_mismatch_and_late_capture_are_disputed_and_freeze_game(pool: PgPool) {
    let psp = ProviderServer::start().await;
    let state = api(pool.clone(), &psp).await;
    let creator = user(&pool, "Creator").await;
    let game = game(&pool, creator, 3).await;
    let payment = initiate(&state, game, creator, PaymentProvider::Momo)
        .await
        .unwrap();
    let order = payment.id.simple().to_string();
    let callback = momo_callback(
        &state.settings,
        &order,
        i64::from(payment.amount) + 1,
        891,
        0,
    );
    let app = build_router(state.clone());
    assert_eq!(
        super::call(&app, "POST", "/webhooks/momo", callback, None)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    let status: PaymentStatus = sqlx::query_scalar("SELECT status FROM payments WHERE id = $1")
        .bind(payment.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, PaymentStatus::Disputed);
    assert!(matches!(
        pickup_server::services::BillSplitService::calculate(&pool, game).await,
        Err(AppError::Conflict(_))
    ));
    let newcomer = user(&pool, "New player").await;
    assert!(matches!(
        GameService::join_game(&pool, game, newcomer).await,
        Err(AppError::Conflict(_))
    ));
    assert!(matches!(
        initiate(&state, game, creator, PaymentProvider::Momo).await,
        Err(AppError::Conflict(_))
    ));
    assert!(GameService::cancel_game(&pool, game, creator)
        .await
        .is_err());
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL and Redis"]
async fn failed_orders_expire_retries_create_new_rows_and_stale_orders_expire(pool: PgPool) {
    let psp = ProviderServer::start().await;
    let state = api(pool.clone(), &psp).await;
    let creator = user(&pool, "Creator").await;
    let game = game(&pool, creator, 3).await;
    let payment = initiate(&state, game, creator, PaymentProvider::Momo)
        .await
        .unwrap();
    let callback = momo_callback(
        &state.settings,
        &payment.id.simple().to_string(),
        i64::from(payment.amount),
        0,
        1006,
    );
    PaymentService::handle_momo_webhook(&pool, &state.settings, &callback.to_string())
        .await
        .unwrap();
    let status: PaymentStatus = sqlx::query_scalar("SELECT status FROM payments WHERE id = $1")
        .bind(payment.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, PaymentStatus::Expired);
    let next = initiate(&state, game, creator, PaymentProvider::Momo)
        .await
        .unwrap();
    assert_ne!(next.id, payment.id);
    sqlx::query("UPDATE payments SET expires_at = now() - interval '1 second' WHERE id = $1")
        .bind(next.id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        PaymentService::expire_stale_pending(&pool).await.unwrap(),
        1
    );
    let blocked: PaymentStatus = sqlx::query_scalar("SELECT status FROM payments WHERE id = $1")
        .bind(next.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(blocked, PaymentStatus::Disputed);
    *psp.mock.query_response.lock().unwrap() =
        Some(json!({"resultCode": 1005, "amount": next.amount}));
    PaymentService::reconcile_pending(&pool, &state.settings)
        .await
        .unwrap();
    psp.mock
        .momo_result_code
        .store(20, std::sync::atomic::Ordering::SeqCst);
    assert!(matches!(
        initiate(&state, game, creator, PaymentProvider::Momo).await,
        Err(AppError::PaymentRejected)
    ));
    let rejected: PaymentStatus = sqlx::query_scalar("SELECT status FROM payments WHERE user_id = $1 AND game_id = $2 ORDER BY created_at DESC LIMIT 1")
        .bind(creator).bind(game).fetch_one(&pool).await.unwrap();
    assert_eq!(rejected, PaymentStatus::Expired);
    psp.mock
        .momo_result_code
        .store(0, std::sync::atomic::Ordering::SeqCst);
    let final_attempt = initiate(&state, game, creator, PaymentProvider::Momo)
        .await
        .unwrap();
    assert_ne!(final_attempt.id, next.id);
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL and Redis"]
async fn zalopay_duplicate_order_response_freezes_the_original_reservation(pool: PgPool) {
    let psp = ProviderServer::start().await;
    let state = api(pool.clone(), &psp).await;
    let creator = user(&pool, "Creator").await;
    let game = game(&pool, creator, 3).await;
    psp.mock
        .zalopay_transport_failure
        .store(true, std::sync::atomic::Ordering::SeqCst);
    assert!(matches!(
        initiate(&state, game, creator, PaymentProvider::Zalopay).await,
        Err(AppError::PaymentGateway)
    ));

    let (first_id, first_order_id): (Uuid, String) = sqlx::query_as(
        "SELECT id, provider_order_id FROM payments WHERE game_id = $1 AND user_id = $2",
    )
    .bind(game)
    .bind(creator)
    .fetch_one(&pool)
    .await
    .unwrap();
    psp.mock
        .zalopay_transport_failure
        .store(false, std::sync::atomic::Ordering::SeqCst);
    psp.mock
        .zalopay_return_code
        .store(2, std::sync::atomic::Ordering::SeqCst);
    psp.mock
        .zalopay_sub_return_code
        .store(-68, std::sync::atomic::Ordering::SeqCst);
    sqlx::query("UPDATE payments SET provider_create_claim_until = now() - interval '1 second' WHERE id = $1")
        .bind(first_id)
        .execute(&pool)
        .await
        .unwrap();

    assert!(matches!(
        initiate(&state, game, creator, PaymentProvider::Zalopay).await,
        Err(AppError::PaymentOrderUncertain)
    ));
    let (status, reason, claim_until): (
        PaymentStatus,
        Option<String>,
        Option<chrono::DateTime<Utc>>,
    ) = sqlx::query_as(
        "SELECT status, dispute_reason, provider_create_claim_until FROM payments WHERE id = $1",
    )
    .bind(first_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, PaymentStatus::Disputed);
    assert_eq!(reason.as_deref(), Some("provider_creation_uncertain"));
    assert!(claim_until.is_none());
    assert!(matches!(
        initiate(&state, game, creator, PaymentProvider::Zalopay).await,
        Err(AppError::Conflict(_))
    ));
    assert_eq!(
        psp.mock.requests("zalopay")[1]["app_trans_id"],
        first_order_id
    );
    let order_ids = psp
        .mock
        .requests("zalopay")
        .into_iter()
        .map(|request| request["app_trans_id"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(order_ids, vec![first_order_id.clone(), first_order_id]);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM payments WHERE game_id = $1")
            .bind(game)
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL and Redis"]
async fn late_zalopay_capture_is_recorded_when_a_replacement_is_pending(pool: PgPool) {
    let psp = ProviderServer::start().await;
    let state = api(pool.clone(), &psp).await;
    let creator = user(&pool, "Creator").await;
    let game = game(&pool, creator, 3).await;
    let original = initiate(&state, game, creator, PaymentProvider::Zalopay)
        .await
        .unwrap();
    let original_request = psp.mock.requests("zalopay")[0].clone();
    sqlx::query("UPDATE payments SET status = 'expired', expires_at = now() - interval '1 second' WHERE id = $1")
        .bind(original.id)
        .execute(&pool)
        .await
        .unwrap();
    let replacement = initiate(&state, game, creator, PaymentProvider::Zalopay)
        .await
        .unwrap();
    assert_ne!(replacement.id, original.id);

    let data = json!({
        "app_id": state.settings.zalopay.app_id.parse::<i64>().unwrap(),
        "app_trans_id": original_request["app_trans_id"],
        "app_time": original_request["app_time"],
        "server_time": Utc::now().timestamp_millis(),
        "amount": original.amount,
        "zp_trans_id": 9_901_234_i64,
    })
    .to_string();
    let callback = json!({
        "data": data,
        "mac": signature(&state.settings.zalopay.key2, &data),
        "type": 1,
    });
    let app = build_router(state);
    let (status, response) =
        super::call(&app, "POST", "/webhooks/zalopay", callback.clone(), None).await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["return_code"], 1);
    assert_eq!(
        super::call(&app, "POST", "/webhooks/zalopay", callback, None)
            .await
            .0,
        StatusCode::OK,
        "an exact callback replay remains idempotent"
    );

    let (old_status, old_reason): (PaymentStatus, Option<String>) =
        sqlx::query_as("SELECT status, dispute_reason FROM payments WHERE id = $1")
            .bind(original.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(old_status, PaymentStatus::Disputed);
    assert_eq!(old_reason.as_deref(), Some("late_capture"));
    let new_status: PaymentStatus = sqlx::query_scalar("SELECT status FROM payments WHERE id = $1")
        .bind(replacement.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(new_status, PaymentStatus::Pending);
    let event_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM payment_provider_events WHERE payment_id = $1 AND provider_txn_id = '9901234'",
    )
    .bind(original.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(event_count, 1);
    assert!(
        pickup_server::services::BillSplitService::calculate(&pool, game)
            .await
            .is_err()
    );
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL and Redis"]
async fn concurrent_initiations_and_uncertain_timeouts_reuse_one_order(pool: PgPool) {
    let psp = ProviderServer::start().await;
    let state = api(pool.clone(), &psp).await;
    let creator = user(&pool, "Creator").await;
    let first_game = game(&pool, creator, 3).await;
    let (left, right) = tokio::join!(
        initiate(&state, first_game, creator, PaymentProvider::Momo),
        initiate(&state, first_game, creator, PaymentProvider::Momo),
    );
    let first = match (left, right) {
        (Ok(left), Ok(right)) => {
            assert_eq!(left.id, right.id);
            assert_eq!(left.pay_url, right.pay_url);
            left
        }
        (Ok(payment), Err(AppError::Conflict(_))) | (Err(AppError::Conflict(_)), Ok(payment)) => {
            payment
        }
        (left, right) => panic!("unexpected concurrent initiation results: {left:?}, {right:?}"),
    };
    assert_eq!(psp.mock.requests("momo").len(), 1);
    let retry = initiate(&state, first_game, creator, PaymentProvider::Momo)
        .await
        .unwrap();
    assert_eq!(retry.id, first.id);

    let second_creator = user(&pool, "Second creator").await;
    let second_payer = user(&pool, "Second payer").await;
    let second_game = game(&pool, second_creator, 3).await;
    GameService::join_game(&pool, second_game, second_payer)
        .await
        .unwrap();
    psp.mock
        .momo_transport_failure
        .store(true, std::sync::atomic::Ordering::SeqCst);
    assert!(matches!(
        initiate(&state, second_game, second_payer, PaymentProvider::Momo).await,
        Err(AppError::PaymentGateway)
    ));
    let (pending_id, provider_order_id): (Uuid, String) = sqlx::query_as(
        "SELECT id, provider_order_id FROM payments WHERE game_id = $1 AND user_id = $2",
    )
    .bind(second_game)
    .bind(second_payer)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(matches!(
        initiate(&state, second_game, second_payer, PaymentProvider::Momo).await,
        Err(AppError::Conflict(_))
    ));
    sqlx::query("UPDATE payments SET provider_create_claim_until = now() - interval '1 second' WHERE id = $1")
        .bind(pending_id).execute(&pool).await.unwrap();
    psp.mock
        .momo_transport_failure
        .store(false, std::sync::atomic::Ordering::SeqCst);
    let retry = initiate(&state, second_game, second_payer, PaymentProvider::Momo)
        .await
        .unwrap();
    assert_eq!(retry.id, pending_id);
    assert_eq!(
        psp.mock.requests("momo").last().unwrap()["orderId"],
        provider_order_id
    );
    assert_ne!(first.id, retry.id);
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL and Redis"]
async fn callback_can_settle_before_the_provider_create_response(pool: PgPool) {
    let psp = ProviderServer::start().await;
    let state = api(pool.clone(), &psp).await;
    let creator = user(&pool, "Creator").await;
    let game = game(&pool, creator, 3).await;
    *psp.mock.settle_momo_before_response.lock().unwrap() = Some(state.clone());

    let payment = initiate(&state, game, creator, PaymentProvider::Momo)
        .await
        .unwrap();
    assert_eq!(payment.status, PaymentStatus::Paid);
    assert!(payment
        .pay_url
        .as_deref()
        .unwrap()
        .starts_with("https://test-payment.momo.vn/"));
    let stored: PaymentStatus = sqlx::query_scalar("SELECT status FROM payments WHERE id = $1")
        .bind(payment.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored, PaymentStatus::Paid);
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL and Redis"]
async fn one_provider_transaction_cannot_settle_two_concurrent_orders(pool: PgPool) {
    let psp = ProviderServer::start().await;
    let state = api(pool.clone(), &psp).await;
    let creator_a = user(&pool, "Creator A").await;
    let creator_b = user(&pool, "Creator B").await;
    let game_a = game(&pool, creator_a, 3).await;
    let game_b = game(&pool, creator_b, 3).await;
    let payment_a = initiate(&state, game_a, creator_a, PaymentProvider::Momo)
        .await
        .unwrap();
    let payment_b = initiate(&state, game_b, creator_b, PaymentProvider::Momo)
        .await
        .unwrap();
    let callback_a = momo_callback(
        &state.settings,
        &payment_a.id.simple().to_string(),
        i64::from(payment_a.amount),
        880_001,
        0,
    );
    let callback_b = momo_callback(
        &state.settings,
        &payment_b.id.simple().to_string(),
        i64::from(payment_b.amount),
        880_001,
        0,
    );
    let app = build_router(state);
    let (response_a, response_b) = tokio::join!(
        super::call(&app, "POST", "/webhooks/momo", callback_a, None),
        super::call(&app, "POST", "/webhooks/momo", callback_b, None),
    );
    assert_eq!(response_a.0, StatusCode::NO_CONTENT);
    assert_eq!(response_b.0, StatusCode::NO_CONTENT);
    let mut statuses: Vec<PaymentStatus> =
        sqlx::query_scalar("SELECT status FROM payments WHERE id IN ($1, $2) ORDER BY status")
            .bind(payment_a.id)
            .bind(payment_b.id)
            .fetch_all(&pool)
            .await
            .unwrap();
    statuses.sort_by_key(|status| match status {
        PaymentStatus::Paid => 0,
        PaymentStatus::Disputed => 1,
        _ => 2,
    });
    assert_eq!(statuses, vec![PaymentStatus::Paid, PaymentStatus::Disputed]);
    assert_eq!(sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM payment_provider_events WHERE provider = 'momo' AND (provider_txn_id = '880001' OR reported_txn_id = '880001')"
    ).fetch_one(&pool).await.unwrap(), 2);
    assert_eq!(sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM payment_provider_transactions WHERE provider = 'momo' AND transaction_id = '880001'"
    ).fetch_one(&pool).await.unwrap(), 1);
    let disputed_id: Uuid = sqlx::query_scalar("SELECT id FROM payments WHERE status = 'disputed'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let reason: Option<String> =
        sqlx::query_scalar("SELECT dispute_reason FROM payments WHERE id = $1")
            .bind(disputed_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(reason.as_deref(), Some("provider_transaction_reused"));
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL and Redis"]
async fn zalopay_callback_mac_settles_once_and_conflicting_capture_is_disputed(pool: PgPool) {
    let psp = ProviderServer::start().await;
    let state = api(pool.clone(), &psp).await;
    let creator = user(&pool, "Creator").await;
    let game = game(&pool, creator, 3).await;
    let payment = initiate(&state, game, creator, PaymentProvider::Zalopay)
        .await
        .unwrap();
    let request = psp.mock.requests("zalopay").pop().unwrap();
    let app_trans_id = request["app_trans_id"].as_str().unwrap();
    assert!(app_trans_id.starts_with(
        &(Utc::now() + Duration::hours(7))
            .format("%y%m%d_")
            .to_string()
    ));
    let make_callback = |amount: i64| {
        let data = json!({
            "app_id": 2553, "app_trans_id": app_trans_id,
            "app_time": request["app_time"], "app_user": request["app_user"],
            "amount": amount, "embed_data": request["embed_data"], "item": "[]",
            "zp_trans_id": 9001, "server_time": Utc::now().timestamp_millis(),
            "channel": 38, "merchant_user_id": "test", "user_fee_amount": 0,
            "discount_amount": 0
        })
        .to_string();
        json!({ "data": data, "mac": signature(&state.settings.zalopay.key2, &data), "type": 1 })
    };
    let app = build_router(state.clone());
    let callback = make_callback(i64::from(payment.amount));
    let (status, response) =
        super::call(&app, "POST", "/webhooks/zalopay", callback.clone(), None).await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["return_code"], 1);
    assert_eq!(
        super::call(&app, "POST", "/webhooks/zalopay", callback, None)
            .await
            .0,
        StatusCode::OK
    );
    let settled: PaymentStatus = sqlx::query_scalar("SELECT status FROM payments WHERE id = $1")
        .bind(payment.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(settled, PaymentStatus::Paid);

    // A second signed callback cannot overwrite the original captured amount.
    let conflict = make_callback(i64::from(payment.amount) + 1);
    assert_eq!(
        super::call(&app, "POST", "/webhooks/zalopay", conflict, None)
            .await
            .0,
        StatusCode::OK
    );
    let disputed: PaymentStatus = sqlx::query_scalar("SELECT status FROM payments WHERE id = $1")
        .bind(payment.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(disputed, PaymentStatus::Disputed);
    assert!(matches!(
        pickup_server::services::BillSplitService::calculate(&pool, game).await,
        Err(AppError::Conflict(_))
    ));
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL and Redis"]
async fn provider_queries_recover_lost_callbacks_and_sign_the_requests(pool: PgPool) {
    for provider in [PaymentProvider::Momo, PaymentProvider::Zalopay] {
        let psp = ProviderServer::start().await;
        let state = api(pool.clone(), &psp).await;
        let creator = user(&pool, "Query creator").await;
        let game_id = game(&pool, creator, 3).await;
        let order = initiate(&state, game_id, creator, provider).await.unwrap();
        sqlx::query("UPDATE payments SET expires_at = now() - interval '1 second' WHERE id = $1")
            .bind(order.id)
            .execute(&pool)
            .await
            .unwrap();
        PaymentService::expire_stale_pending(&pool).await.unwrap();
        let txn = if provider == PaymentProvider::Momo {
            1234001
        } else {
            1234002
        };
        *psp.mock.query_response.lock().unwrap() = Some(match provider {
            PaymentProvider::Momo => {
                json!({"resultCode": 0, "amount": order.amount, "transId": txn})
            }
            PaymentProvider::Zalopay => {
                json!({"return_code": 1, "is_processing": false, "amount": order.amount, "zp_trans_id": txn})
            }
        });
        assert_eq!(
            PaymentService::reconcile_pending(&pool, &state.settings)
                .await
                .unwrap(),
            1
        );
        let (status, reason, paid_at): (
            PaymentStatus,
            Option<String>,
            Option<chrono::DateTime<Utc>>,
        ) = sqlx::query_as("SELECT status, dispute_reason, paid_at FROM payments WHERE id = $1")
            .bind(order.id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(status, PaymentStatus::Paid);
        assert!(reason.is_none());
        assert!(paid_at.is_some());
        let request = psp.mock.requests("query").pop().unwrap();
        match provider {
            PaymentProvider::Momo => {
                let raw = format!(
                    "accessKey={}&orderId={}&partnerCode={}&requestId={}",
                    state.settings.momo.access_key,
                    request["orderId"].as_str().unwrap(),
                    request["partnerCode"].as_str().unwrap(),
                    request["requestId"].as_str().unwrap()
                );
                assert_eq!(
                    request["signature"],
                    signature(&state.settings.momo.secret_key, &raw)
                );
            }
            PaymentProvider::Zalopay => {
                let raw = format!(
                    "{}|{}|{}",
                    state.settings.zalopay.app_id,
                    request["app_trans_id"].as_str().unwrap(),
                    state.settings.zalopay.key1
                );
                assert_eq!(
                    request["mac"],
                    signature(&state.settings.zalopay.key1, &raw)
                );
            }
        }
        assert_eq!(
            PaymentService::reconcile_pending(&pool, &state.settings)
                .await
                .unwrap(),
            0
        );
    }
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL and Redis"]
async fn unresolved_query_never_unlocks_a_charge_and_workers_claim_once(pool: PgPool) {
    let psp = ProviderServer::start().await;
    let state = api(pool.clone(), &psp).await;
    let creator = user(&pool, "Creator").await;
    let game_id = game(&pool, creator, 3).await;
    let order = initiate(&state, game_id, creator, PaymentProvider::Zalopay)
        .await
        .unwrap();
    sqlx::query("UPDATE payments SET expires_at = now() - interval '1 second' WHERE id = $1")
        .bind(order.id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        initiate(&state, game_id, creator, PaymentProvider::Zalopay).await,
        Err(AppError::PaymentOrderUncertain)
    ));
    let (a, b) = tokio::join!(
        PaymentService::reconcile_pending(&pool, &state.settings),
        PaymentService::reconcile_pending(&pool, &state.settings)
    );
    assert_eq!(a.unwrap() + b.unwrap(), 0);
    assert_eq!(psp.mock.requests("query").len(), 1);
    assert!(initiate(&state, game_id, creator, PaymentProvider::Zalopay)
        .await
        .is_err());
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM payments WHERE game_id = $1")
        .bind(game_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    sqlx::query("UPDATE payments SET reconcile_after = now() WHERE id = $1")
        .bind(order.id)
        .execute(&pool)
        .await
        .unwrap();
    *psp.mock.query_response.lock().unwrap() =
        Some(json!({"return_code": 2, "sub_return_code": -54, "is_processing": false}));
    assert_eq!(
        PaymentService::reconcile_pending(&pool, &state.settings)
            .await
            .unwrap(),
        1
    );
    assert!(initiate(&state, game_id, creator, PaymentProvider::Zalopay)
        .await
        .is_ok());
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL and Redis"]
async fn processing_callback_can_transition_to_paid_with_the_same_transaction(pool: PgPool) {
    let psp = ProviderServer::start().await;
    let state = api(pool.clone(), &psp).await;
    let creator = user(&pool, "Creator").await;
    let game_id = game(&pool, creator, 3).await;
    let order = initiate(&state, game_id, creator, PaymentProvider::Momo)
        .await
        .unwrap();
    for code in [7002, 0, 0] {
        let callback = momo_callback(
            &state.settings,
            &order.id.simple().to_string(),
            i64::from(order.amount),
            76543,
            code,
        );
        PaymentService::handle_momo_webhook(&pool, &state.settings, &callback.to_string())
            .await
            .unwrap();
    }
    let status: PaymentStatus = sqlx::query_scalar("SELECT status FROM payments WHERE id = $1")
        .bind(order.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, PaymentStatus::Paid);
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL and Redis"]
async fn rejected_retry_does_not_release_an_earlier_uncertain_order(pool: PgPool) {
    let psp = ProviderServer::start().await;
    let state = api(pool.clone(), &psp).await;
    let creator = user(&pool, "Creator").await;
    let game_id = game(&pool, creator, 3).await;
    psp.mock
        .momo_transport_failure
        .store(true, std::sync::atomic::Ordering::SeqCst);
    assert!(matches!(
        initiate(&state, game_id, creator, PaymentProvider::Momo).await,
        Err(AppError::PaymentGateway)
    ));
    let id: Uuid = sqlx::query_scalar("SELECT id FROM payments WHERE game_id = $1")
        .bind(game_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE payments SET provider_create_claim_until = now() - interval '1 second' WHERE id = $1")
        .bind(id).execute(&pool).await.unwrap();
    psp.mock
        .momo_transport_failure
        .store(false, std::sync::atomic::Ordering::SeqCst);
    psp.mock
        .momo_result_code
        .store(20, std::sync::atomic::Ordering::SeqCst);
    assert!(matches!(
        initiate(&state, game_id, creator, PaymentProvider::Momo).await,
        Err(AppError::PaymentOrderUncertain)
    ));
    assert!(initiate(&state, game_id, creator, PaymentProvider::Momo)
        .await
        .is_err());
    let (count, held): (i64, i64) = sqlx::query_as("SELECT count(*), count(*) FILTER (WHERE status = 'disputed') FROM payments WHERE game_id = $1")
        .bind(game_id).fetch_one(&pool).await.unwrap();
    assert_eq!((count, held), (1, 1));
}
