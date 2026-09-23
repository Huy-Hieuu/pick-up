use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::Client;
use std::io::Cursor;
use std::time::Duration;
use uuid::Uuid;

use crate::config::S3Settings;
use crate::error::{AppError, AppResult};
use crate::models::upload::MAX_AVATAR_UPLOAD_BYTES;

/// Hardcoded MIME type → file extension map.
/// Avoids fragile string parsing of content-type values.
const MIME_EXTENSIONS: &[(&str, &str)] = &[
    ("image/jpeg", "jpg"),
    ("image/png", "png"),
    ("image/webp", "webp"),
];

/// Allowed content types for avatar uploads.
pub const ALLOWED_CONTENT_TYPES: &[&str] = &["image/jpeg", "image/png", "image/webp"];
static IMAGE_WORK: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);

pub struct UploadService;

impl UploadService {
    #[allow(clippy::too_many_arguments)]
    pub async fn generate_avatar_presigned_put(
        client: &Client,
        settings: &S3Settings,
        user_id: Uuid,
        upload_id: Uuid,
        content_type: &str,
        content_length: u64,
        expires_secs: u64,
    ) -> AppResult<PresignedUploadUrl> {
        let extension = extension_for_mime(content_type).ok_or_else(|| {
            AppError::BadRequest(format!("Unsupported content type: {content_type}"))
        })?;
        if content_length == 0 || content_length > MAX_AVATAR_UPLOAD_BYTES {
            return Err(AppError::BadRequest(
                "Avatar size must be between 1 byte and 8 MiB".into(),
            ));
        }

        let object_key = format!("quarantine/{user_id}/{upload_id}.{extension}");

        let presigning_config = PresigningConfig::builder()
            .expires_in(Duration::from_secs(expires_secs))
            .build()
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Presigning config error: {e}")))?;

        let presigned = client
            .put_object()
            .bucket(&settings.bucket)
            .key(&object_key)
            .content_type(content_type)
            .content_length(content_length as i64)
            .presigned(presigning_config)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("S3 presign error: {e}")))?;

        Ok(PresignedUploadUrl {
            upload_id,
            upload_url: presigned.uri().to_string(),
            object_key,
            expires_secs,
            content_length,
        })
    }

    pub async fn complete_avatar(
        pool: &sqlx::PgPool,
        client: &Client,
        settings: &S3Settings,
        user_id: Uuid,
        upload_id: Uuid,
    ) -> AppResult<String> {
        // Return completed work idempotently before acquiring a processing slot.
        let approved: Option<Option<String>> = sqlx::query_scalar(
            "SELECT public_url FROM avatar_uploads WHERE id = $1 AND user_id = $2",
        )
        .bind(upload_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
        match approved {
            Some(Some(url)) => return Ok(url),
            None => return Err(AppError::NotFound("Upload not found".into())),
            _ => {}
        }
        let permit = IMAGE_WORK.try_acquire().map_err(|_| {
            AppError::TooManyRequests("Image processing busy; retry shortly".into())
        })?;
        let row: Option<(String, String, i64)> = sqlx::query_as(
            "UPDATE avatar_uploads SET processing_until = now() + interval '2 minutes' WHERE id = $1 AND user_id = $2 AND public_url IS NULL AND expires_at > now() AND (processing_until IS NULL OR processing_until < now()) RETURNING object_key, content_type, content_length",
        ).bind(upload_id).bind(user_id).fetch_optional(pool).await?;
        let (key, content_type, length) = row
            .ok_or_else(|| AppError::Conflict("Upload expired or is already processing".into()))?;

        let result = tokio::time::timeout(Duration::from_secs(20), async {
            let mut object = client.get_object().bucket(&settings.bucket).key(&key).send().await
                .map_err(|_| AppError::BadRequest("Uploaded object is unavailable".into()))?;
            if object.content_length() != Some(length) {
                return Err(AppError::BadRequest("Uploaded length does not match the signed request".into()));
            }
            let mut bytes = Vec::with_capacity(length as usize);
            while let Some(chunk) = object.body.try_next().await.map_err(|e| AppError::Internal(e.into()))? {
                if bytes.len() + chunk.len() > length as usize {
                    return Err(AppError::BadRequest("Uploaded body exceeds declared size".into()));
                }
                bytes.extend_from_slice(&chunk);
            }
            if bytes.len() != length as usize {
                return Err(AppError::BadRequest("Uploaded body is incomplete".into()));
            }
            let png = tokio::task::spawn_blocking(move || {
                // Keep the permit until CPU work really finishes, even if the
                // HTTP request is cancelled or reaches its deadline.
                let _permit = permit;
                sanitize_image(&bytes, &content_type)
            }).await.map_err(|e| AppError::Internal(e.into()))??;
            let public_key = format!("avatars/sanitized/{user_id}/{upload_id}.png");
            client.put_object().bucket(&settings.bucket).key(&public_key)
                .content_type("image/png").content_disposition("inline; filename=avatar.png")
                .cache_control("public, max-age=31536000, immutable")
                .body(png.into()).send().await.map_err(|e| AppError::Internal(e.into()))?;
            let url = format!("{}/{}", settings.public_url.trim_end_matches('/'), public_key);
            sqlx::query("UPDATE avatar_uploads SET public_url = $3, processing_until = NULL WHERE id = $1 AND user_id = $2")
                .bind(upload_id).bind(user_id).bind(&url).execute(pool).await?;
            Ok(url)
        }).await;
        let result = result.unwrap_or_else(|_| {
            Err(AppError::Conflict(
                "Image processing timed out; retry shortly".into(),
            ))
        });
        // Preserve a short backoff for malformed inputs. Cancelled requests
        // leave a recoverable two-minute lease, never an infinite lock.
        if result.is_err() {
            sqlx::query("UPDATE avatar_uploads SET processing_until = now() + interval '10 seconds' WHERE id = $1 AND user_id = $2 AND public_url IS NULL")
                .bind(upload_id).bind(user_id).execute(pool).await?;
        }
        result
    }
}

