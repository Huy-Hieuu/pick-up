# Staging operations and release evidence

Use a deployment-owned secret store, separate staging PostgreSQL/Redis/storage,
and the release image digest. Development Compose and the local preview are not
production deployment templates. No production credentials or deployment were
provided or exercised in this session.

## Configuration before starting

`Settings::from_env` requires `DATABASE_URL`, a random `JWT_SECRET` of at least
32 bytes, `S3_ACCESS_KEY` and `S3_SECRET_KEY`. Starting with these values does
not prove that SMTP, storage or payment credentials work.

Set these explicitly for staging:

- `APP_HOST`/`APP_PORT` behind a restricted HTTPS ingress; exact HTTP(S)
  `CORS_ORIGINS` without paths/trailing slash or wildcard mixtures.
- `APP_PUBLIC_URL` and `APP_PUBLIC_API_URL` as actual HTTPS origins/URLs.
  Export the client with `EXPO_PUBLIC_API_URL` pointing to that HTTPS API;
  `EXPO_PUBLIC_` variables must never contain secrets.
- `DATABASE_URL` for the restricted application role;
  `DB_RUN_MIGRATIONS=false` after applying migrations separately using a
  migration role. The current latest migration is `20260923000007`.
- `REDIS_URL` for private authenticated/encrypted networking as provided by
  the deployment, durable storage and tested recovery. A Redis database number
  separates key namespaces; it does not isolate security or persistence.
- `SMTP_HOST`, positive `SMTP_PORT`, registered `SMTP_MAIL_FROM`, credentials,
  and `SMTP_USE_TLS=true`. Booleans must be `true`/`false`; invalid sender/port
  fail startup. TLS-disabled local discard sinks do not prove email acceptance.
- Actual `S3_ENDPOINT`, `S3_INTERNAL_ENDPOINT`, `S3_PUBLIC_URL`, bucket, region
  and scoped credentials; apply private quarantine/legacy and sanitized-public
  policies before enabling uploads. URLs signed for clients must be reachable
  from those clients.
- Merchant sandbox credentials and correct provider endpoints, plus reachable
  signed public callbacks. Empty provider credentials intentionally return 501
  before reserving an order. Never substitute fake credentials for acceptance.
- `TRUSTED_PROXY_CIDRS` only for ingress ranges that overwrite forwarding headers;
  set `APP_REQUESTS_PER_MINUTE` from measured traffic expectations.

Follow the migration preflight and old-worker drain order in the
[release checklist](backend-release-checklist.md),
[billing guide](backend-billing.md), and [payment guide](backend-payments.md).
Keep configuration and prior image digest in restricted deployment records.
Never print resolved secret-bearing Compose configuration into public CI logs.

## Health and acceptance

```powershell
./scripts/check-readiness.ps1 -ApiOrigin https://your-staging-api.example
# Local check only:
./scripts/check-readiness.ps1 -ApiOrigin http://127.0.0.1:8080 -Attempts 1
```

The bounded check verifies HTTP 200 from `/health/ready`, which checks only
PostgreSQL and Redis. It does not validate migrations, SMTP, PSP, storage,
frontend functionality or capacity. The runtime image has the same readiness
endpoint in its Docker healthcheck. Probe the deployed ingress separately from
internal readiness and keep authorization checks in the app acceptance run.

Record actual deployed signup/login/reset, booking/game concurrency, personal
schedule recovery, session logout, game completion and final payment collection,
WebSocket reconnect, and sanitized avatar flow. Cross-reference provider records
for payment retries/callback recovery; browser mocks cannot establish that.

## Backup and restore drill

Use PostgreSQL 17 client tools compatible with the deployed server. Put backup
files in encrypted restricted storage with retention/access owners, not Git.
Use libpq service names backed by deployment-owned secret configuration so
passwords do not appear in shell arguments or command logs. Example commands:

```sh
pg_dump --dbname='service=pickup_backup' --format=custom --no-owner --no-acl --file=pickup.dump
# Restore only into an independently provisioned EMPTY drill database.
pg_restore --dbname='service=pickup_restore_drill' --no-owner --no-acl --exit-on-error pickup.dump
psql 'service=pickup_restore_drill' -c 'SELECT version, success FROM _sqlx_migrations ORDER BY version;'
```

The dump excludes role ownership/ACLs; provision the intended roles/grants
separately and validate application access after restore. A logical dump is one
recovery mechanism, not evidence of point-in-time recovery, acceptable RPO/RTO,
or a complete account/storage/Redis backup. Agree those requirements before
release and drill the actual managed infrastructure procedures.

Locally executed on 2026-10-05: custom-format dump of synthetic `pickup_preview`
restored without errors into a new isolated database
`pickup_restore_20261005_dc1c4c78`. Restored 3 courts, 24 slots, 3 synthetic users
and 14 successful migrations. The 41,817-byte dump is under ignored
`artifacts/verification-services/backup-drill/`; safe counts/results are in
`artifacts/fullstack/backup-drill-evidence.json`. This proves those local commands,
not deployed backups or restore performance. Preview Redis uses no AOF; Redis
persistence/restore was not verified. Follow [email operations](backend-email.md)
for queue durability requirements and the deployment's Redis backup procedure.

## Remaining release gates

Real PSP and refunds/chargebacks; registered SMTP/TLS and deliverability;
actual S3/CDN privacy/signing; HTTPS ingress/WAF/network roles/secrets/monitoring;
deployed PostgreSQL/Redis/storage recovery and RPO/RTO; target load budgets and
staging load tests; native signed builds/physical devices and privacy/store checks;
and mobile dependency advisory resolution or explicit release-owner review.
See [mobile readiness](mobile-readiness.md) and the release checklist for evidence
owners. Local green checks do not close these gates.
