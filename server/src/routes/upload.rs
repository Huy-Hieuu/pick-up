use axum::{extract::State, routing::post, Json, Router};

use crate::error::{AppError, AppResult};
use crate::extractors::auth::AuthUser;
use crate::extractors::ValidatedJson;
use crate::models::upload::{
    CompleteAvatarRequest, CompleteAvatarResponse, GetPresignedUrlRequest, GetPresignedUrlResponse,
    PRESIGNED_URL_EXPIRY_SECS,
};
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
    let count: i32 = redis::Script::new("local n = redis.call('INCR', KEYS[1]); if n == 1 then redis.call('EXPIRE', KEYS[1], 60) end; return n")
        .key(&rate_key)
        .invoke_async(&mut conn)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Redis error: {e}")))?;
    if count > 5 {
        return Err(AppError::TooManyRequests(
            "Too many upload requests. Please wait a moment.".into(),
        ));
    }

    req.validate_upload()?;
    let upload_id = uuid::Uuid::new_v4();

    let presigned = UploadService::generate_avatar_presigned_put(
        &state.s3_upload_client,
        &state.settings.s3,
        auth_user.user_id(),
        upload_id,
        &req.content_type,
        req.content_length,
        PRESIGNED_URL_EXPIRY_SECS,
    )
    .await?;

    sqlx::query("INSERT INTO avatar_uploads (id, user_id, object_key, content_type, content_length, expires_at) VALUES ($1, $2, $3, $4, $5, now() + interval '15 minutes')")
        .bind(upload_id).bind(auth_user.user_id()).bind(&presigned.object_key)
        .bind(&req.content_type).bind(req.content_length as i64).execute(&state.pool).await?;

    Ok(Json(GetPresignedUrlResponse::from_presigned(presigned)))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/avatar", post(get_avatar_upload_url))
        .route("/avatar/complete", post(complete_avatar))
}

async fn complete_avatar(
    State(state): State<AppState>,
    auth_user: AuthUser,
    ValidatedJson(req): ValidatedJson<CompleteAvatarRequest>,
) -> AppResult<Json<CompleteAvatarResponse>> {
    let public_url = UploadService::complete_avatar(
        &state.pool,
        &state.s3_client,
        &state.settings.s3,
        auth_user.user_id(),
        req.upload_id,
    )
    .await?;
    Ok(Json(CompleteAvatarResponse { public_url }))
}
