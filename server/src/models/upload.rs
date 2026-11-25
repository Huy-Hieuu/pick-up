use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::error::AppResult;
use crate::services::upload::{PresignedUploadUrl, ALLOWED_CONTENT_TYPES};

pub const PRESIGNED_URL_EXPIRY_SECS: u64 = 900; // 15 minutes
pub const MAX_AVATAR_UPLOAD_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Debug, Deserialize, Validate)]
pub struct GetPresignedUrlRequest {
    pub content_type: String,
    #[validate(range(
        min = 1,
        max = 8388608,
        message = "Avatar size must be between 1 byte and 8 MiB"
    ))]
    pub content_length: u64,
}

#[derive(Debug, Serialize)]
pub struct GetPresignedUrlResponse {
    pub upload_id: uuid::Uuid,
    pub upload_url: String,
    pub object_key: String,
    pub expires_secs: u64,
    pub content_length: u64,
    pub max_upload_bytes: u64,
}

impl GetPresignedUrlRequest {
    pub fn validate_upload(&self) -> AppResult<()> {
        // Strip parameters (e.g., "; charset=utf-8") before checking.
        let base_type = self
            .content_type
            .split(';')
            .next()
            .unwrap_or(&self.content_type)
            .trim();
        if !ALLOWED_CONTENT_TYPES.contains(&base_type) {
            return Err(crate::error::AppError::BadRequest(format!(
                "Invalid content type. Allowed: {:?}",
                ALLOWED_CONTENT_TYPES
            )));
        }
        if self.content_length == 0 || self.content_length > MAX_AVATAR_UPLOAD_BYTES {
            return Err(crate::error::AppError::BadRequest(
                "Avatar size must be between 1 byte and 8 MiB".into(),
            ));
        }
        Ok(())
    }
}

impl GetPresignedUrlResponse {
    pub fn from_presigned(presigned: PresignedUploadUrl) -> Self {
        Self {
            upload_id: presigned.upload_id,
            upload_url: presigned.upload_url,
            object_key: presigned.object_key,
            expires_secs: presigned.expires_secs,
            content_length: presigned.content_length,
            max_upload_bytes: MAX_AVATAR_UPLOAD_BYTES,
        }
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct CompleteAvatarRequest {
    pub upload_id: uuid::Uuid,
}

#[derive(Debug, Serialize)]
pub struct CompleteAvatarResponse {
    pub public_url: String,
}
