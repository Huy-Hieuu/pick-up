# Full-stack preview runtime — 2026-10-05

## Current connected mobile verification — 2026-11-25

A fresh disposable PostgreSQL 17 container (`pickup-ui-pg`) and Redis 7 container
(`pickup-ui-redis`) were used on loopback ports 55432/56379. The backend ran on
port 18080 with exact CORS origin `http://127.0.0.1:18081`, migrations enabled,
a process-local random JWT secret, and the loopback SMTP discard sink. Synthetic
preview and owner accounts were created through the real registration/OTP APIs;
three courts and 24 future slots were seeded into the isolated database.

The Expo web app ran on port 18081 against that API with no request mocking.
The Playwright full-stack journey passed: readiness, login, court booking, game
creation, authenticated booking/game history recovery, cancellation, and cleanup.
The exported static web build also passed a direct `/court/[id]` deep-link smoke
test. Payment and storage providers remained intentionally unavailable.

## Historical preview runtime — 2026-10-05

Backend built with `cargo +1.98.1 build --locked --manifest-path server/Cargo.toml`
(passed, 46.51 s), then launched as the actual binary, not a test router.

- API: `http://127.0.0.1:8080`, listening PID 26372 (PowerShell launcher 22744).
- Disposable PostgreSQL: `pickup_preview` at loopback 55432, created separately
  from `pickup_test` and any existing development database; migrations applied.
- Redis: loopback 56379, dedicated database 1; integration tests use database 0.
- SMTP: loopback-only discard sink on 1025, PID 572. Message bodies and codes
  are discarded without logging. No real outbound email is delivered.
- CORS permits exactly `http://localhost:8081` and `http://127.0.0.1:8081`.
  Health and readiness return 200; allowed-origin preflight returns that origin;
  untrusted-origin preflight has no allow-origin header (`runtime-evidence.json`).
- A random JWT secret is generated only inside the launch process environment;
  no existing `.env`, secret, or development configuration was overwritten.

Browser test account: `preview@example.com` / `Pickup-test-2026`, Minh Nguyen,
ID `65e45978-6415-4dd0-8155-5f74c68f75f1`. Synthetic court owner:
`owner-preview@example.com` / `Pickup-test-2026`, Alex Tran,
ID `a64e3c79-7acc-48dc-87f6-f2bdff750bf4`.
Both were created through actual HTTP registration, reading their disposable
Redis OTP, then HTTP verification. No passwords were inserted directly into SQL.

Three synthetic courts (empty photo arrays exercise the client fallback):

- Thao Dien Pickleball Club: `10000000-0000-4000-8000-000000000001`.
- Riverside Football Arena: `10000000-0000-4000-8000-000000000002`.
- District 7 Garden Courts: `10000000-0000-4000-8000-000000000003`.

24 future 90-minute slots were seeded at Vietnam 17:00/19:00 today and the next
three days. The owner booked tomorrow's first Thao Dien slot and created the
six-player game through HTTP: `6dbfea83-c018-4214-81d0-b1f7b48fefde`, slot
`f584a5d8-8590-4cea-981d-f0f4200f2bd8`. Court listing returns `{items,...}`;
game listing returns `{games,...}`. `fixture-evidence.json` records retrieval.

`api-smoke.ps1` created another disposable user through actual registration and
verification, joined/left the owner game, fetched personal games and a two-player
360,000 VND split, booked/recovered/cancelled a separate slot, then logged out.
Results: join roster 2; personal games 1; personal bookings 1; cancel available;
logout 204; subsequent access and refresh both 401. See `api-smoke-evidence.json`.
The main browser account remains unjoined/unbooked for interactive verification.

Payment credentials are intentionally absent and provider endpoints are fake
loopback 10999. Actual initiation returns explicit 501 before reserving money.
S3 is fake loopback 10998 with local synthetic credentials; uploads are unavailable
in this preview. Share links use reserved `.example` HTTPS hosts. These limitations
must be displayed honestly by the client and do not prove payment/storage launch
acceptance. SMTP is a local sink, not real delivery acceptance.

Launch helpers: `start-preview.ps1`, `smtp-sink.cjs`. One-time seed helpers:
`seed-preview.ps1`, `seed-courts.sql` (rerunning the seed against populated preview
data is unsupported). API logs: `api.log`, `api-error.log`; sink logs: `smtp.log`.
All artifacts are disposable and contain synthetic identities only.

Local operations drill: `scripts/check-readiness.ps1` returned 200. A custom
PostgreSQL dump of this synthetic preview restored into new isolated database
`pickup_restore_20261005_dc1c4c78` with 3 courts, 24 slots, 3 users and all
14 migrations. See `backup-drill-evidence.json` and
`docs/staging-operations.md`. Dump bytes are in the ignored verification-services
directory. Production recovery and Redis persistence/restore remain unverified.
