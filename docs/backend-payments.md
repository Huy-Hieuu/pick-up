# Payment API and settlement protocol

`POST /games/{id}/payments/initiate` creates a fixed-amount MoMo or ZaloPay
order for an active game member. Only open, full, and in-progress games accept
payments. MoMo's documented 1,000–50,000,000 VND limits are enforced; amounts
under 1,000 VND are rejected for both providers. The response contains `id`,
`amount`, `provider`, `status`, `pay_url`, and `expires_at`. Returned payment
links must use HTTPS and a provider-owned domain.

## Idempotency and split locking

One pending or paid payment may exist per game/player. Historical disputes can
coexist with a newer attempt, and any dispute blocks collection for the game.
A partial unique index enforces the collectible-order constraint. Initiation locks the game row, reads the bill,
reserves the share, and commits a stable PSP order ID before calling the PSP.
Player joins/leaves are blocked while a payment is pending; the split cannot
change after a provider has accepted an amount. Concurrent initiation returns
the same link after creation, without creating a second order.

The provider request has a ten-second timeout and does not follow redirects. If
the response is uncertain, the pending record and stable order ID are retained.
MoMo duplicate/in-progress result codes and ZaloPay processing or duplicate
transaction responses are treated as uncertain, not definitive rejection. The
reservation is disputed and remains locked until a signed callback or provider
status query resolves it; the server never replaces an unresolved order.
Only confirmed terminal failures expire a reservation. MoMo
receives the same stable value for `requestId`/`orderId`;
ZaloPay receives `YYMMDD_{payment_uuid}` in Vietnam time. Both requests ask the
PSP to expire the order after 15 minutes. A late signed capture is retained in
the callback ledger and creates a reconciliation hold, including when a newer
attempt exists. Do not delete an uncertain pending row: reconcile its provider
order first.

## Signatures, settlement and replay

- MoMo's canonical sorted-field HMAC-SHA256 is checked before partner, order,
  request ID, amount, result code and transaction ID. Only result `0` with a
  positive provider transaction ID settles as paid. Code `9000` is an
  authorization, not proof of captured funds, and requires reconciliation.
  Valid callbacks receive the documented HTTP 204 response.
- ZaloPay create MAC uses `key1` over the documented seven pipe-separated
  fields. Callback MAC uses `key2` over the exact raw `data` string. The callback
  type and app ID are checked, then order, amount and transaction ID are settled
  in a transaction. A valid callback receives `{"return_code":1}`.
- Callback payloads and signatures are not logged or stored. A SHA-256 payload
  fingerprint, transaction ID, received amount and outcome are stored for replay
  protection and reconciliation. Provider transaction IDs and payload
  fingerprints are unique per provider.
- Provider transaction IDs are recorded across every signed callback, including
  amount mismatches and late captures. If the same PSP transaction is presented
  for a second order, that order becomes disputed; a simultaneous race between
  orders can settle no more than one. `payments.dispute_reason` and
  `payment_provider_events` preserve the anomaly for manual reconciliation.
- A signed amount mismatch, late capture or contradictory transaction marks
  the payment disputed. Split reads then stop, and joins,
  leaves, new payments and cancellation are blocked pending manual
  reconciliation. It is never reported as paid.

PSP calls occur outside SQL transactions. Stable order IDs and a bounded claim
lease cover request timeouts; callbacks can settle before or after the response.

## Automatic reconciliation

The background worker polls a bounded batch of four due orders per pass, using
database claims that expire after two minutes. No SQL locks are held during
network calls. Query endpoints resolve to `query` beside each configured create
endpoint. Requests use the provider's signed query format, a 35-second timeout,
and a 64 KiB response cap. Unknown responses, processing results, network failures,
and missing orders retain the hold. A local 15-minute deadline alone never proves
that no money moved; it changes the attempt to `awaiting_provider_status`.

Queries can settle exact confirmed captures or release documented terminal
failures. They cannot clear amount mismatches, reused transactions, late captures
after replacement, or conflicting callbacks: those require operator review.
MoMo pending/authorization observations are retained but never treated as a
terminal failure. Every conflicting callback is retained; the separate
`payment_provider_transactions` table enforces one owning payment per transaction.

Migration `20260923000006_payment_reconciliation.sql` is required before rollout.
Drain old application instances before starting this version: older binaries
can still expire orders based only on local time. Existing disputed orders keep
their holds. Monitor the age and count of unresolved disputed payments.

## Configuration and rollout

Set HTTPS `APP_PUBLIC_URL` and `APP_PUBLIC_API_URL`; their paths are sent as the
PSP redirect and callback URLs. The callback host must be reachable from the
provider. Default MoMo and ZaloPay URLs point to their sandbox environments.
Keep merchant credentials in the deployment secret store. If provider
credentials are absent, initiation fails closed with 501. Local HTTP PSP
endpoints are allowed only on loopback for tests.

For Docker Compose, provide `MOMO_PARTNER_CODE`, `MOMO_ACCESS_KEY`,
`MOMO_SECRET_KEY`, `ZALOPAY_APP_ID`, `ZALOPAY_KEY1`, and `ZALOPAY_KEY2` in the
ignored repository-root `.env`. Explicit endpoint overrides are
`MOMO_ENDPOINT` and `ZALOPAY_ENDPOINT`; defaults are sandbox URLs. Restart the
API after rotating credentials.

Migrations `20260923000003_payment_disputed_status.sql` and
`20260923000004_payment_attempts.sql` add the dispute state, provider-order
identifiers, link expiry, creation leases and callback ledger. Existing
installations must reconcile legacy pending payments before applying migration
00004; it deliberately fails closed if any exist because their provider order
IDs cannot be safely reconstructed. Inspect active-payment duplicates before
applying the earlier active-payment unique-index migration, as explained in
[billing migration checks](backend-billing.md).

## Verification and remaining launch gates

The disposable integration suite uses local PSP mocks to verify signed provider
requests, URL validation, idempotent and concurrent requests, uncertain timeouts,
callback signatures and replay, exact settlement, amount disputes, and expiry.
Mocks do not prove merchant onboarding, test-app payment completion, callback
reachability from the real provider or real status-query/refund behavior.
The query protocol is tested locally, but real provider sandbox acceptance and
refund/chargeback operations remain launch gates. Complete both PSP sandbox flows and the merchant operational checks
before accepting real money.

The request and signature formats follow [MoMo's Collection Link API](https://developers.momo.vn/v3/docs/payment/api/collection-link/),
[MoMo's IPN contract](https://developers.momo.vn/v3/docs/payment/api/result-handling/notification/),
[ZaloPay order creation](https://docs.zalopay.vn/docs/specs/order-create/), and
[ZaloPay callback contract](https://docs.zalopay.vn/docs/developer-tools/knowledge-base/callback/).
Reconciliation follows [MoMo order query](https://developers.momo.vn/v3/vi/docs/payment/api/payment-api/query/)
and [ZaloPay order query](https://docs.zalopay.vn/docs/specs/order-query/).
