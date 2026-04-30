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
