pub mod health;
pub mod auth;
pub mod courts;
pub mod games;
pub mod payments;
pub mod webhooks;
pub mod upload;

use axum::{routing::get, Router, middleware};

use crate::state::AppState;

/// Build the complete API router tree.
///
/// Structure:
/// ```text
/// /health
/// /auth/*         (public)
/// /courts/*       (public GET, auth POST)
/// /games/*        (auth required)
/// /games/:id/payments/* (auth required)
/// /webhooks/*     (public, signature verified)
/// /ws/games/:id   (WebSocket, JWT in query)
/// ```
pub fn build_router(state: AppState) -> Router {
    let public_routes = Router::new()
        .route("/health", get(health::health_check))
        .nest("/auth", auth::public_router())
        .nest("/courts", courts::router())
        .nest("/webhooks", webhooks::router());

    let protected_routes = Router::new()
        .nest("/games", games::router())
        .nest("/upload", upload::router())
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::middleware::auth::require_auth,
        ));

    // Payments nested under /games/{game_id}/payments
    let payment_routes = Router::new()
        .nest("/games/{game_id}/payments", payments::router())
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::middleware::auth::require_auth,
        ));

    // Protected auth routes (/auth/me) behind require_auth middleware
    let protected_auth_routes = Router::new()
        .nest("/auth", auth::protected_router())
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::middleware::auth::require_auth,
        ));

    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .merge(payment_routes)
        .merge(protected_auth_routes)
        .route("/ws/games/{game_id}", get(crate::ws::lobby::ws_game_lobby))
        .with_state(state.clone())
        .layer(crate::middleware::cors::cors_layer(&state.settings))
        .layer(tower_http::trace::TraceLayer::new_for_http())
}
