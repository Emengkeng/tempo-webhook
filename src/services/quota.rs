use crate::error::{AppError, AppResult};
use crate::models::SubscriptionPlan;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct PlanLimits {
    pub max_subscriptions: i64,
    pub max_webhook_deliveries: i64,
    pub max_api_requests_per_minute: i64,
    pub can_use_mainnet: bool,
    pub can_use_testnet: bool,
    pub max_filters_per_subscription: i64,
}

impl PlanLimits {
    pub fn for_tier(tier: &str) -> Self {
        match tier {
            "free" => Self {
                max_subscriptions: 10,
                max_webhook_deliveries: 1_000,
                max_api_requests_per_minute: 10,
                can_use_mainnet: false,
                can_use_testnet: true,
                max_filters_per_subscription: 3,
            },
            "starter" => Self {
                max_subscriptions: 100,
                max_webhook_deliveries: 10_000,
                max_api_requests_per_minute: 100,
                can_use_mainnet: true,
                can_use_testnet: true,
                max_filters_per_subscription: 5,
            },
            "pro" => Self {
                max_subscriptions: 1_000,
                max_webhook_deliveries: 100_000,
                max_api_requests_per_minute: 1_000,
                can_use_mainnet: true,
                can_use_testnet: true,
                max_filters_per_subscription: 10,
            },
            "enterprise" => Self {
                max_subscriptions: i64::MAX,
                max_webhook_deliveries: i64::MAX,
                max_api_requests_per_minute: 10_000,
                can_use_mainnet: true,
                can_use_testnet: true,
                max_filters_per_subscription: 20,
            },
            _ => Self::for_tier("free"),
        }
    }
}

pub async fn check_subscription_quota(
    db: &PgPool,
    organization_id: Uuid,
    network: &str,
) -> AppResult<()> {
    // Get organization's plan
    let plan = SubscriptionPlan::get_by_organization(db, organization_id)
        .await?
        .unwrap_or_else(|| {
            // Default to free plan if not set
            SubscriptionPlan {
                id: Uuid::new_v4(),
                organization_id,
                plan_tier: "free".to_string(),
                billing_cycle: None,
                price_usd: None,
                status: "active".to_string(),
                current_period_start: None,
                current_period_end: None,
                polar_subscription_id: None,
                created_at: chrono::Utc::now(),
            }
        });

    // Check if plan is active
    if plan.status != "active" && plan.status != "trialing" {
        return Err(AppError::BadRequest(format!(
            "Your subscription is {}. Please update your payment method.",
            plan.status
        )));
    }

    let limits = PlanLimits::for_tier(&plan.plan_tier);

    // Check network access
    if network == "mainnet" && !limits.can_use_mainnet {
        return Err(AppError::BadRequest(
            "Your plan does not include mainnet access. Please upgrade to Starter or higher."
                .to_string(),
        ));
    }

    // Count active subscriptions
    let current_count = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM subscriptions WHERE organization_id = $1 AND active = true",
        organization_id
    )
    .fetch_one(db)
    .await?
    .unwrap_or(0);

    if current_count >= limits.max_subscriptions {
        return Err(AppError::BadRequest(format!(
            "Subscription limit reached. Your {} plan allows {} subscriptions. Please upgrade or remove inactive subscriptions.",
            plan.plan_tier, limits.max_subscriptions
        )));
    }

    Ok(())
}

