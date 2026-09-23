# Backend authentication contract

The session migration requires existing clients to sign in again. Deploy migrations
before serving the new binary. Do not roll back to stateless token validation.

- Access and refresh JWTs carry a session ID (`sid`) and unique token ID (`jti`).
- PostgreSQL stores session IDs and refresh-token IDs, not raw bearer tokens.
- Refresh is single-use and atomic. Clients must serialize refresh requests and
  replace both tokens after success. A repeated/expired/revoked token returns 401.
- Password changes revoke all sessions in the same database transaction. HTTP
  requests check session validity; open WebSockets recheck it before snapshots
  and at least every 15 seconds while idle, and close at access-token expiry.
- Login OTP and password-reset OTP are separate purposes. Codes expire without
  extension on failed attempts and can succeed only once, including concurrently.
- Login has a five-attempt/300-second budget per exact email, including unknown
  accounts and successful attempts. Deploy additional IP/global limits at ingress.
- Argon2 runs on bounded blocking workers; saturation returns 429.
- REST requests and WebSocket handshakes use a Redis-backed per-IP rate limit
  (300 per fixed minute by default); liveness/readiness probes are exempt. Redis
  keys contain an HMAC of the client IP, not the raw address. Direct peer IPs are
  trusted unless `TRUSTED_PROXY_CIDRS` explicitly allows the socket peer to
  supply `X-Forwarded-For`; the proxy-facing chain is trusted first. Set this
  only for proxies that strip/overwrite incoming client headers. This API quota
  does not replace load-balancer/WAF protections against distributed or
  volumetric traffic.
- `JWT_SECRET` must be a random value of at least 32 bytes. Generate one with
  `openssl rand -hex 32`; keep it in your deployment secret store. Access TTL is
  limited to 1–60 minutes and refresh TTL to 1–90 days.

Schedule `DELETE FROM auth_sessions WHERE expires_at <= now()` for expired-session
cleanup. Issuing a session also removes that user's expired sessions.

Email/password registration now returns the same 202 message without a session
for available and already-registered addresses, queues an OTP only for a new
account, and issues a session only after `POST /auth/verify-registration`
consumes the single-use code. Duplicate phone/email conflicts also produce the
generic message. `POST /auth/resend-registration-otp` returns a generic response
for unknown/already-verified addresses and shares the cooldown. Email login OTP
and password reset are restricted to verified addresses. Phone numbers still
aren't verified by the current registration flow, so do not treat the phone as
proof of identity. The verification OTP/outbound email queue lives in Redis; it
is sensitive infrastructure.

Migration `20260923000005_verify_account_email.sql` marks legacy email addresses
unverified and revokes their existing sessions. Those users must verify their
email and sign in again. Before deploying it, prepare support/onboarding for the
one-time verification step. Then configure TLS, restrict database/Redis access,
and redact Authorization headers and WebSocket token query parameters from proxy
logs.

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
