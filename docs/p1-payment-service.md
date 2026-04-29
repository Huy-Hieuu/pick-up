# Phase 1 — Payment Service

Bill splitting is the #1 viral growth hook. It solves the real-world pain of "who owes what" after playing sports together. Every design decision here optimizes for low friction, real-time visibility, and social pressure to pay.

---

## Core Concept

```
Court fee is known (e.g., 600,000 VND)
        │
        ▼
Players join the game (e.g., 6 players)
        │
        ▼
System auto-calculates: 600,000 / 6 = 100,000 VND each
        │
        ▼
Each player sees their share + who has paid (social pressure)
        │
        ▼
Player taps "Pay" → opens Momo/ZaloPay → completes payment
        │
        ▼
Webhook confirms → game screen updates in real-time for everyone
```

---

## API Endpoints

### GET /games/:id/split — Current split calculation

Returns the live bill split based on current player count.

**Response:** `200 OK`
```json
{
  "game_id": "uuid",
  "total_amount": 600000,
  "player_count": 6,
  "per_player_amount": 100000,
  "creator_amount": 100000,
  "players": [
    {
      "user_id": "uuid",
      "display_name": "Hieu",
      "amount": 100000,
      "payment_status": "paid",
      "is_creator": true
    },
    {
      "user_id": "uuid",
      "display_name": "Minh",
      "amount": 100000,
      "payment_status": "pending",
      "is_creator": false
    }
  ],
  "paid_count": 2,
  "paid_total": 200000,
  "remaining_total": 400000
}
```

---

### POST /games/:id/payments/initiate — Start payment

Initiates payment for the calling user via their chosen provider.

**Request:**
```json
{
  "provider": "momo"
}
```

**Response:** `200 OK`
```json
{
  "payment_id": "uuid",
  "amount": 100000,
  "provider": "momo",
  "payment_url": "https://test-payment.momo.vn/...",
  "expires_at": "2026-05-01T11:30:00Z"
}
```

**Errors:**
- `404` — not a member of this game
- `409` — already has an active (pending/paid) payment for this game
- `422` — game not in payable state (must be `open`, `full`, or `in_progress`)
- `502` — payment provider returned an error

---

### GET /games/:id/payments — All payments for a game

**Response:** `200 OK`
```json
{
  "payments": [
    {
      "id": "uuid",
      "user_id": "uuid",
      "display_name": "Hieu",
      "amount": 100000,
      "provider": "momo",
      "status": "paid",
      "paid_at": "2026-05-01T10:30:00Z"
    }
  ]
}
```

---

### POST /webhooks/momo — Momo payment callback

No JWT — verified by HMAC signature.

**Request body (from Momo):**
```json
{
  "partnerCode": "PICKUP",
  "orderId": "payment-uuid",
  "requestId": "req-uuid",
  "amount": 100000,
  "orderInfo": "PickUp game payment",
  "orderType": "momo_wallet",
  "transId": 12345678,
  "resultCode": 0,
  "message": "Successful.",
  "responseTime": 1714560000000,
  "extraData": "",
  "signature": "abc123..."
}
```

**Response:** Always `200 OK` (even on internal error — prevents provider retries).

---

### POST /webhooks/zalopay — ZaloPay payment callback

**Request body (from ZaloPay):**
```json
{
  "data": "{\"app_id\":1234,\"app_trans_id\":\"payment-uuid\",\"app_time\":1714560000,\"amount\":100000,...}",
  "mac": "hmac-sha256-signature",
  "type": 1
}
```

**Response:** Always `200 OK` with `{"return_code": 1}`.

---

## Split Calculation Rules

### Base formula

```
per_player_amount = floor(total_amount / player_count)
```

All amounts are in VND (integers, no decimals).

### Remainder handling

```
remainder = total_amount % player_count
creator_amount = per_player_amount + remainder
```

