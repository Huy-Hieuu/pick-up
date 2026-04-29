# Phase 1 — Auth Service

Phone OTP authentication with JWT token management. Users authenticate with their phone number — no passwords, no social login. This matches Vietnamese user behavior (phone-first) and keeps the onboarding friction minimal.

---

## API Endpoints

### POST /auth/otp — Request OTP

Sends a 6-digit OTP to the given phone number via SMS.

**Request:**
```json
{
  "phone": "+84901234567"
}
```

**Response:** `202 Accepted`
```json
{
  "message": "OTP sent",
  "expires_in": 300
}
```

**Errors:**
- `422` — invalid phone format
- `429` — rate limited (max 1 OTP per 60 seconds per phone)

**Flow:**
1. Validate phone format (Vietnamese: `+84` prefix, 9-10 digits after)
2. Check rate limit: if an unexpired OTP exists for this phone created within the last 60 seconds, return 429
3. Generate 6-digit random code
4. Store in `otp_codes` table with 5-minute expiry
5. Send via SMS provider (in dev mode, log to console)
6. Return 202

---

### POST /auth/verify — Verify OTP and get tokens

Verifies the OTP and returns JWT tokens. Creates the user on first login.

**Request:**
```json
{
  "phone": "+84901234567",
  "code": "123456"
}
```

**Response:** `200 OK`
```json
{
  "access_token": "eyJ...",
  "refresh_token": "eyJ...",
  "user": {
    "id": "uuid",
    "phone": "+84901234567",
    "display_name": null,
    "avatar_url": null
  }
}
```

**Errors:**
- `401` — invalid or expired OTP
- `401` — max attempts exceeded (3 attempts)
- `422` — missing fields

**Flow:**
1. Find the latest unexpired OTP for this phone
2. If no OTP found or expired → 401
3. Increment `attempts` counter
4. If `attempts > 3` → 401, mark OTP as exhausted
5. If code doesn't match → 401
6. Delete the OTP row (consumed)
7. Upsert user: find by phone, or create with `display_name = null`
8. Generate access token (15 min TTL) and refresh token (30 day TTL)
9. Store refresh token hash in database (for revocation)
10. Return tokens + user profile

---

### POST /auth/refresh — Refresh access token

**Request:**
```json
{
  "refresh_token": "eyJ..."
}
```

**Response:** `200 OK`
```json
{
  "access_token": "eyJ...",
  "refresh_token": "eyJ..."
}
```

**Errors:**
- `401` — invalid, expired, or revoked refresh token

**Flow:**
1. Decode and verify refresh token
2. Check token hash exists in database (not revoked)
3. Delete old refresh token hash
4. Issue new access token + new refresh token (rotation)
5. Store new refresh token hash
6. Return both tokens

---

### GET /auth/me — Current user profile

**Headers:** `Authorization: Bearer <access_token>`

**Response:** `200 OK`
```json
{
  "id": "uuid",
  "phone": "+84901234567",
  "display_name": "Hieu",
  "avatar_url": "https://...",
  "created_at": "2026-04-01T10:00:00Z"
}
```

### PATCH /auth/me — Update profile

**Headers:** `Authorization: Bearer <access_token>`

**Request:**
```json
{
  "display_name": "Hieu Nguyen"
}
```

**Response:** `200 OK` with updated user object.

---

## JWT Token Format

### Access Token

```json
{
  "sub": "user-uuid",
  "phone": "+84901234567",
  "iat": 1714300000,
  "exp": 1714300900
}
```

- Algorithm: HS256
- TTL: 15 minutes
- Signed with `JWT_SECRET`

### Refresh Token

```json
{
  "sub": "user-uuid",
  "type": "refresh",
  "iat": 1714300000,
  "exp": 1716892000
}
```

- Algorithm: HS256
- TTL: 30 days
- Signed with `JWT_REFRESH_SECRET` (separate key)
- SHA-256 hash of the token stored in DB for revocation check

---

## OTP Rules

| Rule | Value |
|------|-------|
| Code length | 6 digits |
| Expiry | 5 minutes |
| Max verification attempts | 3 per OTP |
| Rate limit | 1 OTP per 60 seconds per phone |
| Brute-force lockout | After 3 failed attempts, OTP is invalidated |

---

## Security Considerations

### Brute-force protection
- 6-digit code = 1,000,000 combinations
- Max 3 attempts per OTP limits guessing probability to 0.0003%
- Rate limit of 1 OTP per 60s prevents rapid regeneration

### Refresh token rotation
- Each refresh consumes the old token and issues a new one
- If a stolen refresh token is used after the legitimate user has already refreshed, it will fail (hash no longer in DB)
- This detects token theft

### SMS provider abstraction
```rust
// server/src/services/sms.rs
#[async_trait]
pub trait SmsProvider: Send + Sync {
    async fn send_otp(&self, phone: &str, code: &str) -> Result<(), AppError>;
}
```

Implementations:
- `ConsoleSmsProvider` — logs OTP to stdout (dev mode)
- `EsmsSmsProvider` — calls eSMS API (production)

Selected via `SMS_PROVIDER` env var.

### OTP cleanup
- Expired OTPs should be periodically deleted
- P1: clean up on each new OTP request for the same phone
- P2+: background job for bulk cleanup

---

## Data Models (Rust)

```rust
// server/src/models/user.rs
pub struct User {
    pub id: Uuid,
    pub phone: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// server/src/models/auth.rs
pub struct OtpCode {
    pub id: Uuid,
    pub phone: String,
    pub code: String,
    pub attempts: i16,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

// Request/response types in routes/auth.rs
pub struct OtpRequest {
    pub phone: String,  // validated: +84 prefix, 9-10 digits
}

pub struct VerifyRequest {
    pub phone: String,
    pub code: String,   // validated: exactly 6 digits
}

pub struct RefreshRequest {
    pub refresh_token: String,
}

pub struct AuthResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub user: User,
}
```

---

## Files to Implement

| File | Purpose |
|------|---------|
| `server/src/routes/auth.rs` | HTTP handlers for /auth/* endpoints |
| `server/src/services/auth_service.rs` | OTP generation/verification, JWT issue/refresh |
| `server/src/services/sms.rs` | SMS provider trait + implementations |
| `server/src/db/users.rs` | User CRUD queries |
| `server/src/db/auth.rs` | OTP and refresh token queries |
| `server/src/models/user.rs` | User struct |
| `server/src/models/auth.rs` | OtpCode struct, request/response types |
| `server/src/extractors/claims.rs` | JWT Claims extractor (shared with gateway) |

---

## Related Docs

- [API Gateway](./p1-api-gateway.md) — Claims extractor, JWT middleware
- [Database Schema](./p1-database-schema.md) — `users` and `otp_codes` tables
- [External Integrations](./p1-external-integrations.md) — SMS provider details
