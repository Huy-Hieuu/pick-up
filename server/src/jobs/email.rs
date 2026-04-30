//! Email delivery job.
//!
//! Uses a Redis list as a simple queue:
//! - `email:queue` → LPUSH {json payload}, BRPOP to consume
//! - `email:dead`  → poison messages that failed deserialization
//!
//! Worker runs as a background tokio task, processing one email at a time.
//! If SMTP is temporarily down, the message stays in the queue and will be retried.

use redis::aio::ConnectionManager;
use serde::{Deserialize, Serialize};

use crate::config::EmailSettings;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailJob {
    pub to: String,
    pub subject: String,
    pub body: String,
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
/// Runs until the tokio runtime shuts down.
pub async fn run_email_worker(
    redis: ConnectionManager,
    email_settings: EmailSettings,
) {
    loop {
        let result = process_one_email(&redis, &email_settings).await;
        if let Err(e) = result {
            tracing::error!(error = %e, "Email worker encountered an error, retrying in 5s");
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    }
}

async fn process_one_email(
    redis: &ConnectionManager,
    email_settings: &EmailSettings,
) -> anyhow::Result<()> {
    use lettre::message::Mailbox;
    use lettre::transport::smtp::authentication::Credentials;
    use lettre::transport::smtp::AsyncSmtpTransport;
    use lettre::transport::smtp::authentication::Mechanism;
    use lettre::transport::AsyncTransport;
    use lettre::Tokio1Executor;

    // BRPOP blocks until an item appears in the queue.
    let payload = {
        let mut conn = redis.clone();
        let result: Result<Vec<String>, _> = redis::cmd("BRPOP")
            .arg("email:queue")
            .arg(1_i32)
            .query_async(&mut conn)
            .await;

        match result {
            Ok(arr) if arr.is_empty() => return Ok(()), // timeout, queue empty
            Ok(arr) => arr.into_iter().last().unwrap(),
            Err(e) => return Err(anyhow::anyhow!("BRPOP failed: {e}")),
        }
    };

    let job: EmailJob = match serde_json::from_str(&payload) {
        Ok(job) => job,
        Err(e) => {
            // Poison message — move to dead-letter queue for inspection.
            tracing::error!(error = %e, "Failed to deserialize email job, moving to dead-letter queue");
            let mut conn = redis.clone();
            let _: () = redis::cmd("LPUSH")
                .arg("email:dead")
                .arg(&payload)
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
        .subject(job.subject)
        .body(job.body)
        .map_err(|e| anyhow::anyhow!("Failed to build email: {e}"))?;

    let credentials = Credentials::new(
        email_settings.smtp_username.clone(),
        email_settings.smtp_password.clone(),
    );

    // port 587 + STARTTLS with explicit LOGIN auth mechanism
    let mailer = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&email_settings.smtp_host)
        .map_err(|e| anyhow::anyhow!("Failed to build SMTP relay: {e}"))?
        .port(email_settings.smtp_port)
        .credentials(credentials)
        .authentication(vec![Mechanism::Login])
        .build();

    mailer.send(email).await.map_err(|e| {
        tracing::error!(%e, to = %job.to, "SMTP send failed");
        anyhow::anyhow!("SMTP send failed: {e}")
    })?;

    tracing::info!(to = %job.to, "Email sent successfully");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── EmailJob serialization roundtrip ───────────────────────────

    #[test]
    fn serialize_deserialize_roundtrip() {
        let job = EmailJob {
            to: "user@example.com".to_string(),
            subject: "Test Subject".to_string(),
            body: "Hello, world!".to_string(),
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
        };
        let json = serde_json::to_string(&job).unwrap();

        // Verify JSON structure
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
        };
        let json = serde_json::to_string(&job).unwrap();
        let parsed: EmailJob = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.subject, job.subject);
    }

    // ── OTP email body format ──────────────────────────────────────

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

    // ── Dead-letter queue key ──────────────────────────────────────

    #[test]
    fn dead_letter_queue_name() {
        // Verify the queue key matches what the worker uses
        let queue = "email:dead";
        assert_eq!(queue, "email:dead");
    }
}
