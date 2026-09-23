-- Notifications are delivered only after COMMIT. Identical game IDs are
-- coalesced within a transaction, so cancellation never leaks partial state.
-- These are invalidations, not an event log: clients read current snapshots.
CREATE FUNCTION notify_game_lobby() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    game_id UUID;
BEGIN
    IF TG_TABLE_NAME = 'games' THEN
        IF TG_OP = 'DELETE' THEN game_id := OLD.id; ELSE game_id := NEW.id; END IF;
    ELSE
        IF TG_OP = 'DELETE' THEN game_id := OLD.game_id; ELSE game_id := NEW.game_id; END IF;
        IF TG_OP = 'UPDATE' AND OLD.game_id IS DISTINCT FROM NEW.game_id THEN
            PERFORM pg_notify('pickup_game_changed', OLD.game_id::text);
        END IF;
    END IF;
    PERFORM pg_notify('pickup_game_changed', game_id::text);
    RETURN NULL;
END;
$$;

CREATE TRIGGER games_lobby_changed AFTER INSERT OR UPDATE OR DELETE ON games
    FOR EACH ROW EXECUTE FUNCTION notify_game_lobby();
CREATE TRIGGER players_lobby_changed AFTER INSERT OR UPDATE OR DELETE ON game_players
    FOR EACH ROW EXECUTE FUNCTION notify_game_lobby();
CREATE TRIGGER payments_lobby_changed AFTER INSERT OR UPDATE OR DELETE ON payments
    FOR EACH ROW EXECUTE FUNCTION notify_game_lobby();
