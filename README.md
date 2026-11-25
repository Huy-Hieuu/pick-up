# PickUp

Court bookings, pickup games and bill splitting, with a Rust/Axum API in
`server/` and an Expo/React Native client in `mobile/` for web, Android and iOS.
The responsive client includes court discovery/filtering and saved courts,
email registration/login/password recovery, booking and game creation,
joining/leaving, creator status controls, bill shares and payment-provider
redirects, profile and avatar editing, server-side logout, and personal booking/game
recovery after restarting the app. Game details refresh while the screen is
active. A separate court-owner application is not implemented.

## Run locally

Requires Rust 1.98.1 and Docker. From the repository root:

```sh
cp .env.example .env
# Set JWT_SECRET in .env to output from: openssl rand -hex 32
# Set SMTP credentials if testing email delivery.
docker compose up --build -d
curl --fail http://localhost:8080/health/ready
```

The Compose stack is **local development only**: dependencies bind to loopback
and use development database/storage credentials. Run migrations automatically
at startup, or set `DB_RUN_MIGRATIONS=false` when deploying migrations separately.
The MinIO development images are pinned on Quay because the previous Docker Hub
images are unavailable. They are not a supported production storage deployment;
use a maintained S3-compatible service and verify its bucket policy. Override
`MINIO_IMAGE` / `MINIO_MC_IMAGE` only when updating the development images.

To run Rust outside Docker:

```sh
docker compose -f docker-compose.dev.yml up -d
cp server/.env.example server/.env
# Set JWT_SECRET and SMTP credentials; for local MinIO use minioadmin for both S3 keys.
cd server
cargo run --locked
```

MinIO initialization permits anonymous reads only for sanitized avatars; raw
uploads are quarantined and require signed requests. `S3_ENDPOINT` is embedded in upload URLs, so it must be reachable by the
client (not an internal container hostname). For phones on a LAN, adjust it and
the public media URL appropriately. Never expose the development credentials.

## Verify

### Mobile/web client

Use Node 22.23.3 and npm 11.19.0. Start the API first, then:

```sh
cd mobile
npm ci
cp .env.example .env
# EXPO_PUBLIC_API_URL must point to the API reachable from this browser/device.
npm run web
```