Example: 600,000 VND / 7 players = 85,714 VND each. Remainder = 2 VND. Creator pays 85,716 VND.

This is documented clearly in the UI: "Creator pays the extra X VND to round up."

### Creator override

The game creator can optionally set their own contribution:

```
PATCH /games/:id/split
{ "creator_amount": 200000 }
```

Remaining amount is split equally among other players:
```
other_player_amount = floor((total_amount - creator_amount) / (player_count - 1))
```

### Recalculation triggers

The split is recalculated every time:
- A player **joins** the game
- A player **leaves** the game

On each recalculation:
1. Count current players
2. Recompute per-player amounts
3. **Already-paid players are NOT affected** — their payment stands at the amount they paid
4. Only `pending` and new players get the updated amount
5. Broadcast `split_recalculated` via WebSocket

### Edge case: player leaves after others paid

If 6 players at 100,000 each, 2 have paid, then 1 unpaid player leaves:
- New split: 600,000 / 5 = 120,000 each
- The 2 who paid 100,000 are NOT asked to pay more (their payments are final)
- Deficit: (120,000 - 100,000) * 2 = 40,000 VND
- This deficit is spread across the 3 remaining unpaid players: 120,000 + (40,000 / 3) = ~133,333 each
- Exact formula: `remaining_unpaid = total_amount - sum_of_paid_amounts` / `unpaid_count`

### Minimum payment

Momo minimum transaction: 10,000 VND. If the per-player split is below this, the "Pay" button is disabled with a message: "Split amount is below the minimum payment threshold."

---

## Payment Flow — Step by Step

```
┌──────────┐     ┌──────────┐     ┌──────────┐     ┌──────────┐
│  Mobile   │     │  Server  │     │  Momo/   │     │  Other   │
│  App      │     │          │     │  ZaloPay │     │  Clients │
└─────┬────┘     └─────┬────┘     └─────┬────┘     └─────┬────┘
      │                 │                │                │
  1.  │ POST /payments  │                │                │
      │ /initiate       │                │                │
      │ {provider:momo} │                │                │
      │────────────────►│                │                │
      │                 │                │                │
  2.  │                 │  Create payment row             │
      │                 │  status: pending                │
      │                 │                │                │
  3.  │                 │  POST create   │                │
      │                 │  order         │                │
      │                 │───────────────►│                │
      │                 │                │                │
  4.  │                 │  {payment_url} │                │
      │                 │◄───────────────│                │
      │                 │                │                │
  5.  │  {payment_url}  │                │                │
      │◄────────────────│                │                │
      │                 │                │                │
  6.  │  Open URL ──────┼───────────────►│                │
      │  (Momo app)     │                │                │
      │                 │                │                │
  7.  │                 │                │  User pays     │
      │                 │                │  in Momo app   │
      │                 │                │                │
  8.  │                 │  POST webhook  │                │
      │                 │◄───────────────│                │
      │                 │                │                │
  9.  │                 │  Verify sig    │                │
      │                 │  Update status │                │
      │                 │  → paid        │                │
      │                 │                │                │
 10.  │                 │  WS: payment   │                │
      │  WS: payment    │  _updated      │  WS: payment  │
      │  _updated       │────────────────┼──────────────►│
      │◄────────────────│                │                │
```

**Detailed steps:**

1. User taps "Pay my share" on game screen, chooses Momo or ZaloPay
2. Server creates `payments` row: `status = pending`, `amount = per_player_amount`
3. Server calls Momo/ZaloPay API to create a payment order, passing `payment_id` as the order reference
4. Provider returns a `payment_url`
5. Server returns `payment_url` to mobile app
6. Mobile app calls `Linking.openURL(payment_url)` — opens Momo/ZaloPay app
7. User completes payment in the provider's app
8. Provider sends HTTP POST webhook to our server
9. Server verifies webhook signature, updates payment status to `paid`, sets `paid_at`
10. Server broadcasts `payment_updated` to all WebSocket clients in the game lobby

