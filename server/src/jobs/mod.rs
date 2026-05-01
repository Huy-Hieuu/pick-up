//! Background jobs module.

pub mod email;

use crate::state::AppState;
use tokio_util::sync::CancellationToken;

/// Start all background job workers.
///
/// Called from `main.rs` during server startup.
/// Workers check the CancellationToken for graceful shutdown.
pub async fn start_workers(state: &AppState, cancel: CancellationToken) -> anyhow::Result<()> {
    // Spawn the email delivery worker.
    let redis = state.redis.clone();
    let email_settings = state.settings.email.clone();
    tokio::spawn(crate::jobs::email::run_email_worker(redis, email_settings, cancel));

    tracing::info!("Background workers started");

    Ok(())
}

/// Placeholder: Poll pending payment status from providers.
#[allow(dead_code)]
async fn _job_payment_status_poll() {
    // TODO
}

/// Placeholder: Release expired locked slots.
#[allow(dead_code)]
async fn _job_slot_expiry() {
    // TODO
}
