# Platform production review

Review date: 2026-10-05. Scope: backend authentication, configuration, middleware,
email delivery, avatar upload, startup/shutdown, Docker/Compose, CI, and release
documentation. The initial review was read-only; references below describe the
reviewed baseline. No secret values were inspected.

## Inventory and existing protections

- This checkout initially contains the Rust/Axum backend. README identifies the
  mobile and court-owner applications as unimplemented; CLAUDE.md describes the
  intended frontend architecture rather than an existing product implementation.
- Auth uses verified email, Argon2 on four bounded blocking workers, short-lived
  access JWTs and PostgreSQL-backed sessions. Refresh-token rotation is atomic and
  single-use; password resets revoke sessions in the password-update transaction.
- OTP challenges are purpose-bound, expire without extension, and have atomic
  consumption/attempt limits. OTP and outbound email are queued atomically.
- API quotas are shared through Redis and use the socket peer unless explicitly
  configured trusted proxies supply forwarding headers. Limits use HMAC-derived
  IP keys. The router caps bodies at 64 KiB and applies a 30-second deadline.
- Email delivery uses Redis leases, delayed retries, bounded redacted dead letters,
  and a reusable SMTP transport. Delivery is at least once. Local Compose enables
  Redis AOF persistence.
- Raw avatars upload to private quarantine. Completion checks ownership, exact
  length and decoded format/dimensions, re-encodes PNG pixels, and approves the
  public URL. Blocking image work is limited to two workers per process.
- The Docker runtime runs as an unprivileged user. Compose dependency ports bind
  to loopback and the development MinIO images are pinned. Compose is explicitly
  documented as a development stack, not a production deployment.
- Readiness checks PostgreSQL and Redis within two seconds. It does not prove
  SMTP, payment-provider, storage, or frontend functionality.

## Actionable priorities

### P2: server-side logout is missing

`server/src/routes/auth.rs:48` exposes GET/PATCH `/auth/me`, but no logout or session
revocation route exists. `server/src/services/auth_security.rs:228` rotates and
extends sessions. A client that only deletes local tokens cannot invalidate copied
credentials. Add authenticated `POST /auth/logout`, delete only the caller's
session, and verify that both its access and refresh tokens are rejected afterward.

### P2: malformed SMTP configuration can leave auth healthy but unusable

`server/src/jobs/mod.rs:16` validates transport construction at startup.
`server/src/jobs/email.rs:99` does not validate the configured sender. Sender
parsing first occurs for each email at line 213, and a malformed sender permanently
dead-letters every job with `invalid_sender`. Readiness remains green. Parse the
sender before accepting traffic; require a positive SMTP port and strict boolean
parsing for `SMTP_USE_TLS`, which currently silently falls back to true on malformed
input (`server/src/config/settings.rs:184`). Add configuration regressions.

### P2: password-work saturation consumes a valid reset grant

`server/src/services/auth.rs:339` consumes the reset token with `GETDEL` before
hashing the new password at line 348. `auth_security.rs:17` can reject hashing with
429 when all four password workers are busy. The response asks the client to
retry, but the grant is already gone, so a retry produces 401 while the password
has not changed. Acquire password-work capacity before consuming the grant while
preserving atomic single-use consumption and the existing password-hash binding.
Test worker saturation and a later successful retry with the same grant.

Registration verification also consumes its OTP before hashing (`auth.rs:197`).
The current registration contract explicitly asks callers to request a fresh code
after failed completion; that intentional contract should be distinguished from
the reset retry defect.

### P2: malformed CORS origins are silently dropped

`server/src/middleware/cors.rs:19` filters out values that cannot become headers.
The process can start with configuration that blocks all browser clients. Validate
origins as actual HTTP(S) origins and reject malformed configuration at startup.
Keep wildcard development mode explicit; reject mixtures of wildcard and origins.

## Baseline verification prerequisites and coverage

