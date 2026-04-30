use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::user::UserRow;

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

pub async fn update_user_profile(
    pool: &PgPool,
    user_id: Uuid,
    display_name: Option<&str>,
    avatar_url: Option<&str>,
) -> sqlx::Result<UserRow> {
    sqlx::query_as::<_, UserRow>(
        r#"
        UPDATE users
        SET display_name = COALESCE($2, display_name),
            avatar_url = COALESCE($3, avatar_url)
        WHERE id = $1
        RETURNING id, phone, email, password_hash, display_name, avatar_url, created_at, updated_at
        "#,
    )
    .bind(user_id)
    .bind(display_name)
    .bind(avatar_url)
    .fetch_one(pool)
    .await
}

pub async fn update_password_hash(pool: &PgPool, user_id: Uuid, password_hash: &str) -> sqlx::Result<UserRow> {
    sqlx::query_as::<_, UserRow>(
        r#"
        UPDATE users
        SET password_hash = $2
        WHERE id = $1
        RETURNING id, phone, email, password_hash, display_name, avatar_url, created_at, updated_at
        "#,
    )
    .bind(user_id)
    .bind(password_hash)
    .fetch_one(pool)
    .await
}