//! Background jobs module.
//!
//! Phase 2+ features:
//! - Email delivery (via Redis queue + background worker)
//! - Payment status polling (for pending payments)
//! - Expired slot release
//! - Push notification dispatch

pub mod email;

use crate::state::AppState;

/// Start all background job workers.
///
/// Called from `main.rs` during server startup.
pub async fn start_workers(state: &AppState) -> anyhow::Result<()> {
    // Spawn the email delivery worker.
    let redis = state.redis.clone();
    let email_settings = state.settings.email.clone();
    tokio::spawn(crate::jobs::email::run_email_worker(redis, email_settings));

    tracing::info!("Background workers started");

    // TODO: Spawn more jobs:
    // tokio::spawn(job_otp_cleanup(pool.clone()));
    // tokio::spawn(job_payment_status_poll(pool.clone()));
    // tokio::spawn(job_slot_expiry(pool.clone()));

    Ok(())
}

/// Placeholder: Poll pending payment status from providers.
#[allow(dead_code)]
async fn _job_payment_status_poll() {
    // TODO: SELECT payments WHERE status = 'pending' AND created_at < NOW() - interval '5 minutes'
    // For each, call Momo/ZaloPay query API
    // Run every 2 minutes
}

/// Placeholder: Release expired locked slots.
#[allow(dead_code)]
async fn _job_slot_expiry() {
    // TODO: UPDATE court_slots SET status = 'available'
    //       WHERE status = 'locked' AND ... (locked too long without payment)
    // Run every 1 minute
}
