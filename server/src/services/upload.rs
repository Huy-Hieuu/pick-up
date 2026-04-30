use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::Client;
use std::time::Duration;
use uuid::Uuid;

use crate::config::S3Settings;
use crate::error::{AppError, AppResult};

pub struct UploadService;

impl UploadService {
    pub async fn generate_avatar_presigned_put(
        client: &Client,
        settings: &S3Settings,
        user_id: Uuid,
        content_type: &str,
        expires_secs: u64,
    ) -> AppResult<PresignedUploadUrl> {
        let extension = content_type
            .rsplit('/')
            .next()
            .unwrap_or("jpeg");
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

#[derive(Debug, serde::Serialize)]
pub struct PresignedUploadUrl {
    pub upload_url: String,
    pub object_key: String,
    pub public_url: String,
    pub expires_secs: u64,
}