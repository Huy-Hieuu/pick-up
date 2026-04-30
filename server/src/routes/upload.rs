use axum::{
    extract::State,
    routing::post,
    Json, Router,
};

use crate::error::AppResult;
use crate::extractors::auth::AuthUser;
use crate::extractors::ValidatedJson;
use crate::models::upload::{GetPresignedUrlRequest, GetPresignedUrlResponse, PRESIGNED_URL_EXPIRY_SECS};
use crate::services::upload::UploadService;
use crate::state::AppState;

/// POST /upload/avatar
/// Returns a presigned URL for direct browser upload to MinIO.
pub async fn get_avatar_upload_url(
    State(state): State<AppState>,
    auth_user: AuthUser,
    ValidatedJson(req): ValidatedJson<GetPresignedUrlRequest>,
) -> AppResult<Json<GetPresignedUrlResponse>> {
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
