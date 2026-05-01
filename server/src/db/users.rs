use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::user::{PatchValue, UserRow};

pub async fn find_user_by_email(pool: &PgPool, email: &str) -> sqlx::Result<Option<UserRow>> {
    sqlx::query_as::<_, UserRow>(
        "SELECT id, phone, email, password_hash, display_name, avatar_url, created_at, updated_at FROM users WHERE email = $1",
    )
    .bind(email)
    .fetch_optional(pool)
    .await
}

pub async fn find_user_by_email_and_password(pool: &PgPool, email: &str) -> sqlx::Result<Option<UserRow>> {
    sqlx::query_as::<_, UserRow>(
        "SELECT id, phone, email, password_hash, display_name, avatar_url, created_at, updated_at FROM users WHERE email = $1 AND password_hash IS NOT NULL",
    )
    .bind(email)
    .fetch_optional(pool)
    .await
}

pub fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2.hash_password(password.as_bytes(), &salt)?;
    Ok(password_hash.to_string())
}

pub fn verify_password(password: &str, password_hash: &str) -> Result<bool, argon2::password_hash::Error> {
    let parsed_hash = PasswordHash::new(password_hash)?;
    Ok(Argon2::default().verify_password(password.as_bytes(), &parsed_hash).is_ok())
}

pub async fn find_user_by_id(pool: &PgPool, user_id: Uuid) -> sqlx::Result<Option<UserRow>> {
    sqlx::query_as::<_, UserRow>(
        "SELECT id, phone, email, password_hash, display_name, avatar_url, created_at, updated_at FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

pub async fn create_user(
    pool: &PgPool,
    phone: &str,
    email: &str,
    password_hash: Option<&str>,
    display_name: Option<&str>,
    avatar_url: Option<&str>,
) -> sqlx::Result<UserRow> {
    sqlx::query_as::<_, UserRow>(
        r#"
        INSERT INTO users (phone, email, password_hash, display_name, avatar_url)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id, phone, email, password_hash, display_name, avatar_url, created_at, updated_at
        "#,
    )
    .bind(phone)
    .bind(email)
    .bind(password_hash)
    .bind(display_name)
    .bind(avatar_url)
    .fetch_one(pool)
    .await
}

/// Update user profile using PatchValue to distinguish absent / null / set.
///
/// For each field:
/// - `Absent` → keep current value (CASE WHEN false)
/// - `Null`   → set to NULL
/// - `Value`  → set to new value
pub async fn update_user_profile_patch(
    pool: &PgPool,
    user_id: Uuid,
    display_name: &PatchValue<String>,
    avatar_url: &PatchValue<String>,
) -> sqlx::Result<UserRow> {
    let (dn_provided, dn_value): (bool, Option<&str>) = match display_name {
        PatchValue::Absent => (false, None),
        PatchValue::Null => (true, None),
        PatchValue::Value(v) => (true, Some(v)),
    };
    let (av_provided, av_value): (bool, Option<&str>) = match avatar_url {
        PatchValue::Absent => (false, None),
        PatchValue::Null => (true, None),
        PatchValue::Value(v) => (true, Some(v)),
    };

    sqlx::query_as::<_, UserRow>(
        r#"
        UPDATE users
        SET display_name = CASE WHEN $2 THEN $3 ELSE display_name END,
            avatar_url   = CASE WHEN $4 THEN $5 ELSE avatar_url END,
            updated_at    = NOW()
        WHERE id = $1
        RETURNING id, phone, email, password_hash, display_name, avatar_url, created_at, updated_at
        "#,
    )
    .bind(user_id)
    .bind(dn_provided)
    .bind(dn_value)
    .bind(av_provided)
    .bind(av_value)
    .fetch_one(pool)
    .await
}

pub async fn update_password_hash(pool: &PgPool, user_id: Uuid, password_hash: &str) -> sqlx::Result<UserRow> {
    sqlx::query_as::<_, UserRow>(
        r#"
        UPDATE users
        SET password_hash = $2,
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, phone, email, password_hash, display_name, avatar_url, created_at, updated_at
        "#,
    )
    .bind(user_id)
    .bind(password_hash)
    .fetch_one(pool)
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── hash_password ──────────────────────────────────────────────

    #[test]
    fn hash_password_produces_valid_argon2_hash() {
        let hash = hash_password("mypassword123").unwrap();
        assert!(hash.starts_with("$argon2"), "Expected Argon2 hash prefix");
        assert!(hash.len() > 30);
    }

    #[test]
    fn hash_password_uses_random_salts() {
        let h1 = hash_password("samepassword").unwrap();
        let h2 = hash_password("samepassword").unwrap();
        assert_ne!(h1, h2, "Same password must produce different hashes (random salt)");
    }

    // ── verify_password ────────────────────────────────────────────

    #[test]
    fn verify_password_correct() {
        let hash = hash_password("correcthorsebatterystaple").unwrap();
        assert!(verify_password("correcthorsebatterystaple", &hash).unwrap());
    }

    #[test]
    fn verify_password_incorrect() {
        let hash = hash_password("password123").unwrap();
        assert!(!verify_password("wrongpassword", &hash).unwrap());
    }

    #[test]
    fn verify_password_rejects_empty_hash() {
        assert!(verify_password("anypass", "").is_err());
    }

    #[test]
    fn verify_password_rejects_garbage_hash() {
        assert!(verify_password("anypass", "not-a-real-hash").is_err());
    }

    #[test]
    fn verify_password_handles_empty_password() {
        let hash = hash_password("").unwrap();
        assert!(verify_password("", &hash).unwrap());
        assert!(!verify_password("notempty", &hash).unwrap());
    }

    #[test]
    fn verify_password_handles_unicode() {
        let hash = hash_password("Mật khẩu tiếng Việt 🎾").unwrap();
        assert!(verify_password("Mật khẩu tiếng Việt 🎾", &hash).unwrap());
        assert!(!verify_password("Mat khau tieng Viet", &hash).unwrap());
    }

    #[test]
    fn verify_password_handles_long_password() {
        let long_pw = "a".repeat(128);
        let hash = hash_password(&long_pw).unwrap();
        assert!(verify_password(&long_pw, &hash).unwrap());
    }
}
