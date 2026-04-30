use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// ── PatchValue ────────────────────────────────────────────────

/// Wrapper for PATCH request fields to distinguish "absent" from "set to null".
///
/// - `Absent` — field not present in JSON, don't change the stored value
/// - `Null`   — explicitly `null` in JSON, clear the stored value
/// - `Value(T)` — set to new value
///
/// Usage with serde:
/// ```ignore
/// #[serde(default, deserialize_with = "deserialize_patch")]
/// pub field: PatchValue<String>,
/// ```
#[derive(Debug, Clone, Default)]
pub enum PatchValue<T> {
    #[default]
    Absent,
    Null,
    Value(T),
}

impl<T> PatchValue<T> {
    pub fn is_absent(&self) -> bool {
        matches!(self, PatchValue::Absent)
    }
}

/// Serde helper: `Some(v)` → `Value(v)`, `None` → `Null`.
/// Paired with `#[serde(default)]` which maps absent fields to `Absent`.
pub fn deserialize_patch<'de, D, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<PatchValue<T>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<T>::deserialize(deserializer).map(|opt| match opt {
        None => PatchValue::Null,
        Some(v) => PatchValue::Value(v),
    })
}

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

#[derive(Debug, Deserialize)]
pub struct UpdateProfileRequest {
    #[serde(default, deserialize_with = "deserialize_patch")]
    pub display_name: PatchValue<String>,
    #[serde(default, deserialize_with = "deserialize_patch")]
    pub avatar_url: PatchValue<String>,
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

    // ── PatchValue ──────────────────────────────────────────────────

    #[test]
    fn patch_value_default_is_absent() {
        let v: PatchValue<String> = PatchValue::default();
        assert!(v.is_absent());
    }

    #[test]
    fn patch_value_roundtrip_json() {
        #[derive(Debug, Deserialize)]
        struct Test {
            #[serde(default, deserialize_with = "deserialize_patch")]
            name: PatchValue<String>,
        }

        // Field present with value
        let t: Test = serde_json::from_str(r#"{"name":"Alice"}"#).unwrap();
        assert!(matches!(t.name, PatchValue::Value(ref s) if s == "Alice"));

        // Field present as null
        let t: Test = serde_json::from_str(r#"{"name":null}"#).unwrap();
        assert!(matches!(t.name, PatchValue::Null));

        // Field absent
        let t: Test = serde_json::from_str(r#"{}"#).unwrap();
        assert!(t.name.is_absent());
    }
}
