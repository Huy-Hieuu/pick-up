//! Background jobs module.

pub mod email;

use crate::state::AppState;
use tokio_util::sync::CancellationToken;

/// Start all background job workers.
///
/// Called from `main.rs` during server startup.
/// Workers check the CancellationToken for graceful shutdown.
pub async fn start_workers(state: &AppState, cancel: CancellationToken) -> anyhow::Result<()> {
    crate::jobs::email::build_smtp_transport(&state.settings.email)?;
    state
        .lobby
        .start_listener(&state.pool, cancel.clone())
        .await?;
    // Spawn the email delivery worker.
    // A blocking pop must not share the multiplexed request connection.
    let redis = crate::db::create_redis(&state.settings.redis).await?;
    let email_settings = state.settings.email.clone();
    tokio::spawn(crate::jobs::email::run_email_worker(
        redis,
        email_settings,
        cancel.clone(),
    ));
    tokio::spawn(run_payment_expiry_worker(state.pool.clone(), cancel));

    tracing::info!("Background workers started");

    Ok(())
}

async fn run_payment_expiry_worker(pool: sqlx::PgPool, cancel: CancellationToken) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = cancel.cancelled() => break,
            _ = interval.tick() => match crate::services::PaymentService::expire_stale_pending(&pool).await {
                Ok(expired) if expired > 0 => tracing::info!(expired, "Expired provider payment orders"),
                Ok(_) => {},
                Err(error) => tracing::error!(%error, "Payment expiry pass failed"),
            }
        }
    }
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