pub async fn check_webhook_quota(
    db: &PgPool,
    organization_id: Uuid,
) -> AppResult<()> {
    // Get organization's plan
    let plan = SubscriptionPlan::get_by_organization(db, organization_id)
        .await?
        .unwrap_or_else(|| {
            SubscriptionPlan {
                id: Uuid::new_v4(),
                organization_id,
                plan_tier: "free".to_string(),
                billing_cycle: None,
                price_usd: None,
                status: "active".to_string(),
                current_period_start: None,
                current_period_end: None,
                polar_subscription_id: None,
                created_at: chrono::Utc::now(),
            }
        });

    let limits = PlanLimits::for_tier(&plan.plan_tier);

    // Get current period usage
    let period_start = plan
        .current_period_start
        .unwrap_or_else(|| chrono::Utc::now());

    let current_usage = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) 
        FROM webhook_logs 
        WHERE organization_id = $1 
        AND first_attempt >= $2
        AND billable = true
        "#,
        organization_id,
        period_start
    )
    .fetch_one(db)
    .await?
    .unwrap_or(0);

    if current_usage >= limits.max_webhook_deliveries {
        return Err(AppError::BadRequest(format!(
            "Webhook delivery quota exceeded. Your {} plan allows {} deliveries per month. Current usage: {}.",
            plan.plan_tier, limits.max_webhook_deliveries, current_usage
        )));
    }

    Ok(())
}

pub async fn check_filter_quota(
    filter_count: usize,
    db: &PgPool,
    organization_id: Uuid,
) -> AppResult<()> {
    let plan = SubscriptionPlan::get_by_organization(db, organization_id)
        .await?
        .unwrap_or_else(|| {
            SubscriptionPlan {
                id: Uuid::new_v4(),
                organization_id,
                plan_tier: "free".to_string(),
                billing_cycle: None,
                price_usd: None,
                status: "active".to_string(),
                current_period_start: None,
                current_period_end: None,
                polar_subscription_id: None,
                created_at: chrono::Utc::now(),
            }
        });

    let limits = PlanLimits::for_tier(&plan.plan_tier);

    if filter_count as i64 > limits.max_filters_per_subscription {
        return Err(AppError::BadRequest(format!(
            "Too many filters. Your {} plan allows {} filters per subscription.",
            plan.plan_tier, limits.max_filters_per_subscription
        )));
    }

    Ok(())
}

pub async fn check_api_rate_limit(
    redis: &redis::Client,
    organization_id: Uuid,
    db: &PgPool,
) -> AppResult<()> {
    use redis::AsyncCommands;

    let plan = SubscriptionPlan::get_by_organization(db, organization_id)
        .await?
        .unwrap_or_else(|| {
            SubscriptionPlan {
                id: Uuid::new_v4(),
                organization_id,
                plan_tier: "free".to_string(),
                billing_cycle: None,
                price_usd: None,
                status: "active".to_string(),
                current_period_start: None,
                current_period_end: None,
                polar_subscription_id: None,
                created_at: chrono::Utc::now(),
            }
        });

    let limits = PlanLimits::for_tier(&plan.plan_tier);

    let mut conn = redis
        .get_multiplexed_async_connection()
        .await
        .map_err(|e| AppError::Internal(format!("Redis error: {}", e)))?;

    // Use a 1-minute sliding window
    let key = format!("rate:{}:{}", organization_id, chrono::Utc::now().timestamp() / 60);

    let count: i64 = conn
        .incr(&key, 1)
        .await
        .map_err(|e| AppError::Internal(format!("Redis error: {}", e)))?;

    // Set expiry on first increment
    if count == 1 {
        let _: () = conn
            .expire(&key, 60)
            .await
            .map_err(|e| AppError::Internal(format!("Redis error: {}", e)))?;
    }

    if count > limits.max_api_requests_per_minute {
        return Err(AppError::RateLimitExceeded);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_limits() {
        let free = PlanLimits::for_tier("free");
        assert_eq!(free.max_subscriptions, 10);
        assert!(!free.can_use_mainnet);
        assert!(free.can_use_testnet);

        let starter = PlanLimits::for_tier("starter");
        assert_eq!(starter.max_subscriptions, 100);
        assert!(starter.can_use_mainnet);

        let pro = PlanLimits::for_tier("pro");
        assert_eq!(pro.max_subscriptions, 1_000);

        let enterprise = PlanLimits::for_tier("enterprise");
        assert_eq!(enterprise.max_subscriptions, i64::MAX);
    }
}
