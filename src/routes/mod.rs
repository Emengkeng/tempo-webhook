pub mod health;
pub mod subscriptions;
pub mod webhooks;
pub mod polar_webhooks;
pub mod auth;

use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

use crate::state::AppState;

pub fn create_router(state: Arc<AppState>) -> Router {
    // Public routes
    let public_routes = Router::new()
        .route("/health", get(health::health_check))
        .route("/webhooks/polar", post(polar_webhooks::handle_polar_webhook));

    // Protected API routes (require API key)
    let api_routes = Router::new()
        .route("/api/v1/subscriptions", get(subscriptions::list_subscriptions).post(subscriptions::create_subscription))
        .route("/api/v1/subscriptions/:id", get(subscriptions::get_subscription).delete(subscriptions::delete_subscription))
        .route("/api/v1/webhooks/logs", get(webhooks::list_webhook_logs))
        .route("/api/v1/usage", get(webhooks::get_usage_stats))
        .layer(middleware::from_fn_with_state(
            Arc::new(state.db.clone()),
            crate::utils::auth::authenticate_api_key,
        ));

    // Auth routes
    let auth_routes = Router::new()
        .route("/auth/register", post(auth::register))
        .route("/auth/login", post(auth::login))
        .route("/auth/api-keys", post(auth::create_api_key).get(auth::list_api_keys))
        .route("/auth/api-keys/:id", axum::routing::delete(auth::delete_api_key));

    // Combine routes
    Router::new()
        .merge(public_routes)
        .merge(api_routes)
        .merge(auth_routes)
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(state)
}
