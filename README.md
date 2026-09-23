# PickUp backend

Rust/Axum API for court bookings, pickup games and bill splitting. This checkout
contains the backend only; mobile and owner applications are not implemented.

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

To run Rust outside Docker:

```sh
docker compose -f docker-compose.dev.yml up -d
cp server/.env.example server/.env
# Set JWT_SECRET and SMTP credentials; for local MinIO use minioadmin for both S3 keys.
cd server
cargo run --locked
```

MinIO initialization permits anonymous **reads only**; uploading requires a signed
request. `S3_ENDPOINT` is embedded in upload URLs, so it must be reachable by the
client (not an internal container hostname). For phones on a LAN, adjust it and
the public media URL appropriately. Never expose the development credentials.

## Verify

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

Dependency auditing uses `cargo audit --ignore RUSTSEC-2023-0071`. The single
exception is RSA pulled into the lockfile by SQLx's **disabled MySQL backend**;
it is absent from the compiled graph (`cargo tree --target all`). CI first
asserts that neither `rsa` nor `sqlx-mysql` is compiled. Remove and re-evaluate
this exception before enabling MySQL or any RSA dependency. Plain `cargo audit`
still reports that lockfile advisory. AWS legacy HTTP/TLS features are disabled.
See [RustSec's RSA advisory](https://rustsec.org/advisories/RUSTSEC-2023-0071.html)
and [SQLx feature documentation](https://docs.rs/sqlx/0.8.6/sqlx/).

`GET /health` is process liveness. `GET /health/ready` checks PostgreSQL and Redis
within two seconds, returning 503 when unavailable or shutting down. Requests
have a 64 KiB body cap and 30-second deadline. Application tracing excludes query
strings so WebSocket bearer tokens are not logged.

## Release status

This is a hardened backend foundation, **not yet a complete production release**.
See [authentication changes](docs/backend-security.md) and
[billing behavior and migration checks](docs/backend-billing.md).

Remaining launch gates:

- Momo/ZaloPay initiation, authenticated callbacks, reconciliation, retries and
  refunds. These endpoints deliberately return 501 until implemented/tested.
- Game lobby event broadcasting (the WebSocket currently authenticates, pings
  and closes on expiry/revocation/shutdown but does not broadcast mutations).
- Verified registration/contact ownership and IP/global ingress rate limiting.
- Durable email acknowledgement/recovery: the current list queue can lose an
  in-flight job if a process crashes after popping it. SMTP delivery must be
  tested with your provider; monitor the `email:dead` queue.
- Production HTTPS/CORS/storage configuration, restricted network access,
  secrets, migration preflight, monitoring/alerts, backups and restore drills.
- Load tests and end-to-end provider/storage acceptance tests.

Do not use real-money payment traffic until the provider launch gates are met.
