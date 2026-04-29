# Phase 1 — API Gateway

The API gateway is not a separate service — it is the Axum router and middleware layer within the single Rust binary that fronts all service endpoints.

---

## Route Tree

```
/health                          GET     No auth     Health check
/auth/otp                        POST    No auth     Request OTP
/auth/verify                     POST    No auth     Verify OTP → JWT
/auth/refresh                    POST    No auth     Refresh access token
/auth/me                         GET     JWT         Current user profile
/courts                          GET     No auth     List courts (public)
/courts/:id                      GET     No auth     Court detail (public)
/courts/:id/slots                GET     No auth     Available slots (public)
/courts/:id/slots/:slot_id/book  POST    JWT         Book a slot
/games                           GET     JWT         List open games
/games                           POST    JWT         Create game
/games/:id                       GET     JWT         Game detail
/games/:id/join                  POST    JWT         Join game
/games/:id/leave                 POST    JWT         Leave game
/games/:id/cancel                POST    JWT         Cancel game (creator)
/games/:id/status                PATCH   JWT         Update game status (creator)
/games/:id/split                 GET     JWT         Current split calculation
/games/:id/share                 GET     JWT         Share deeplink URL
/games/:id/payments              GET     JWT         Payment status for all players
/games/:id/payments/initiate     POST    JWT         Start payment flow
/webhooks/momo                   POST    No auth*    Momo payment callback
/webhooks/zalopay                POST    No auth*    ZaloPay payment callback
/ws/games/:id                    GET     JWT (query) WebSocket upgrade
```

*Webhook routes bypass JWT but verify provider signatures instead.

---

## Router Composition

```rust
// server/src/routes/mod.rs
pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .nest("/auth", auth_routes())           // no auth middleware
        .nest("/webhooks", webhook_routes())     // no auth, signature-verified
        .nest("/courts", public_court_routes())  // no auth for GET
        .nest("/courts", protected_court_routes().layer(auth_middleware()))
        .nest("/games", game_routes().layer(auth_middleware()))
        .nest("/ws", ws_routes())                // JWT verified on upgrade
        .layer(cors_layer())
        .layer(request_logging_layer())
        .with_state(state)
}
```

---

## Middleware Stack

Applied in order (outermost first):

| Order | Middleware | Purpose |
|-------|-----------|---------|
| 1 | Request logging | `tracing` spans with request ID, method, path, status, duration |
| 2 | CORS | Allow mobile app origins, required headers |
| 3 | JWT verification | Applied to protected route groups only |

### CORS Configuration

```rust
CorsLayer::new()
    .allow_origin([
        "http://localhost:8081".parse().unwrap(),        // Expo dev
        "https://pickup.app".parse().unwrap(),           // production
    ])
    .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
    .allow_headers([AUTHORIZATION, CONTENT_TYPE])
    .max_age(Duration::from_secs(3600))
```

### Request Logging

Every request gets a `tracing` span with:
- Auto-generated request ID (UUID)
- HTTP method and path
- Response status code and duration
- User ID (if authenticated)

---

## AppState

Shared application state passed to all handlers via Axum's `State` extractor.

```rust
// server/src/state.rs
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub jwt_secret: String,
    pub jwt_refresh_secret: String,
    pub sms_config: SmsConfig,
    pub momo_config: MomoConfig,
    pub zalopay_config: ZaloPayConfig,
    pub app_url: String,                // for deeplink generation
}
```

Constructed at startup in `main.rs` from environment variables:

```
DATABASE_URL=postgres://user:pass@localhost:5432/pickup
JWT_SECRET=<random-64-bytes>
JWT_REFRESH_SECRET=<random-64-bytes>
SMS_PROVIDER=esms|speedsms|console
SMS_API_KEY=<key>
MOMO_PARTNER_CODE=<code>
MOMO_ACCESS_KEY=<key>
MOMO_SECRET_KEY=<key>
MOMO_API_URL=https://test-payment.momo.vn/v2/gateway/api
ZALOPAY_APP_ID=<id>
ZALOPAY_KEY1=<key1>
ZALOPAY_KEY2=<key2>
ZALOPAY_API_URL=https://sb-openapi.zalopay.vn/v2
APP_URL=https://pickup.app
```

---

## Custom Extractors

### Claims (JWT authentication)

Extracts user identity from the `Authorization: Bearer <token>` header.

```rust
// server/src/extractors/claims.rs
pub struct Claims {
    pub sub: Uuid,          // user ID
    pub phone: String,
    pub iat: i64,
    pub exp: i64,
}
```

**Behavior:**
- Missing header → `401 Unauthorized`
- Invalid/expired token → `401 Unauthorized`
- Valid token → `Claims` available in handler

### JsonBody\<T\> (validated request body)

Deserializes and validates the request body.

```rust
// server/src/extractors/json.rs
pub struct JsonBody<T: DeserializeOwned + Validate>(pub T);
```

**Behavior:**
- Invalid JSON → `422` with parse error details
- Validation failure → `422` with field-level errors:
  ```json
  {
    "error": "validation",
    "message": "Request validation failed",
    "details": [
      {"field": "phone", "message": "must be a valid phone number"},
      {"field": "max_players", "message": "must be between 2 and 30"}
    ]
  }
  ```

---

## Error Handling

```rust
// server/src/error.rs
pub enum AppError {
    Unauthorized,
    Forbidden,
    NotFound,
    Conflict(String),
    Validation(Vec<FieldError>),
    PaymentError(String),
    Internal(anyhow::Error),
}

pub struct FieldError {
    pub field: String,
    pub message: String,
}
```

`impl IntoResponse for AppError` maps each variant:

| Variant | Status | Body `error` field |
|---------|--------|-------------------|
| `Unauthorized` | 401 | `"unauthorized"` |
| `Forbidden` | 403 | `"forbidden"` |
| `NotFound` | 404 | `"not_found"` |
| `Conflict(msg)` | 409 | `"conflict"` |
| `Validation(errs)` | 422 | `"validation"` |
| `PaymentError(msg)` | 502 | `"payment_error"` |
| `Internal(err)` | 500 | `"internal_error"` |

Internal errors log the full error with `tracing::error!` but return a generic message to the client.

---

## Health Check

```
GET /health
→ 200 { "status": "ok", "version": "0.1.0" }
```

Checks database connectivity with a `SELECT 1` query. Returns 503 if the database is unreachable.

---

## Files to Implement

| File | Purpose |
|------|---------|
| `server/src/main.rs` | Server startup, AppState init, router assembly |
| `server/src/state.rs` | AppState struct |
| `server/src/error.rs` | AppError enum + IntoResponse |
| `server/src/config.rs` | Environment variable parsing |
| `server/src/routes/mod.rs` | Router composition |
| `server/src/middleware/auth.rs` | JWT verification layer |
| `server/src/middleware/cors.rs` | CORS configuration |
| `server/src/extractors/claims.rs` | JWT Claims extractor |
| `server/src/extractors/json.rs` | Validated JSON body extractor |

---

## Related Docs

- [Auth Service](./p1-auth-service.md) — JWT token format consumed by Claims extractor
- [Payment Service](./p1-payment-service.md) — webhook routes bypass JWT, use signature verification
- [Overview](./p1-overview.md) — shared error taxonomy
