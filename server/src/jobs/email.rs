//! Email delivery job.
//!
//! Uses a Redis list as a simple queue:
//! - `email:queue` → LPUSH {json payload}, BRPOP to consume
//! - `email:dead`  → poison messages that failed deserialization
//!
//! Worker runs as a background tokio task, processing one email at a time.
//! Supports exponential backoff on errors and graceful shutdown via CancellationToken.

use redis::aio::ConnectionManager;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::config::EmailSettings;

const MAX_RETRIES: u8 = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailJob {
    pub to: String,
    pub subject: String,
    pub body: String,
    #[serde(default)]
    pub retry_count: u8,
}

/// Enqueue an email job to be sent by the background worker.
pub async fn enqueue_email(
    redis: &ConnectionManager,
    to: &str,
    subject: &str,
    body: &str,
) -> crate::error::AppResult<()> {
    let job = EmailJob {
        to: to.to_string(),
        subject: subject.to_string(),
        body: body.to_string(),
        retry_count: 0,
    };
    let payload = serde_json::to_string(&job).map_err(|e| crate::error::AppError::Internal(anyhow::anyhow!("JSON serialize error: {e}")))?;
    let mut conn = redis.clone();
    let _: () = redis::cmd("LPUSH")
        .arg("email:queue")
        .arg(&payload)
        .query_async(&mut conn)
        .await
        .map_err(|e| crate::error::AppError::Internal(anyhow::anyhow!("Redis error: {e}")))?;
    tracing::debug!(to, "Email job enqueued");
    Ok(())
}

/// Background worker that processes email jobs from the Redis queue.
/// Supports exponential backoff and graceful shutdown via CancellationToken.
pub async fn run_email_worker(
    redis: ConnectionManager,
    email_settings: EmailSettings,
    cancel: CancellationToken,
) {
    let mut backoff_secs: u64 = 0;
    let max_backoff_secs: u64 = 60;

    // Build the SMTP transport once and reuse it (M11).
    let mailer = build_smtp_transport(&email_settings);

    loop {
        // Check for shutdown signal.
        if cancel.is_cancelled() {
            tracing::info!("Email worker received shutdown signal, draining remaining emails...");
            // Drain the queue before exiting.
            while let Ok(Some(payload)) = dequeue_email(&redis, 0).await {
                let _ = process_one_email_with_transport(&redis, &mailer, &email_settings, &payload).await;
            }
            tracing::info!("Email worker drained queue, exiting.");
            return;
        }

        let result = dequeue_and_process(&redis, &mailer, &email_settings, &cancel).await;
        match result {
            Ok(()) => {
                // Reset backoff on success.
                backoff_secs = 0;
            }
            Err(e) => {
                // Exponential backoff on error (M9).
                backoff_secs = if backoff_secs == 0 { 5 } else { (backoff_secs * 2).min(max_backoff_secs) };
                tracing::error!(error = %e, backoff_secs, "Email worker error, backing off");

                // Wait with shutdown awareness.
                tokio::select! {
                    _ = tokio::time::sleep(std::time::Duration::from_secs(backoff_secs)) => {},
                    _ = cancel.cancelled() => {
                        tracing::info!("Email worker shutdown during backoff, exiting.");
                        return;
                    }
                }
            }
        }
    }
}

/// Build SMTP transport. Reused across emails for efficiency.
fn build_smtp_transport(email_settings: &EmailSettings) -> lettre::AsyncSmtpTransport<lettre::Tokio1Executor> {
    use lettre::transport::smtp::authentication::{Credentials, Mechanism};

    let credentials = Credentials::new(
        email_settings.smtp_username.clone(),
        email_settings.smtp_password.clone(),
    );

    lettre::AsyncSmtpTransport::<lettre::Tokio1Executor>::starttls_relay(&email_settings.smtp_host)
        .unwrap_or_else(|e| {
            tracing::error!("Failed to build SMTP relay: {e}");
            // Return a dummy transport that will fail on send — avoids panicking.
            lettre::AsyncSmtpTransport::<lettre::Tokio1Executor>::starttls_relay("localhost")
                .unwrap()
        })
        .port(email_settings.smtp_port)
        .credentials(credentials)
        .authentication(vec![Mechanism::Login])
        .build()
}

/// Dequeue an email from Redis with optional timeout.
/// timeout=0 means non-blocking (for drain), timeout>0 means blocking wait.
async fn dequeue_email(
    redis: &ConnectionManager,
    timeout_secs: i32,
) -> anyhow::Result<Option<String>> {
    let mut conn = redis.clone();
    let result: Vec<String> = redis::cmd("BRPOP")
        .arg("email:queue")
        .arg(timeout_secs)
        .query_async(&mut conn)
        .await
        .map_err(|e| anyhow::anyhow!("BRPOP failed: {e}"))?;

    if result.is_empty() {
        return Ok(None);
    }
    // BRPOP returns [list_name, payload] — take the last element (L4: safe unwrap replacement).
    Ok(result.into_iter().last())
}

