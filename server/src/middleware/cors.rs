use axum::Router;
use tower_http::cors::{Any, CorsLayer};

/// Production-ready CORS layer.
///
/// In development, allows all origins.
/// For production, replace `Any` with specific allowed origins.
pub fn cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any)
}

/// Extension trait to conveniently add CORS to a router.
pub trait CorsExt {
    fn with_cors(self) -> Self;
}

impl<S: Clone + Send + Sync + 'static> CorsExt for Router<S> {
    fn with_cors(self) -> Self {
        self.layer(cors_layer())
    }
}
