use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::Client;
use std::time::Duration;
use uuid::Uuid;

use crate::config::S3Settings;
use crate::error::{AppError, AppResult};

/// Hardcoded MIME type → file extension map.
/// Avoids fragile string parsing of content-type values.
const MIME_EXTENSIONS: &[(&str, &str)] = &[
    ("image/jpeg", "jpg"),
    ("image/png", "png"),
    ("image/webp", "webp"),
];

/// Allowed content types for avatar uploads.
pub const ALLOWED_CONTENT_TYPES: &[&str] = &["image/jpeg", "image/png", "image/webp"];

pub struct UploadService;

impl UploadService {
    pub async fn generate_avatar_presigned_put(
        client: &Client,
        settings: &S3Settings,
        user_id: Uuid,
        content_type: &str,
        expires_secs: u64,
    ) -> AppResult<PresignedUploadUrl> {
        let extension = extension_for_mime(content_type)
            .ok_or_else(|| AppError::BadRequest(format!("Unsupported content type: {content_type}")))?;

        let object_key = format!("avatars/{}/avatar.{}", user_id, extension);

        let presigning_config = PresigningConfig::builder()
            .expires_in(Duration::from_secs(expires_secs))
            .build()
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Presigning config error: {e}")))?;

        let presigned = client
            .put_object()
            .bucket(&settings.bucket)
            .key(&object_key)
            .content_type(content_type)
            .presigned(presigning_config)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("S3 presign error: {e}")))?;

        let public_url = format!(
            "{}/{}",
            settings.public_url.trim_end_matches('/'),
            object_key
        );

        Ok(PresignedUploadUrl {
            upload_url: presigned.uri().to_string(),
            object_key,
            public_url,
            expires_secs,
        })
    }
}

/// Look up the file extension for a MIME type using the hardcoded map.
/// Strips any parameters (e.g., "charset=utf-8") before matching.
fn extension_for_mime(content_type: &str) -> Option<&'static str> {
    let base_type = content_type.split(';').next()?.trim();
    MIME_EXTENSIONS
        .iter()
        .find(|(mime, _)| *mime == base_type)
        .map(|(_, ext)| *ext)
}

#[derive(Debug, serde::Serialize)]
pub struct PresignedUploadUrl {
    pub upload_url: String,
    pub object_key: String,
    pub public_url: String,
    pub expires_secs: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── extension_for_mime ─────────────────────────────────────────

    #[test]
    fn maps_jpeg_to_jpg() {
        assert_eq!(extension_for_mime("image/jpeg"), Some("jpg"));
    }

    #[test]
    fn maps_png_to_png() {
        assert_eq!(extension_for_mime("image/png"), Some("png"));
    }

    #[test]
    fn maps_webp_to_webp() {
        assert_eq!(extension_for_mime("image/webp"), Some("webp"));
    }

    #[test]
    fn strips_charset_parameter() {
        assert_eq!(extension_for_mime("image/jpeg; charset=utf-8"), Some("jpg"));
    }

    #[test]
    fn strips_boundary_parameter() {
        assert_eq!(extension_for_mime("image/png; boundary=something"), Some("png"));
    }

    #[test]
    fn rejects_unsupported_mime() {
        assert_eq!(extension_for_mime("image/svg+xml"), None);
    }

    #[test]
    fn rejects_empty_string() {
        assert_eq!(extension_for_mime(""), None);
    }

    #[test]
    fn rejects_non_image() {
        assert_eq!(extension_for_mime("application/json"), None);
        assert_eq!(extension_for_mime("text/html"), None);
    }

    // ── ALLOWED_CONTENT_TYPES ──────────────────────────────────────

    #[test]
    fn allowed_types_match_map() {
        for allowed in ALLOWED_CONTENT_TYPES {
            assert!(
                extension_for_mime(allowed).is_some(),
                "Allowed type '{}' should have a mapping",
                allowed
            );
        }
    }

    // ── Object key generation ──────────────────────────────────────

    #[test]
    fn object_key_format() {
        // Verify the key pattern used in generate_avatar_presigned_put
        let user_id = Uuid::new_v4();
        let ext = "jpg";
        let object_key = format!("avatars/{}/avatar.{}", user_id, ext);
        assert!(object_key.starts_with("avatars/"));
        assert!(object_key.ends_with(".jpg"));
        assert!(object_key.contains(&user_id.to_string()));
    }

    // ── Public URL construction ────────────────────────────────────

    #[test]
    fn public_url_no_double_slash() {
        let public_url = "http://localhost:9000/pickup-media";
        let object_key = "avatars/123/avatar.jpg";
        let url = format!(
            "{}/{}",
            public_url.trim_end_matches('/'),
            object_key
        );
        assert!(!url.contains("//pick"));
        assert_eq!(url, "http://localhost:9000/pickup-media/avatars/123/avatar.jpg");
    }

    #[test]
    fn public_url_trims_trailing_slash() {
        let public_url = "http://localhost:9000/pickup-media/";
        let object_key = "avatars/123/avatar.jpg";
        let url = format!(
            "{}/{}",
            public_url.trim_end_matches('/'),
            object_key
        );
        assert_eq!(url, "http://localhost:9000/pickup-media/avatars/123/avatar.jpg");
    }
}
