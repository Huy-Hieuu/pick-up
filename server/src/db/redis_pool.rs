use redis::aio::ConnectionManager;

use crate::config::RedisSettings;

/// Create a Redis connection manager (automatic reconnection built-in).
pub async fn create_redis(settings: &RedisSettings) -> anyhow::Result<ConnectionManager> {
    let client = redis::Client::open(settings.url.as_str())?;
    let conn = ConnectionManager::new(client).await?;
    tracing::info!("Redis connection manager created");
    Ok(conn)
}
