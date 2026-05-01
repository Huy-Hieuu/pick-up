use axum::http::{HeaderValue, Method};
use tower_http::cors::{Any, CorsLayer};

use crate::config::Settings;

/// Build a CORS layer from application settings.
///
/// Set `CORS_ORIGINS=*` for development (allows all).
/// For production, set `CORS_ORIGINS=https://app.pickup.vn,https://owner.pickup.vn`.
pub fn cors_layer(settings: &Settings) -> CorsLayer {
    let origins = &settings.app.cors_origins;

    if origins.len() == 1 && origins[0] == "*" {
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any)
    } else {
        let allowed: Vec<HeaderValue> = origins
            .iter()
            .filter_map(|o| o.parse::<HeaderValue>().ok())
            .collect();

        CorsLayer::new()
            .allow_origin(allowed)
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PATCH,
                Method::PUT,
                Method::DELETE,
                Method::OPTIONS,
            ])
            .allow_headers(Any)
    }
}
