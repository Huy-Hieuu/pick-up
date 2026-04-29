# Phase 1 — MVP Overview

## Scope

Phase 1 (weeks 5–12) delivers the core loop: **authenticate → find a court → book a slot → create a pickup game → invite friends → split the bill → pay**.

### What's in

- Phone OTP authentication with JWT tokens
- Court browsing by sport type and location
- Time-slot booking with concurrency-safe locking
- Pickup game creation and joining
- Real-time game lobby via WebSocket
- Automatic bill splitting with Momo/ZaloPay payment
- Game sharing via Zalo deeplink
- Mobile app (Expo + React Native) for all of the above

### What's explicitly out

- Court owner portal (P2)
- Push notifications (P2)
- Player ratings and reviews (P3)
- Friend system (P3)
- Redis caching (P4)
- Full-text search (P4)
- Admin dashboard

---

## Service Dependency Graph

```
                    ┌──────────────┐
                    │  API Gateway │
                    │  (Router +   │
                    │  Middleware)  │
                    └──────┬───────┘
                           │
              ┌────────────┼────────────┐
              ▼            ▼            ▼
        ┌──────────┐ ┌──────────┐ ┌──────────┐
        │   Auth   │ │  Court   │ │ External │
        │ Service  │ │ Service  │ │ Integr.  │
        └────┬─────┘ └────┬─────┘ └──────────┘
             │             │
             │        ┌────┴─────┐
             │        ▼          │
             │  ┌──────────┐    │
             ├─►│   Game   │    │
             │  │ Service  │    │
             │  └────┬─────┘    │
             │       │          │
             │       ▼          │
             │  ┌──────────┐    │
             └─►│ Payment  │◄───┘
                │ Service  │
                └──────────┘
```

**Dependencies:**
- **Auth** is foundational — every other service requires authenticated users
- **Court** is standalone after auth — manages courts and slots
- **Game** depends on Court (games reference a court_slot) and Auth (players)
- **Payment** depends on Game (bill split per game) and Auth (per-player payments)
- **External integrations** are consumed by Auth (SMS), Payment (Momo/ZaloPay), Game (Zalo deeplink), and the mobile app (Google Maps)

---

## Build Sequence

| Weeks | Focus | Deliverables |
|-------|-------|-------------|
| 5–6 | Foundation | PostgreSQL schema + migrations, Auth service (OTP + JWT), API gateway skeleton (router, middleware, error handling), mobile project scaffolding |
| 7–8 | Courts | Court service (CRUD + slot booking), mobile auth flow (login → OTP → verify), court browsing screen with map |
| 9–10 | Games | Game service (create/join/leave), WebSocket lobby, mobile game screens, Zalo deeplink sharing |
| 11–12 | Payments | Payment service + bill splitting, Momo/ZaloPay integration, mobile payment flow, integration testing, polish |

---

## Shared Error Taxonomy

All services return `Result<T, AppError>`. The gateway maps `AppError` to HTTP responses:

| AppError Variant | HTTP Status | When |
|-----------------|-------------|------|
| `Unauthorized` | 401 | Missing/invalid/expired JWT |
| `Forbidden` | 403 | User lacks permission for this action |
| `NotFound` | 404 | Resource doesn't exist |
| `Conflict` | 409 | Slot already booked, game already full, duplicate join |
| `Validation(Vec<FieldError>)` | 422 | Request body fails validation |
| `PaymentError(String)` | 502 | Payment provider returned an error |
| `Internal(anyhow::Error)` | 500 | Unexpected server error |

Response body format:

```json
{
  "error": "conflict",
  "message": "This time slot is already booked",
  "details": null
}
```

---

## Acceptance Criteria

P1 is complete when all of the following work end-to-end:

- [ ] User can sign up / log in with phone OTP
- [ ] User can browse courts filtered by sport type and location
- [ ] User can view available time slots for a court
- [ ] User can book a slot with no double-booking under concurrent requests
- [ ] User can create a pickup game tied to a booked slot
- [ ] Other users can join/leave an open game
- [ ] Game lobby updates in real-time via WebSocket (player join/leave, payment status)
- [ ] Bill is automatically split equally among players
- [ ] Each player can pay their share via Momo or ZaloPay
- [ ] Payment webhook updates status in real-time
- [ ] Game can be shared via Zalo deeplink
- [ ] 6 players can split a 600,000 VND bill and each pay via Momo with correct amounts

---

## Testing Strategy

- **Unit tests** for service layer business logic (split calculation, status transitions, OTP validation)
- **Integration tests** for route handlers with a test PostgreSQL database
- **WebSocket tests** using `tokio::test` with in-memory channels
- **Webhook tests** with mock signature generation to verify security
- Run with `cargo test` — all tests use a shared test database setup

---

## Document Index

| Document | Description |
|----------|-------------|
| [API Gateway](./p1-api-gateway.md) | Router, middleware, AppState, extractors, error handling |
| [Auth Service](./p1-auth-service.md) | Phone OTP, JWT tokens, user creation |
| [Court Service](./p1-court-service.md) | Court browsing, slot booking, concurrency |
| [Game Service](./p1-game-service.md) | Game CRUD, WebSocket lobby, sharing |
| [Payment Service](./p1-payment-service.md) | Bill splitting, Momo/ZaloPay, webhooks |
| [Database Schema](./p1-database-schema.md) | Tables, enums, indexes, migrations |
| [Mobile App](./p1-mobile-app.md) | Expo screens, hooks, stores, API client |
| [External Integrations](./p1-external-integrations.md) | SMS, Momo, ZaloPay, Google Maps, Zalo deeplink |
