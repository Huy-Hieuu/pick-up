use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use validator::Validate;

// ── Database row type (maps 1:1 to `users` table) ─────────────

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct UserRow {
    pub id: Uuid,
    pub phone: String,
    pub email: Option<String>,
    #[serde(skip)]
    pub password_hash: Option<String>,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ── Request DTOs ───────────────────────────────────────────────

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateProfileRequest {
    #[validate(length(min = 1, max = 50, message = "Display name must be 1–50 characters"))]
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}

impl UpdateProfileRequest {
    /// Trim display_name whitespace. Must be called before validation.
    pub fn trimmed(mut self) -> Self {
        if let Some(ref name) = self.display_name {
            let trimmed = name.trim().to_string();
            if trimmed.is_empty() {
                self.display_name = None;
            } else {
                self.display_name = Some(trimmed);
            }
        }
        self
    }
}

// ── Response DTOs ──────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct UserProfile {
    pub id: Uuid,
    pub phone: String,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl From<UserRow> for UserProfile {
    fn from(row: UserRow) -> Self {
        Self {
            id: row.id,
            phone: row.phone,
            email: row.email,
            display_name: row.display_name,
            avatar_url: row.avatar_url,
            created_at: row.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub user: UserProfile,
}

#[cfg(test)]
mod tests {
    use super::*;
    use validator::Validate;

    // ── UserRow serialization skips password_hash ──────────────────

    #[test]
    fn user_row_serializes_without_password_hash() {
        let row = UserRow {
            id: Uuid::new_v4(),
            phone: "+84912345678".into(),
            email: Some("test@example.com".into()),
            password_hash: Some("supersecret".into()),
            display_name: Some("Test User".into()),
            avatar_url: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let json = serde_json::to_string(&row).unwrap();
        assert!(!json.contains("password_hash"), "password_hash must not appear in JSON");
        assert!(!json.contains("supersecret"), "password value must not leak");
        assert!(json.contains("display_name"));
    }

    #[test]
    fn user_row_serializes_null_password_hash() {
        let row = UserRow {
            id: Uuid::new_v4(),
            phone: "+84912345678".into(),
            email: Some("otp@example.com".into()),
            password_hash: None,
            display_name: None,
            avatar_url: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let json = serde_json::to_string(&row).unwrap();
        assert!(!json.contains("password_hash"));
    }

    // ── UserProfile from UserRow ───────────────────────────────────

    #[test]
    fn user_profile_excludes_password_and_updated_at() {
        let row = UserRow {
            id: Uuid::new_v4(),
            phone: "+84912345678".into(),
            email: Some("test@example.com".into()),
            password_hash: Some("hash".into()),
            display_name: Some("Test".into()),
            avatar_url: Some("http://img".into()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let profile = UserProfile::from(row);
        assert_eq!(profile.display_name, Some("Test".into()));
        assert_eq!(profile.avatar_url, Some("http://img".into()));
        // Profile does not have password_hash or updated_at fields
    }

    // ── UpdateProfileRequest validation ────────────────────────────

    #[test]
    fn update_profile_valid_display_name() {
        let req = UpdateProfileRequest {
            display_name: Some("New Name".into()),
            avatar_url: None,
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn update_profile_empty_display_name_rejected() {
        let req = UpdateProfileRequest {
            display_name: Some("".into()),
            avatar_url: None,
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn update_profile_too_long_display_name() {
        let req = UpdateProfileRequest {
            display_name: Some("x".repeat(51)),
            avatar_url: None,
        };
        assert!(req.validate().is_err());
    }

    // ── UpdateProfileRequest trimmed() ─────────────────────────────

    #[test]
    fn trimmed_converts_whitespace_to_none() {
        let req = UpdateProfileRequest {
            display_name: Some("   ".into()),
            avatar_url: None,
        };
        let trimmed = req.trimmed();
        assert!(trimmed.display_name.is_none());
    }

    #[test]
    fn trimmed_strips_leading_trailing_whitespace() {
        let req = UpdateProfileRequest {
            display_name: Some("  Nguyen Van A  ".into()),
            avatar_url: None,
        };
        let trimmed = req.trimmed();
        assert_eq!(trimmed.display_name, Some("Nguyen Van A".into()));
    }

    #[test]
    fn trimmed_keeps_none_as_none() {
        let req = UpdateProfileRequest {
            display_name: None,
            avatar_url: None,
        };
        let trimmed = req.trimmed();
        assert!(trimmed.display_name.is_none());
    }

    // ── AuthResponse structure ─────────────────────────────────────

    #[test]
    fn auth_response_serializes_correctly() {
        let user = UserProfile {
            id: Uuid::new_v4(),
            phone: "+84912345678".into(),
            email: Some("test@example.com".into()),
            display_name: Some("Test".into()),
            avatar_url: None,
            created_at: Utc::now(),
        };
        let resp = AuthResponse {
            access_token: "access_tok".into(),
            refresh_token: "refresh_tok".into(),
            user,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("access_token"));
        assert!(json.contains("refresh_token"));
        assert!(json.contains("user"));
        assert!(!json.contains("password"));
    }
}
