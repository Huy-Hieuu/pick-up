use sqlx::postgres::{PgPool, PgPoolOptions};

use crate::config::DatabaseSettings;

/// Create a PostgreSQL connection pool with sensible defaults.
///
/// Runs pending migrations automatically when `run_migrations` is true.
pub async fn create_pool(settings: &DatabaseSettings) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(settings.max_connections)
        .connect(&settings.url)
        .await?;

    if settings.run_migrations {
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .map_err(|e| anyhow::anyhow!("Migration failed: {e}"))?;
        tracing::info!(
            max_connections = settings.max_connections,
            "Database pool created and migrations applied"
        );
    } else {
        tracing::info!(
            max_connections = settings.max_connections,
            "Database pool created (migrations skipped)"
        );
    }

    Ok(pool)
}
