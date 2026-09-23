# Backend authentication contract

The session migration requires existing clients to sign in again. Deploy migrations
before serving the new binary. Do not roll back to stateless token validation.

- Access and refresh JWTs carry a session ID (`sid`) and unique token ID (`jti`).
- PostgreSQL stores session IDs and refresh-token IDs, not raw bearer tokens.
- Refresh is single-use and atomic. Clients must serialize refresh requests and
  replace both tokens after success. A repeated/expired/revoked token returns 401.
- Password changes revoke all sessions in the same database transaction. HTTP
  requests check session validity; open WebSockets recheck it every 30 seconds.
- Login OTP and password-reset OTP are separate purposes. Codes expire without
  extension on failed attempts and can succeed only once, including concurrently.
- Login has a five-attempt/300-second budget per exact email, including unknown
  accounts and successful attempts. Deploy additional IP/global limits at ingress.
- Argon2 runs on bounded blocking workers; saturation returns 429.
- `JWT_SECRET` must be a random value of at least 32 bytes. Generate one with
  `openssl rand -hex 32`; keep it in your deployment secret store. Access TTL is
  limited to 1–60 minutes and refresh TTL to 1–90 days.

Schedule `DELETE FROM auth_sessions WHERE expires_at <= now()` for expired-session
cleanup. Issuing a session also removes that user's expired sessions.

Registration currently creates an account without email/phone verification.
Do not treat those contact fields as verified identity. Before public launch,
add registration verification and ingress abuse controls; configure TLS, restrict
database/Redis access, and redact Authorization headers and WebSocket token query
parameters from proxy logs. Redis stores short-lived OTP/reset grants and queued
email bodies; treat it as sensitive infrastructure.

## Regression tests

Run unit tests with `cd server && cargo test --all-targets`. Integration tests are
explicitly ignored by default so ordinary builds do not connect to databases.
Use disposable PostgreSQL (the user needs permission to create test databases)
and Redis, then run:

```sh
DATABASE_URL=postgres://pickup:pickup-test@127.0.0.1:55432/pickup_test \
TEST_REDIS_URL=redis://127.0.0.1:56379 \
cargo test --test auth_integration -- --include-ignored --test-threads=1
```

Never point integration tests at production services.
