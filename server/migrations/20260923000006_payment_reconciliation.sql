-- Historical anomalies must coexist with a newer active attempt. Disputes
-- block billing in the service; uniqueness applies only to collectible rows.
DROP INDEX idx_payments_active_player;
CREATE UNIQUE INDEX idx_payments_active_player ON payments (game_id, user_id)
    WHERE status IN ('pending', 'paid');
DROP INDEX idx_payments_provider_txn;
CREATE UNIQUE INDEX idx_payments_provider_txn ON payments (provider, provider_txn_id)
    WHERE provider_txn_id IS NOT NULL;

-- Store transaction ownership separately from observations so conflicting
-- signed callbacks remain auditable instead of violating a unique index.
CREATE TABLE payment_provider_transactions (
    provider payment_provider NOT NULL,
    transaction_id TEXT NOT NULL,
    payment_id UUID NOT NULL REFERENCES payments(id),
    PRIMARY KEY (provider, transaction_id)
);
INSERT INTO payment_provider_transactions (provider, transaction_id, payment_id)
SELECT provider, reported_txn_id, payment_id FROM payment_provider_events
WHERE reported_txn_id IS NOT NULL ON CONFLICT DO NOTHING;
INSERT INTO payment_provider_transactions (provider, transaction_id, payment_id)
SELECT provider, provider_txn_id, payment_id FROM payment_provider_events
WHERE provider_txn_id IS NOT NULL ON CONFLICT DO NOTHING;
INSERT INTO payment_provider_transactions (provider, transaction_id, payment_id)
SELECT provider, provider_txn_id, id FROM payments
WHERE provider_txn_id IS NOT NULL ON CONFLICT DO NOTHING;
DROP INDEX idx_provider_event_reported_txn;
DROP INDEX idx_provider_event_txn;
CREATE INDEX idx_provider_event_reported_txn ON payment_provider_events (provider, reported_txn_id);
ALTER TABLE payment_provider_events DROP CONSTRAINT payment_provider_event_outcome;
ALTER TABLE payment_provider_events ADD CONSTRAINT payment_provider_event_outcome
    CHECK (outcome IN ('paid', 'failed', 'review', 'processing'));
ALTER TABLE payments ADD COLUMN reconcile_after TIMESTAMPTZ NOT NULL DEFAULT now();
CREATE INDEX idx_payments_reconcile ON payments (reconcile_after)
    WHERE status IN ('pending', 'disputed');
