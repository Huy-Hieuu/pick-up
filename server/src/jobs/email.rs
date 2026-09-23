//! Email delivery job.
//!
//! Redis queue with atomic ready → leased transitions, reclaimable leases,
//! scheduled retries, and a bounded dead-letter list.

use redis::aio::ConnectionManager;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::config::EmailSettings;

const MAX_RETRIES: u8 = 3;
const MAX_QUEUE_LENGTH: i64 = 10_000;
const MAX_DEAD_LETTERS: usize = 1_000;
const LEASE_SECONDS: i64 = 60;
const MAX_RECLAIM_BATCH: i64 = 25;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailJob {
    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,
    pub to: String,
    pub subject: String,
    pub body: String,
    #[serde(default)]
    pub retry_count: u8,
}

/// Enqueue an email job to be sent by the background worker.
pub async fn enqueue_email(
    redis: &ConnectionManager,
    job: &EmailJob,
) -> crate::error::AppResult<()> {
    let payload = serde_json::to_string(job).map_err(|e| {
        crate::error::AppError::Internal(anyhow::anyhow!("JSON serialize error: {e}"))
    })?;
    let mut conn = redis.clone();
    let accepted: i32 = redis::Script::new(r#"
        local length = redis.call('LLEN', KEYS[1]) + redis.call('HLEN', KEYS[2]) + redis.call('ZCARD', KEYS[3]) + redis.call('HLEN', KEYS[4])
        if length >= tonumber(ARGV[2]) then return 0 end
        redis.call('LPUSH', KEYS[1], ARGV[1])
        if redis.call('LLEN', KEYS[5]) == 0 then redis.call('LPUSH', KEYS[5], 'ready') end
        return 1
    "#).key("email:queue").key("email:processing").key("email:retry")
        .key("email:retry-payloads").key("email:wake").arg(&payload).arg(MAX_QUEUE_LENGTH)
        .invoke_async(&mut conn)
        .await
        .map_err(|e| crate::error::AppError::Internal(anyhow::anyhow!("Redis error: {e}")))?;
    if accepted == 0 {
        return Err(crate::error::AppError::TooManyRequests(
            "Email queue is full; retry later".into(),
        ));
    }
    tracing::debug!(recipient = %redact_email(&job.to), "Email job enqueued");
    Ok(())
}

/// Background worker that reclaims leases and retries email delivery.
pub async fn run_email_worker(
    redis: ConnectionManager,
    email_settings: EmailSettings,
    cancel: CancellationToken,
) {
    let consumer_id = Uuid::new_v4();

    // Build the SMTP transport once and reuse it (M11).
    let mailer = match build_smtp_transport(&email_settings) {
        Ok(mailer) => mailer,
        Err(error) => {
            tracing::error!(%error, "Invalid SMTP configuration; worker stopped");
            return;
        }
    };

    loop {
        // Check for shutdown signal.
        if cancel.is_cancelled() {
            // Leave ready, delayed and processing queues for the next worker.
            tracing::info!("Email worker stopping; queued mail retained");
            return;
        }

        match dequeue_and_process(&redis, &mailer, &email_settings, &cancel, consumer_id).await {
            Ok(_) => {}
            Err(error) => {
                tracing::error!(%error, "Email queue operation failed; retrying shortly");
                tokio::select! {
                    _ = tokio::time::sleep(std::time::Duration::from_secs(2)) => {},
                    _ = cancel.cancelled() => return,
                }
            }
        }
    }
}

/// Build SMTP transport. Reused across emails for efficiency.
pub(crate) fn build_smtp_transport(
    email_settings: &EmailSettings,
) -> anyhow::Result<lettre::AsyncSmtpTransport<lettre::Tokio1Executor>> {
    use lettre::transport::smtp::authentication::{Credentials, Mechanism};

    let credentials = Credentials::new(
        email_settings.smtp_username.clone(),
        email_settings.smtp_password.clone(),
    );

    let builder = if email_settings.use_tls {
        lettre::AsyncSmtpTransport::<lettre::Tokio1Executor>::starttls_relay(
            &email_settings.smtp_host,
        )?
    } else {
        // Only for trusted local development mail sinks.
        lettre::AsyncSmtpTransport::<lettre::Tokio1Executor>::builder_dangerous(
            &email_settings.smtp_host,
        )
    };
    let mut builder = builder
        .port(email_settings.smtp_port)
        .timeout(Some(std::time::Duration::from_secs(10)));
    if !email_settings.smtp_username.is_empty() {
        builder = builder
            .credentials(credentials)
            .authentication(vec![Mechanism::Login]);
    }
    Ok(builder.build())
}

/// Atomically claim one queued message, then acknowledge, retry or dead-letter.
async fn dequeue_and_process(
    redis: &ConnectionManager,
    mailer: &lettre::AsyncSmtpTransport<lettre::Tokio1Executor>,
    email_settings: &EmailSettings,
    cancel: &CancellationToken,
    consumer_id: Uuid,
) -> anyhow::Result<bool> {
    reclaim_expired_leases(redis).await?;
    promote_due_retries(redis).await?;
    let lease_id = format!("{consumer_id}:{}", Uuid::new_v4());
    let Some(payload) = claim_next(redis, &lease_id).await? else {
        let mut conn = redis.clone();
        let _: Option<(String, String)> = redis::cmd("BLPOP")
            .arg("email:wake")
            .arg(1_i32)
            .query_async(&mut conn)
            .await?;
        return Ok(false);
    };
    if cancel.is_cancelled() {
        release_lease(redis, &lease_id).await?;
        return Ok(false);
    }

    match process_one_email_with_transport(mailer, email_settings, &payload).await {
        Ok(()) => acknowledge(redis, &lease_id).await?,
        Err(DeliveryError::Permanent(reason)) => {
            tracing::warn!(%lease_id, reason, "Email is permanently undeliverable");
            move_to_dead_letter(redis, &lease_id, &payload, reason).await?;
        }
        Err(DeliveryError::Transient) => {
            let mut job: EmailJob = serde_json::from_str(&payload)?;
            if job.retry_count >= MAX_RETRIES {
                tracing::warn!(%lease_id, retries = job.retry_count, "Email retry limit reached");
                move_to_dead_letter(redis, &lease_id, &payload, "retry_limit_reached").await?;
            } else {
                job.retry_count += 1;
                let delay = (5_u64.saturating_mul(1_u64 << (job.retry_count - 1))).min(300);
                let retry_count = job.retry_count;
                retry_later(redis, &lease_id, job, delay).await?;
                tracing::warn!(%lease_id, retries = retry_count, delay, "Email retry scheduled");
            }
        }
    }
    Ok(true)
}

async fn claim_next(redis: &ConnectionManager, lease_id: &str) -> anyhow::Result<Option<String>> {
    let mut conn = redis.clone();
    let payload: Option<String> = redis::Script::new(
        r#"
        local payload = redis.call('RPOP', KEYS[1])
        if not payload then return nil end
        redis.call('HSET', KEYS[2], ARGV[1], payload)
        redis.call('ZADD', KEYS[3], ARGV[2], ARGV[1])
        return payload
    "#,
    )
    .key("email:queue")
    .key("email:processing")
    .key("email:leases")
    .arg(lease_id)
    .arg(UtcMillis::after_seconds(LEASE_SECONDS))
    .invoke_async(&mut conn)
    .await?;
    Ok(payload)
}

/// Process a single email payload using the shared transport.
async fn process_one_email_with_transport(
    mailer: &lettre::AsyncSmtpTransport<lettre::Tokio1Executor>,
    email_settings: &EmailSettings,
    payload: &str,
) -> Result<(), DeliveryError> {
    use lettre::message::Mailbox;
    use lettre::transport::AsyncTransport;

    let job: EmailJob = match serde_json::from_str(payload) {
        Ok(job) => job,
        Err(_) => return Err(DeliveryError::Permanent("malformed_payload")),
    };

    let from: Mailbox = email_settings
        .mail_from
        .parse()
        .map_err(|_| DeliveryError::Permanent("invalid_sender"))?;

    let to: Mailbox = job
        .to
        .parse()
        .map_err(|_| DeliveryError::Permanent("invalid_recipient"))?;

    let email = lettre::Message::builder()
        .from(from)
        .to(to)
        .subject(job.subject.clone())
        .body(job.body.clone())
        .map_err(|_| DeliveryError::Permanent("invalid_content"))?;

    if mailer.send(email).await.is_err() {
        // SMTP replies may include recipient data. Record only the error class.
        return Err(DeliveryError::Transient);
    }

    Ok(())
}

#[derive(Debug)]
enum DeliveryError {
    Permanent(&'static str),
    Transient,
}

async fn acknowledge(redis: &ConnectionManager, lease: &str) -> anyhow::Result<()> {
    redis::Script::new(
        "redis.call('HDEL', KEYS[1], ARGV[1]); redis.call('ZREM', KEYS[2], ARGV[1]); return 1",
    )
    .key("email:processing")
    .key("email:leases")
    .arg(lease)
    .invoke_async::<i32>(&mut redis.clone())
    .await?;
    Ok(())
}

async fn release_lease(redis: &ConnectionManager, lease: &str) -> anyhow::Result<()> {
    redis::Script::new(
        r#"
        local payload = redis.call('HGET', KEYS[1], ARGV[1])
        if payload then
            redis.call('LPUSH', KEYS[2], payload)
            if redis.call('LLEN', KEYS[4]) == 0 then redis.call('LPUSH', KEYS[4], 'ready') end
        end
        redis.call('HDEL', KEYS[1], ARGV[1]); redis.call('ZREM', KEYS[3], ARGV[1])
        return 1
    "#,
    )
    .key("email:processing")
    .key("email:queue")
    .key("email:leases")
    .key("email:wake")
    .arg(lease)
    .invoke_async::<i32>(&mut redis.clone())
    .await?;
    Ok(())
}

async fn reclaim_expired_leases(redis: &ConnectionManager) -> anyhow::Result<()> {
    redis::Script::new(
        r#"
        local ids = redis.call('ZRANGEBYSCORE', KEYS[1], '-inf', ARGV[1], 'LIMIT', 0, ARGV[2])
        for _, id in ipairs(ids) do
            local payload = redis.call('HGET', KEYS[2], id)
            if payload then redis.call('LPUSH', KEYS[3], payload) end
            redis.call('HDEL', KEYS[2], id); redis.call('ZREM', KEYS[1], id)
        end
        if #ids > 0 and redis.call('LLEN', KEYS[4]) == 0 then redis.call('LPUSH', KEYS[4], 'ready') end
        return #ids
    "#,
    )
    .key("email:leases")
    .key("email:processing")
    .key("email:queue")
    .key("email:wake")
    .arg(UtcMillis::now())
    .arg(MAX_RECLAIM_BATCH)
    .invoke_async::<i32>(&mut redis.clone())
    .await?;
    Ok(())
}

async fn promote_due_retries(redis: &ConnectionManager) -> anyhow::Result<()> {
    redis::Script::new(
        r#"
        local ids = redis.call('ZRANGEBYSCORE', KEYS[1], '-inf', ARGV[1], 'LIMIT', 0, ARGV[2])
        for _, id in ipairs(ids) do
            local payload = redis.call('HGET', KEYS[2], id)
            if payload then redis.call('LPUSH', KEYS[3], payload) end
            redis.call('HDEL', KEYS[2], id); redis.call('ZREM', KEYS[1], id)
        end
        if #ids > 0 and redis.call('LLEN', KEYS[4]) == 0 then redis.call('LPUSH', KEYS[4], 'ready') end
        return #ids
    "#,
    )
    .key("email:retry")
    .key("email:retry-payloads")
    .key("email:queue")
    .key("email:wake")
    .arg(UtcMillis::now())
    .arg(MAX_RECLAIM_BATCH)
    .invoke_async::<i32>(&mut redis.clone())
    .await?;
    Ok(())
}

async fn retry_later(
    redis: &ConnectionManager,
    lease: &str,
    job: EmailJob,
    delay: u64,
) -> anyhow::Result<()> {
    let payload = serde_json::to_string(&job)?;
    let due = UtcMillis::after_seconds(delay as i64);
    redis::Script::new(
        r#"
        redis.call('ZADD', KEYS[1], ARGV[1], ARGV[2])
        redis.call('HSET', KEYS[2], ARGV[2], ARGV[3])
        redis.call('HDEL', KEYS[3], ARGV[4]); redis.call('ZREM', KEYS[4], ARGV[4])
        return 1
    "#,
    )
    .key("email:retry")
    .key("email:retry-payloads")
    .key("email:processing")
    .key("email:leases")
    .arg(due)
    .arg(job.id.to_string())
    .arg(payload)
    .arg(lease)
    .invoke_async::<i32>(&mut redis.clone())
    .await?;
    Ok(())
}

async fn move_to_dead_letter(
    redis: &ConnectionManager,
    lease: &str,
    payload: &str,
    reason: &str,
) -> anyhow::Result<()> {
    // OTP email bodies are credentials. Retain a bounded, redacted dead-letter
    // record; a fresh OTP must be issued instead of replaying expired secrets.
    let record = match serde_json::from_str::<EmailJob>(payload) {
        Ok(job) => serde_json::json!({
            "id": job.id,
            "recipient": redact_email(&job.to),
            "retry_count": job.retry_count,
            "reason": reason,
        }),
        Err(_) => serde_json::json!({
            "id": lease,
            "reason": reason,
            "payload_sha256": hex::encode(sha2::Sha256::digest(payload.as_bytes())),
        }),
    }
    .to_string();
    redis::Script::new(
        r#"
        redis.call('LPUSH', KEYS[1], ARGV[1]); redis.call('LTRIM', KEYS[1], 0, ARGV[2])
        redis.call('HDEL', KEYS[2], ARGV[3]); redis.call('ZREM', KEYS[3], ARGV[3])
        return 1
    "#,
    )
    .key("email:dead")
    .key("email:processing")
    .key("email:leases")
    .arg(record)
    .arg(MAX_DEAD_LETTERS - 1)
    .arg(lease)
    .invoke_async::<i32>(&mut redis.clone())
    .await?;
    Ok(())
}

struct UtcMillis;
impl UtcMillis {
    fn now() -> i64 {
        chrono::Utc::now().timestamp_millis()
    }
    fn after_seconds(seconds: i64) -> i64 {
        Self::now().saturating_add(seconds.saturating_mul(1000))
    }
}

fn redact_email(email: &str) -> String {
    let Some((name, domain)) = email.split_once('@') else {
        return "[invalid]".into();
    };
    let first = name.chars().next().unwrap_or('*');
    format!("{first}***@{domain}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_deserialize_roundtrip() {
        let job = EmailJob {
            id: Uuid::new_v4(),
            to: "user@example.com".to_string(),
            subject: "Test Subject".to_string(),
            body: "Hello, world!".to_string(),
            retry_count: 0,
        };
        let json = serde_json::to_string(&job).unwrap();
        let parsed: EmailJob = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.to, job.to);
        assert_eq!(parsed.subject, job.subject);
        assert_eq!(parsed.body, job.body);
    }

    #[test]
    fn serialize_produces_valid_json() {
        let job = EmailJob {
            id: Uuid::new_v4(),
            to: "test@pickup.app".to_string(),
            subject: "Your code is 123456".to_string(),
            body: "Use this code to verify.".to_string(),
            retry_count: 0,
        };
        let json = serde_json::to_string(&job).unwrap();

        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["to"], "test@pickup.app");
        assert_eq!(v["subject"], "Your code is 123456");
        assert_eq!(v["body"], "Use this code to verify.");
    }

    #[test]
    fn deserialize_rejects_missing_fields() {
        let json = r#"{"to": "user@example.com"}"#;
        let result = serde_json::from_str::<EmailJob>(json);
        assert!(result.is_err());
    }

    #[test]
    fn deserialize_rejects_invalid_json() {
        let json = r#"not valid json"#;
        let result = serde_json::from_str::<EmailJob>(json);
        assert!(result.is_err());
    }

    #[test]
    fn handles_unicode_in_body() {
        let job = EmailJob {
            id: Uuid::new_v4(),
            to: "user@example.com".to_string(),
            subject: "Mã xác nhận PickUp".to_string(),
            body: "Mã của bạn là: 123456\nMã có hiệu lực trong 5 phút.".to_string(),
            retry_count: 0,
        };
        let json = serde_json::to_string(&job).unwrap();
        let parsed: EmailJob = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.body, job.body);
    }

    #[test]
    fn handles_special_characters_in_subject() {
        let job = EmailJob {
            id: Uuid::new_v4(),
            to: "user@example.com".to_string(),
            subject: "Welcome to PickUp! 🎾 Courts & Games".to_string(),
            body: "Let's play!".to_string(),
            retry_count: 0,
        };
        let json = serde_json::to_string(&job).unwrap();
        let parsed: EmailJob = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.subject, job.subject);
    }

    #[test]
    fn otp_email_body_contains_code() {
        let code = "123456";
        let body = format!(
            "Your PickUp verification code is: {}\n\nThis code expires in 5 minutes.",
            code
        );
        assert!(body.contains(code));
        assert!(body.contains("expires in 5 minutes"));
    }

    #[tokio::test]
    #[ignore = "requires TEST_REDIS_URL to point at disposable Redis"]
    async fn leased_message_is_recovered_after_consumer_crash_and_ack_is_atomic() {
        use redis::AsyncCommands;
        let connection = redis::Client::open(std::env::var("TEST_REDIS_URL").unwrap())
            .unwrap()
            .get_connection_manager()
            .await
            .unwrap();
        let job = EmailJob {
            id: Uuid::new_v4(),
            to: "recover@example.test".into(),
            subject: "OTP".into(),
            body: "123456".into(),
            retry_count: 0,
        };
        let original = serde_json::to_string(&job).unwrap();
        enqueue_email(&connection, &job).await.unwrap();
        let mut unrelated = Vec::new();
        let (lease, claimed) = loop {
            let lease = format!("test:{}", Uuid::new_v4());
            let payload = claim_next(&connection, &lease)
                .await
                .unwrap()
                .expect("the test email must be present in the ready queue");
            if payload.contains(&job.id.to_string()) {
                break (lease, payload);
            }
            unrelated.push(lease);
        };
        assert_eq!(claimed, original);
        // Simulate SIGKILL while the message is in-flight.
        let _: usize = redis::cmd("ZADD")
            .arg("email:leases")
            .arg(0)
            .arg(&lease)
            .query_async(&mut connection.clone())
            .await
            .unwrap();
        reclaim_expired_leases(&connection).await.unwrap();
        assert_eq!(
            claim_next(&connection, "test-recovered").await.unwrap(),
            Some(original.clone())
        );
        acknowledge(&connection, "test-recovered").await.unwrap();
        for lease in unrelated.into_iter().rev() {
            release_lease(&connection, &lease).await.unwrap();
        }
        let pending: Option<String> = connection
            .clone()
            .hget("email:processing", &lease)
            .await
            .unwrap();
        assert!(pending.is_none());
        let held: Option<f64> = connection
            .clone()
            .zscore("email:leases", &lease)
            .await
            .unwrap();
        assert!(held.is_none());
        let ready: Vec<String> = connection
            .clone()
            .lrange("email:queue", 0, -1)
            .await
            .unwrap();
        assert!(!ready
            .iter()
            .any(|payload| payload.contains(&job.id.to_string())));
        // Discard only this test job; other test/database workers may share Redis.
        let _: i64 = redis::cmd("LREM")
            .arg("email:queue")
            .arg(1)
            .arg(&original)
            .query_async(&mut connection.clone())
            .await
            .unwrap();
    }

    #[tokio::test]
    #[ignore = "requires TEST_REDIS_URL to point at disposable Redis"]
    async fn retries_are_delayed_and_dead_letters_redact_otp_content() {
        use redis::AsyncCommands;
        let redis = redis::Client::open(std::env::var("TEST_REDIS_URL").unwrap())
            .unwrap()
            .get_connection_manager()
            .await
            .unwrap();
        let job = EmailJob {
            id: Uuid::new_v4(),
            to: "recovery@example.test".into(),
            subject: "OTP".into(),
            body: "private-code-654321".into(),
            retry_count: 0,
        };
        enqueue_email(&redis, &job).await.unwrap();
        let original = serde_json::to_string(&job).unwrap();
        let mut unrelated = Vec::new();
        let (lease, _) = loop {
            let lease = format!("test:{}", Uuid::new_v4());
            let payload = claim_next(&redis, &lease)
                .await
                .unwrap()
                .expect("queued job");
            if payload.contains(&job.id.to_string()) {
                break (lease, payload);
            }
            unrelated.push(lease);
        };
        let mut retry = job.clone();
        retry.retry_count = 1;
        retry_later(&redis, &lease, retry, 300).await.unwrap();
        let due: i64 = redis
            .clone()
            .zscore("email:retry", job.id.to_string())
            .await
            .unwrap();
        assert!(due > UtcMillis::now());
        let _: i64 = redis::cmd("ZADD")
            .arg("email:retry")
            .arg(0)
            .arg(job.id.to_string())
            .query_async(&mut redis.clone())
            .await
            .unwrap();
        promote_due_retries(&redis).await.unwrap();
        let retried_lease = format!("test:{}", Uuid::new_v4());
        let retried = claim_next(&redis, &retried_lease).await.unwrap().unwrap();
        let retried_job: EmailJob = serde_json::from_str(&retried).unwrap();
        assert_eq!(retried_job.retry_count, 1);
        move_to_dead_letter(&redis, &retried_lease, &retried, "retry_limit_reached")
            .await
            .unwrap();
        let dead: Vec<String> = redis.clone().lrange("email:dead", 0, 1000).await.unwrap();
        let record = dead
            .iter()
            .find(|record| record.contains(&job.id.to_string()))
            .unwrap();
        assert!(record.contains("r***@example.test"));
        assert!(record.contains("retry_limit_reached"));
        assert!(!record.contains("private-code-654321"));
        let _: i64 = redis::cmd("LREM")
            .arg("email:dead")
            .arg(1)
            .arg(record)
            .query_async(&mut redis.clone())
            .await
            .unwrap();
        for lease in unrelated.into_iter().rev() {
            release_lease(&redis, &lease).await.unwrap();
        }
        assert!(!original.is_empty());
    }
}
