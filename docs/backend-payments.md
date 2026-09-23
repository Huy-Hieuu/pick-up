# Payment API and settlement protocol

`POST /games/{id}/payments/initiate` creates a fixed-amount MoMo or ZaloPay
order for an active game member. Only open, full, and in-progress games accept
payments. MoMo's documented 1,000–50,000,000 VND limits are enforced; amounts
under 1,000 VND are rejected for both providers. The response contains `id`,
`amount`, `provider`, `status`, `pay_url`, and `expires_at`. Returned payment
links must use HTTPS and a provider-owned domain.

## Idempotency and split locking

One pending, paid, or disputed payment may exist per game/player. A partial
unique index enforces this. Initiation locks the game row, reads the bill,
reserves the share, and commits a stable PSP order ID before calling the PSP.
Player joins/leaves are blocked while a payment is pending; the split cannot
change after a provider has accepted an amount. Concurrent initiation returns
the same link after creation, without creating a second order.

The provider request has a ten-second timeout and does not follow redirects. If
the response is uncertain, the pending record and stable order ID are retained;
after the creation lease expires, retries reuse that order ID. Definitive PSP
rejections expire the reservation. MoMo receives the same stable value for
`requestId`/`orderId`; ZaloPay receives `YYMMDD_{payment_uuid}` in Vietnam time.
Both requests ask the PSP to expire the order after 15 minutes. A late signed
capture becomes disputed. Do not delete an uncertain pending row: reconcile its
provider order first.

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
- A signed amount mismatch, late capture, contradictory transaction, or
  authorization marks the payment disputed. Split reads then stop, and joins,
  leaves, new payments and cancellation are blocked pending manual
  reconciliation. It is never reported as paid.

PSP calls occur outside SQL transactions. Stable order IDs and a bounded claim
lease cover request timeouts; callbacks can settle before or after the response.

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
reachability from the real provider, status-query/reconciliation or refund
behavior. Complete both PSP sandbox flows and the merchant operational checks
before accepting real money.

The request and signature formats follow [MoMo's Collection Link API](https://developers.momo.vn/v3/docs/payment/api/collection-link/),
[MoMo's IPN contract](https://developers.momo.vn/v3/docs/payment/api/result-handling/notification/),
[ZaloPay order creation](https://docs.zalopay.vn/docs/specs/order-create/), and
[ZaloPay callback contract](https://docs.zalopay.vn/docs/developer-tools/knowledge-base/callback/).
