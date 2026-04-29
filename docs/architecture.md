# PickUp — System Architecture

## 1. Overview

PickUp is a mobile-first platform that unifies sports court booking, pickup game organization, and automatic bill splitting. The system follows a monorepo structure with a React Native mobile app, a Rust backend API, and a court-owner web portal.

```
┌─────────────────────────────────────────────────────┐
│                     Clients                         │
│  ┌──────────────────┐    ┌───────────────────────┐  │
│  │  Mobile App      │    │  Court Owner Portal   │  │
│  │  Expo + React    │    │  Vite + React SPA     │  │
│  │  Native          │    │  (Phase 2)            │  │
│  └────────┬─────────┘    └───────────┬───────────┘  │
└───────────┼──────────────────────────┼──────────────┘
            │  REST + WebSocket        │  REST
            ▼                          ▼
┌─────────────────────────────────────────────────────┐
│              Rust Backend (Axum)                    │
│  ┌─────────────────────────────────────────────┐    │
│  │  Router + Middleware                        │    │
│  │  CORS · JWT verify · Request logging        │    │
│  └─────────────────────────────────────────────┘    │
│  ┌──────────┐ ┌──────────┐ ┌────────────────────┐   │
│  │  Court   │ │  Game    │ │  Payment           │   │
│  │  Service │ │  Service │ │  Service           │   │
│  └──────────┘ └──────────┘ └────────────────────┘   │
│  ┌──────────┐ ┌──────────┐ ┌────────────────────┐   │
│  │  Auth    │ │  Notif   │ │  Social            │   │
│  │  Service │ │  Service │ │  Service           │   │
│  │          │ │  (P2)    │ │  (P3)              │   │
│  └──────────┘ └──────────┘ └────────────────────┘   │
│  ┌─────────────────────────────────────────────┐    │
│  │  Shared: AppState · Errors · Extractors     │    │
│  └─────────────────────────────────────────────┘    │
└──────────────┬──────────────────────────────────────┘
               │  SQLx (compile-time checked)
               ▼
┌─────────────────────────────────────────────────────┐
│                   Data Layer                        │
│  ┌────────────┐  ┌────────────┐  ┌──────────────┐   │
│  │ PostgreSQL │  │ S3 / MinIO │  │ Redis (P4)   │   │
│  │ Core DB    │  │ Media      │  │ Cache        │   │
│  └────────────┘  └────────────┘  └──────────────┘   │
└──────────────┬──────────────────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────────────────┐
│              External Integrations                  │
│  Momo/ZaloPay · SMS Provider · Google Maps          │
│  Expo Push · Zalo Deeplink                          │
└─────────────────────────────────────────────────────┘
```

## 2. Monorepo Structure

```
pickup/
├── mobile/          # Expo + React Native app
├── web-owner/       # Court owner portal — Vite + React (Phase 2)
├── server/          # Rust backend — Axum + SQLx
├── migrations/      # SQL migrations (sqlx-cli)
├── docker/          # Docker Compose + Dockerfiles
├── docs/            # Architecture & design docs
└── .github/         # CI/CD workflows
```

## 3. Backend Architecture

### 3.1 Layered Design

The Rust backend follows a strict layered architecture. Each layer has a single responsibility and dependencies flow downward only.

```
HTTP Request
    │
    ▼
┌──────────────┐
│   Routes     │  Axum handlers — parse request, call service, return response
└──────┬───────┘
       ▼
┌──────────────┐
│  Services    │  Business logic — orchestrates DB queries, validates rules
└──────┬───────┘
       ▼
┌──────────────┐
│     DB       │  Raw SQLx queries — one function per query
└──────┬───────┘
       ▼
┌──────────────┐
│   Models     │  SQLx row types + domain structs (used across all layers)
└──────────────┘
```

**Rules:**

- Route handlers must be thin — no business logic, only request parsing and response formatting
- Services never touch HTTP types (no `axum::Json`, no status codes)
- DB functions are pure queries — no business decisions
- Models are plain data structs shared across layers

### 3.2 Key Modules

| Module          | Responsibility                        | Key Patterns                                       |
| --------------- | ------------------------------------- | -------------------------------------------------- |
| `routes/`     | HTTP handlers, request/response types | `axum::extract`, JSON responses                  |
| `services/`   | Business rules, orchestration         | Takes `&PgPool`, returns `Result<T, AppError>` |
| `db/`         | SQL queries via SQLx                  | `sqlx::query_as!`, compile-time checked          |
| `models/`     | Data structs                          | `#[derive(sqlx::FromRow, Serialize)]`            |
| `extractors/` | Custom Axum extractors                | JWT `Claims`, validated `JsonBody<T>`          |
| `middleware/` | Request pipeline layers               | Auth verification, CORS                            |
| `ws/`         | WebSocket handlers                    | Game lobby real-time updates                       |
| `jobs/`       | Background tasks (Phase 2+)           | Tokio-spawned, cron-like scheduling                |

