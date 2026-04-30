use axum::{
    extract::State,
    routing::post,
    Json, Router,
};

use crate::error::{AppError, AppResult};
use crate::extractors::auth::AuthUser;
use crate::extractors::ValidatedJson;
use crate::models::upload::{GetPresignedUrlRequest, GetPresignedUrlResponse, PRESIGNED_URL_EXPIRY_SECS};
use crate::services::upload::UploadService;
use crate::state::AppState;

/// POST /upload/avatar
/// Returns a presigned URL for direct browser upload to MinIO.
/// Rate limited to 5 requests per minute per user.
pub async fn get_avatar_upload_url(
    State(state): State<AppState>,
    auth_user: AuthUser,
    ValidatedJson(req): ValidatedJson<GetPresignedUrlRequest>,
) -> AppResult<Json<GetPresignedUrlResponse>> {
    // Per-user rate limiting: max 5 presigned URLs per minute.
    let mut conn = state.redis.clone();
    let rate_key = format!("upload:rate:{}", auth_user.user_id());
    let count: i32 = redis::cmd("INCR")
        .arg(&rate_key)
        .query_async(&mut conn)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Redis error: {e}")))?;
    if count == 1 {
        let _: () = redis::cmd("EXPIRE")
            .arg(&rate_key)
            .arg(60_i32)
            .query_async(&mut conn)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Redis error: {e}")))?;
    }
    if count > 5 {
        return Err(AppError::BadRequest("Too many upload requests. Please wait a moment.".into()));
    }

    req.validate_content_type()?;

    let presigned = UploadService::generate_avatar_presigned_put(
        &state.s3_client,
        &state.settings.s3,
        auth_user.user_id(),
        &req.content_type,
        PRESIGNED_URL_EXPIRY_SECS,
    )
    .await?;

    Ok(Json(GetPresignedUrlResponse::from_presigned(presigned)))
}

pub fn router() -> Router<AppState> {
    Router::new().route("/avatar", post(get_avatar_upload_url))
}