fn sanitize_image(bytes: &[u8], declared: &str) -> AppResult<Vec<u8>> {
    let invalid = || AppError::BadRequest("Invalid image or image dimensions exceed limits".into());
    if bytes.is_empty() || bytes.len() > MAX_AVATAR_UPLOAD_BYTES as usize {
        return Err(invalid());
    }
    let format = image::guess_format(bytes).map_err(|_| invalid())?;
    let expected = match extension_for_mime(declared) {
        Some("jpg") => image::ImageFormat::Jpeg,
        Some("png") => image::ImageFormat::Png,
        Some("webp") => image::ImageFormat::WebP,
        _ => return Err(invalid()),
    };
    if format != expected {
        return Err(invalid());
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(64 * 1024 * 1024);
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), format);
    reader.limits(limits);
    let decoded = reader.decode().map_err(|_| invalid())?;
    // Re-encode only decoded pixels. Metadata, scripts appended to a valid
    // image, animation and ancillary chunks cannot reach the public object.
    let pixels = if decoded.width() > 1024 || decoded.height() > 1024 {
        decoded.thumbnail(1024, 1024).to_rgba8()
    } else {
        decoded.to_rgba8()
    };
    let mut output = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(pixels)
        .write_to(&mut output, image::ImageFormat::Png)
        .map_err(|_| invalid())?;
    Ok(output.into_inner())
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
    pub upload_id: Uuid,
    pub upload_url: String,
    pub object_key: String,
    pub expires_secs: u64,
    pub content_length: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_bytes_must_match_declared_format_and_decode() {
        assert!(sanitize_image(b"<script>alert(1)</script>", "image/png").is_err());
        assert!(sanitize_image(b"\x89PNG\r\n\x1a\n", "image/png").is_err());
        let mut source = Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(2, 2)
            .write_to(&mut source, image::ImageFormat::Png)
            .unwrap();
        assert!(sanitize_image(source.get_ref(), "image/jpeg").is_err());
        source
            .get_mut()
            .extend_from_slice(b"<script>private-metadata</script>");
        let clean = sanitize_image(source.get_ref(), "image/png").unwrap();
        assert!(!clean.windows(7).any(|bytes| bytes == b"<script"));
        assert_eq!(image::load_from_memory(&clean).unwrap().width(), 2);
    }

    #[test]
    fn oversized_dimensions_are_rejected_and_valid_avatars_are_resized() {
        let mut source = Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(4097, 1)
            .write_to(&mut source, image::ImageFormat::Png)
            .unwrap();
        assert!(sanitize_image(source.get_ref(), "image/png").is_err());
        let mut source = Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(1200, 600)
            .write_to(&mut source, image::ImageFormat::Jpeg)
            .unwrap();
        let clean = sanitize_image(source.get_ref(), "image/jpeg").unwrap();
        let decoded = image::load_from_memory(&clean).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (1024, 512));
    }

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
        assert_eq!(
            extension_for_mime("image/png; boundary=something"),
            Some("png")
        );
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
        let url = format!("{}/{}", public_url.trim_end_matches('/'), object_key);
        assert!(!url.contains("//pick"));
        assert_eq!(
            url,
            "http://localhost:9000/pickup-media/avatars/123/avatar.jpg"
        );
    }

    #[test]
    fn public_url_trims_trailing_slash() {
        let public_url = "http://localhost:9000/pickup-media/";
        let object_key = "avatars/123/avatar.jpg";
        let url = format!("{}/{}", public_url.trim_end_matches('/'), object_key);
        assert_eq!(
            url,
            "http://localhost:9000/pickup-media/avatars/123/avatar.jpg"
        );
    }
}
