pub mod health;
pub mod subscriptions;
pub mod webhooks;
pub mod polar_webhooks;
pub mod auth;

use axum::{
    middleware,
    routing::{get, post, delete, patch},
    Router,
};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

use crate::state::AppState;

pub fn create_router(state: Arc<AppState>) -> Router {
    // Public routes
    let public_routes = Router::new()
        .route("/health", get(health::health_check))
        .route("/webhooks/polar", post(polar_webhooks::handle_polar_webhook))
        .route("/auth/register", post(auth::register))
        .route("/auth/login", post(auth::login));

    // Protected API routes (require API key)
    let api_routes = Router::new()
        // Subscriptions
        .route(
            "/api/v1/subscriptions",
            get(subscriptions::list_subscriptions).post(subscriptions::create_subscription),
        )
        .route(
            "/api/v1/subscriptions/:id",
            get(subscriptions::get_subscription)
                .patch(subscriptions::update_subscription)
                .delete(subscriptions::delete_subscription),
        )
        // Webhooks
        .route("/api/v1/webhooks/logs", get(webhooks::list_webhook_logs))
        .route("/api/v1/usage", get(webhooks::get_usage_stats))
        // API Keys
        .route(
            "/api/v1/api-keys",
            get(auth::list_api_keys).post(auth::create_api_key),
        )
        .route("/api/v1/api-keys/:id", delete(auth::delete_api_key))
        .layer(middleware::from_fn_with_state(
            Arc::new(state.db.clone()),
            crate::utils::auth::authenticate_api_key,
        ));

    // Combine routes
    Router::new()
        .merge(public_routes)
        .merge(api_routes)
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(state)
}
