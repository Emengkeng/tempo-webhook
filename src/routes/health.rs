use axum::{extract::State, Json};
use serde_json::{json, Value};
use std::sync::Arc;

use crate::state::AppState;

pub async fn health_check(State(state): State<Arc<AppState>>) -> Json<Value> {
    let mut checks = Vec::new();

    // Check database
    let db_healthy = sqlx::query("SELECT 1")
        .fetch_one(&state.db)
        .await
        .is_ok();
    checks.push(("database", db_healthy));

    // Check Redis
    let redis_healthy = state
        .redis
        .get_multiplexed_async_connection()
        .await
        .is_ok();
    checks.push(("redis", redis_healthy));

    // Check NATS
    let nats_healthy = !state.nats.is_closed();
    checks.push(("nats", nats_healthy));

    let all_healthy = checks.iter().all(|(_, status)| *status);

    Json(json!({
        "status": if all_healthy { "healthy" } else { "unhealthy" },
        "checks": checks.iter().map(|(name, status)| {
            json!({
                "name": name,
                "status": if *status { "up" } else { "down" }
            })
        }).collect::<Vec<_>>(),
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "version": env!("CARGO_PKG_VERSION"),
    }))
}
