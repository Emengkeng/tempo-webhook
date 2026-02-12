use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::error::AppResult;
use crate::models::{Organization, SubscriptionPlan};
use crate::state::AppState;
use crate::utils::crypto::verify_webhook_signature;

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum PolarWebhookEvent {
    #[serde(rename = "subscription.created")]
    SubscriptionCreated { data: PolarSubscription },
    #[serde(rename = "subscription.updated")]
    SubscriptionUpdated { data: PolarSubscription },
    #[serde(rename = "subscription.active")]
    SubscriptionActive { data: PolarSubscription },
    #[serde(rename = "subscription.canceled")]
    SubscriptionCanceled { data: PolarSubscription },
    #[serde(rename = "subscription.revoked")]
    SubscriptionRevoked { data: PolarSubscription },
    #[serde(rename = "customer.created")]
    CustomerCreated { data: Value },
    #[serde(rename = "order.created")]
    OrderCreated { data: Value },
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PolarSubscription {
    pub id: String,
    pub status: String,
    pub customer_id: String,
    pub product_id: String,
    pub amount: i64,
    pub currency: String,
    pub current_period_start: String,
    pub current_period_end: String,
    pub cancel_at_period_end: bool,
    pub metadata: Option<Value>,
}

pub async fn handle_polar_webhook(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> AppResult<StatusCode> {
    // Get signature from header
    let signature = headers
        .get("webhook-signature")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| crate::error::AppError::Unauthorized("Missing signature".to_string()))?;

    // Verify signature
    let body_str = String::from_utf8(body.to_vec())
        .map_err(|_| crate::error::AppError::BadRequest("Invalid body".to_string()))?;

    let is_valid = verify_webhook_signature(&body_str, signature, &state.config.polar_webhook_secret)
        .map_err(|e| {
            error!("Signature verification failed: {}", e);
            crate::error::AppError::Unauthorized("Invalid signature".to_string())
        })?;

    if !is_valid {
        warn!("Invalid Polar webhook signature");
        return Err(crate::error::AppError::Unauthorized(
            "Invalid signature".to_string(),
        ));
    }

    // Parse webhook event
    let event: PolarWebhookEvent = serde_json::from_str(&body_str)
        .map_err(|e| {
            error!("Failed to parse Polar webhook: {}", e);
            crate::error::AppError::BadRequest("Invalid webhook payload".to_string())
        })?;

    // Process event
    match event {
        PolarWebhookEvent::SubscriptionCreated { data } => {
            handle_subscription_created(&state, data).await?;
        }
        PolarWebhookEvent::SubscriptionUpdated { data } => {
            handle_subscription_updated(&state, data).await?;
        }
        PolarWebhookEvent::SubscriptionActive { data } => {
            handle_subscription_active(&state, data).await?;
        }
        PolarWebhookEvent::SubscriptionCanceled { data } => {
            handle_subscription_canceled(&state, data).await?;
        }
        PolarWebhookEvent::SubscriptionRevoked { data } => {
            handle_subscription_revoked(&state, data).await?;
        }
        PolarWebhookEvent::CustomerCreated { data } => {
            info!("Customer created event received: {:?}", data);
        }
        PolarWebhookEvent::OrderCreated { data } => {
            info!("Order created event received: {:?}", data);
        }
    }

    Ok(StatusCode::OK)
}

async fn handle_subscription_created(
    state: &AppState,
    subscription: PolarSubscription,
) -> AppResult<()> {
    info!("Handling subscription.created for subscription: {}", subscription.id);

    // Extract organization ID from metadata
    let org_id = subscription
        .metadata
        .as_ref()
        .and_then(|m| m.get("organization_id"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            error!("Missing organization_id in subscription metadata");
            crate::error::AppError::BadRequest("Missing organization_id".to_string())
        })?;

    let org_uuid = uuid::Uuid::parse_str(org_id)
        .map_err(|_| crate::error::AppError::BadRequest("Invalid organization_id".to_string()))?;

    // Determine plan tier based on product or amount
    let plan_tier = determine_plan_tier(subscription.amount);

    // Create or update subscription plan
    SubscriptionPlan::create_or_update(
        &state.db,
        org_uuid,
        plan_tier,
        "active".to_string(),
        Some(subscription.id.clone()),
    )
    .await?;

    info!("Subscription plan created/updated for organization: {}", org_id);

    Ok(())
}

async fn handle_subscription_updated(
    state: &AppState,
    subscription: PolarSubscription,
) -> AppResult<()> {
    info!("Handling subscription.updated for subscription: {}", subscription.id);

    // Find organization by polar subscription ID
    let plan = sqlx::query_as!(
        SubscriptionPlan,
        r#"
        SELECT 
            id, organization_id, plan_tier, billing_cycle, price_usd,
            status, current_period_start, current_period_end,
            polar_subscription_id, created_at
        FROM subscription_plans
        WHERE polar_subscription_id = $1
        "#,
        subscription.id
    )
    .fetch_optional(&state.db)
    .await?;

    if let Some(plan) = plan {
        let new_tier = determine_plan_tier(subscription.amount);
        let new_status = map_polar_status(&subscription.status);

        SubscriptionPlan::create_or_update(
            &state.db,
            plan.organization_id,
            new_tier,
            new_status,
            Some(subscription.id),
        )
        .await?;

        info!("Subscription plan updated for organization: {}", plan.organization_id);
    } else {
        warn!("Subscription plan not found for polar ID: {}", subscription.id);
    }

    Ok(())
}

async fn handle_subscription_active(
    state: &AppState,
    subscription: PolarSubscription,
) -> AppResult<()> {
    info!("Handling subscription.active for subscription: {}", subscription.id);

    let plan = sqlx::query_as!(
        SubscriptionPlan,
        r#"
        SELECT 
            id, organization_id, plan_tier, billing_cycle, price_usd,
            status, current_period_start, current_period_end,
            polar_subscription_id, created_at
        FROM subscription_plans
        WHERE polar_subscription_id = $1
        "#,
        subscription.id
    )
    .fetch_optional(&state.db)
    .await?;

    if let Some(plan) = plan {
        sqlx::query!(
            "UPDATE subscription_plans SET status = 'active' WHERE id = $1",
            plan.id
        )
        .execute(&state.db)
        .await?;

        info!("Subscription activated for organization: {}", plan.organization_id);
    }

    Ok(())
}

async fn handle_subscription_canceled(
    state: &AppState,
    subscription: PolarSubscription,
) -> AppResult<()> {
    info!("Handling subscription.canceled for subscription: {}", subscription.id);

    sqlx::query!(
        "UPDATE subscription_plans SET status = 'cancelled' WHERE polar_subscription_id = $1",
        subscription.id
    )
    .execute(&state.db)
    .await?;

    Ok(())
}

async fn handle_subscription_revoked(
    state: &AppState,
    subscription: PolarSubscription,
) -> AppResult<()> {
    info!("Handling subscription.revoked for subscription: {}", subscription.id);

    sqlx::query!(
        "UPDATE subscription_plans SET status = 'cancelled' WHERE polar_subscription_id = $1",
        subscription.id
    )
    .execute(&state.db)
    .await?;

    // Optionally deactivate all subscriptions for this organization
    let plan = sqlx::query_as!(
        SubscriptionPlan,
        r#"
        SELECT 
            id, organization_id, plan_tier, billing_cycle, price_usd,
            status, current_period_start, current_period_end,
            polar_subscription_id, created_at
        FROM subscription_plans
        WHERE polar_subscription_id = $1
        "#,
        subscription.id
    )
    .fetch_optional(&state.db)
    .await?;

    if let Some(plan) = plan {
        sqlx::query!(
            "UPDATE subscriptions SET active = false WHERE organization_id = $1",
            plan.organization_id
        )
        .execute(&state.db)
        .await?;

        info!("All subscriptions deactivated for organization: {}", plan.organization_id);
    }

    Ok(())
}

fn determine_plan_tier(amount: i64) -> String {
    match amount {
        0..=0 => "free".to_string(),
        1..=2900 => "starter".to_string(),
        2901..=9900 => "pro".to_string(),
        _ => "enterprise".to_string(),
    }
}

fn map_polar_status(status: &str) -> String {
    match status {
        "active" => "active".to_string(),
        "canceled" | "cancelled" => "cancelled".to_string(),
        "past_due" => "past_due".to_string(),
        "trialing" => "trialing".to_string(),
        _ => "active".to_string(),
    }
}
