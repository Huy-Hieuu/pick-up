use axum::{
    body::Bytes,
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Router,
};
use pickup_server::{
    models::user::{PatchValue, UpdateProfileRequest},
    services::{upload::UploadService, AuthService},
};
use sqlx::PgPool;
use std::{
    io::Cursor,
    sync::{Arc, Mutex},
};
use uuid::Uuid;

#[derive(Clone, Default)]
struct Storage {
    input: Arc<Mutex<Vec<u8>>>,
    published: Arc<Mutex<Vec<PublishedObject>>>,
}

type PublishedObject = (String, Vec<u8>);

async fn read(State(storage): State<Storage>) -> Vec<u8> {
    storage.input.lock().unwrap().clone()
}
async fn write(
    State(storage): State<Storage>,
    Path(key): Path<String>,
    bytes: Bytes,
) -> StatusCode {
    storage
        .published
        .lock()
        .unwrap()
        .push((key, bytes.to_vec()));
    StatusCode::OK
}

#[sqlx::test]
#[ignore = "requires disposable PostgreSQL and Redis"]
async fn quarantined_upload_is_sanitized_owned_and_idempotent(pool: PgPool) {
    let storage = Storage::default();
    let mut png = Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(16, 16)
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    png.get_mut()
        .extend_from_slice(b"<script>remove-this</script>");
    *storage.input.lock().unwrap() = png.into_inner();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new()
        .route("/{*key}", get(read).put(write))
        .with_state(storage.clone());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let mut state = super::state(pool.clone()).await;
    state.s3_client = aws_sdk_s3::Client::from_conf(
        aws_sdk_s3::config::Builder::new()
            .behavior_version_latest()
            .region(aws_types::region::Region::new("us-east-1"))
            .credentials_provider(aws_credential_types::Credentials::new(
                "test", "test", None, None, "test",
            ))
            .endpoint_url(format!("http://{address}"))
            .force_path_style(true)
            .build(),
    );
    let owner: Uuid = sqlx::query_scalar(
        "INSERT INTO users (phone, email_verified) VALUES ($1, TRUE) RETURNING id",
    )
    .bind(Uuid::new_v4().to_string()[..20].to_owned())
    .fetch_one(&pool)
    .await
    .unwrap();
    let upload_id = Uuid::new_v4();
    let length = storage.input.lock().unwrap().len() as i64;
    sqlx::query("INSERT INTO avatar_uploads (id, user_id, object_key, content_type, content_length, expires_at) VALUES ($1, $2, $3, 'image/png', $4, now() + interval '15 minutes')")
        .bind(upload_id).bind(owner).bind(format!("quarantine/{owner}/{upload_id}.png")).bind(length)
        .execute(&pool).await.unwrap();
    assert!(UploadService::complete_avatar(
        &pool,
        &state.s3_client,
        &state.settings.s3,
        Uuid::new_v4(),
        upload_id
    )
    .await
    .is_err());
    assert!(storage.published.lock().unwrap().is_empty());
    let public = UploadService::complete_avatar(
        &pool,
        &state.s3_client,
        &state.settings.s3,
        owner,
        upload_id,
    )
    .await
    .unwrap();
    assert!(public.contains("/avatars/sanitized/"));
    let again = UploadService::complete_avatar(
        &pool,
        &state.s3_client,
        &state.settings.s3,
        owner,
        upload_id,
    )
    .await
    .unwrap();
    assert_eq!(again, public);
    {
        let published = storage.published.lock().unwrap();
        assert_eq!(published.len(), 1);
        assert!(!published[0].1.windows(7).any(|chunk| chunk == b"<script"));
        assert_eq!(
            image::load_from_memory(&published[0].1).unwrap().width(),
            16
        );
    }
    let profile = AuthService::update_profile(
        &pool,
        owner,
        UpdateProfileRequest {
            display_name: PatchValue::Absent,
            avatar_url: PatchValue::Value(public.clone()),
        },
    )
    .await
    .unwrap();
    assert_eq!(profile.avatar_url.as_deref(), Some(public.as_str()));
    assert!(AuthService::update_profile(
        &pool,
        Uuid::new_v4(),
        UpdateProfileRequest {
            display_name: PatchValue::Absent,
            avatar_url: PatchValue::Value(public),
        }
    )
    .await
    .is_err());
    assert!(AuthService::update_profile(
        &pool,
        owner,
        UpdateProfileRequest {
            display_name: PatchValue::Absent,
            avatar_url: PatchValue::Value("https://example.com/raw.svg".into()),
        }
    )
    .await
    .is_err());

    // A valid storage upload containing non-image bytes is never published.
    *storage.input.lock().unwrap() = vec![b'x'; length as usize];
    let invalid_id = Uuid::new_v4();
    sqlx::query("INSERT INTO avatar_uploads (id, user_id, object_key, content_type, content_length, expires_at) VALUES ($1, $2, $3, 'image/png', $4, now() + interval '15 minutes')")
        .bind(invalid_id).bind(owner).bind(format!("quarantine/{owner}/{invalid_id}.png")).bind(length)
        .execute(&pool).await.unwrap();
    assert!(UploadService::complete_avatar(
        &pool,
        &state.s3_client,
        &state.settings.s3,
        owner,
        invalid_id
    )
    .await
    .is_err());
    assert_eq!(storage.published.lock().unwrap().len(), 1);
    server.abort();
}
