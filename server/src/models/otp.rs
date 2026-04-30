use serde::{Deserialize, Serialize};
use validator::Validate;

// ── Request DTOs ───────────────────────────────────────────────
// OTP is stored in Redis, not PostgreSQL — no row type needed.

#[derive(Debug, Deserialize, Validate)]
pub struct SendOtpRequest {
    #[validate(email(message = "Invalid email address"))]
    pub email: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct VerifyOtpRequest {
    #[validate(email(message = "Invalid email address"))]
    pub email: String,
    pub phone: String,
    #[validate(length(min = 6, max = 6))]
    pub code: String,
}

#[derive(Debug, Deserialize)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}

// ── Register ────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Validate)]
pub struct RegisterRequest {
    pub phone: String,
    #[validate(email(message = "Invalid email address"))]
    pub email: String,
    #[validate(length(min = 1, max = 50))]
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub password: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct LoginRequest {
    #[validate(email(message = "Invalid email address"))]
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ForgotPasswordRequest {
    #[validate(email(message = "Invalid email address"))]
    pub email: String,
}

// ── Reset password via OTP (3-step flow) ────────────────────────

#[derive(Debug, Deserialize, Validate)]
pub struct VerifyResetOtpRequest {
    #[validate(email(message = "Invalid email address"))]
    pub email: String,
    #[validate(length(min = 6, max = 6))]
    pub code: String,
}

#[derive(Debug, Serialize)]
pub struct VerifyResetOtpResponse {
    pub reset_token: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct SetNewPasswordRequest {
    pub reset_token: String,
    #[validate(length(min = 8, message = "Password must be at least 8 characters"))]
    pub new_password: String,
}

#[derive(Debug, Serialize)]
pub struct SetNewPasswordResponse {
    pub message: String,
}