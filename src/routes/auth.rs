use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use tower_sessions::Session;

use crate::error::{AppError, AppResult};
use crate::models::{
    ApiKey, CreateApiKeyRequest, CreateOrganizationRequest, CreateUserRequest, Organization, User,
};
use crate::state::AppState;
use crate::utils::auth::AuthenticatedUser;
use crate::utils::crypto::{generate_api_key, hash_api_key};
use crate::utils::validation::validate_email;

#[derive(Debug, Serialize)]
pub struct RegisterResponse {
    user: UserResponse,
    organization: Organization,
    // api_key: ApiKeyResponse,
    message: String,
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
    id: Uuid,
    email: String,
    full_name: Option<String>,
    role: String,
    created_at: chrono::DateTime<chrono::Utc>,
    email_verified: bool,
}

#[derive(Debug, Serialize)]
pub struct ApiKeyResponse {
    id: Uuid,
    key: String,
    key_prefix: String,
    name: Option<String>,
    network: String,
    created_at: chrono::DateTime<chrono::Utc>,
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    email: String,
    password: String,
    full_name: Option<String>,
    organization_name: String,
    organization_slug: String,
}

pub async fn register(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<RegisterRequest>,
) -> AppResult<(StatusCode, Json<RegisterResponse>)> {
    // Validate email
    validate_email(&payload.email)?;

    // Validate password strength
    if payload.password.len() < 8 {
        return Err(AppError::BadRequest(
            "Password must be at least 8 characters".to_string(),
        ));
    }

    // Check if user already exists
    if User::get_by_email(&state.db, &payload.email).await?.is_some() {
        return Err(AppError::BadRequest("Email already registered".to_string()));
    }

    // Check if organization slug is available
    if Organization::get_by_slug(&state.db, &payload.organization_slug)
        .await?
        .is_some()
    {
        return Err(AppError::BadRequest(
            "Organization slug already taken".to_string(),
        ));
    }

    // Hash password
    let password_hash = bcrypt::hash(&payload.password, bcrypt::DEFAULT_COST)
        .map_err(|e| AppError::Internal(format!("Password hashing failed: {}", e)))?;

    // Create organization (we need a temporary user_id, will update later)
    let temp_user_id = Uuid::new_v4();
    let organization = Organization::create(
        &state.db,
        payload.organization_name,
        payload.organization_slug,
        "individual".to_string(),
        temp_user_id,
    )
    .await?;

    // Create user
    let user = User::create(
        &state.db,
        organization.id,
        payload.email.clone(),
        password_hash,
        payload.full_name,
        "owner".to_string(),
    )
    .await?;

    // Update organization owner
    sqlx::query!(
        "UPDATE organizations SET owner_id = $1 WHERE id = $2",
        user.id,
        organization.id
    )
    .execute(&state.db)
    .await?;

    // Create default API key
    // let (api_key_str, key_hash) = generate_api_key("sdk_live");
    // let key_prefix = api_key_str.split('_').take(3).collect::<Vec<_>>().join("_");

    // let api_key = ApiKey::create(
    //     &state.db,
    //     organization.id,
    //     user.id,
    //     key_hash,
    //     key_prefix,
    //     Some("Default API Key".to_string()),
    //     "both".to_string(),
    //     None,
    // )
    // .await?;

    // Generate verification token
    let verification_token = uuid::Uuid::new_v4().to_string();
    let expires_at = Utc::now() + chrono::Duration::hours(24);
    
    User::set_verification_token(&state.db, user.id, verification_token.clone(), expires_at).await?;

    // Send verification email
    state
        .email
        .send_verification_email(&payload.email, &verification_token)
        .await?;

    // Create free tier subscription plan
    crate::models::SubscriptionPlan::create_or_update(
        &state.db,
        organization.id,
        "free".to_string(),
        "active".to_string(),
        None,
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(RegisterResponse {
            user: UserResponse {
                id: user.id,
                email: user.email,
                full_name: user.full_name,
                role: user.role,
                created_at: user.created_at,
                email_verified: user.email_verified,
            },
            organization,
            message: "Registration successful! Please check your email to verify your account.".to_string(),
        }),
    ))
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    user: UserResponse,
    organization: Organization,
    message: String, 
}

