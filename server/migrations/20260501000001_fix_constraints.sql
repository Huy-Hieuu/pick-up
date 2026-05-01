-- Fix FK cascade actions (C6).
ALTER TABLE courts
    ALTER COLUMN owner_id SET NOT NULL;

-- owner_id: no ON DELETE CASCADE needed — owners should be soft-deleted.
-- But add explicit RESTRICT for documentation.
-- (Already RESTRICT by default, no change needed.)

-- court_slots.booked_by: SET NULL on user deletion.
-- Note: This only affects the FK; app logic should also reset status.
ALTER TABLE court_slots
    DROP CONSTRAINT IF EXISTS court_slots_booked_by_fkey,
    ADD CONSTRAINT court_slots_booked_by_fkey
        FOREIGN KEY (booked_by) REFERENCES users(id) ON DELETE SET NULL;

-- payments.game_id: CASCADE on game deletion.
ALTER TABLE payments
    DROP CONSTRAINT IF EXISTS payments_game_id_fkey,
    ADD CONSTRAINT payments_game_id_fkey
        FOREIGN KEY (game_id) REFERENCES games(id) ON DELETE CASCADE;

-- No overlapping court slot time ranges (C7).
CREATE EXTENSION IF NOT EXISTS btree_gist;
ALTER TABLE court_slots
    ADD CONSTRAINT no_overlapping_slots
    EXCLUDE USING gist (
        court_id WITH =,
        tstzrange(start_time, end_time, '[)') WITH &&
    );

-- Missing CHECK constraints (H12).
ALTER TABLE courts
    ADD CONSTRAINT price_per_slot_positive CHECK (price_per_slot > 0);

ALTER TABLE courts
    ADD CONSTRAINT valid_lat CHECK (lat BETWEEN -90 AND 90);

ALTER TABLE courts
    ADD CONSTRAINT valid_lng CHECK (lng BETWEEN -180 AND 180);

-- Add updated_at to court_slots (L7).
ALTER TABLE court_slots ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ NOT NULL DEFAULT now();

-- Add trigger for updated_at on court_slots.
CREATE OR REPLACE FUNCTION update_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS set_court_slots_updated_at ON court_slots;
CREATE TRIGGER set_court_slots_updated_at
    BEFORE UPDATE ON court_slots
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at();

-- Add index on court_slots(booked_by) for "my bookings" queries.
CREATE INDEX IF NOT EXISTS idx_court_slots_booked_by ON court_slots (booked_by) WHERE booked_by IS NOT NULL;
