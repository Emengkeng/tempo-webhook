use crate::error::{AppError, AppResult};
use crate::models::{ApiKey, User};
use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct AuthenticatedUser {
    pub user: User,
    pub api_key: ApiKey,
}

/// Middleware to authenticate API requests with full state access for rate limiting
pub async fn authenticate_api_key_with_state(
    State(state): State<Arc<crate::state::AppState>>,
    mut req: Request,
    next: Next,
) -> AppResult<Response> {
    // Extract API key from header
    let api_key = req
        .headers()
        .get("X-API-Key")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::Unauthorized("Missing API key".to_string()))?;

    // Hash the provided key
    let key_hash = crate::utils::crypto::hash_api_key(api_key);

    // Lookup API key in database
    let api_key_record = ApiKey::get_by_hash(&state.db, &key_hash)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Invalid API key".to_string()))?;

    // Check if key is expired
    if let Some(expires_at) = api_key_record.expires_at {
        if expires_at < chrono::Utc::now() {
            return Err(AppError::Unauthorized("API key expired".to_string()));
        }
    }

    // Get user
    let user = User::get_by_id(&state.db, api_key_record.user_id)
        .await?
        .ok_or_else(|| AppError::Unauthorized("User not found".to_string()))?;

    if !user.email_verified {
        return Err(AppError::Unauthorized(
            "Email not verified. Please verify your email to use the API.".to_string()
        ));
    }

    // Check organization is active
    let org = crate::models::Organization::get_by_id(&state.db, user.organization_id)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Organization not found".to_string()))?;
    
    if !org.active {
        return Err(AppError::Unauthorized("Organization is inactive".to_string()));
    }

    // Check rate limit
    crate::services::quota::check_api_rate_limit(
        &state.redis,
        api_key_record.organization_id,
        &state.db,
    )
    .await?;

    // Update last used timestamp (fire and forget)
    let key_id = api_key_record.id;
    let pool_clone = state.db.clone();
    tokio::spawn(async move {
        let _ = ApiKey::update_last_used(&pool_clone, key_id).await;
    });

    // Store authenticated context in request extensions
    req.extensions_mut().insert(AuthenticatedUser {
        user,
        api_key: api_key_record,
    });

    Ok(next.run(req).await)
}

/// Middleware to authenticate API requests (deprecated - use authenticate_api_key_with_state)
pub async fn authenticate_api_key(
    State(pool): State<Arc<PgPool>>,
    mut req: Request,
    next: Next,
) -> AppResult<Response> {
    // Extract API key from header
    let api_key = req
        .headers()
        .get("X-API-Key")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::Unauthorized("Missing API key".to_string()))?;

    // Hash the provided key
    let key_hash = crate::utils::crypto::hash_api_key(api_key);

    // Lookup API key in database
    let api_key_record = ApiKey::get_by_hash(&pool, &key_hash)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Invalid API key".to_string()))?;

    // Check if key is expired
    if let Some(expires_at) = api_key_record.expires_at {
        if expires_at < chrono::Utc::now() {
            return Err(AppError::Unauthorized("API key expired".to_string()));
        }
    }

    // Get user
    let user = User::get_by_id(&pool, api_key_record.user_id)
        .await?
        .ok_or_else(|| AppError::Unauthorized("User not found".to_string()))?;

    // Check organization is active
    let org = crate::models::Organization::get_by_id(&pool, user.organization_id)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Organization not found".to_string()))?;
    
    if !org.active {
        return Err(AppError::Unauthorized("Organization is inactive".to_string()));
    }

    // Update last used timestamp (fire and forget)
    let key_id = api_key_record.id;
    let pool_clone = pool.clone();
    tokio::spawn(async move {
        let _ = ApiKey::update_last_used(&pool_clone, key_id).await;
    });

    // Store authenticated context in request extensions
    req.extensions_mut().insert(AuthenticatedUser {
        user,
        api_key: api_key_record,
    });

    Ok(next.run(req).await)
}

/// Extract authenticated user from request
pub fn get_authenticated_user(req: &Request) -> AppResult<&AuthenticatedUser> {
    req.extensions()
        .get::<AuthenticatedUser>()
        .ok_or_else(|| AppError::Unauthorized("Not authenticated".to_string()))
}
