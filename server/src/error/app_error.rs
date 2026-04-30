use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

/// Unified error type for the entire application.
///
/// Every handler and service returns `Result<T, AppError>`.
/// The `IntoResponse` impl converts each variant to the correct HTTP response.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Unprocessable: {0}")]
    Unprocessable(String),

    #[error("Payment required: {0}")]
    PaymentRequired(String),

    #[error("Not implemented: {0}")]
    Unimplemented(String),

    #[error("Internal server error")]
    Internal(#[from] anyhow::Error),

    #[error("Database error")]
    Database(#[from] sqlx::Error),
}

/// JSON error body returned to clients.
#[derive(Serialize)]
struct ErrorBody {
    error: ErrorDetail,
}

#[derive(Serialize)]
struct ErrorDetail {
    code: &'static str,
    message: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, "NOT_FOUND", msg.clone()),
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, "UNAUTHORIZED", msg.clone()),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, "FORBIDDEN", msg.clone()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "BAD_REQUEST", msg.clone()),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, "CONFLICT", msg.clone()),
            AppError::Unprocessable(msg) => {
                (StatusCode::UNPROCESSABLE_ENTITY, "UNPROCESSABLE", msg.clone())
            }
            AppError::PaymentRequired(msg) => {
                (StatusCode::PAYMENT_REQUIRED, "PAYMENT_REQUIRED", msg.clone())
            }
            AppError::Unimplemented(msg) => {
                (StatusCode::NOT_IMPLEMENTED, "NOT_IMPLEMENTED", msg.clone())
            }
            AppError::Internal(err) => {
                tracing::error!("Internal error: {err:#}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_ERROR",
                    "An unexpected error occurred".into(),
                )
            }
            // Map common SQLx errors to proper HTTP statuses.
            AppError::Database(err) => match err {
                sqlx::Error::RowNotFound => {
                    (StatusCode::NOT_FOUND, "NOT_FOUND", "Resource not found".into())
                }
                sqlx::Error::Database(db_err) => match db_err.code().as_deref() {
                    // 23505 = unique_violation
                    Some("23505") => {
                        (StatusCode::CONFLICT, "CONFLICT", "Resource already exists".into())
                    }
                    // 23503 = foreign_key_violation
                    Some("23503") => {
                        (StatusCode::BAD_REQUEST, "BAD_REQUEST", "Referenced resource not found".into())
                    }
                    // 23514 = check_violation
                    Some("23514") => {
                        (StatusCode::BAD_REQUEST, "BAD_REQUEST", "Constraint violation".into())
                    }
                    _ => {
                        tracing::error!("Database error: {err:#}");
                        (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", "Database error".into())
                    }
                },
                sqlx::Error::PoolTimedOut => {
                    tracing::warn!("Database connection pool exhausted");
                    (StatusCode::SERVICE_UNAVAILABLE, "SERVICE_UNAVAILABLE", "Service temporarily unavailable, please retry".into())
                }
                _ => {
                    tracing::error!("Database error: {err:#}");
                    (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", "Database error".into())
                }
            },
        };

        let body = ErrorBody {
            error: ErrorDetail {
                code,
                message,
            },
        };

        let mut response = (status, Json(body)).into_response();
        if status == StatusCode::SERVICE_UNAVAILABLE {
            response.headers_mut().insert(
                axum::http::header::RETRY_AFTER,
                "5".parse().unwrap(),
            );
        }
        response
    }
}

/// Convenience type alias used across handlers and services.
pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    fn into_status_code(error: AppError) -> StatusCode {
        let response = error.into_response();
        response.status().to_owned()
    }

    #[test]
    fn not_found_maps_to_404() {
        assert_eq!(into_status_code(AppError::NotFound("test".into())), StatusCode::NOT_FOUND);
    }

    #[test]
    fn unauthorized_maps_to_401() {
        assert_eq!(into_status_code(AppError::Unauthorized("test".into())), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn forbidden_maps_to_403() {
        assert_eq!(into_status_code(AppError::Forbidden("test".into())), StatusCode::FORBIDDEN);
    }

    #[test]
    fn bad_request_maps_to_400() {
        assert_eq!(into_status_code(AppError::BadRequest("test".into())), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn conflict_maps_to_409() {
        assert_eq!(into_status_code(AppError::Conflict("test".into())), StatusCode::CONFLICT);
    }

    #[test]
    fn unprocessable_maps_to_422() {
        assert_eq!(into_status_code(AppError::Unprocessable("test".into())), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[test]
    fn payment_required_maps_to_402() {
        assert_eq!(into_status_code(AppError::PaymentRequired("test".into())), StatusCode::PAYMENT_REQUIRED);
    }

    #[test]
    fn unimplemented_maps_to_501() {
        assert_eq!(into_status_code(AppError::Unimplemented("test".into())), StatusCode::NOT_IMPLEMENTED);
    }

    #[test]
    fn internal_maps_to_500() {
        assert_eq!(
            into_status_code(AppError::Internal(anyhow::anyhow!("oops"))),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn database_row_not_found_maps_to_404() {
        let err = AppError::Database(sqlx::Error::RowNotFound);
        assert_eq!(into_status_code(err), StatusCode::NOT_FOUND);
    }

    #[test]
    fn database_pool_timeout_maps_to_503() {
        let err = AppError::Database(sqlx::Error::PoolTimedOut);
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert!(response.headers().contains_key("retry-after"));
    }

    #[tokio::test]
    async fn internal_error_hides_details() {
        let response = AppError::Internal(anyhow::anyhow!("secret db password")).into_response();
        let body = axum::body::to_bytes(response.into_body(), 1024).await.unwrap();
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert!(!body_str.contains("secret db password"));
        assert!(body_str.contains("An unexpected error occurred"));
    }

    #[tokio::test]
    async fn error_response_is_valid_json() {
        let response = AppError::BadRequest("field required".into()).into_response();
        let body = axum::body::to_bytes(response.into_body(), 1024).await.unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(parsed["error"]["code"], "BAD_REQUEST");
        assert_eq!(parsed["error"]["message"], "field required");
    }
}