### 3.3 Authentication Flow

```
┌──────────┐      ┌──────────┐      ┌──────────┐
│  Client  │      │  Server  │      │   SMS    │
└────┬─────┘      └────┬─────┘      └────┬─────┘
     │  POST /auth/otp  │                 │
     │  {phone}         │                 │
     │─────────────────►│  Generate OTP   │
     │                  │────────────────►│
     │                  │   Send SMS      │
     │  202 Accepted    │                 │
     │◄─────────────────│                 │
     │                  │                 │
     │  POST /auth/verify                 │
     │  {phone, otp}    │                 │
     │─────────────────►│  Verify OTP     │
     │                  │  Issue JWT pair │
     │  {access, refresh}                 │
     │◄─────────────────│                 │
     │                  │                 │
     │  Authenticated requests            │
     │  Authorization: Bearer <token>     │
     │─────────────────►│  JWT extractor  │
     │                  │  validates      │
```

### 3.4 Slot Booking — Concurrency Model

Court time-slot booking uses `SELECT ... FOR UPDATE` to prevent double-booking:

```sql
BEGIN;
  SELECT * FROM court_slots
  WHERE court_id = $1 AND start_time = $2
  FOR UPDATE;              -- Row-level lock

  -- Check slot is still available
  -- Insert booking record

COMMIT;                    -- Lock released
```

This ensures that even under concurrent requests, only one booking succeeds per slot.

### 3.5 Bill Splitting Flow

Bill splitting is the core viral feature. The flow:

```
Game creator books court
        │
        ▼
Court fee is known (e.g., 600,000 VND)
        │
        ▼
Players join game (e.g., 6 players)
        │
        ▼
System calculates split: 600,000 / 6 = 100,000 VND each
        │
        ▼
Each player receives payment request
        │
        ▼
Player pays via Momo/ZaloPay ──► Webhook confirms payment
        │
        ▼
Game screen shows real-time payment status per player
        │
        ▼
Payment nudge reminders for unpaid players (Phase 2)
```

**Design decisions:**

- Split is recalculated whenever players join/leave (before game starts)
- Game creator can optionally pay a different share
- Payment status is tracked per-player with states: `pending → paid → refunded`
- Webhook signature verification is mandatory for all payment callbacks

## 4. Frontend Architecture

### 4.1 Mobile App (Expo + React Native)

**Navigation** — File-based routing via Expo Router:

```
app/
├── _layout.tsx          # Root layout, auth gate
├── (auth)/              # Unauthenticated stack
│   ├── login.tsx        # Phone input
│   └── verify.tsx       # OTP verification
├── (tabs)/              # Authenticated bottom tabs
│   ├── explore.tsx      # Court map + search
│   ├── games.tsx        # My games
│   └── profile.tsx      # User profile
├── court/[id].tsx       # Court detail (dynamic route)
└── game/
    ├── create.tsx       # Create pickup game
    └── [id].tsx         # Game detail + join
```

**State management:**

- **Zustand stores** for global state (auth tokens, active game)
- **React hooks** for server state (useCourts, useGame wrapping API calls)
- **WebSocket** in useGame hook for live game lobby updates

**API client pattern:**

```
src/api/client.ts    →  Base fetch/axios wrapper with JWT injection
src/api/courts.ts    →  getCourts(), getCourtById(), bookSlot()
src/api/games.ts     →  createGame(), joinGame(), leaveGame()
src/api/payments.ts  →  getSplit(), initiatePayment()
```

### 4.2 Court Owner Portal (Phase 2)

Vite + React SPA with three main views:

- **Dashboard** — booking overview and revenue stats
- **Schedule** — weekly calendar grid to manage available time slots
- **Bookings** — history table with filtering

## 5. Data Model

### 5.1 Core Tables (Phase 1)

