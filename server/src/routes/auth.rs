/// Auth routes — phone/email OTP via Redis + lettre email delivery.
///
/// Exposes:
/// - `POST /auth/otp`           — request OTP to email
/// - `POST /auth/verify`        — verify OTP, get JWT tokens
/// - `POST /auth/refresh`       — refresh access token
/// - `POST /auth/register`      — register with email + password
/// - `POST /auth/login`          — login with email + password
/// - `POST /auth/forgot-password` — step 1: send OTP for reset
/// - `POST /auth/verify-reset-otp` — step 2: verify OTP, get temp reset_token
/// - `POST /auth/set-new-password` — step 3: set new password with reset_token
/// - `GET  /auth/me`             — get current user profile
/// - `PATCH /auth/me`            — update current user profile

use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};

use crate::error::AppResult;
use crate::extractors::{AuthUser, ValidatedJson};
use crate::models::user::{AuthResponse, UserProfile};
use crate::models::otp::{
    ForgotPasswordRequest, LoginRequest, RefreshTokenRequest, RegisterRequest,
    SendOtpRequest, SetNewPasswordRequest, SetNewPasswordResponse,
    VerifyOtpRequest, VerifyResetOtpRequest, VerifyResetOtpResponse,
};
use crate::models::user::UpdateProfileRequest;
use crate::state::AppState;

/// Public auth routes — no auth middleware required.
pub fn public_router() -> Router<AppState> {
    Router::new()
        .route("/otp", post(send_otp))
        .route("/verify", post(verify_otp))
        .route("/refresh", post(refresh_token))
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/forgot-password", post(forgot_password))
        .route("/verify-reset-otp", post(verify_reset_otp))
        .route("/set-new-password", post(set_new_password))
}

/// Protected auth routes — require_auth middleware applied.
pub fn protected_router() -> Router<AppState> {
    Router::new()
        .route("/me", get(get_me).patch(update_me))
}

/// `POST /auth/otp` — request a 6-digit OTP via email.
async fn send_otp(
    State(state): State<AppState>,
    ValidatedJson(req): ValidatedJson<SendOtpRequest>,
) -> AppResult<Json<serde_json::Value>> {
    crate::services::AuthService::send_otp(
        &state.redis,
        &req.email,
        state.settings.redis.otp_ttl_seconds,
    )
    .await?;
    Ok(Json(serde_json::json!({ "message": "OTP sent" })))
}

/// `POST /auth/verify` — verify OTP, get JWT tokens.
async fn verify_otp(
    State(state): State<AppState>,
    ValidatedJson(req): ValidatedJson<VerifyOtpRequest>,
) -> AppResult<Json<AuthResponse>> {
    let resp = crate::services::AuthService::verify_otp(
        &state.pool,
        &state.redis,
        req,
        state.settings.redis.otp_max_attempts,
        state.settings.redis.otp_ttl_seconds,
        &state.settings.jwt.secret,
        state.settings.jwt.access_ttl_minutes,
        state.settings.jwt.refresh_ttl_days,
    )
    .await?;
    Ok(Json(resp))
}

/// `POST /auth/refresh` — refresh access token.
async fn refresh_token(
    State(state): State<AppState>,
    ValidatedJson(req): ValidatedJson<RefreshTokenRequest>,
) -> AppResult<Json<AuthResponse>> {
    let resp = crate::services::AuthService::refresh_token(
        &state.pool,
        &state.redis,
        &req.refresh_token,
        &state.settings.jwt.secret,
        state.settings.jwt.access_ttl_minutes,
        state.settings.jwt.refresh_ttl_days,
    )
    .await?;
    Ok(Json(resp))
}

/// `POST /auth/register` — register a new user with email and password.
async fn register(
    State(state): State<AppState>,
    ValidatedJson(req): ValidatedJson<RegisterRequest>,
) -> AppResult<Json<AuthResponse>> {
    let resp = crate::services::AuthService::register(
        &state.pool,
        req,
        &state.settings.jwt.secret,
        state.settings.jwt.access_ttl_minutes,
        state.settings.jwt.refresh_ttl_days,
    )
    .await?;
    Ok(Json(resp))
}

/// `POST /auth/login` — login with email and password.
async fn login(
    State(state): State<AppState>,
    ValidatedJson(req): ValidatedJson<LoginRequest>,
) -> AppResult<Json<AuthResponse>> {
    let resp = crate::services::AuthService::login(
        &state.pool,
        &state.redis,
        req,
        &state.settings.jwt.secret,
        state.settings.jwt.access_ttl_minutes,
        state.settings.jwt.refresh_ttl_days,
    )
    .await?;
    Ok(Json(resp))
}

/// `POST /auth/forgot-password` — step 1: send OTP to reset password.
async fn forgot_password(
    State(state): State<AppState>,
    ValidatedJson(req): ValidatedJson<ForgotPasswordRequest>,
) -> AppResult<Json<serde_json::Value>> {
    crate::services::AuthService::forgot_password(
        &state.pool,
        &state.redis,
        &req.email,
        state.settings.redis.otp_ttl_seconds,
    )
    .await?;
    // Always return the same response regardless of whether the email exists.
    Ok(Json(serde_json::json!({ "message": "If the email is registered, an OTP has been sent" })))
}

/// `POST /auth/verify-reset-otp` — step 2: verify OTP, get temp reset_token.
async fn verify_reset_otp(
    State(state): State<AppState>,
    ValidatedJson(req): ValidatedJson<VerifyResetOtpRequest>,
) -> AppResult<Json<VerifyResetOtpResponse>> {
    let resp = crate::services::AuthService::verify_reset_otp(
        &state.pool,
        &state.redis,
        req,
        state.settings.redis.otp_max_attempts,
        state.settings.redis.otp_ttl_seconds,
    )
    .await?;
    Ok(Json(resp))
}

/// `POST /auth/set-new-password` — step 3: set new password using reset_token.
async fn set_new_password(
    State(state): State<AppState>,
    ValidatedJson(req): ValidatedJson<SetNewPasswordRequest>,
) -> AppResult<Json<SetNewPasswordResponse>> {
    let resp = crate::services::AuthService::set_new_password(
        &state.pool,
        &state.redis,
        req,
    )
    .await?;
    Ok(Json(resp))
}

/// `GET /auth/me` — get current user profile.
async fn get_me(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<UserProfile>> {
    let profile = crate::services::AuthService::get_profile(&state.pool, auth.user_id()).await?;
    Ok(Json(profile))
}

/// `PATCH /auth/me` — update current user profile.
async fn update_me(
    State(state): State<AppState>,
    auth: AuthUser,
    ValidatedJson(req): ValidatedJson<UpdateProfileRequest>,
) -> AppResult<Json<UserProfile>> {
    let profile = crate::services::AuthService::update_profile(&state.pool, auth.user_id(), req).await?;
    Ok(Json(profile))
}