# Phase 1 — Database Schema

PostgreSQL 16 with SQLx compile-time checked queries. Migrations managed by `sqlx-cli`.

---

## Entity Relationship Diagram

```
┌──────────────┐       ┌──────────────────┐       ┌──────────────────┐
│    users     │       │     courts       │       │   court_slots    │
├──────────────┤       ├──────────────────┤       ├──────────────────┤
│ id (PK)      │◄──┐   │ id (PK)          │◄──────│ court_id (FK)    │
│ phone        │   │   │ name             │       │ id (PK)          │
│ display_name │   │   │ sport_type       │       │ start_time       │
│ avatar_url   │   │   │ lat, lng         │       │ end_time         │
│ created_at   │   │   │ address          │       │ status           │
│ updated_at   │   │   │ price_per_slot   │       │ booked_by (FK)───┼──┐
└──────────────┘   │   │ photo_urls       │       └────────┬─────────┘  │
       ▲           │   │ owner_id (FK)────┼──┐             │            │
       │           │   │ created_at       │  │             │            │
       │           │   └──────────────────┘  │             │            │
       │           │                         │             │            │
       │           │   ┌──────────────────┐  │   ┌────────┴─────────┐  │
       │           │   │   otp_codes      │  │   │     games        │  │
       │           │   ├──────────────────┤  │   ├──────────────────┤  │
       │           │   │ id (PK)          │  │   │ id (PK)          │  │
       │           ├───│ user_id (FK)     │  │   │ court_slot_id(FK)│  │
       │           │   │ code             │  │   │ creator_id (FK)──┼──┤
       │           │   │ expires_at       │  │   │ sport_type       │  │
       │           │   │ attempts         │  │   │ max_players      │  │
       │           │   │ created_at       │  │   │ description      │  │
       │           │   └──────────────────┘  │   │ status           │  │
       │           │                         │   │ created_at       │  │
       │           │                         │   └────────┬─────────┘  │
       │           │                         │            │            │
       │           │   ┌──────────────────┐  │   ┌────────┴─────────┐  │
       │           │   │  game_players    │  │   │    payments      │  │
       │           │   ├──────────────────┤  │   ├──────────────────┤  │
       │           ├───│ user_id (FK, PK) │  │   │ id (PK)          │  │
       │           │   │ game_id (FK, PK) │  │   │ game_id (FK)     │  │
       │           │   │ joined_at        │  │   │ user_id (FK)─────┼──┘
       │           │   └──────────────────┘  │   │ amount           │
       │           │                         │   │ provider         │
       │           └─────────────────────────┘   │ provider_txn_id  │
       │                                         │ status           │
       └─────────────────────────────────────────│ paid_at          │
                                                 │ created_at       │
                                                 └──────────────────┘
```

---

## Enum Types

```sql
CREATE TYPE sport_type AS ENUM ('pickleball', 'mini_football');

CREATE TYPE slot_status AS ENUM ('available', 'booked', 'locked');

CREATE TYPE game_status AS ENUM ('open', 'full', 'in_progress', 'completed', 'cancelled');

CREATE TYPE payment_status AS ENUM ('pending', 'paid', 'expired', 'refunded', 'disputed');

CREATE TYPE payment_provider AS ENUM ('momo', 'zalopay');
```

---

## Table Definitions

### users

```sql
CREATE TABLE users (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    phone       VARCHAR(15) NOT NULL UNIQUE,
    display_name VARCHAR(100),
    avatar_url  TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_users_phone ON users(phone);
```

### otp_codes

Stores OTP codes for phone verification. Rows are short-lived and cleaned up after expiry.

```sql
CREATE TABLE otp_codes (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    phone       VARCHAR(15) NOT NULL,
    code        VARCHAR(6) NOT NULL,
    attempts    SMALLINT NOT NULL DEFAULT 0,
    expires_at  TIMESTAMPTZ NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_otp_codes_phone ON otp_codes(phone, expires_at);
```

### courts

