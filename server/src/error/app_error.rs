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

        (status, Json(body)).into_response()
    }
}

/// Convenience type alias used across handlers and services.
pub type AppResult<T> = Result<T, AppError>;
