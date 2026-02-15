use axum::{extract::State, Extension, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::error::AppResult;
use crate::models::SubscriptionPlan;
use crate::services::quota::PlanLimits;
use crate::state::AppState;
use crate::utils::auth::AuthenticatedUser;

#[derive(Debug, Serialize)]
pub struct PlanInfoResponse {
    pub current_plan: PlanDetails,
    pub usage: CurrentUsage,
    pub limits: PlanLimitDetails,
    pub available_plans: Vec<PlanOption>,
}

#[derive(Debug, Serialize)]
pub struct PlanDetails {
    pub tier: String,
    pub status: String,
    pub billing_cycle: Option<String>,
    pub price_usd: Option<f64>,
    pub current_period_start: Option<chrono::DateTime<chrono::Utc>>,
    pub current_period_end: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Serialize)]
pub struct CurrentUsage {
    pub active_subscriptions: i64,
    pub webhook_deliveries_this_month: i64,
    pub api_requests_this_minute: i64,
}

#[derive(Debug, Serialize)]
pub struct PlanLimitDetails {
    pub max_subscriptions: i64,
    pub max_webhook_deliveries: i64,
    pub max_api_requests_per_minute: i64,
    pub can_use_mainnet: bool,
    pub can_use_testnet: bool,
    pub max_filters_per_subscription: i64,
}

#[derive(Debug, Serialize)]
pub struct PlanOption {
    pub tier: String,
    pub name: String,
    pub price_usd: f64,
    pub billing_cycle: String,
    pub features: Vec<String>,
    pub limits: PlanLimitDetails,
}

pub async fn get_plan_info(
    State(state): State<Arc<AppState>>,
    Extension(session_user): Extension<crate::utils::session_auth::SessionUser>,
) -> AppResult<Json<PlanInfoResponse>> {
    // Get current plan
    let plan = SubscriptionPlan::get_by_organization(&state.db, session_user.organization_id)
        .await?
        .unwrap_or_else(|| SubscriptionPlan {
            id: uuid::Uuid::new_v4(),
            organization_id: session_user.organization_id,
            plan_tier: "free".to_string(),
            billing_cycle: None,
            price_usd: None,
            status: "active".to_string(),
            current_period_start: None,
            current_period_end: None,
            polar_subscription_id: None,
            created_at: chrono::Utc::now(),
        });

    let limits = PlanLimits::for_tier(&plan.plan_tier);

    // Get current usage
    let active_subscriptions = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM subscriptions WHERE organization_id = $1 AND active = true",
        session_user.organization_id
    )
    .fetch_one(&state.db)
    .await?
    .unwrap_or(0);

    let period_start = plan
        .current_period_start
        .unwrap_or_else(|| chrono::Utc::now());

    let webhook_deliveries = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) 
        FROM webhook_logs 
        WHERE organization_id = $1 
        AND first_attempt >= $2
        AND billable = true
        "#,
        session_user.organization_id,
        period_start
    )
    .fetch_one(&state.db)
    .await?
    .unwrap_or(0);

    // Check API rate limit usage
    let api_requests = check_current_api_usage(&state, session_user.organization_id).await?;

    // Build available plans
    let available_plans = vec![
        create_plan_option("free", 0.0),
        create_plan_option("starter", 29.0),
        create_plan_option("pro", 99.0),
        create_plan_option("enterprise", 0.0), // Contact sales
    ];

    Ok(Json(PlanInfoResponse {
        current_plan: PlanDetails {
            tier: plan.plan_tier.clone(),
            status: plan.status,
            billing_cycle: plan.billing_cycle,
            price_usd: plan.price_usd.map(|p| p.to_string().parse().unwrap_or(0.0)),
            current_period_start: plan.current_period_start,
            current_period_end: plan.current_period_end,
        },
        usage: CurrentUsage {
            active_subscriptions,
            webhook_deliveries_this_month: webhook_deliveries,
            api_requests_this_minute: api_requests,
        },
        limits: PlanLimitDetails {
            max_subscriptions: limits.max_subscriptions,
            max_webhook_deliveries: limits.max_webhook_deliveries,
            max_api_requests_per_minute: limits.max_api_requests_per_minute,
            can_use_mainnet: limits.can_use_mainnet,
            can_use_testnet: limits.can_use_testnet,
            max_filters_per_subscription: limits.max_filters_per_subscription,
        },
        available_plans,
    }))
}

