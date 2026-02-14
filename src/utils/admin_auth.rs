use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use std::sync::Arc;

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::utils::session_auth::SessionUser;

/// Middleware specifically for admin-only routes
pub async fn require_admin(
    State(_state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> AppResult<Response> {
    // Get session user from extensions (already authenticated by session middleware)
    let session_user = req
        .extensions()
        .get::<SessionUser>()
        .ok_or_else(|| AppError::Unauthorized("Not authenticated".to_string()))?;

    // Verify admin role
    if session_user.user.role != "owner" && session_user.user.role != "admin" {
        return Err(AppError::Unauthorized(
            "Admin access required".to_string(),
        ));
    }

    Ok(next.run(req).await)
}