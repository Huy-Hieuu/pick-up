//! Email delivery job.
//!
//! Uses a Redis list as a simple queue:
//! - `email:queue` → LPUSH {json payload}, BRPOP to consume
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

    let job: EmailJob = serde_json::from_str(&payload)
        .map_err(|e| anyhow::anyhow!("Failed to deserialize email job: {e}"))?;

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
