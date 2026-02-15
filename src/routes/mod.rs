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
        .with_expiry(Expiry::OnInactivity(Duration::new(60 * 60 * 24 * 7,0)))
        .with_secure(false)
        .with_same_site(tower_sessions::cookie::SameSite::Lax);

    // CORS for dashboard (strict - only allowed origins)
    let dashboard_cors = {
        let allowed_origins: Vec<HeaderValue> = state
            .config
            .allowed_origins
            .iter()
            .filter_map(|origin| origin.parse().ok())
            .collect();

        CorsLayer::new()
            .allow_origin(
                if cfg!(debug_assertions) && allowed_origins.is_empty() {
                    AllowOrigin::any()
                } else {
                    AllowOrigin::list(allowed_origins)
                }
            )
            .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE, Method::PUT, Method::OPTIONS])
            .allow_headers([
                axum::http::header::CONTENT_TYPE,
                axum::http::header::AUTHORIZATION,
                axum::http::header::ACCEPT,
            ])
            .allow_credentials(true)
    };

    // CORS for API (permissive - any origin with API key)
    let api_cors = CorsLayer::new()
        .allow_origin(AllowOrigin::any())
        .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE, Method::PUT])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::AUTHORIZATION,
            axum::http::header::ACCEPT,
            "X-API-Key".parse::<axum::http::header::HeaderName>().unwrap(),
        ])
        .allow_credentials(false); // No credentials for API routes

    // Public routes (permissive CORS)
    let public_routes = Router::new()
        .route("/health", get(health::health_check))
        .route("/auth/register", post(auth::register))
        .route("/auth/login", post(auth::login))
        .route("/auth/verify-email", post(auth::verify_email))
        .layer(session_layer.clone())
        .layer(dashboard_cors.clone());

    // Webhook callback routes (permissive)
    let webhook_callback_routes = Router::new()
        .route("/webhooks/polar", post(polar_webhooks::handle_polar_webhook))
        .layer(api_cors.clone());

    // Dashboard routes (strict CORS with credentials)
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
        .layer(session_layer.clone())
        .layer(dashboard_cors.clone());

    // Admin routes (strict CORS with credentials)
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
            crate::utils::session_auth::authenticate_session,
        ))
        .layer(session_layer)
        .layer(dashboard_cors.clone());

    // API routes (permissive CORS, API key auth)
    let api_routes = Router::new()
        .route("/api/v1/subscriptions", 
            get(subscriptions::list_subscriptions)
            .post(subscriptions::create_subscription))
        .route("/api/v1/subscriptions/:id", 
            get(subscriptions::get_subscription)
            .patch(subscriptions::update_subscription)
            .delete(subscriptions::delete_subscription))
        .route("/api/v1/webhooks/logs", get(webhooks::list_webhook_logs))
        .route("/api/v1/usage", get(webhooks::get_usage_stats))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::utils::auth::authenticate_api_key_with_state,
        ))
        .layer(api_cors);

    // Combine routes (NO global CORS layer)
    Router::new()
        .merge(public_routes)
        .merge(dashboard_routes)
        .merge(webhook_callback_routes)
        .merge(admin_routes)
        .merge(api_routes)
        .with_state(state)
}