use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::error::AppResult;
use crate::services::upload::{PresignedUploadUrl, ALLOWED_CONTENT_TYPES};

pub const PRESIGNED_URL_EXPIRY_SECS: u64 = 900; // 15 minutes

#[derive(Debug, Deserialize, Validate)]
pub struct GetPresignedUrlRequest {
    pub content_type: String,
}

#[derive(Debug, Serialize)]
pub struct GetPresignedUrlResponse {
    pub upload_url: String,
    pub object_key: String,
    pub public_url: String,
    pub expires_secs: u64,
}

impl GetPresignedUrlRequest {
    pub fn validate_content_type(&self) -> AppResult<()> {
        // Strip parameters (e.g., "; charset=utf-8") before checking.
        let base_type = self.content_type.split(';').next().unwrap_or(&self.content_type).trim();
        if !ALLOWED_CONTENT_TYPES.contains(&base_type) {
            return Err(crate::error::AppError::BadRequest(format!(
                "Invalid content type. Allowed: {:?}",
                ALLOWED_CONTENT_TYPES
            )));
        }
        Ok(())
    }
}

impl GetPresignedUrlResponse {
    pub fn from_presigned(presigned: PresignedUploadUrl) -> Self {
        Self {
            upload_url: presigned.upload_url,
            object_key: presigned.object_key,
            public_url: presigned.public_url,
            expires_secs: presigned.expires_secs,
        }
    }
}
