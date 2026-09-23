# Backend release acceptance

Status as of 2026-09-24: the reviewed auth, payment retry/audit, and upload-byte
defects are fixed and covered by regression tests. This is not a production
approval. The checkout has no configured `.env` or `server/.env` with staging
credentials, and no deployed environment was exercised.

## Verified locally

- 132 unit/integration tests, including disposable PostgreSQL/Redis, local
  SMTP, provider mocks, WebSockets across instances and storage mocks.
- Formatting, strict Clippy, dependency audit using the documented unused-RSA
  exception, and release Docker image build. The excluded dependencies remain
  absent from the compiled graph.
- Real local MinIO: signed PUT accepted; anonymous quarantine read denied;
  wrong signed MIME/length rejected; re-encoded public PNG served; repeated
  completion idempotent; reusing a raw PUT cannot overwrite the public image.
- Both development Compose definitions and Postman collection parse correctly.

## Required rollout order

1. Back up PostgreSQL and verify restoration to a separate environment. Save
   current image digest, application configuration and storage bucket policy.
2. Drain all old API instances and workers before changing payment semantics.
   Older workers can release orders from a local timer; do not mix versions.
3. Apply all migrations through `20260923000007_avatar_quarantine.sql` using a
   migration role. Check the earlier payment migration preflight documented in
   `backend-billing.md` and `backend-payments.md`. Never edit applied migrations.
4. Apply the private-quarantine storage policy before enabling new upload
   endpoints. Migration 00007 preserves previous avatar references in
   `legacy_avatar_references` and clears public profile links. Revalidate old
   images through the new pipeline; never restore their URLs blindly.
5. Deploy the verified image with a restricted application DB role,
   `DB_RUN_MIGRATIONS=false`, private Redis/PostgreSQL networking, HTTPS ingress,
   explicit CORS origins, and correct trusted-proxy CIDRs. Use a maintained
   storage provider; the pinned development MinIO image is not production support.
6. Update clients for email-only signup followed by ownership-proven profile /
   password submission, and the upload → completion → profile-assignment flow.
7. Verify readiness and the following acceptance cases on the deployed system.
   Roll forward on failure; do not restore an older binary that bypasses the
   new auth/payment/media protections.

## External acceptance still required

| Area | Required evidence |
| --- | --- |
| MoMo / ZaloPay | Merchant sandbox credentials configured in the secret store; real checkout completion; signed public callbacks received; forced lost-callback recovered by a status query; terminal failure releases once; no duplicate collection after retry. |
| Refunds / chargebacks | Decide who can approve refunds, cancellation rules and full/partial refunds. Implement an authorized, idempotent refund workflow and provider reconciliation, then verify it in sandbox. Until then, paid-game cancellation remains blocked. No automated refund code is implemented. |
| Phone identity | Choose/configure the SMS provider and verify ownership before giving phone numbers any identity/recovery authority. Current phone fields are unverified contact data; login and recovery use verified email only. |
| Email | Deliver registration/login/reset messages through the chosen SMTP service; verify TLS, sender-domain setup, retry behavior and bounce handling. |
| Storage / CDN | Verify quarantine and legacy objects are private; only sanitized paths are public; HTTPS, nosniff, CSP, exact signed PUT headers and quarantine lifecycle work on the actual provider. |
| Infrastructure | Record ingress/WAF configuration, private service access, secret rotation, DB/Redis backup and restore evidence, and monitoring ownership. |
| Capacity | Agree a target concurrency and latency/error budget. Run a staging load test covering signup, games, checkout, WebSockets and avatar completion. Local unit tests do not establish capacity. |

Provide credentials through a local/staging secret store rather than chat or
tracked files. Never send real-money traffic until the payment acceptance and
refund/chargeback requirements have been met.

## Operational checks

- Alert on oldest unresolved `payments` rows, `disputed` counts/reasons, worker
  query failures, provider callback errors, HTTP 5xx and database pool saturation.
  Review every `late_capture`, `amount_mismatch`, `conflicting_provider_callback`
  or `provider_transaction_reused` against merchant records. Never clear a hold
  just to allow a retry. Preserve callback/transaction records for audit.
- Monitor Redis ready/processing/retry email queues and the bounded redacted
  dead-letter list. Restore Redis persistence in a drill; delivery is at-least-once.
- Remove expired sessions on a schedule. Set a one-day object lifecycle for
  `quarantine/` and remove expired unfinished upload rows after that window.
  Keep approved upload records referenced by profiles.
- Record successful staging acceptance, image digest and migration versions
  before production release. This checklist documents work still required; it
  does not substitute for those checks.
