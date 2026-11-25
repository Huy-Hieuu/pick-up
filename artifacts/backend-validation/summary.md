# Backend validation — 2026-11-25

Environment: Rust 1.98.1; disposable local PostgreSQL 17 and Redis 7.4.
`DATABASE_URL=postgres://pickup@127.0.0.1:55432/pickup_test`
`TEST_REDIS_URL=redis://127.0.0.1:56379`

From `server`:

```
cargo +1.98.1 fmt --all -- --check
cargo +1.98.1 clippy --locked --all-targets -- -D warnings
cargo +1.98.1 test --locked --all-targets -- --include-ignored --test-threads=1
```

All passed. 146 tests: 91 library, 11 auth integration, 11 billing/booking
integration, 33 HTTP/payment/media/realtime integration. Zero failures or ignored
tests. HTTP group includes the 70-second continuous invalidation heartbeat stress
and forced database-listener disconnect/recovery test; group elapsed 106.42 s.
Full output: `tests.log`; strict Clippy output: `clippy.log`.

GitNexus `detect-changes --scope all --repo pick-up --limit 100` passed and
reported 22 changed tracked files, 65 mapped symbols, LOW risk and zero indexed
affected processes. See `gitnexus-changes.log`. Git diff mapping omits untracked
new artifacts and is not runtime coverage. No commit or branch was created.

Before/after failures and repairs are recorded in `realtime-reproduction.md`.
Mock PSP/SMTP/storage coverage and local integration passes do not replace real
provider acceptance, actual deployment security, backups/restores or load tests.