---

## Webhook Processing

### Signature Verification

**Momo:**
```
raw_signature = "accessKey={}&amount={}&extraData={}&message={}&orderId={}&orderInfo={}&orderType={}&partnerCode={}&payType={}&requestId={}&responseTime={}&resultCode={}&transId={}"
signature = HMAC-SHA256(raw_signature, secret_key)
```
Fields are sorted alphabetically and concatenated with `&`.

**ZaloPay:**
```
raw_data = request_body["data"]  // JSON string
signature = HMAC-SHA256(raw_data, key2)
```

### Idempotency

```rust
// Check if this provider transaction was already processed
let existing = db::payments::find_by_provider_txn_id(pool, &txn_id).await?;
if existing.is_some() {
    return Ok(());  // Already processed, return success
}
```

The unique index on `provider_txn_id` also prevents duplicate inserts at the database level.

### Amount Validation

```rust
if webhook_amount != expected_payment.amount {
    tracing::warn!(
        payment_id = %payment.id,
        expected = expected_payment.amount,
        received = webhook_amount,
        "Payment amount mismatch"
    );
    // Mark as disputed for manual review
    db::payments::update_status(pool, payment.id, PaymentStatus::Disputed).await?;
    return Ok(());
}
```

### Transaction Safety

The entire webhook handler runs inside a database transaction:

```rust
let mut tx = pool.begin().await?;
// 1. Verify signature
// 2. Find payment by order_id
// 3. Check idempotency
// 4. Validate amount
// 5. Update payment status
// 6. Commit
tx.commit().await?;
// 7. Broadcast WebSocket event (after commit)
```

### Response to Provider

**Always return 200 OK**, even if internal processing fails. If we return an error, the provider will retry, but our internal state may be inconsistent. Instead:
- Log the error with full context
- Return 200 to stop retries
- Fix manually or via a retry mechanism

---

## Payment Status State Machine

```
              ┌──────────────┐
  initiated   │              │
  ──────────► │   pending    │
              │              │
              └──────┬───────┘
                     │
            ┌────────┼────────┐
            ▼        │        ▼
     ┌───────────┐   │   ┌──────────┐
     │   paid    │   │   │ expired  │
     └─────┬─────┘   │   └──────────┘
           │         │     (30 min TTL)
           ▼         │
     ┌───────────┐   │
     │ refunded  │   │
     └───────────┘   │
      (manual/cancel)│
                     ▼
              ┌───────────┐
              │ disputed  │
              └───────────┘
              (amount mismatch)
```

- `pending` → `paid`: webhook confirms successful payment
- `pending` → `expired`: no webhook received within 30 minutes (cleanup job)
- `paid` → `refunded`: game cancelled or admin action (manual in P1)
- `pending` → `disputed`: webhook amount doesn't match expected amount

---

## Edge Cases

### Player leaves after paying

- The leaving player's payment remains `paid` — no auto-refund in P1
- The split is recalculated for remaining players
- The paid player's contribution reduces the remaining total:
  ```
  remaining_total = total_amount - sum(paid_amounts)
  per_unpaid_player = remaining_total / unpaid_player_count
  ```

### Game cancelled after some payments

- All `pending` payments are set to `expired`
- All `paid` payments are flagged for manual refund (admin tool in P2)
- A `game_cancelled` WebSocket event is broadcast
- The game detail page shows "Game cancelled — refunds will be processed within 3 business days"

### Webhook arrives before payment row exists (race condition)

Possible if provider sends webhook extremely fast. Handle with:
1. Look up payment by our `order_id` (which equals `payment_id`)
2. If not found, return 200 but log a warning
3. The provider will retry (most retry 3 times over 15 minutes)
4. By the next retry, the payment row should exist

### Double webhook delivery

Handled by the idempotency check on `provider_txn_id`. The second delivery finds the payment already processed and returns 200 immediately.