```sql
CREATE TABLE courts (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name            VARCHAR(200) NOT NULL,
    sport_type      sport_type NOT NULL,
    lat             DOUBLE PRECISION NOT NULL,
    lng             DOUBLE PRECISION NOT NULL,
    address         TEXT NOT NULL,
    price_per_slot  INTEGER NOT NULL,  -- VND
    photo_urls      TEXT[] NOT NULL DEFAULT '{}',
    owner_id        UUID REFERENCES users(id),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_courts_sport_type ON courts(sport_type);
```

### court_slots

```sql
CREATE TABLE court_slots (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    court_id    UUID NOT NULL REFERENCES courts(id) ON DELETE CASCADE,
    start_time  TIMESTAMPTZ NOT NULL,
    end_time    TIMESTAMPTZ NOT NULL,
    status      slot_status NOT NULL DEFAULT 'available',
    booked_by   UUID REFERENCES users(id),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT valid_time_range CHECK (end_time > start_time)
);

CREATE INDEX idx_court_slots_court_date ON court_slots(court_id, start_time);
CREATE INDEX idx_court_slots_status ON court_slots(status) WHERE status = 'available';
```

### games

```sql
CREATE TABLE games (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    court_slot_id   UUID NOT NULL REFERENCES court_slots(id),
    creator_id      UUID NOT NULL REFERENCES users(id),
    sport_type      sport_type NOT NULL,
    max_players     SMALLINT NOT NULL,
    description     TEXT,
    status          game_status NOT NULL DEFAULT 'open',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT valid_max_players CHECK (max_players BETWEEN 2 AND 30)
);

CREATE INDEX idx_games_status ON games(status) WHERE status IN ('open', 'full');
CREATE INDEX idx_games_creator ON games(creator_id);
CREATE INDEX idx_games_court_slot ON games(court_slot_id);
```

### game_players

```sql
CREATE TABLE game_players (
    game_id     UUID NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    user_id     UUID NOT NULL REFERENCES users(id),
    joined_at   TIMESTAMPTZ NOT NULL DEFAULT now(),

    PRIMARY KEY (game_id, user_id)
);

CREATE INDEX idx_game_players_user ON game_players(user_id);
```

### payments

```sql
CREATE TABLE payments (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    game_id         UUID NOT NULL REFERENCES games(id),
    user_id         UUID NOT NULL REFERENCES users(id),
    amount          INTEGER NOT NULL,  -- VND
    provider        payment_provider NOT NULL,
    provider_txn_id TEXT,
    status          payment_status NOT NULL DEFAULT 'pending',
    paid_at         TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT positive_amount CHECK (amount > 0)
);

CREATE INDEX idx_payments_game ON payments(game_id);
CREATE INDEX idx_payments_user ON payments(user_id);
CREATE UNIQUE INDEX idx_payments_provider_txn ON payments(provider_txn_id)
    WHERE provider_txn_id IS NOT NULL;
```

---

## Migration Files

### 001_initial.sql

Creates foundational tables and enums.

```
- CREATE TYPE sport_type, slot_status
- CREATE TABLE users
- CREATE TABLE otp_codes
- CREATE TABLE courts
- CREATE TABLE court_slots
- All indexes for the above tables
```

### 002_games.sql

Adds game-related tables.

```
- CREATE TYPE game_status
- CREATE TABLE games
- CREATE TABLE game_players
- All indexes for the above tables
```

### 003_payments.sql

Adds payment tracking.

```
- CREATE TYPE payment_status, payment_provider
- CREATE TABLE payments
- All indexes for the above table
```

---

## Seed Data

Development seed file (`seed.sql`) includes:

- 2 test users (with known phone numbers for OTP bypass in dev)
- 8 courts in Ho Chi Minh City:
  - 4 pickleball courts (District 1, 2, 7, Thu Duc)
  - 4 mini football courts (District 3, 7, Binh Thanh, Go Vap)
- Time slots generated for the next 14 days per court (hourly slots, 6 AM to 10 PM)
- Prices ranging from 200,000 to 500,000 VND per slot

---

## Related Docs

- [Auth Service](./p1-auth-service.md) — uses `users`, `otp_codes`
- [Court Service](./p1-court-service.md) — uses `courts`, `court_slots`
- [Game Service](./p1-game-service.md) — uses `games`, `game_players`
- [Payment Service](./p1-payment-service.md) — uses `payments`
