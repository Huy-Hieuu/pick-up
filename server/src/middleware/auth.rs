use axum::{
    extract::Request,
    middleware::Next,
    response::Response,
};

/// Auth middleware — currently a placeholder.
///
/// Actual implementation will:
/// 1. Extract the Bearer token from `Authorization` header
/// 2. Decode and validate the JWT
/// 3. Inject `AuthUser` into request extensions
///
/// For now, handlers use the `AuthUser` extractor directly,
/// so this middleware is optional. It becomes useful when you need
/// to enforce auth on an entire router group without repeating the
/// extractor in every handler.
pub async fn require_auth(request: Request, next: Next) -> Response {
    // TODO: Implement JWT verification here when needed for router-level guards.
    // For now, individual handlers use the `AuthUser` extractor.
    next.run(request).await
}