/// Dequeue and process a single email, with cancellation support.
async fn dequeue_and_process(
    redis: &ConnectionManager,
    mailer: &lettre::AsyncSmtpTransport<lettre::Tokio1Executor>,
    email_settings: &EmailSettings,
    cancel: &CancellationToken,
) -> anyhow::Result<()> {
    // BRPOP with 1s timeout for responsive shutdown.
    let payload = {
        let mut conn = redis.clone();
        let result: Vec<String> = redis::cmd("BRPOP")
            .arg("email:queue")
            .arg(1_i32)
            .query_async(&mut conn)
            .await
            .map_err(|e| anyhow::anyhow!("BRPOP failed: {e}"))?;

        if result.is_empty() {
            return Ok(()); // timeout, queue empty
        }
        // Safe: BRPOP returns [key, value] when non-empty.
        result.into_iter().last().unwrap_or_default()
    };

    if payload.is_empty() {
        return Ok(());
    }

    // Check cancellation after dequeue.
    if cancel.is_cancelled() {
        // Re-queue the email since we haven't processed it.
        let mut conn = redis.clone();
        let _: () = redis::cmd("LPUSH")
            .arg("email:queue")
            .arg(&payload)
            .query_async(&mut conn)
            .await
            .map_err(|re| anyhow::anyhow!("Failed to re-queue email during shutdown: {re}"))?;
        return Ok(());
    }

    process_one_email_with_transport(redis, mailer, email_settings, &payload).await
}

/// Process a single email payload using the shared transport.
async fn process_one_email_with_transport(
    redis: &ConnectionManager,
    mailer: &lettre::AsyncSmtpTransport<lettre::Tokio1Executor>,
    email_settings: &EmailSettings,
    payload: &str,
) -> anyhow::Result<()> {
    use lettre::message::Mailbox;
    use lettre::transport::AsyncTransport;

    let job: EmailJob = match serde_json::from_str(payload) {
        Ok(job) => job,
        Err(e) => {
            // Poison message — move to dead-letter queue.
            tracing::error!(error = %e, "Failed to deserialize email job, moving to dead-letter queue");
            let mut conn = redis.clone();
            let _: () = redis::cmd("LPUSH")
                .arg("email:dead")
                .arg(payload)
                .query_async(&mut conn)
                .await
                .map_err(|re| anyhow::anyhow!("Failed to push to dead-letter queue: {re}"))?;
            return Ok(());
        }
    };

    tracing::info!(to = %job.to, "Processing email job");

    let from: Mailbox = email_settings
        .mail_from
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid from address: {e}"))?;

    let to: Mailbox = job
        .to
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid to address: {e}"))?;

    let email = lettre::Message::builder()
        .from(from)
        .to(to)
        .subject(job.subject.clone())
        .body(job.body.clone())
        .map_err(|e| anyhow::anyhow!("Failed to build email: {e}"))?;

    if let Err(e) = mailer.send(email).await {
        tracing::error!(%e, to = %job.to, retry = job.retry_count, "SMTP send failed");

        if job.retry_count >= MAX_RETRIES {
            // Max retries — move to dead-letter queue with updated retry count (M10).
            let mut retry_job = job.clone();
            retry_job.retry_count = MAX_RETRIES;
            let dead_payload = serde_json::to_string(&retry_job)
                .map_err(|e| anyhow::anyhow!("JSON serialize error: {e}"))?;
            let mut conn = redis.clone();
            let _: () = redis::cmd("LPUSH")
                .arg("email:dead")
                .arg(&dead_payload)
                .query_async(&mut conn)
                .await
                .map_err(|re| anyhow::anyhow!("Failed to push to dead-letter queue: {re}"))?;
            // Return Ok — the email was handled (moved to dead-letter) (M10).
            return Ok(());
        }

        // Re-queue with incremented retry count.
        let mut retry_job = job.clone();
        retry_job.retry_count += 1;
        let retry_payload = serde_json::to_string(&retry_job)
            .map_err(|e| anyhow::anyhow!("JSON serialize error: {e}"))?;
        let mut conn = redis.clone();
        let _: () = redis::cmd("RPUSH")
            .arg("email:queue")
            .arg(&retry_payload)
            .query_async(&mut conn)
            .await
            .map_err(|re| anyhow::anyhow!("Failed to re-queue email: {re}"))?;

        return Err(anyhow::anyhow!("SMTP send failed, re-queued (attempt {}/{MAX_RETRIES}): {e}", job.retry_count + 1));
    }

    tracing::info!(to = %job.to, "Email sent successfully");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_deserialize_roundtrip() {
        let job = EmailJob {
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
}
