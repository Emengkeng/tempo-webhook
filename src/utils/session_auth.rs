use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use tower_sessions::Session;
use uuid::Uuid;
use std::sync::Arc;

use crate::error::{AppError, AppResult};
use crate::models::User;
use crate::state::AppState;

pub const USER_ID_KEY: &str = "user_id";
pub const ORG_ID_KEY: &str = "org_id";

#[derive(Clone, Debug)]
pub struct SessionUser {
    pub user: User,
    pub organization_id: Uuid,
}

/// Middleware to authenticate web/dashboard requests with sessions
pub async fn authenticate_session(
    State(state): State<Arc<AppState>>,
    session: Session,
    mut req: Request,
    next: Next,
) -> AppResult<Response> {
    // Get user ID from session
    let user_id: Uuid = session
        .get(USER_ID_KEY)
        .await
        .map_err(|_| AppError::Unauthorized("Session error".to_string()))?
        .ok_or_else(|| AppError::Unauthorized("Not authenticated".to_string()))?;

    // Get user from database
    let user = User::get_by_id(&state.db, user_id)
        .await?
        .ok_or_else(|| AppError::Unauthorized("User not found".to_string()))?;

    if !user.email_verified {
        return Err(AppError::Unauthorized(
            "Email not verified".to_string()
        ));
    }

    // Check organization is active
    let org = crate::models::Organization::get_by_id(&state.db, user.organization_id)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Organization not found".to_string()))?;
    
    if !org.active {
        return Err(AppError::Unauthorized("Organization is inactive".to_string()));
    }

    // Store in request extensions
    req.extensions_mut().insert(SessionUser {
        user,
        organization_id: org.id,
    });

    Ok(next.run(req).await)
}

/// Extract authenticated session user from request
pub fn get_session_user(req: &Request) -> AppResult<&SessionUser> {
    req.extensions()
        .get::<SessionUser>()
        .ok_or_else(|| AppError::Unauthorized("Not authenticated".to_string()))
}