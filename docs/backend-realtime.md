# Game lobby protocol (version 1)

The backend now pushes committed lobby changes across API instances. It uses
one dedicated PostgreSQL LISTEN connection per process, database triggers, and
bounded in-process subscription state. No Redis subscription or sticky session
is required. PostgreSQL must support session-level LISTEN; do not route the
listener through a transaction-mode connection pooler.

## Connect

`GET /ws/games/{game_id}` with `Authorization: Bearer <access_token>` is preferred
for native clients. Browsers may use `?token=<access_token>`. Redact query strings
at the reverse proxy. If an Origin header is present it must match `CORS_ORIGINS`
(or the explicit development wildcard). Missing/invalid/revoked/refresh tokens
return 401; non-members return 403. Only current game members may subscribe.

Each process permits at most 1,000 connections, with at most 5 per user. Excess
handshakes return 429. Apply ingress/global connection limits as well when using
multiple instances. Disconnection releases capacity and unused room entries.

## Server messages

The earlier phase-one document sketched delta messages that were never
implemented. The implemented version 1 protocol uses **full replacement
snapshots**, so mobile clients need no missed-event replay or financial delta
arithmetic. There is no client-to-server mutation/chat protocol.

```json
{
  "type": "lobby_snapshot",
  "protocol_version": 1,
  "game_id": "uuid",
  "status": "open",
  "max_players": 6,
  "players": [],
  "split": {
    "total_amount": 600001,
    "per_player": 300000,
    "remainder": 1,
    "creator_pays_extra": true,
    "player_count": 2,
    "paid_total": 0,
    "remaining_total": 600001,
    "payments": []
  },
  "split_error": null
}
```

The arrays above are abbreviated. `players` uses the game-detail player schema;
`split` uses the split-endpoint schema. Replace local state with each snapshot;
do not add its amounts to previous balances. Session, membership, roster, game
status and bill are read from one repeatable-read transaction. If billing needs
manual reconciliation, `split` is null and `split_error` is
`RECONCILIATION_REQUIRED`; roster updates still work.

After cancellation, existing subscribers receive
`{"type":"game_cancelled","game_id":"uuid"}` and close code 1000. This terminal
message contains no player/payment data, because cancellation removes members.
Ordinary membership removal closes with code 1008 and no subsequent snapshot.

## Consistency and recovery

- Game, membership and payment writes notify **only on commit**, including writes
  by another API instance. Rollbacks produce no updates. Notification payloads
  contain only a game UUID, never personal or payment data.
- Notifications invalidate cached client state; they are not a durable event
  log. Rapid commits may coalesce. Every new connection starts with current
  state, and active connections recheck state/access every 15 seconds to recover
  missed notifications and profile changes. Unchanged snapshots are suppressed.
- SQLx reconnects/re-subscribes after listener interruption; all active rooms
  refresh after recovery. A slow client cannot retain an unbounded event queue.
- Access-token expiry closes the socket; revocation is rechecked before updates
  and within 15 seconds while idle. Refresh over REST and reconnect with the new
  access token. Do not reconnect after deliberate membership removal.
- Refreshes coalesce to at most 10/second per connection. Database snapshot reads
  and writes to a socket each have a five-second deadline; failures close the
  connection (1013 for unavailable snapshots) so clients can reconnect with
  exponential backoff and jitter.
- Inbound application messages close with 1008; inbound frames/messages are
  capped at 64 KiB and outgoing snapshots at 256 KiB. Protocol pings occur every
  30 seconds; matching pongs are required within 60 seconds. Shutdown closes
  connections with 1001.

The 15-second recheck is recovery, not an audit trail or payment receipt. Payment
provider acceptance/reconciliation remains a separate release gate.

## Operations and tests

Apply migration `20260923000002_lobby_notifications.sql` before serving the new
binary. Startup establishes LISTEN before accepting requests. Budget one extra
PostgreSQL connection per API instance beyond `DB_MAX_CONNECTIONS`. Monitor
`pg_notification_queue_usage()`, listener retry logs, database query latency,
connection count and reconnect rates. Public load limits still require a
representative deployment load test.

Run against disposable PostgreSQL/Redis:

```sh
cd server
DATABASE_URL=postgres://pickup:pickup-test@127.0.0.1:55432/pickup_test \
TEST_REDIS_URL=redis://127.0.0.1:56379 \
cargo test --locked --test http_integration realtime -- --include-ignored
```

Implementation semantics follow PostgreSQL's
[transactional NOTIFY documentation](https://www.postgresql.org/docs/17/sql-notify.html)
and SQLx's [reconnecting PgListener](https://docs.rs/sqlx/0.8.6/sqlx/postgres/struct.PgListener.html).
