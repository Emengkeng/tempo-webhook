use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Organization {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub org_type: String, // individual, team, enterprise
    pub owner_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub active: bool,
}

#[derive(Debug, Clone, FromRow)]
pub struct SubscriptionPlan {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub plan_tier: String, // free, starter, pro, enterprise
    pub billing_cycle: Option<String>, // monthly, yearly
    pub price_usd: Option<sqlx::types::BigDecimal>,
    pub status: String, // active, cancelled, past_due, trialing
    pub current_period_start: Option<DateTime<Utc>>,
    pub current_period_end: Option<DateTime<Utc>>,
    pub polar_subscription_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

// Serializable version for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionPlanResponse {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub plan_tier: String,
    pub billing_cycle: Option<String>,
    pub price_usd: Option<String>, // Convert BigDecimal to String
    pub status: String,
    pub current_period_start: Option<DateTime<Utc>>,
    pub current_period_end: Option<DateTime<Utc>>,
    pub polar_subscription_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl From<SubscriptionPlan> for SubscriptionPlanResponse {
    fn from(plan: SubscriptionPlan) -> Self {
        Self {
            id: plan.id,
            organization_id: plan.organization_id,
            plan_tier: plan.plan_tier,
            billing_cycle: plan.billing_cycle,
            price_usd: plan.price_usd.map(|bd| bd.to_string()),
            status: plan.status,
            current_period_start: plan.current_period_start,
            current_period_end: plan.current_period_end,
            polar_subscription_id: plan.polar_subscription_id,
            created_at: plan.created_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateOrganizationRequest {
    pub name: String,
    pub slug: String,
    pub org_type: String,
}

impl Organization {
    pub async fn create(
        pool: &sqlx::PgPool,
        name: String,
        slug: String,
        org_type: String,
        owner_id: Uuid,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
            Organization,
            r#"
            INSERT INTO organizations (name, slug, org_type, owner_id)
            VALUES ($1, $2, $3, $4)
            RETURNING id, name, slug, org_type, owner_id, created_at, active
            "#,
            name,
            slug,
            org_type,
            owner_id
        )
        .fetch_one(pool)
        .await
    }

    pub async fn get_by_id(pool: &sqlx::PgPool, id: Uuid) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            Organization,
            "SELECT * FROM organizations WHERE id = $1 AND active = true",
            id
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn get_by_slug(
        pool: &sqlx::PgPool,
        slug: &str,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            Organization,
            "SELECT * FROM organizations WHERE slug = $1 AND active = true",
            slug
        )
        .fetch_optional(pool)
        .await
    }
}

impl SubscriptionPlan {
    pub async fn get_by_organization(
        pool: &sqlx::PgPool,
        org_id: Uuid,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            SubscriptionPlan,
            r#"
            SELECT 
                id,
                organization_id,
                plan_tier,
                billing_cycle,
                price_usd,
                status,
                current_period_start,
                current_period_end,
                polar_subscription_id,
                created_at
            FROM subscription_plans 
            WHERE organization_id = $1
            "#,
            org_id
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn create_or_update(
        pool: &sqlx::PgPool,
        org_id: Uuid,
        plan_tier: String,
        status: String,
        polar_subscription_id: Option<String>,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
            SubscriptionPlan,
            r#"
            INSERT INTO subscription_plans (organization_id, plan_tier, status, polar_subscription_id)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (organization_id) 
            DO UPDATE SET 
                plan_tier = EXCLUDED.plan_tier,
                status = EXCLUDED.status,
                polar_subscription_id = EXCLUDED.polar_subscription_id
            RETURNING 
                id,
                organization_id,
                plan_tier,
                billing_cycle,
                price_usd,
                status,
                current_period_start,
                current_period_end,
                polar_subscription_id,
                created_at
            "#,
            org_id,
            plan_tier,
            status,
            polar_subscription_id
        )
        .fetch_one(pool)
        .await
    }
}