# Email queue operations

OTP and password-reset emails enter a bounded Redis queue atomically with their
Redis challenge and cooldown. The Compose Redis services persist the queue with
Append Only File (AOF), `appendfsync everysec`; production Redis needs equivalent
persistence, backup and restore checks.

- Jobs move atomically from ready to a per-worker lease. Redis stores lease
  metadata separately from the payload and reclaims expired leases after 60
  seconds. Workers poll/recover due retries at one-second intervals.
- SMTP sends have a ten-second timeout. Transient errors retry after 5, 10 and
  20 seconds, for four total attempts. Malformed jobs and exhausted retries go
  to a dead-letter list capped at 1,000 entries.
- Dead letters contain only a job ID, masked recipient, retry count and error
  class. Message bodies and OTP codes are never retained there or logged.
- The previous worker wrote complete payloads to `email:dead`. Before upgrading
  a Redis instance created by that version, treat those records as OTP material:
  inspect/export them to restricted storage or clear the exact `email:dead` key
  during a planned migration. Do not expose the Redis console publicly.
- Shutdown stops new claims, releases a claim if shutdown wins immediately
  after dequeue, finishes any in-flight bounded SMTP send, then joins workers
  before process exit. A crash leaves jobs leased until recovery.
- SMTP delivery is **at least once**. If the SMTP server accepts a message but
  its response or worker acknowledgement is lost, lease recovery can resend it.
  Single-use OTPs and the resend cooldown limit the impact.
- The ready, leased and delayed queue is capped at 10,000 jobs. At capacity,
  enqueue fails instead of reporting that an OTP was sent.

One worker runs per API instance; Redis leases allow multiple replicas. Monitor
queue depth, lease age, delayed retries, dead letters, SMTP failure rates and AOF
health. The fsync interval leaves roughly one second of writes to lose during a
host failure. Production Redis also needs durable storage, replication,
monitoring and restore tests.

An ignored test simulates a process crash after lease, recovery, retry scheduling
and dead-letter redaction. Test SMTP credentials, sender registration and
deliverability with the actual provider; local queue tests cannot verify those.
