-- Prevent duplicate games for the same court slot.
CREATE UNIQUE INDEX idx_games_court_slot_unique ON games (court_slot_id);

-- Align max_players range with documentation (2-30, not 2-50).
ALTER TABLE games DROP CONSTRAINT max_players_range;
ALTER TABLE games ADD CONSTRAINT max_players_range CHECK (max_players BETWEEN 2 AND 30);