#[derive(Debug, Serialize)]
pub struct ApiKeyListItem {
    id: Uuid,
    key_prefix: String,
    name: Option<String>,
    network: String,
    created_at: chrono::DateTime<chrono::Utc>,
    last_used: Option<chrono::DateTime<chrono::Utc>>,
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    session: Session,
    Json(payload): Json<LoginRequest>,
) -> AppResult<Json<LoginResponse>> {
    // Get user
    let user = User::get_by_email(&state.db, &payload.email)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Invalid credentials".to_string()))?;

    // Verify password
    let is_valid = bcrypt::verify(&payload.password, &user.password_hash)
        .map_err(|e| AppError::Internal(format!("Password verification failed: {}", e)))?;

    if !is_valid {
        return Err(AppError::Unauthorized("Invalid credentials".to_string()));
    }

    // Get organization
    let organization = Organization::get_by_id(&state.db, user.organization_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Organization not found".to_string()))?;

    // Get API keys
    let api_keys = ApiKey::list_by_organization(&state.db, organization.id).await?;
    let api_key_list: Vec<ApiKeyListItem> = api_keys
        .into_iter()
        .map(|k| ApiKeyListItem {
            id: k.id,
            key_prefix: k.key_prefix,
            name: k.name,
            network: k.network,
            created_at: k.created_at,
            last_used: k.last_used,
        })
        .collect();

    session
        .insert(crate::utils::session_auth::USER_ID_KEY, user.id)
        .await
        .map_err(|_| AppError::Internal("Failed to create session".to_string()))?;
    
    session
        .insert(crate::utils::session_auth::ORG_ID_KEY, organization.id)
        .await
        .map_err(|_| AppError::Internal("Failed to create session".to_string()))?;

    Ok(Json(LoginResponse {
        user: UserResponse {
            id: user.id,
            email: user.email,
            full_name: user.full_name,
            role: user.role,
            created_at: user.created_at,
            email_verified: user.email_verified,
        },
        organization,
        message: "Login successful".to_string(),
    }))
}


#[derive(Debug, Serialize)]
pub struct LogoutResponse {
    message: String,
}

pub async fn logout(
    session: Session,
) -> AppResult<Json<LogoutResponse>> {
    session
        .flush()
        .await
        .map_err(|_| AppError::Internal("Failed to logout".to_string()))?;

    Ok(Json(LogoutResponse {
        message: "Logged out successfully".to_string(),
    }))
}

pub async fn create_api_key(
    State(state): State<Arc<AppState>>,
    Extension(session_user): Extension<crate::utils::session_auth::SessionUser>,
    Json(payload): Json<CreateApiKeyRequest>,
) -> AppResult<(StatusCode, Json<ApiKeyResponse>)> {
    // Generate API key
    let (api_key_str, key_hash, key_prefix) = generate_api_key("sdk_live");

    // Calculate expiration
    let expires_at = payload.expires_in_days.map(|days| {
        Utc::now() + Duration::days(days as i64)
    });

    // Create API key
    let api_key = ApiKey::create(
        &state.db,
        session_user.organization_id,
        session_user.user.id,
        key_hash,
        key_prefix,
        payload.name,
        payload.network,
        expires_at,
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiKeyResponse {
            id: api_key.id,
            key: api_key_str,
            key_prefix: api_key.key_prefix,
            name: api_key.name,
            network: api_key.network,
            created_at: api_key.created_at,
            expires_at: api_key.expires_at,
        }),
    ))
}

pub async fn list_api_keys(
    State(state): State<Arc<AppState>>,
    Extension(session_user): Extension<crate::utils::session_auth::SessionUser>,
) -> AppResult<Json<Vec<ApiKeyListItem>>> {
    let api_keys = ApiKey::list_by_organization(&state.db, session_user.organization_id).await?;

    let list: Vec<ApiKeyListItem> = api_keys
        .into_iter()
        .map(|k| ApiKeyListItem {
            id: k.id,
            key_prefix: k.key_prefix,
            name: k.name,
            network: k.network,
            created_at: k.created_at,
            last_used: k.last_used,
        })
        .collect();

    Ok(Json(list))
}

pub async fn delete_api_key(
    State(state): State<Arc<AppState>>,
    Extension(session_user): Extension<crate::utils::session_auth::SessionUser>,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    ApiKey::delete(&state.db, id, session_user.organization_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Serialize)]
pub struct WebhookSecretResponse {
    webhook_secret: String,
}

pub async fn get_webhook_secret(
    State(state): State<Arc<AppState>>,
    Extension(session_user): Extension<crate::utils::session_auth::SessionUser>,
) -> AppResult<Json<WebhookSecretResponse>> {
    let org = Organization::get_by_id(&state.db, session_user.organization_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Organization not found".to_string()))?;

    Ok(Json(WebhookSecretResponse {
        webhook_secret: org.webhook_secret,
    }))
}

#[derive(Debug, Deserialize)]
pub struct VerifyEmailRequest {
    token: String,
}

#[derive(Debug, Serialize)]
pub struct VerifyEmailResponse {
    message: String,
    // api_key: ApiKeyResponse,
}

pub async fn verify_email(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<VerifyEmailRequest>,
) -> AppResult<Json<VerifyEmailResponse>> {
    // Verify the token and update user
    let user = User::verify_email(&state.db, &payload.token)
        .await?
        .ok_or_else(|| AppError::BadRequest("Invalid or expired verification token".to_string()))?;

    // Create default API key after email verification
    let (api_key_str, key_hash, key_prefix) = generate_api_key("sdk_live");

    let api_key = ApiKey::create(
        &state.db,
        user.organization_id,
        user.id,
        key_hash,
        key_prefix,
        Some("Default API Key".to_string()),
        "both".to_string(),
        None,
    )
    .await?;

    Ok(Json(VerifyEmailResponse {
        message: "Email verified successfully!".to_string(),
        // api_key: ApiKeyResponse {
        //     id: api_key.id,
        //     key: api_key_str,
        //     key_prefix: api_key.key_prefix,
        //     name: api_key.name,
        //     network: api_key.network,
        //     created_at: api_key.created_at,
        //     expires_at: api_key.expires_at,
        // },
    }))
}