use std::sync::Arc;

use aws_sdk_s3::Client;
use redis::aio::ConnectionManager;
use sqlx::PgPool;

use crate::config::Settings;

/// Shared application state passed to every Axum handler via extension.
///
/// Cheaply cloneable — `PgPool`, `ConnectionManager`, and `Arc<Settings>` use Arc internally.
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub redis: ConnectionManager,
    pub settings: Arc<Settings>,
    pub s3_client: Client,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("pool", &"PgPool")
            .field("redis", &"ConnectionManager")
            .field("settings", &"<redacted>")
            .field("s3_client", &"S3Client")
            .finish()
    }
}

impl AppState {
    pub fn new(pool: PgPool, redis: ConnectionManager, settings: Settings, s3_client: Client) -> Self {
        Self {
            pool,
            redis,
            settings: Arc::new(settings),
            s3_client,
        }
    }
}
