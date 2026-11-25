# Billing, booking, games and realtime review

Reviewed on 2026-10-05. This is a source audit of the current checkout. It is not
provider acceptance or production approval. No application files were edited in
the initial audit, and none of the findings below has been reproduced against a
running database in this pass yet.

## Coverage

Read in full: `services/game.rs`, `services/court.rs`, `services/bill_split.rs`,
`services/payment.rs`, `services/payment_provider.rs`, `db/games.rs`,
`db/courts.rs`, `db/pool.rs`, the court/game/payment/query models, game/court/
payment/webhook routes, `ws/lobby.rs`, `ws/hub.rs`, background worker setup,
initial schema and game/payment/lobby migrations, billing-booking integration
tests, payment integration tests, and realtime integration tests. Also checked
the HTTP test harness, application state/startup, and release/billing/payment/
realtime documentation. Auth, upload and infrastructure receive separate reviews.

GitNexus tools are not exposed to this agent at initial audit time. The required
symbol impact analysis remains a prerequisite for fixes. The current environment
does not expose `cargo`, `docker` or `psql` through PATH, so this audit does not
claim freshly executed Rust or database tests.

## Findings

### P1 — Completing a game permanently prevents collection of its unpaid shares

`services/game.rs:393–405` permits the creator to transition an open/full game to
in-progress and then completed. `services/payment.rs:121–127` rejects payments
for a completed game, and `services/game.rs:349–350` also prevents cancellation.
There is no reopening operation.

Trigger: two participants join, one pays their share, the creator marks the game
in-progress and then completed, and the other participant tries to pay. That
payment receives a conflict even though the bill still has a positive balance.
This also occurs during the ordinary finish-game-then-pay flow, without an
attacker or concurrency. Already-created orders may settle after completion;
new orders cannot collect the remainder.

Fix: preserve payment eligibility for completed games, independently of roster
state. Do not invent a rule requiring everyone to pay before starting. Add an
integration test completing a partially paid game, paying the remainder, and
asserting final paid/remaining totals and no duplicate payment.

### P1 — Booking and game membership overlap checks do not check each other

`services/court.rs:118–126` checks only `has_overlapping_booking_tx`, whose query
looks exclusively at `court_slots.booked_by` (`db/courts.rs:207–212`). Joining and
creating games check only `game_players` (`services/game.rs:137–143,260–267`;
`db/games.rs:309–316`). Locking the user serializes requests but does not make
either query consider the other commitment.

Trigger: book slot A without creating its game, then join another creator's game
B with overlapping times. Both operations succeed. Joining B and then booking A
also succeeds, as can concurrent mixed requests despite the user-row lock.

Fix: use one transaction-level scheduling invariant covering booked slots and
active game memberships. Exclude the target slot when creating a game from its
own booking and retain half-open interval semantics so adjacent slots work.
`billing_booking_integration.rs:146,179` cover booking-vs-booking and
join-vs-join, but not mixed requests; add both sequential orders, concurrent
book/join and adjacent-time cases.

### P2 — A future completed game's slot can be rebooked but cannot host a new game

`services/court.rs:166–175` releases a booking when
`db/games.rs:334` finds no open/full/in-progress game. However,
`db/games.rs:290` and `20260923000001_game_billing.sql:3–4` treat every
noncancelled game, including completed, as occupying its slot.

Trigger: create a future game, mark in-progress then completed (currently
allowed), cancel the booking, and let another user book the slot. The second
user cannot create a game: the completed game permanently occupies the unique
slot. Cancellation also changes the booking under the retained game history.

Fix: align cancel-booking with the noncancelled-game invariant. Blocking release
of an occupied slot is sufficient; this does not require new timing or payment
business rules. Add an integration regression proving a completed game's slot
cannot be released/rebooked, while cancelled-game reuse still succeeds.

### P2 — Vietnam day filtering and share text depend on UTC/database timezone

`db/courts.rs:145–151` compares `TIMESTAMPTZ` with a bound `NaiveDate`; four game
queries do the same (`db/games.rs:50,97,132,166`). `db/pool.rs` sets no database
timezone. PostgreSQL's session timezone therefore determines day boundaries.
The usual UTC deployment includes Vietnam 00:00–06:59 slots in the previous
calendar day and includes next-day early-morning slots in the requested day.

`routes/games.rs:130–142` formats a `DateTime<Utc>` directly for share text, so a
Vietnam 19:00 game is advertised as 12 PM. The share link at line 136 is also
hardcoded to `https://pickup.app`, instead of the configured application URL.

Fix: define explicit Vietnam local-day start/end instants for all date filters
and format share text in the same timezone. Build links from `public_app_url`.
Add tests around local midnight and the day boundary, with a UTC database
session, plus configured-host and share-time assertions. The exact intended
timezone contract should be documented; the existing VN phone/payment domain
and ZaloPay order IDs already use Vietnam time.

### P2 — Geo coordinates accept nonfinite and out-of-range numbers

Court query latitude/longitude (`models/court.rs:92–95`) and game coordinates
(`models/game.rs:62–65`) have no validation attributes.
`models/query.rs:21–27` parses arbitrary `f64`, including NaN and infinity.
Services only require both coordinates together; values then reach
`haversine_km` in PostgreSQL. Infinite arguments to cosine can produce database
errors rather than a client validation response; out-of-range finite arguments
produce misleading proximity results. Exact PostgreSQL responses still need
runtime reproduction.