### Payment timeout

`pending` payments older than 30 minutes are marked `expired`. In P1, this is checked:
- On each `GET /games/:id/split` request (lazy cleanup)
- Before initiating a new payment (expire old ones first)

P2 adds a background job for periodic cleanup.

### Concurrent payment initiation

A user should not be able to initiate two payments for the same game. The `POST /initiate` endpoint checks:
1. If an active (`pending` or `paid`) payment exists → return 409
2. If an `expired` payment exists → allow re-initiation (creates a new payment row)

---

## Observability

Even in P1, log these events with structured `tracing` fields:

| Event | Fields |
|-------|--------|
| Payment initiated | `game_id`, `user_id`, `amount`, `provider` |
| Webhook received | `provider`, `txn_id`, `amount`, `result_code` |
| Webhook signature valid/invalid | `provider`, `is_valid` |
| Payment status updated | `payment_id`, `old_status`, `new_status` |
| Split recalculated | `game_id`, `player_count`, `per_player_amount`, `total_amount` |
| Amount mismatch | `payment_id`, `expected`, `received` |

---

## Data Models (Rust)

```rust
// server/src/models/payment.rs
pub struct Payment {
    pub id: Uuid,
    pub game_id: Uuid,
    pub user_id: Uuid,
    pub amount: i32,
    pub provider: PaymentProvider,
    pub provider_txn_id: Option<String>,
    pub status: PaymentStatus,
    pub paid_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

pub struct SplitInfo {
    pub total_amount: i32,
    pub player_count: i32,
    pub per_player_amount: i32,
    pub creator_amount: i32,
    pub paid_count: i32,
    pub paid_total: i32,
    pub remaining_total: i32,
}

pub struct PlayerSplitEntry {
    pub user_id: Uuid,
    pub display_name: Option<String>,
    pub amount: i32,
    pub payment_status: Option<PaymentStatus>,
    pub is_creator: bool,
}

pub struct InitiatePaymentRequest {
    pub provider: PaymentProvider,
}

pub struct InitiatePaymentResponse {
    pub payment_id: Uuid,
    pub amount: i32,
    pub provider: PaymentProvider,
    pub payment_url: String,
    pub expires_at: DateTime<Utc>,
}

// Webhook request bodies
pub struct MomoWebhook {
    pub partner_code: String,
    pub order_id: String,
    pub request_id: String,
    pub amount: i64,
    pub order_info: String,
    pub order_type: String,
    pub trans_id: i64,
    pub result_code: i32,
    pub message: String,
    pub response_time: i64,
    pub extra_data: String,
    pub signature: String,
}

pub struct ZaloPayWebhook {
    pub data: String,    // JSON string
    pub mac: String,     // HMAC-SHA256 signature
    pub r#type: i32,
}
```

---

## Files to Implement

| File | Purpose |
|------|---------|
| `server/src/routes/payments.rs` | HTTP handlers for split, initiate, list payments |
| `server/src/routes/webhooks.rs` | Momo and ZaloPay webhook handlers |
| `server/src/services/payment_service.rs` | Split calculation, payment initiation, webhook processing |
| `server/src/services/momo.rs` | Momo API client (create order, verify signature) |
| `server/src/services/zalopay.rs` | ZaloPay API client (create order, verify signature) |
| `server/src/db/payments.rs` | Payment CRUD queries |
| `server/src/models/payment.rs` | Payment, SplitInfo, webhook structs |

---

## Related Docs

- [Game Service](./p1-game-service.md) — join/leave triggers recalculation, WebSocket broadcasts payment updates
- [Database Schema](./p1-database-schema.md) — `payments` table
- [External Integrations](./p1-external-integrations.md) — Momo/ZaloPay API details and sandbox setup
- [API Gateway](./p1-api-gateway.md) — webhook routes bypass JWT, use signature verification instead