```
users
├── id (UUID, PK)
├── phone (unique)
├── display_name
├── avatar_url
└── created_at

courts
├── id (UUID, PK)
├── name
├── sport_type (enum: pickleball, mini_football, ...)
├── location (lat, lng)
├── address
├── price_per_slot (integer, VND)
├── photo_urls (text[])
└── owner_id (FK → users)

court_slots
├── id (UUID, PK)
├── court_id (FK → courts)
├── start_time (timestamptz)
├── end_time (timestamptz)
├── status (enum: available, booked, locked)
└── booked_by (FK → users, nullable)

games
├── id (UUID, PK)
├── court_slot_id (FK → court_slots)
├── creator_id (FK → users)
├── sport_type
├── max_players (integer)
├── status (enum: open, full, in_progress, completed, cancelled)
└── created_at

game_players
├── game_id (FK → games)
├── user_id (FK → users)
├── joined_at
└── PRIMARY KEY (game_id, user_id)

payments
├── id (UUID, PK)
├── game_id (FK → games)
├── user_id (FK → users)
├── amount (integer, VND)
├── provider (enum: momo, zalopay)
├── provider_txn_id (text, nullable)
├── status (enum: pending, paid, refunded)
└── paid_at (timestamptz, nullable)
```

### 5.2 Migration Strategy

Sequential numbered SQL files managed by `sqlx-cli`:

| Migration          | Phase | Tables                     |
| ------------------ | ----- | -------------------------- |
| 001_initial        | P1    | users, courts, court_slots |
| 002_games          | P1    | games, game_players        |
| 003_payments       | P1    | payments, split_records    |
| 004_owner_profiles | P2    | owner portal tables        |
| 005_push_tokens    | P2    | notification tokens        |
| 006_ratings        | P3    | ratings + avg triggers     |
| 007_friendships    | P3    | friend_requests            |
| 008_player_stats   | P3    | materialized view          |
| 009_search_indexes | P4    | GIN + pg_trgm indexes      |

## 6. External Integrations

| Service        | Purpose                   | Phase | Integration Pattern          |
| -------------- | ------------------------- | ----- | ---------------------------- |
| Momo / ZaloPay | Payment processing        | P1    | REST API + webhook callbacks |
| SMS Provider   | OTP delivery              | P1    | REST API (fire-and-forget)   |
| Google Maps    | Court locations, distance | P1    | Client-side SDK + geocoding  |
| Expo Push      | Push notifications        | P2    | Server-side REST to Expo API |
| Zalo Deeplink  | Share game invites        | P1    | URL scheme generation        |

### Payment Webhook Security

All payment webhooks (`routes/webhooks.rs`) must:

1. Verify the request signature using the provider's secret key
2. Check idempotency (ignore duplicate webhook deliveries)
3. Validate the payment amount matches the expected split amount
4. Update payment status within a database transaction

## 7. Infrastructure

### Development

```yaml
# docker/docker-compose.yml
services:
  postgres:
    image: postgres:16
    ports: ["5432:5432"]
  server:
    build: ../server
    depends_on: [postgres]
```

### Production (Phase 2+)

- **Hosting:** Fly.io (Rust binary runs as a single process)
- **Database:** Fly Postgres or Supabase managed Postgres
- **Media storage:** S3-compatible (MinIO for dev, S3 or Cloudflare R2 for prod)
- **CI/CD:** GitHub Actions — `cargo test` + `cargo clippy` on PR, auto-deploy on merge to main

### Production additions (Phase 4)

```yaml
# docker/docker-compose.prod.yml (extends base)
services:
  redis:
    image: redis:7-alpine
  # + monitoring stack
```

## 8. Phased Delivery

| Phase                     | Weeks  | Scope                                                                                                           |
| ------------------------- | ------ | --------------------------------------------------------------------------------------------------------------- |
| **P1 — MVP**       | 5–12  | Auth (OTP+JWT), court browsing/booking, game create/join with WebSocket lobby, bill splitting with Momo/ZaloPay |
| **P2 — Iterate**   | 13–18 | Court owner portal, push notifications, payment tracking UI, Fly.io deployment                                  |
| **P3 — Community** | 19–24 | Player profiles + ratings, friend system, game history/recaps                                                   |
| **P4 — Scale**     | TBD    | Redis caching, full-text search (pg_trgm + tsvector), rate limiting, monitoring                                 |

## 9. Key Design Decisions

| Decision                          | Rationale                                                                             |
| --------------------------------- | ------------------------------------------------------------------------------------- |
| Rust + Axum over Node.js          | Type safety, performance for WebSocket + concurrent booking, single binary deployment |
| SQLx over ORM (Diesel/SeaORM)     | Compile-time checked SQL, full control over queries, simpler mental model             |
| Expo Router over React Navigation | File-based routing, better DX, built-in deep linking support                          |
| Zustand over Redux                | Minimal boilerplate, works well with hooks pattern, sufficient for app complexity     |
| Phone OTP over email/social login | Matches Vietnamese user behavior (phone-first), simpler flow                          |
| Monorepo over multi-repo          | Shared types possible, single CI pipeline, easier for 3-person team                   |
| PostgreSQL over Supabase backend  | Full control, compile-time SQL checks with SQLx, custom WebSocket logic               |
