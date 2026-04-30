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

#[cfg(test)]
mod tests {
    use super::*;

    // ── Content type validation ────────────────────────────────────

    #[test]
    fn validate_jpeg() {
        let req = GetPresignedUrlRequest { content_type: "image/jpeg".into() };
        assert!(req.validate_content_type().is_ok());
    }

    #[test]
    fn validate_png() {
        let req = GetPresignedUrlRequest { content_type: "image/png".into() };
        assert!(req.validate_content_type().is_ok());
    }

    #[test]
    fn validate_webp() {
        let req = GetPresignedUrlRequest { content_type: "image/webp".into() };
        assert!(req.validate_content_type().is_ok());
    }

    #[test]
    fn reject_svg() {
        let req = GetPresignedUrlRequest { content_type: "image/svg+xml".into() };
        assert!(req.validate_content_type().is_err());
    }

    #[test]
    fn reject_gif() {
        let req = GetPresignedUrlRequest { content_type: "image/gif".into() };
        assert!(req.validate_content_type().is_err());
    }

    #[test]
    fn reject_empty() {
        let req = GetPresignedUrlRequest { content_type: "".into() };
        assert!(req.validate_content_type().is_err());
    }

    #[test]
    fn accept_jpeg_with_charset() {
        // Strip parameters before checking
        let req = GetPresignedUrlRequest { content_type: "image/jpeg; charset=utf-8".into() };
        assert!(req.validate_content_type().is_ok());
    }

    #[test]
    fn reject_with_wrong_param_base() {
        let req = GetPresignedUrlRequest { content_type: "image/svg+xml; charset=utf-8".into() };
        assert!(req.validate_content_type().is_err());
    }

    // ── Response construction ──────────────────────────────────────

    #[test]
    fn from_presigned_maps_all_fields() {
        let presigned = PresignedUploadUrl {
            upload_url: "https://s3.example.com/upload".into(),
            object_key: "avatars/123/avatar.jpg".into(),
            public_url: "https://cdn.example.com/avatars/123/avatar.jpg".into(),
            expires_secs: 900,
        };
        let resp = GetPresignedUrlResponse::from_presigned(presigned);
        assert_eq!(resp.upload_url, "https://s3.example.com/upload");
        assert_eq!(resp.object_key, "avatars/123/avatar.jpg");
        assert_eq!(resp.expires_secs, 900);
    }
}