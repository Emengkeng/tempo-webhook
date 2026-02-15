use axum::{
    extract::{Query, State},
    Extension, Json,
};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::error::AppResult;
use crate::models::{WebhookLog, WebhookLogResponse};
use crate::state::AppState;
use crate::utils::auth::AuthenticatedUser;

#[derive(Debug, Deserialize)]
pub struct WebhookLogsQuery {
    subscription_id: Option<Uuid>,
    #[serde(default = "default_limit")]
    limit: i64,
    #[serde(default)]
    offset: i64,
}

fn default_limit() -> i64 {
    50
}

#[derive(Debug, Serialize)]
pub struct WebhookLogsResponse {
    logs: Vec<WebhookLogResponse>,
    total: usize,
    limit: i64,
    offset: i64,
    has_more: bool,
}

pub async fn list_webhook_logs(
    State(state): State<Arc<AppState>>,
    Extension(session_user): Extension<crate::utils::session_auth::SessionUser>,
    Query(query): Query<WebhookLogsQuery>,
) -> AppResult<Json<WebhookLogsResponse>> {
    let limit = query.limit.min(100);
    let offset = query.offset;

    let logs = if let Some(subscription_id) = query.subscription_id {
        WebhookLog::list_by_subscription(&state.db, subscription_id, limit, offset).await?
    } else {
        WebhookLog::list_by_organization(
            &state.db,
            session_user.organization_id,
            limit,
            offset,
        )
        .await?
    };

    let total = logs.len();
    let has_more = total as i64 == limit;

    let log_responses: Vec<WebhookLogResponse> = logs.into_iter().map(Into::into).collect();

    Ok(Json(WebhookLogsResponse {
        logs: log_responses,
        total,
        limit,
        offset,
        has_more,
    }))
}

#[derive(Debug, Deserialize)]
pub struct UsageStatsQuery {
    start_date: Option<String>,
    end_date: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UsageStatsResponse {
    period: Period,
    usage: Usage,
    quota: Quota,
    plan: String,
}

#[derive(Debug, Serialize)]
pub struct Period {
    start: String,
    end: String,
}

#[derive(Debug, Serialize)]
pub struct Usage {
    webhook_deliveries: i64,
    api_requests: i64,
    active_subscriptions: i64,
}

#[derive(Debug, Serialize)]
pub struct Quota {
    webhook_deliveries: i64,
    overage: i64,
    overage_cost: f64,
}

pub async fn get_usage_stats(
    State(state): State<Arc<AppState>>,
    Extension(session_user): Extension<crate::utils::session_auth::SessionUser>,
    Query(query): Query<UsageStatsQuery>,
) -> AppResult<Json<UsageStatsResponse>> {
    // Parse dates or use current month
    let end_date = if let Some(end) = query.end_date {
        NaiveDate::parse_from_str(&end, "%Y-%m-%d")
            .map_err(|_| crate::error::AppError::BadRequest("Invalid end_date format".to_string()))?
    } else {
        Utc::now().date_naive()
    };

    let start_date = if let Some(start) = query.start_date {
        NaiveDate::parse_from_str(&start, "%Y-%m-%d")
            .map_err(|_| crate::error::AppError::BadRequest("Invalid start_date format".to_string()))?
    } else {
        NaiveDate::from_ymd_opt(end_date.year() as i32, end_date.month(), 1).unwrap()
    };

    // Get subscription plan
    let plan = crate::models::SubscriptionPlan::get_by_organization(
        &state.db,
        session_user.organization_id,
    )
    .await?;

    let plan_tier = plan.as_ref().map(|p| p.plan_tier.clone()).unwrap_or_else(|| "free".to_string());

    // Query usage data
    let usage_data = sqlx::query!(
        r#"
        SELECT 
            COUNT(*) FILTER (WHERE billable = true AND status != 'cancelled') as total_webhooks,
            0 as total_api_requests,
            (
                SELECT COUNT(*) 
                FROM subscriptions 
                WHERE organization_id = $1 AND active = true
            ) as max_subscriptions
        FROM webhook_logs 
        WHERE organization_id = $1 
        AND first_attempt BETWEEN $2 AND $3
        "#,
        session_user.organization_id,
        start_date.and_hms_opt(0, 0, 0).unwrap().and_utc(),
        end_date.and_hms_opt(23, 59, 59).unwrap().and_utc()
    )
    .fetch_one(&state.db)
    .await?;

    let webhook_deliveries = usage_data.total_webhooks.unwrap_or(0);
    let api_requests = usage_data.total_api_requests.unwrap_or(0);
    let active_subscriptions = usage_data.max_subscriptions.unwrap_or(0) as i64;

    // Calculate quota based on plan
    let (quota_webhooks, overage_rate) = match plan_tier.as_str() {
        "free" => (1_000, 0.0),
        "starter" => (10_000, 0.01),
        "pro" => (100_000, 0.005),
        "enterprise" => (1_000_000, 0.001),
        _ => (1_000, 0.0),
    };

    let overage = (webhook_deliveries - quota_webhooks).max(0);
    let overage_cost = (overage as f64 / 1000.0) * overage_rate;

    Ok(Json(UsageStatsResponse {
        period: Period {
            start: start_date.to_string(),
            end: end_date.to_string(),
        },
        usage: Usage {
            webhook_deliveries,
            api_requests,
            active_subscriptions,
        },
        quota: Quota {
            webhook_deliveries: quota_webhooks,
            overage,
            overage_cost,
        },
        plan: plan_tier,
    }))
}