For browser development, permit the exact web origin in the backend's
`CORS_ORIGINS` (normally `http://localhost:8081`). A physical phone needs the
computer's LAN address instead of `localhost`, and the API must listen on an
appropriate local interface. Android Emulator uses `http://10.0.2.2:8080` to
reach the host API ([Android networking](https://developer.android.com/studio/run/emulator-networking-address)). Native launch commands are `npm run android` and
`npm run ios`; physical-device release acceptance remains outstanding.
`EXPO_PUBLIC_` values are embedded in the client and must contain no secrets.
Production builds require an absolute HTTPS API origin; the app shows a setup
error instead of entering broken data flows when it is missing or invalid.
The browser test server defaults to ports 18081/18080 to avoid common local
service conflicts; override with `PICKUP_WEB_PORT`, `PICKUP_WEB_URL`, and
`PICKUP_API_URL` when needed.

```sh
npm run check:expo
npm run doctor
npm run typecheck
npm test
npm run export:web
npm run export:android
npx playwright install chromium
npm run test:e2e
npm run test:e2e:static
npm audit

# Lightweight production-style web view (after export:web)
PORT=8082 npm run serve:web
```

See [mobile verification and release readiness](docs/mobile-readiness.md) for
the pinned dependency set, browser-test scope and unresolved security advisories.
The [UI feature map](docs/ui-design.md) describes visual patterns and user journeys;
[current mobile evidence](artifacts/mobile-validation/summary.md) records the
browser, actual API and Android bundle checks.
The dependency audit is a separate release gate; a passing build does not make
those advisories disappear.

### Backend

```sh
cd server
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

Database/Redis tests are ignored by default. Against **disposable** services:

```sh
DATABASE_URL=postgres://pickup:pickup-test@127.0.0.1:55432/pickup_test \
TEST_REDIS_URL=redis://127.0.0.1:56379 \
cargo test --locked --all-targets -- --include-ignored --test-threads=1
```

The PostgreSQL role needs `CREATEDB`; SQLx creates isolated databases and applies
all migrations. CI runs these integration tests plus formatting, strict Clippy,
and the release container build. `Cargo.lock` is tracked; use `--locked`.

Dependency auditing uses a narrow exception because the lockfile retains RSA
via SQLx's disabled MySQL backend. CI checks `cargo tree --target all` and fails
if RSA, MySQL, or Quinn becomes compiled. Re-evaluate the RSA exception before
enabling those features. Plain `cargo audit` reports that lockfile advisory; the
compiled dependency graph is checked separately. AWS legacy HTTP/TLS features
are disabled.
See [RustSec's RSA advisory](https://rustsec.org/advisories/RUSTSEC-2023-0071.html)
and [SQLx feature documentation](https://docs.rs/sqlx/0.8.6/sqlx/).

`GET /health` is process liveness. `GET /health/ready` checks PostgreSQL and Redis
within two seconds, returning 503 when unavailable or shutting down. Requests
have a 64 KiB body cap and 30-second deadline. Application tracing excludes query
strings so WebSocket bearer tokens are not logged.

## Release status

The implemented API and responsive client are **not yet a complete production
release**. Local integration evidence is recorded in the
[backend validation summary](artifacts/backend-validation/summary.md) (146 tests,
formatting and strict Clippy) and
[actual API preview verification](artifacts/fullstack/summary.md) (isolated
PostgreSQL/Redis, actual registration, game/booking mutations and logout).
The preview uses a local SMTP discard sink and has no configured payment or
storage provider. It does not establish real provider acceptance or native
release readiness.
The [release acceptance checklist](docs/backend-release-checklist.md) records
verified fixes, rollout order and the remaining external/product requirements.
The [staging operations runbook](docs/staging-operations.md) covers explicit
configuration, bounded readiness checks and the locally verified backup/restore
commands, with deployed acceptance still outstanding.
See [authentication changes](docs/backend-security.md),
[billing behavior and migration checks](docs/backend-billing.md), and the
[payment protocol and remaining launch checks](docs/backend-payments.md).
The [personal schedule API](docs/backend-personal-schedule.md) documents
authenticated booking recovery and game history. Current source-review findings
and completed fixes are recorded in the [platform review](docs/review-platform.md)
and [billing/game review](docs/review-billing-games.md).
The [real-time lobby protocol](docs/backend-realtime.md) documents committed
cross-instance updates, connection/access limits, and reconnect behavior.
The [email queue operations guide](docs/backend-email.md) covers leases, retries
and Redis persistence. [API rate limits and proxy trust](docs/backend-rate-limits.md)
documents the default shared quota and safe reverse-proxy setup.
The [avatar upload contract](docs/backend-uploads.md) documents the signed
8 MiB size limit, private quarantine, image validation and the mobile completion step.

Remaining launch gates:

- PSP sandbox acceptance for order creation/callbacks/status queries, refunds,
  chargebacks and real-credential rollout. Complete those checks before accepting
  real money.
- Phone-number ownership verification and edge/WAF protection against
  distributed or volumetric abuse. The API has a per-IP rate limit; it does not
  replace upstream DDoS protection.
- SMTP delivery must still be tested with your provider; monitor the bounded,
  redacted `email:dead` list. Compose enables Redis Append Only File persistence;
  production Redis needs equivalent durable storage and recovery.
- Production HTTPS/CORS/storage configuration, restricted network access,
  secrets, migration preflight, monitoring/alerts, backups and restore drills.
- Load tests and end-to-end provider/storage acceptance tests.
- Android/iOS signed release builds and physical-device acceptance, privacy/store
  requirements, and resolution or explicit release-owner review of the mobile
  dependency advisories documented above.

Do not use real-money payment traffic until the provider launch gates are met.
