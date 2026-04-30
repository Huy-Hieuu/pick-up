use axum::{
    extract::FromRequest,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::de::DeserializeOwned;
use validator::Validate;

use crate::error::AppError;

/// Axum extractor that deserializes JSON **and** runs `validator` checks.
///
/// Usage:
/// ```ignore
/// async fn create_court(
///     ValidatedJson(body): ValidatedJson<CreateCourtRequest>,
/// ) -> AppResult<Json<Court>> { ... }
/// ```
#[derive(Debug)]
pub struct ValidatedJson<T>(pub T);

impl<T, S> FromRequest<S> for ValidatedJson<T>
where
    T: DeserializeOwned + Validate + 'static,
    S: Send + Sync,
{
    type Rejection = ValidatedRejection;

    async fn from_request(
        req: axum::extract::Request,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let Json(value) = Json::<T>::from_request(req, state)
            .await
            .map_err(ValidatedRejection::JsonRejection)?;

        value
            .validate()
            .map_err(ValidatedRejection::Validation)?;

        Ok(ValidatedJson(value))
    }
}

#[derive(Debug)]
pub enum ValidatedRejection {
    JsonRejection(axum::extract::rejection::JsonRejection),
    Validation(validator::ValidationErrors),
}

impl IntoResponse for ValidatedRejection {
    fn into_response(self) -> Response {
        match self {
            Self::JsonRejection(e) => {
                let body = serde_json::json!({
                    "error": {
                        "code": "BAD_REQUEST",
                        "message": e.body_text(),
                    }
                });
                (StatusCode::BAD_REQUEST, axum::Json(body)).into_response()
            }
            Self::Validation(errors) => {
                let field_errors: std::collections::HashMap<_, _> = errors
                    .field_errors()
                    .into_iter()
                    .map(|(field, errs)| {
                        let messages: Vec<_> = errs
                            .iter()
                            .map(|e| {
                                e.message
                                    .clone()
                                    .unwrap_or_else(|| "Invalid value".into())
                                    .to_string()
                            })
                            .collect();
                        (field.to_string(), messages)
                    })
                    .collect();

                let body = serde_json::json!({
                    "error": {
                        "code": "VALIDATION_ERROR",
                        "message": "Request validation failed",
                        "fields": field_errors,
                    }
                });
                (StatusCode::UNPROCESSABLE_ENTITY, axum::Json(body)).into_response()
            }
        }
    }
}

impl From<ValidatedRejection> for AppError {
    fn from(rejection: ValidatedRejection) -> Self {
        match rejection {
            ValidatedRejection::JsonRejection(e) => AppError::BadRequest(e.body_text()),
            ValidatedRejection::Validation(e) => AppError::Unprocessable(e.to_string()),
        }
    }
}

// ── Custom validators ──────────────────────────────────────────

/// Validate a Vietnamese phone number: +84 or 0 prefix, then 9 digits.
/// Used by `validator::Validate` custom function attribute.
pub fn is_valid_vn_phone(phone: &str) -> Result<(), validator::ValidationError> {
    let valid = phone
        .strip_prefix("+84")
        .or_else(|| phone.strip_prefix('0'))
        .map(|rest| rest.len() == 9 && rest.chars().all(|c| c.is_ascii_digit()))
        .unwrap_or(false);

    if valid {
        Ok(())
    } else {
        let mut err = validator::ValidationError::new("invalid_phone");
        err.message = Some("Invalid Vietnamese phone number".into());
        Err(err)
    }
}
