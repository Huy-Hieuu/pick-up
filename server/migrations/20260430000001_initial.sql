-- PickUp initial schema — Phase 1 (MVP)
-- All monetary values in VND (integer, no decimals).

-- ── Custom enum types ──────────────────────────────────────────

CREATE TYPE sport_type AS ENUM ('pickleball', 'mini_football');
CREATE TYPE slot_status AS ENUM ('available', 'booked', 'locked');
CREATE TYPE game_status AS ENUM ('open', 'full', 'in_progress', 'completed', 'cancelled');
CREATE TYPE payment_provider AS ENUM ('momo', 'zalopay');
CREATE TYPE payment_status AS ENUM ('pending', 'paid', 'expired', 'refunded');

-- ── Updated_at trigger helper (used by multiple tables) ────────

CREATE OR REPLACE FUNCTION update_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- ── Users ──────────────────────────────────────────────────────

CREATE TABLE users (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    phone       VARCHAR(20) NOT NULL UNIQUE,
    display_name VARCHAR(50),
    avatar_url  TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TRIGGER trg_users_updated_at
    BEFORE UPDATE ON users
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();

-- ── Courts ─────────────────────────────────────────────────────

CREATE TABLE courts (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name            VARCHAR(200) NOT NULL,
    sport_type      sport_type NOT NULL,
    lat             DOUBLE PRECISION NOT NULL,
    lng             DOUBLE PRECISION NOT NULL,
    address         TEXT NOT NULL,
    price_per_slot  INTEGER NOT NULL,          -- VND
    photo_urls      TEXT[] NOT NULL DEFAULT '{}',
    owner_id        UUID REFERENCES users(id),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_courts_sport_type ON courts (sport_type);
CREATE INDEX idx_courts_location ON courts (lat, lng);

CREATE TRIGGER trg_courts_updated_at
    BEFORE UPDATE ON courts
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();

-- ── Court slots ────────────────────────────────────────────────

CREATE TABLE court_slots (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    court_id    UUID NOT NULL REFERENCES courts(id) ON DELETE CASCADE,
    start_time  TIMESTAMPTZ NOT NULL,
    end_time    TIMESTAMPTZ NOT NULL,
    status      slot_status NOT NULL DEFAULT 'available',
    booked_by   UUID REFERENCES users(id),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT slot_time_range CHECK (end_time > start_time)
);

CREATE INDEX idx_court_slots_court_status ON court_slots (court_id, status);
CREATE INDEX idx_court_slots_time ON court_slots (start_time, end_time);

-- ── Games ──────────────────────────────────────────────────────

CREATE TABLE games (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    court_slot_id   UUID NOT NULL REFERENCES court_slots(id) ON DELETE RESTRICT,
    creator_id      UUID NOT NULL REFERENCES users(id),
    sport_type      sport_type NOT NULL,
    max_players     SMALLINT NOT NULL DEFAULT 10,
    description     TEXT,
    status          game_status NOT NULL DEFAULT 'open',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT max_players_range CHECK (max_players BETWEEN 2 AND 50)
);

CREATE INDEX idx_games_status ON games (status);
CREATE INDEX idx_games_creator ON games (creator_id);
CREATE INDEX idx_games_sport ON games (sport_type);

CREATE TRIGGER trg_games_updated_at
    BEFORE UPDATE ON games
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();

-- ── Game players (join table) ──────────────────────────────────

CREATE TABLE game_players (
    game_id     UUID NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    joined_at   TIMESTAMPTZ NOT NULL DEFAULT now(),

    PRIMARY KEY (game_id, user_id)
);

-- Index for "all games a user has joined" queries.
CREATE INDEX idx_game_players_user ON game_players (user_id);

-- ── Payments ───────────────────────────────────────────────────

CREATE TABLE payments (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    game_id         UUID NOT NULL REFERENCES games(id),
    user_id         UUID NOT NULL REFERENCES users(id),
    amount          INTEGER NOT NULL,           -- VND
    provider        payment_provider NOT NULL,
    provider_txn_id TEXT,
    status          payment_status NOT NULL DEFAULT 'pending',
    paid_at         TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT amount_positive CHECK (amount > 0)
);

CREATE INDEX idx_payments_game ON payments (game_id);
CREATE INDEX idx_payments_user_status ON payments (user_id, status);
CREATE UNIQUE INDEX idx_payments_provider_txn ON payments (provider_txn_id) WHERE provider_txn_id IS NOT NULL;

CREATE TRIGGER trg_payments_updated_at
    BEFORE UPDATE ON payments
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();
