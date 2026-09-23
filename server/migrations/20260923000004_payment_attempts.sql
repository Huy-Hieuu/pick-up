-- Persist the PSP order/link and an expiring lease so concurrent callers and
-- retries cannot create a second provider order after an uncertain timeout.
ALTER TABLE payments ADD COLUMN provider_order_id TEXT;
ALTER TABLE payments ADD COLUMN provider_payment_url TEXT;
ALTER TABLE payments ADD COLUMN expires_at TIMESTAMPTZ;
ALTER TABLE payments ADD COLUMN provider_create_claim UUID;
ALTER TABLE payments ADD COLUMN provider_create_claim_until TIMESTAMPTZ;

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM payments WHERE status = 'pending') THEN
        RAISE EXCEPTION 'Reconcile legacy pending payments before enabling provider-order tracking';
    END IF;
END;
$$;

CREATE UNIQUE INDEX idx_payments_provider_order
    ON payments (provider, provider_order_id) WHERE provider_order_id IS NOT NULL;
DROP INDEX idx_payments_active_player;
CREATE UNIQUE INDEX idx_payments_active_player ON payments (game_id, user_id)
    WHERE status IN ('pending', 'paid', 'disputed');
ALTER TABLE payments ADD COLUMN dispute_reason TEXT;

CREATE TABLE payment_provider_events (
    id BIGSERIAL PRIMARY KEY,
    payment_id UUID NOT NULL REFERENCES payments(id),
    provider payment_provider NOT NULL,
    provider_txn_id TEXT,
    reported_txn_id TEXT,
    payload_hash TEXT NOT NULL,
    amount_received BIGINT NOT NULL,
    outcome TEXT NOT NULL,
    received_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT payment_provider_event_outcome CHECK (outcome IN ('paid', 'failed', 'review'))
);
CREATE UNIQUE INDEX idx_provider_event_payload ON payment_provider_events (provider, payload_hash);
CREATE UNIQUE INDEX idx_provider_event_txn ON payment_provider_events (provider, provider_txn_id)
    WHERE provider_txn_id IS NOT NULL;
CREATE UNIQUE INDEX idx_provider_event_reported_txn ON payment_provider_events (provider, reported_txn_id)
    WHERE reported_txn_id IS NOT NULL;
CREATE INDEX idx_provider_events_payment ON payment_provider_events (payment_id, received_at DESC);