fn create_plan_option(tier: &str, price: f64) -> PlanOption {
    let limits = PlanLimits::for_tier(tier);

    let (name, features) = match tier {
        "free" => (
            "Free".to_string(),
            vec![
                "Testnet only".to_string(),
                "10 subscriptions".to_string(),
                "1,000 webhooks/month".to_string(),
                "Community support".to_string(),
            ],
        ),
        "starter" => (
            "Starter".to_string(),
            vec![
                "Mainnet + Testnet".to_string(),
                "100 subscriptions".to_string(),
                "10,000 webhooks/month".to_string(),
                "Email support".to_string(),
                "Advanced filters".to_string(),
            ],
        ),
        "pro" => (
            "Pro".to_string(),
            vec![
                "Mainnet + Testnet".to_string(),
                "1,000 subscriptions".to_string(),
                "100,000 webhooks/month".to_string(),
                "Priority support".to_string(),
                "Advanced filters".to_string(),
                "Custom retention".to_string(),
            ],
        ),
        "enterprise" => (
            "Enterprise".to_string(),
            vec![
                "Unlimited subscriptions".to_string(),
                "Unlimited webhooks".to_string(),
                "Dedicated support".to_string(),
                "SLA guarantee".to_string(),
                "Custom features".to_string(),
                "On-premise option".to_string(),
            ],
        ),
        _ => ("Unknown".to_string(), vec![]),
    };

    PlanOption {
        tier: tier.to_string(),
        name,
        price_usd: price,
        billing_cycle: if price > 0.0 {
            "monthly".to_string()
        } else {
            "free".to_string()
        },
        features,
        limits: PlanLimitDetails {
            max_subscriptions: limits.max_subscriptions,
            max_webhook_deliveries: limits.max_webhook_deliveries,
            max_api_requests_per_minute: limits.max_api_requests_per_minute,
            can_use_mainnet: limits.can_use_mainnet,
            can_use_testnet: limits.can_use_testnet,
            max_filters_per_subscription: limits.max_filters_per_subscription,
        },
    }
}

async fn check_current_api_usage(
    state: &AppState,
    organization_id: uuid::Uuid,
) -> AppResult<i64> {
    use redis::AsyncCommands;

    let mut conn = state
        .redis
        .get_multiplexed_async_connection()
        .await
        .map_err(|e| crate::error::AppError::Internal(format!("Redis error: {}", e)))?;

    let key = format!(
        "rate:{}:{}",
        organization_id,
        chrono::Utc::now().timestamp() / 60
    );

    let count: Option<i64> = conn.get(&key).await.ok();

    Ok(count.unwrap_or(0))
}

#[derive(Debug, Serialize)]
pub struct QuotaWarning {
    pub warning_type: String,
    pub message: String,
    pub current_usage: i64,
    pub limit: i64,
    pub percentage_used: f64,
}

pub async fn check_quota_warnings(
    State(state): State<Arc<AppState>>,
    Extension(session_user): Extension<crate::utils::session_auth::SessionUser>,
) -> AppResult<Json<Vec<QuotaWarning>>> {
    let mut warnings = Vec::new();

    // Get plan
    let plan = crate::models::SubscriptionPlan::get_by_organization(
        &state.db,
        session_user.organization_id,
    )
    .await?
    .unwrap_or_else(|| {
        crate::models::SubscriptionPlan {
            id: uuid::Uuid::new_v4(),
            organization_id: session_user.organization_id,
            plan_tier: "free".to_string(),
            billing_cycle: None,
            price_usd: None,
            status: "active".to_string(),
            current_period_start: Some(chrono::Utc::now()),
            current_period_end: None,
            polar_subscription_id: None,
            created_at: chrono::Utc::now(),
        }
    });

    let limits = crate::services::quota::PlanLimits::for_organization(
        &state.db,
        session_user.organization_id,
        &plan.plan_tier,
    )
    .await?;

    let period_start = plan.current_period_start.unwrap_or_else(|| chrono::Utc::now());

    // Check webhook quota
    let webhook_usage = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) 
        FROM webhook_logs 
        WHERE organization_id = $1 
        AND first_attempt >= $2
        AND billable = true
        AND status != 'cancelled'
        "#,
        session_user.organization_id,
        period_start
    )
    .fetch_one(&state.db)
    .await?
    .unwrap_or(0);

    let webhook_percentage = (webhook_usage as f64 / limits.max_webhook_deliveries as f64) * 100.0;

    if webhook_percentage >= 80.0 {
        warnings.push(QuotaWarning {
            warning_type: "webhooks".to_string(),
            message: format!(
                "You're using {:.1}% of your monthly webhook quota",
                webhook_percentage
            ),
            current_usage: webhook_usage,
            limit: limits.max_webhook_deliveries,
            percentage_used: webhook_percentage,
        });
    }

    // Check subscription quota
    let subscription_count = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM subscriptions WHERE organization_id = $1 AND active = true",
        session_user.organization_id
    )
    .fetch_one(&state.db)
    .await?
    .unwrap_or(0);

    let subscription_percentage = (subscription_count as f64 / limits.max_subscriptions as f64) * 100.0;

    if subscription_percentage >= 80.0 {
        warnings.push(QuotaWarning {
            warning_type: "subscriptions".to_string(),
            message: format!(
                "You're using {:.1}% of your subscription limit",
                subscription_percentage
            ),
            current_usage: subscription_count,
            limit: limits.max_subscriptions,
            percentage_used: subscription_percentage,
        });
    }

    Ok(Json(warnings))
}
