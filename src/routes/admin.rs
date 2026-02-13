use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::utils::auth::AuthenticatedUser;

/// Custom quota overrides for enterprise customers
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CustomQuota {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub max_subscriptions: Option<i64>,
    pub max_webhook_deliveries: Option<i64>,
    pub max_api_requests_per_minute: Option<i64>,
    pub can_use_mainnet: Option<bool>,
    pub can_use_testnet: Option<bool>,
    pub max_filters_per_subscription: Option<i64>,
    pub custom_features: Option<serde_json::Value>,
    pub notes: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateCustomQuotaRequest {
    pub organization_id: Uuid,
    pub max_subscriptions: Option<i64>,
    pub max_webhook_deliveries: Option<i64>,
    pub max_api_requests_per_minute: Option<i64>,
    pub can_use_mainnet: Option<bool>,
    pub can_use_testnet: Option<bool>,
    pub max_filters_per_subscription: Option<i64>,
    pub custom_features: Option<serde_json::Value>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCustomQuotaRequest {
    pub max_subscriptions: Option<i64>,
    pub max_webhook_deliveries: Option<i64>,
    pub max_api_requests_per_minute: Option<i64>,
    pub can_use_mainnet: Option<bool>,
    pub can_use_testnet: Option<bool>,
    pub max_filters_per_subscription: Option<i64>,
    pub custom_features: Option<serde_json::Value>,
    pub notes: Option<String>,
}

/// Admin-only: Create custom quota for enterprise customer
pub async fn create_custom_quota(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthenticatedUser>,
    Json(payload): Json<CreateCustomQuotaRequest>,
) -> AppResult<(StatusCode, Json<CustomQuota>)> {
    // Verify admin access
    verify_admin_access(&auth)?;

    // Verify organization exists
    crate::models::Organization::get_by_id(&state.db, payload.organization_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Organization not found".to_string()))?;

    let quota = sqlx::query_as!(
        CustomQuota,
        r#"
        INSERT INTO custom_quotas (
            organization_id, max_subscriptions, max_webhook_deliveries,
            max_api_requests_per_minute, can_use_mainnet, can_use_testnet,
            max_filters_per_subscription, custom_features, notes
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        RETURNING 
            id, organization_id, max_subscriptions, max_webhook_deliveries,
            max_api_requests_per_minute, can_use_mainnet, can_use_testnet,
            max_filters_per_subscription, custom_features, notes,
            created_at, updated_at
        "#,
        payload.organization_id,
        payload.max_subscriptions,
        payload.max_webhook_deliveries,
        payload.max_api_requests_per_minute,
        payload.can_use_mainnet,
        payload.can_use_testnet,
        payload.max_filters_per_subscription,
        payload.custom_features,
        payload.notes
    )
    .fetch_one(&state.db)
    .await?;

    Ok((StatusCode::CREATED, Json(quota)))
}

/// Admin-only: Update custom quota
pub async fn update_custom_quota(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthenticatedUser>,
    Path(org_id): Path<Uuid>,
    Json(payload): Json<UpdateCustomQuotaRequest>,
) -> AppResult<Json<CustomQuota>> {
    // Verify admin access
    verify_admin_access(&auth)?;

    // Get existing quota
    let existing = get_custom_quota_by_org(&state.db, org_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Custom quota not found".to_string()))?;

    let quota = sqlx::query_as!(
        CustomQuota,
        r#"
        UPDATE custom_quotas
        SET 
            max_subscriptions = COALESCE($2, max_subscriptions),
            max_webhook_deliveries = COALESCE($3, max_webhook_deliveries),
            max_api_requests_per_minute = COALESCE($4, max_api_requests_per_minute),
            can_use_mainnet = COALESCE($5, can_use_mainnet),
            can_use_testnet = COALESCE($6, can_use_testnet),
            max_filters_per_subscription = COALESCE($7, max_filters_per_subscription),
            custom_features = COALESCE($8, custom_features),
            notes = COALESCE($9, notes),
            updated_at = NOW()
        WHERE organization_id = $1
        RETURNING 
            id, organization_id, max_subscriptions, max_webhook_deliveries,
            max_api_requests_per_minute, can_use_mainnet, can_use_testnet,
            max_filters_per_subscription, custom_features, notes,
            created_at, updated_at
        "#,
        org_id,
        payload.max_subscriptions,
        payload.max_webhook_deliveries,
        payload.max_api_requests_per_minute,
        payload.can_use_mainnet,
        payload.can_use_testnet,
        payload.max_filters_per_subscription,
        payload.custom_features,
        payload.notes
    )
    .fetch_one(&state.db)
    .await?;

    Ok(Json(quota))
}

/// Admin-only: Get custom quota for organization
pub async fn get_custom_quota(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthenticatedUser>,
    Path(org_id): Path<Uuid>,
) -> AppResult<Json<CustomQuota>> {
    // Verify admin access
    verify_admin_access(&auth)?;

    let quota = get_custom_quota_by_org(&state.db, org_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Custom quota not found".to_string()))?;

    Ok(Json(quota))
}

/// Admin-only: List all custom quotas
pub async fn list_custom_quotas(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthenticatedUser>,
) -> AppResult<Json<Vec<CustomQuota>>> {
    // Verify admin access
    verify_admin_access(&auth)?;

    let quotas = sqlx::query_as!(
        CustomQuota,
        r#"
        SELECT 
            id, organization_id, max_subscriptions, max_webhook_deliveries,
            max_api_requests_per_minute, can_use_mainnet, can_use_testnet,
            max_filters_per_subscription, custom_features, notes,
            created_at, updated_at
        FROM custom_quotas
        ORDER BY created_at DESC
        "#
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(quotas))
}

/// Admin-only: Delete custom quota (revert to plan defaults)
pub async fn delete_custom_quota(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthenticatedUser>,
    Path(org_id): Path<Uuid>,
) -> AppResult<StatusCode> {
    // Verify admin access
    verify_admin_access(&auth)?;

    sqlx::query!(
        "DELETE FROM custom_quotas WHERE organization_id = $1",
        org_id
    )
    .execute(&state.db)
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

// Helper functions

async fn get_custom_quota_by_org(
    db: &sqlx::PgPool,
    org_id: Uuid,
) -> Result<Option<CustomQuota>, sqlx::Error> {
    sqlx::query_as!(
        CustomQuota,
        r#"
        SELECT 
            id, organization_id, max_subscriptions, max_webhook_deliveries,
            max_api_requests_per_minute, can_use_mainnet, can_use_testnet,
            max_filters_per_subscription, custom_features, notes,
            created_at, updated_at
        FROM custom_quotas
        WHERE organization_id = $1
        "#,
        org_id
    )
    .fetch_optional(db)
    .await
}

fn verify_admin_access(auth: &AuthenticatedUser) -> AppResult<()> {
    // Check if user has admin role
    if auth.user.role != "owner" && auth.user.role != "admin" {
        return Err(AppError::Unauthorized(
            "Admin access required".to_string(),
        ));
    }

    // TODO: Add additional admin verification
    // might want a separate admin_users table
    // or check against a list of admin emails/organizations

    Ok(())
}

/// Public function to get effective limits for an organization
/// This checks custom quotas first, then falls back to plan limits
pub async fn get_effective_limits(
    db: &sqlx::PgPool,
    org_id: uuid::Uuid,
) -> crate::error::AppResult<crate::services::quota::PlanLimits> {
    // Check for custom quota first
    if let Some(custom) = get_custom_quota_by_org(db, org_id).await? {
        // Custom quota overrides plan limits
        return Ok(crate::services::quota::PlanLimits {
            max_subscriptions: custom.max_subscriptions.unwrap_or(i64::MAX),
            max_webhook_deliveries: custom.max_webhook_deliveries.unwrap_or(i64::MAX),
            max_api_requests_per_minute: custom
                .max_api_requests_per_minute
                .unwrap_or(10_000),
            can_use_mainnet: custom.can_use_mainnet.unwrap_or(true),
            can_use_testnet: custom.can_use_testnet.unwrap_or(true),
            max_filters_per_subscription: custom
                .max_filters_per_subscription
                .unwrap_or(50),
        });
    }

    // Fall back to plan limits
    let plan = crate::models::SubscriptionPlan::get_by_organization(db, org_id).await?;

    let tier = plan
        .as_ref()
        .map(|p| p.plan_tier.as_str())
        .unwrap_or("free");

    Ok(crate::services::quota::PlanLimits::for_tier(tier))
}
