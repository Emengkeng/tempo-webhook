pub mod health;
pub mod subscriptions;
pub mod webhooks;
pub mod polar_webhooks;
pub mod auth;
pub mod plans;
pub mod admin;

use axum::{
    Router, http::HeaderValue, middleware, routing::{delete, get, patch, post, put}
};
use reqwest::Method;
use std::sync::Arc;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tower_sessions::{SessionManagerLayer, Expiry, cookie::time::Duration};

use crate::state::AppState;

pub fn create_router(state: Arc<AppState>) -> Router {
    // Session layer for web routes
    let session_layer = SessionManagerLayer::new(state.session_store.clone())
        .with_expiry(Expiry::OnInactivity(Duration::new(60 * 60 * 24 * 7, 0))) // 7 days
        .with_secure(false) // Set to true in production with HTTPS
        .with_same_site(tower_sessions::cookie::SameSite::Lax);

    // Public routes
    let public_routes = Router::new()
        .route("/health", get(health::health_check))
        .route("/webhooks/polar", post(polar_webhooks::handle_polar_webhook))
        .route("/auth/register", post(auth::register))
        .route("/auth/login", post(auth::login))
        .route("/auth/verify-email", post(auth::verify_email));

    // Dashboard routes (session auth)
    let dashboard_routes = Router::new()
        .route("/auth/logout", post(auth::logout))
        .route("/api-keys", get(auth::list_api_keys).post(auth::create_api_key))
        .route("/api-keys/:id", delete(auth::delete_api_key))
        .route("/webhook-secret", get(auth::get_webhook_secret))
        .route("/subscriptions", get(subscriptions::list_subscriptions).post(subscriptions::create_subscription))
        .route("/subscriptions/:id", get(subscriptions::get_subscription)
            .patch(subscriptions::update_subscription)
            .delete(subscriptions::delete_subscription))
        .route("/webhooks/logs", get(webhooks::list_webhook_logs))
        .route("/usage", get(webhooks::get_usage_stats))
        .route("/plan", get(plans::get_plan_info))
        .route("/quota/warnings", get(plans::check_quota_warnings))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::utils::session_auth::authenticate_session,
        ))
        .layer(session_layer.clone());

    // Admin routes (session auth + admin role check)
    let admin_routes = Router::new()
        .route("/admin/custom-quotas", 
            get(admin::list_custom_quotas)
            .post(admin::create_custom_quota))
        .route("/admin/custom-quotas/:org_id",
            get(admin::get_custom_quota)
            .patch(admin::update_custom_quota)
            .delete(admin::delete_custom_quota))
        .route("/admin/organizations", get(admin::list_all_organizations))
        .route("/admin/organizations/:id/status", patch(admin::toggle_organization_status))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::utils::admin_auth::require_admin,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::utils::session_auth::authenticate_session,
        ))
        .layer(session_layer.clone());

    // API routes (API key auth)
    let api_routes = Router::new()
        .route("/api/v1/subscriptions", post(subscriptions::create_subscription))
        .route("/api/v1/subscriptions/:id", get(subscriptions::get_subscription))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::utils::auth::authenticate_api_key_with_state,
        ));

    let allowed_origins: Vec<HeaderValue> = state
        .config
        .allowed_origins
        .iter()
        .filter_map(|origin| origin.parse().ok())
        .collect();
    
    // Combine routes
    Router::new()
        .merge(public_routes)
        .merge(dashboard_routes)
        .merge(admin_routes)
        .merge(api_routes)
        .layer(
            CorsLayer::new()
                .allow_origin(
                    if cfg!(debug_assertions) && allowed_origins.is_empty() {
                        // Development mode with no explicit origins
                        AllowOrigin::any()
                    } else {
                        // Production or explicit origins set
                        AllowOrigin::list(allowed_origins)
                    }
                )
                .allow_methods([
                    Method::GET, 
                    Method::POST, 
                    Method::PATCH, 
                    Method::DELETE, 
                    Method::PUT
                ])
                .allow_headers([
                    axum::http::header::CONTENT_TYPE,
                    axum::http::header::AUTHORIZATION,
                    axum::http::header::ACCEPT,
                    "X-API-Key".parse().unwrap(),
                ])
                .allow_credentials(true),
        )
        .with_state(state)
}