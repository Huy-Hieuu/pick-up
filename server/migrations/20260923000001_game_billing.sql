-- Cancelled games retain history but must not prevent rebooking the slot.
DROP INDEX idx_games_court_slot_unique;
CREATE UNIQUE INDEX idx_games_court_slot_unique ON games (court_slot_id)
    WHERE status != 'cancelled';

-- Freeze the agreed price; subsequent court price edits must not change a bill.
ALTER TABLE games ADD COLUMN total_amount INTEGER;
UPDATE games g SET total_amount = c.price_per_slot
FROM court_slots s JOIN courts c ON c.id = s.court_id
WHERE s.id = g.court_slot_id;
ALTER TABLE games ALTER COLUMN total_amount SET NOT NULL;
ALTER TABLE games ADD CONSTRAINT game_total_positive CHECK (total_amount > 0);

-- A retry may create a new expired record, never two collectible charges.
CREATE UNIQUE INDEX idx_payments_active_player ON payments (game_id, user_id)
    WHERE status IN ('pending', 'paid');
