use aws_config::BehaviorVersion;
use aws_credential_types::Credentials;
use pickup_server::config::Settings;
use pickup_server::db;
use pickup_server::routes;
use pickup_server::state::AppState;
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file if present (local dev).
    dotenvy::dotenv().ok();

    // Initialize tracing.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,pickup_server=debug".into()),
        )
        .json()
        .init();

    // Load configuration from environment.
    let settings = Settings::from_env()?;
    tracing::info!(
        host = %settings.app.host,
        port = %settings.app.port,
        "Starting PickUp server"
    );

    // Create database connection pool & run migrations.
    let pool = db::create_pool(&settings.database).await?;

    // Create Redis connection manager.
    let redis = db::create_redis(&settings.redis).await?;

    // Create S3/MinIO client for presigned URLs.
    let s3_client = {
        let creds = Credentials::new(
            &settings.s3.access_key,
            &settings.s3.secret_key,
            None,
            None,
            "static",
        );
        let region = aws_types::region::Region::new("auto");
        let cfg = aws_config::defaults(BehaviorVersion::latest())
            .endpoint_url(&settings.s3.endpoint)
            .credentials_provider(creds)
            .region(region)
            .no_credentials()
            .behavior_version(aws_config::BehaviorVersion::latest());
        let s3_config = cfg.load().await;

        let mut s3_builder = aws_sdk_s3::config::Builder::from(&s3_config);
        s3_builder.set_force_path_style(Some(true));
        s3_builder.set_region(Some(aws_types::region::Region::new("auto")));
        let s3_config = s3_builder.build();

        aws_sdk_s3::Client::from_conf(s3_config)
    };

    // Build shared application state.
    let state = AppState::new(pool, redis, settings.clone(), s3_client);

    // Cancellation token for coordinated shutdown.
    let cancel_token = CancellationToken::new();
    let cancel_clone = cancel_token.clone();

    // Start background jobs (email queue worker, etc.).
    pickup_server::jobs::start_workers(&state, cancel_clone).await?;

    // Build the Axum router with all routes.
    let app = routes::build_router(state);

    // Bind and serve with graceful shutdown.
    let listener = tokio::net::TcpListener::bind(format!("{}:{}", settings.app.host, settings.app.port)).await?;
    tracing::info!("Server listening on {}", listener.local_addr()?);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(cancel_token))
        .await?;

    tracing::info!("Server shutdown complete");
    Ok(())
}

async fn shutdown_signal(cancel_token: tokio_util::sync::CancellationToken) {
    // Wait for SIGINT or SIGTERM.
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to listen for ctrl+c");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to listen for SIGTERM")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            tracing::info!("Received Ctrl+C, starting graceful shutdown...");
        },
        _ = terminate => {
            tracing::info!("Received SIGTERM, starting graceful shutdown...");
        },
    }

    // Cancel background workers and WebSocket connections.
    cancel_token.cancel();
}
