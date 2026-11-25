use axum::Json;
use serde::Serialize;

use crate::error::AppResult;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
}

/// `GET /health` — simple health check for load balancers and monitoring.
pub async fn health_check() -> AppResult<Json<HealthResponse>> {
    Ok(Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    }))
}

/// Dependency readiness is separate from process liveness.
pub async fn readiness_check(
    axum::extract::State(state): axum::extract::State<crate::state::AppState>,
) -> axum::http::StatusCode {
    let ready = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        let db = sqlx::query("SELECT 1").execute(&state.pool);
        let mut conn = state.redis.clone();
        let ping = redis::cmd("PING");
        let (db, redis) = tokio::join!(db, ping.query_async::<String>(&mut conn));
        db.is_ok() && redis.is_ok()
    })
    .await
    .unwrap_or(false);
    if ready && !state.shutdown.is_cancelled() {
        axum::http::StatusCode::OK
    } else {
        axum::http::StatusCode::SERVICE_UNAVAILABLE
    }
}
