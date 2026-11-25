# Runtime regressions, 2026-10-05

Disposable PostgreSQL 17 on loopback 55432 and Redis 7.4 on loopback 56379.
Rust 1.98.1, Windows. Real TCP WebSocket clients, actual PostgreSQL notifications.

`continuous_invalidations_do_not_starve_control_frames`: publish committed
`pg_notify('pickup_game_changed', game_id)` every 20 ms (50/sec), consume server
frames and answer every Ping. Original biased notification-before-read selection
failed after 60.80 s with one answered Ping and socket reset 10054. Prioritizing
control reads ahead of notifications passed the same 70-second test at 70.59 s.
Shutdown/expiry/heartbeat/recheck priority and 100 ms snapshot coalescing remain.

`listener_reconnect_and_periodic_recheck_recover_a_missed_notification`:
terminate the dedicated listener via `pg_terminate_backend`. Original loop failed
the 10-second reconnect deadline both in the full suite and isolation (10.51 s).
SQLx 0.8.6 retains ConnectionReset sockets on this platform. Explicitly replacing
errored listeners, restoring LISTEN, and invalidating rooms passed the existing
regression. Retries remain bounded/cancellable with 1–30 s backoff.

Final commands/results are recorded in `clippy.log` and `tests.log`. Provider
mocks and local services do not establish real payment, SMTP, storage or
production-load acceptance.
