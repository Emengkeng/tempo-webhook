use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::{error::{AppError, AppResult}, models::Organization, utils::SessionUser};
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
    Extension(session_user): Extension<crate::utils::session_auth::SessionUser>,
    Json(payload): Json<CreateCustomQuotaRequest>,
) -> AppResult<(StatusCode, Json<CustomQuota>)> {
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
    Extension(session_user): Extension<SessionUser>,
    Path(org_id): Path<Uuid>,
    Json(payload): Json<UpdateCustomQuotaRequest>,
) -> AppResult<Json<CustomQuota>> {
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
    Extension(session_user): Extension<SessionUser>,
    Path(org_id): Path<Uuid>,
) -> AppResult<Json<CustomQuota>> {
    let quota = get_custom_quota_by_org(&state.db, org_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Custom quota not found".to_string()))?;

    Ok(Json(quota))
}

/// Admin-only: List all custom quotas
pub async fn list_custom_quotas(
    State(state): State<Arc<AppState>>,
    Extension(session_user): Extension<SessionUser>,
) -> AppResult<Json<Vec<CustomQuota>>> {
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
    Extension(session_user): Extension<SessionUser>,
    Path(org_id): Path<Uuid>,
) -> AppResult<StatusCode> {
    sqlx::query!(
        "DELETE FROM custom_quotas WHERE organization_id = $1",
        org_id
    )
    .execute(&state.db)
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Serialize)]
pub struct OrganizationListItem {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub org_type: String,
    pub active: bool,
    pub plan_tier: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub user_count: i64,
    pub subscription_count: i64,
}

pub async fn list_all_organizations(
    State(state): State<Arc<AppState>>,
    Extension(session_user): Extension<SessionUser>,
) -> AppResult<Json<Vec<OrganizationListItem>>> {
    let orgs = sqlx::query_as!(
        OrganizationListItem,
        r#"
        SELECT 
            o.id,
            o.name,
            o.slug,
            o.org_type,
            o.active,
            sp.plan_tier,
            o.created_at,
            COUNT(DISTINCT u.id) as "user_count!",
            COUNT(DISTINCT s.id) as "subscription_count!"
        FROM organizations o
        LEFT JOIN subscription_plans sp ON sp.organization_id = o.id
        LEFT JOIN users u ON u.organization_id = o.id AND u.active = true
        LEFT JOIN subscriptions s ON s.organization_id = o.id AND s.active = true
        GROUP BY o.id, sp.plan_tier
        ORDER BY o.created_at DESC
        "#
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(orgs))
}

#[derive(Debug, Deserialize)]
pub struct ToggleOrganizationStatusRequest {
    pub active: bool,
    pub reason: Option<String>,
}

/// Activate/deactivate organization
pub async fn toggle_organization_status(
    State(state): State<Arc<AppState>>,
    Extension(session_user): Extension<SessionUser>,
    Path(org_id): Path<Uuid>,
    Json(payload): Json<ToggleOrganizationStatusRequest>,
) -> AppResult<Json<Organization>> {
    // Don't allow admins to deactivate their own organization
    if org_id == session_user.organization_id && !payload.active {
        return Err(AppError::BadRequest(
            "Cannot deactivate your own organization".to_string(),
        ));
    }

    let org = sqlx::query_as!(
        Organization,
        r#"
        UPDATE organizations
        SET active = $2, updated_at = NOW()
        WHERE id = $1
        RETURNING id, name, slug, org_type, owner_id, webhook_secret, 
                  created_at, active, updated_at
        "#,
        org_id,
        payload.active
    )
    .fetch_one(&state.db)
    .await?;

    // TODO: Log admin action
    // tracing::info!(
    //     admin_user_id = %session_user.user.id,
    //     organization_id = %org_id,
    //     new_status = payload.active,
    //     reason = ?payload.reason,
    //     "Organization status changed by admin"
    // );

    Ok(Json(org))
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