The backend workflow (`.github/workflows/backend.yml:37`) pins Rust 1.98.1 and runs
formatting, strict all-target Clippy, all tests including ignored tests serially,
dependency auditing, a release Docker image build, and Compose/shell validation.

Full database tests require disposable PostgreSQL 17 with a role allowed to create
isolated test databases, Redis 7, `DATABASE_URL`, and `TEST_REDIS_URL`. SMTP,
payment-provider, WebSocket, and storage mocks bind ephemeral loopback ports.
Do not point these tests at production infrastructure.

```sh
cd server
cargo +1.98.1 fmt --all -- --check
cargo +1.98.1 clippy --locked --all-targets -- -D warnings
cargo +1.98.1 test --locked --all-targets -- --include-ignored --test-threads=1
```

Ordinary `cargo test` skips essential database/Redis regressions. Existing tests
cover concurrent refresh/OTP consumption, reset-session revocation and stale
grants, email ownership, upload sanitization/ownership/idempotency, mock SMTP
delivery, worker cancellation, request quotas, and readiness. HTTP tests construct
`Settings` directly (`server/tests/http_integration.rs:43`), so that evidence does
not cover environment parsing. No reset-worker-saturation test was present in the
baseline. Media tests use an S3 mock (`server/tests/media/mod.rs:56`); CI does not
currently exercise real signed PUTs or the deployed anonymous-read policy.

At initial review, `cargo`, `rustc`, `docker`, and `openssl` were not discoverable
in the current PowerShell PATH. This is not proof that installations are absent
elsewhere. No baseline execution result was obtained by the review agent.

## External production launch gates

These are deployment/product requirements, not results that a local unit test can
prove. See `docs/backend-release-checklist.md:47` for acceptance criteria.

- Configure merchant sandbox credentials and verify real order creation, signed
  callbacks, lost-callback recovery, terminal failure and duplicate prevention.
- Decide and implement authorized idempotent refunds/chargebacks, then verify
  them with the provider. Automated refunds are absent; paid-game cancellation
  currently remains blocked.
- Verify SMTP TLS, registered sender domain, deliverability, retry/bounce behavior,
  and queue monitoring with the actual delivery provider.
- Verify private quarantine/legacy paths, sanitized public paths, exact signed
  PUT headers, HTTPS, security response headers, and quarantine lifecycle on the
  actual storage/CDN provider.
- Record ingress/WAF, CORS and trusted-proxy configuration, restricted service
  networks and database role, secret rotation, migrations, monitoring ownership,
  PostgreSQL/Redis backups and successful restore drills.
- Set target concurrency and latency/error budgets; run staging load tests across
  signup, games, checkout, WebSocket connections and avatar completion.
- Phone values are unverified contact data. Prove phone ownership before assigning
  phone-based login, recovery or authorization authority.

## Implementation and verification status

Implemented: authenticated session-scoped logout; reset grants survive password
worker saturation; SMTP sender/positive port and strict TLS boolean validation;
exact HTTP(S) CORS origin validation at startup. HTTP logout rejects both access
and refresh reuse; another independent session remains valid. A deterministic
test reserves all four password permits, observes 429 without consuming the
grant or changing the hash, releases permits, and successfully retries once.

Added authenticated, paginated personal booking/game recovery for clients that
restart or clear local state. Identity isolation, missing-token rejection,
membership changes and cancelled creator history have HTTP/database coverage.
See `backend-personal-schedule.md` for the response contract.

Verified on Rust 1.98.1 with disposable local PostgreSQL 17 and Redis 7.4:
formatting, strict all-target Clippy and all 146 tests pass with zero ignored
tests or failures. Full test results and commands are
recorded in `artifacts/backend-validation/tests.log` and `clippy.log`; realtime
before/after evidence is in `realtime-reproduction.md`. GitNexus change detection
reports LOW risk, 65 mapped symbols and zero indexed affected processes. This
mapping does not prove absence of runtime effects; integration tests cover those.

The external production launch gates above remain open. Local mocks do not
establish actual provider, infrastructure, backup/restore or load acceptance.