Fix: reject nonfinite coordinates and constrain latitude to [-90, 90] and
longitude to [-180, 180]. Confirm radius also rejects nonfinite values. Add HTTP
tests for NaN, infinity, out-of-range coordinates, missing coordinate pairs and
valid boundary coordinates, proving invalid values never reach SQL.

### P2 candidate — Continuous notifications can starve WebSocket control reads

`ws/lobby.rs:278–295` uses `tokio::select! { biased; ... }` with
`subscription.changed()` above `socket.recv()`. Notifications arriving throughout
each throttled refresh can keep the subscription perpetually ready. Control
reads, including valid Pongs, then lose every selection. The ping branch closes
the connection after 60 seconds without consuming those queued Pongs
(`ws/lobby.rs:282–301`).

This is a source-derived concurrency candidate, not an established runtime
finding. Reproduce with a real TCP client that responds to Pings while another
connection commits lobby changes above the 10-per-second refresh ceiling for
over 60 seconds. Current realtime tests cover cross-instance commits, rollback,
access, shutdown, reconnect and connection limits, but not continuous
invalidation plus heartbeat. If confirmed, allow control reads to progress
independently or change selection fairness while preserving shutdown/expiry
priority and bounded snapshots.

## Payment and realtime safeguards that are present

Game-row locking serializes roster changes and payment reservation. Pending and
disputed orders freeze roster changes. Shares use the captured game price, paid
contributions are immutable, paid former members still count, and overpayment
fails closed. Stable PSP order IDs and creation leases prevent replacing an
uncertain attempt. Signed callbacks check partner/app, amount and order, with
separate transaction ownership and observation records. A local expiry alone
does not assert nonpayment. Provider calls occur outside SQL transactions.

Realtime snapshots use repeatable-read transactions for session, membership,
roster and split. PostgreSQL notifications arrive on commit. Connections and
per-user capacity are bounded, invalidations coalesce, client mutations are
rejected, snapshots and writes have deadlines, and idle access is rechecked.
These are implementation observations; passing provider mocks or unit tests
alone is not proof that deployment behavior matches them.

## Provider acceptance limitation

The local PSP server in `tests/payment/mod.rs:49–147` echoes merchant order
fields and serves developer-selected results/links. It verifies the implemented
request/signature and state-machine handling but cannot prove real merchant
onboarding, actual payment completion, reachable signed public callbacks,
status-query behavior, refunds or chargebacks. Refund operations remain absent,
as the existing release checklist explicitly records.

The reviewed request/signature conventions match the currently published
[MoMo Collection Link](https://developers.momo.vn/v3/docs/payment/api/collection-link/),
[MoMo order query](https://developers.momo.vn/v3/vi/docs/payment/api/payment-api/query/),
[ZaloPay order creation](https://docs.zalopay.vn/docs/specs/order-create/), and
[ZaloPay order query](https://docs.zalopay.vn/docs/specs/order-query/) contracts.
ZaloPay's published [status codes](https://docs.zalopay.vn/docs/developer-tools/knowledge-base/status-codes/)
identify query -54 as expired; the implementation retains ambiguous outcomes.
This contract comparison does not substitute for both real sandbox flows.

## Required verification after fixes

Run formatting, strict Clippy and all ordinary tests; then all ignored integration
tests on disposable PostgreSQL/Redis. Include mixed scheduling, completed-game
collection/booking protection, timezone/date and coordinate regressions, and a
real WebSocket notification/heartbeat stress test. Run GitNexus change detection
before any commit. Record actual commands and results instead of relying on the
previous release checklist's historical test count.

## Implementation and runtime verification, 2026-10-05

All actionable source findings above are fixed. Completed games accept new
payment orders for unpaid shares; a provider-mock test settles the creator,
completes the game, initiates and settles the remaining share through signed
MoMo callbacks, repeats callbacks, and proves zero balance and no further order.
Completed slots cannot be released; cancelled games still permit slot reuse.

Booking and game requests share the user-serialized schedule check. Both mixed
sequential orders, a concurrent book/join race and adjacent half-open intervals
pass. Vietnam midnight selection passes for slots and all four game list/count
queries; HTTP share tests prove UTC 12:00 appears as 19:00 with the configured
application host. Nonfinite/out-of-range coordinates, invalid radii and missing
pairs return client errors; valid coordinate bounds pass.

The realtime starvation candidate was reproduced: 50 committed invalidations
per second dropped a Pong-answering TCP client after 60.80 seconds. Control
reads now precede notification handling and the same 70-second stress passes.
An additional real defect surfaced in the existing forced-listener-reconnect
test: SQLx retained a Windows ConnectionReset socket and retried it forever.
Receive errors now discard/recreate the listener, restore LISTEN and invalidate
rooms; the existing reconnect test passes. Cancellation-aware bounded retries
and snapshot coalescing remain. Evidence: `artifacts/backend-validation/`.

Personal booking/game retrieval was added for authenticated app recovery, with
bounded pagination and caller isolation. See `backend-personal-schedule.md`.
Formatting and strict all-target Clippy pass on Rust 1.98.1. Full ignored and
ordinary integration execution passed all 146 tests with no ignored tests or
failures using disposable PostgreSQL 17/Redis 7.4; see
`artifacts/backend-validation/tests.log` for the actual final results.

Real merchant acceptance, refunds/chargebacks, provider delivery/storage,
deployed security, backup/restore and target-load evidence remain release gates.
