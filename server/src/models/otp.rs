use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::extractors::validated::is_valid_vn_phone;

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
    #[validate(custom(function = "is_valid_vn_phone"))]
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
    #[validate(custom(function = "is_valid_vn_phone"))]
    pub phone: String,
    #[validate(email(message = "Invalid email address"))]
    pub email: String,
    #[validate(length(min = 1, max = 50))]
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    #[validate(length(min = 8, max = 128, message = "Password must be 8–128 characters"))]
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

#[cfg(test)]
mod tests {
    use super::*;
    use validator::Validate;

    // ── SendOtpRequest ─────────────────────────────────────────────

    #[test]
    fn send_otp_valid_email() {
        let req = SendOtpRequest { email: "user@example.com".into() };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn send_otp_invalid_email() {
        let req = SendOtpRequest { email: "not-an-email".into() };
        assert!(req.validate().is_err());
    }

    #[test]
    fn send_otp_empty_email() {
        let req = SendOtpRequest { email: "".into() };
        assert!(req.validate().is_err());
    }

    // ── VerifyOtpRequest ───────────────────────────────────────────

    #[test]
    fn verify_otp_valid() {
        let req = VerifyOtpRequest {
            email: "user@example.com".into(),
            phone: "0912345678".into(),
            code: "123456".into(),
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn verify_otp_invalid_email() {
        let req = VerifyOtpRequest {
            email: "bad-email".into(),
            phone: "0912345678".into(),
            code: "123456".into(),
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn verify_otp_invalid_phone() {
        let req = VerifyOtpRequest {
            email: "user@example.com".into(),
            phone: "123".into(), // too short, not valid VN phone
            code: "123456".into(),
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn verify_otp_code_too_short() {
        let req = VerifyOtpRequest {
            email: "user@example.com".into(),
            phone: "0912345678".into(),
            code: "1234".into(),
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn verify_otp_code_too_long() {
        let req = VerifyOtpRequest {
            email: "user@example.com".into(),
            phone: "0912345678".into(),
            code: "1234567".into(),
        };
        assert!(req.validate().is_err());
    }

    // ── RegisterRequest ────────────────────────────────────────────

    #[test]
    fn register_valid() {
        let req = RegisterRequest {
            phone: "0912345678".into(),
            email: "newuser@example.com".into(),
            display_name: Some("Nguyen Van A".into()),
            avatar_url: None,
            password: "securepass123".into(),
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn register_invalid_phone() {
        let req = RegisterRequest {
            phone: "abc".into(),
            email: "user@example.com".into(),
            display_name: None,
            avatar_url: None,
            password: "securepass123".into(),
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn register_password_too_short() {
        let req = RegisterRequest {
            phone: "0912345678".into(),
            email: "user@example.com".into(),
            display_name: None,
            avatar_url: None,
            password: "short".into(), // < 8 chars
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn register_password_exactly_8_chars() {
        let req = RegisterRequest {
            phone: "0912345678".into(),
            email: "user@example.com".into(),
            display_name: None,
            avatar_url: None,
            password: "12345678".into(), // exactly 8
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn register_display_name_empty_rejected() {
        let req = RegisterRequest {
            phone: "0912345678".into(),
            email: "user@example.com".into(),
            display_name: Some("".into()), // length(min = 1) should reject
            avatar_url: None,
            password: "securepass123".into(),
        };
        assert!(req.validate().is_err());
    }

    // ── LoginRequest ───────────────────────────────────────────────

    #[test]
    fn login_valid() {
        let req = LoginRequest {
            email: "user@example.com".into(),
            password: "anypassword".into(),
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn login_invalid_email() {
        let req = LoginRequest {
            email: "not-email".into(),
            password: "anypassword".into(),
        };
        assert!(req.validate().is_err());
    }

    // ── SetNewPasswordRequest ───────────────────────────────────────

    #[test]
    fn set_new_password_valid() {
        let req = SetNewPasswordRequest {
            reset_token: "sometoken".into(),
            new_password: "newpassword123".into(),
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn set_new_password_too_short() {
        let req = SetNewPasswordRequest {
            reset_token: "sometoken".into(),
            new_password: "short".into(), // < 8 chars
        };
        assert!(req.validate().is_err());
    }

    // ── ForgotPasswordRequest ──────────────────────────────────────

    #[test]
    fn forgot_password_valid_email() {
        let req = ForgotPasswordRequest { email: "user@example.com".into() };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn forgot_password_invalid_email() {
        let req = ForgotPasswordRequest { email: "not-valid".into() };
        assert!(req.validate().is_err());
    }
}