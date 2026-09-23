# Billing and booking behavior

Games snapshot the court price at creation. Existing games are backfilled from
the current court price by migration `20260923000001`; audit historical prices
before running this migration against existing production data.

`GET /games/{id}/split` now returns the calculated bill, including `paid_total`
and `remaining_total`. Game detail includes `split` for participants, but not
for outsiders or cancelled games. Split and payment-history endpoints require
participation (the creator and former participants with payments are included).

Paid amounts are immutable. Unpaid active players split the remaining balance;
integer remainder goes to the creator if unpaid, otherwise to the first unpaid
player in UUID order. Paid players who leave remain in `payments` and their
contributions still reduce the balance. `player_count` counts current members.
If every current player is paid but a balance remains, `remaining_total` exposes
it; nobody's settled payment is silently increased. Overpayment requires manual
reconciliation and produces a conflict rather than a negative share.

Fixed-amount MoMo/ZaloPay order creation and signed callbacks are implemented.
Pending attempts lock roster changes; disputed/late captures stop split
calculation. See the [payment protocol, migrations and remaining launch
checks](backend-payments.md).

Cancellation is blocked while pending, paid, or disputed payments exist. Reconcile/refund
them first. Cancelled games preserve history and no longer prevent slot reuse.

The new unique index rejects multiple pending, paid, or disputed rows for one game/player.
Before applying it to an existing database, check for legacy duplicates:

```sql
SELECT game_id, user_id, count(*)
FROM payments WHERE status IN ('pending', 'paid', 'disputed')
GROUP BY game_id, user_id HAVING count(*) > 1;
```

Resolve duplicates manually using provider records; the migration intentionally
fails rather than choosing which charge to discard.

Booking and joining lock the user as well as the slot/game, so simultaneous
requests cannot evade same-user overlap checks or overfill a game. New games
and joins are rejected once the slot has started.

Run regression tests against disposable PostgreSQL:

```sh
cd server
DATABASE_URL=postgres://pickup:pickup-test@127.0.0.1:55432/pickup_test \
cargo test --test billing_booking_integration -- --include-ignored
```
