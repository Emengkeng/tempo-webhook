use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Subscription {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub user_id: Uuid,
    pub network: String, // mainnet, testnet
    #[serde(rename = "type")]
    pub event_type: String, // TRANSFER, TRANSFER_WITH_MEMO, etc.
    pub address: String, // Token or contract address
    pub webhook_url: String,
    #[serde(skip_serializing)]
    pub webhook_secret: String,
    pub subscription_type: String, // WALLET, TOKEN, CONTRACT
    pub active: bool,
    pub confirmation_blocks: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Filter {
    pub id: Uuid,
    pub subscription_id: Uuid,
    pub filter_type: String, // amount_min, amount_max, from_address, to_address, memo_pattern
    pub filter_value: String,
    pub active: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateSubscriptionRequest {
    #[serde(rename = "type")]
    pub event_type: String,
    pub network: String,
    pub address: String,
    pub webhook_url: String,
    pub webhook_secret: Option<String>,
    pub filters: Option<Vec<FilterInput>>,
    pub confirmation_blocks: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct FilterInput {
    #[serde(rename = "type")]
    pub filter_type: String,
    pub value: String,
}

#[derive(Debug, Serialize)]
pub struct SubscriptionResponse {
    pub id: Uuid,
    #[serde(rename = "type")]
    pub event_type: String,
    pub network: String,
    pub address: String,
    pub webhook_url: String,
    pub active: bool,
    pub confirmation_blocks: i32,
    pub created_at: DateTime<Utc>,
    pub filters: Vec<Filter>,
}

impl Subscription {
    pub async fn create(
        pool: &sqlx::PgPool,
        org_id: Uuid,
        user_id: Uuid,
        network: String,
        event_type: String,
        address: String,
        webhook_url: String,
        webhook_secret: String,
        confirmation_blocks: i32,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
            Subscription,
            r#"
            INSERT INTO subscriptions (
                organization_id, user_id, network, type, address, 
                webhook_url, webhook_secret, confirmation_blocks
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING 
                id, organization_id, user_id, network, type as event_type, 
                address, webhook_url, webhook_secret, subscription_type, active, 
                confirmation_blocks, created_at, updated_at
            "#,
            org_id,
            user_id,
            network,
            event_type,
            address,
            webhook_url,
            webhook_secret,
            confirmation_blocks
        )
        .fetch_one(pool)
        .await
    }

    pub async fn get_by_id(
        pool: &sqlx::PgPool,
        id: Uuid,
        org_id: Uuid,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            Subscription,
            r#"
            SELECT 
                id, organization_id, user_id, network, type as event_type, 
                address, webhook_url, webhook_secret, subscription_type, active, 
                confirmation_blocks, created_at, updated_at
            FROM subscriptions 
            WHERE id = $1 AND organization_id = $2
            "#,
            id,
            org_id
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn list_by_organization(
        pool: &sqlx::PgPool,
        org_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            Subscription,
            r#"
            SELECT 
                id, organization_id, user_id, network, type as event_type, 
                address, webhook_url, webhook_secret, subscription_type, active, 
                confirmation_blocks, created_at, updated_at
            FROM subscriptions 
            WHERE organization_id = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
            org_id,
            limit,
            offset
        )
        .fetch_all(pool)
        .await
    }

    pub async fn get_active_monitored_addresses(
        pool: &sqlx::PgPool,
        network: &str,
    ) -> Result<Vec<String>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"
            SELECT DISTINCT address 
            FROM subscriptions 
            WHERE active = true 
            AND network = $1
            AND type IN ('TRANSFER', 'TRANSFER_WITH_MEMO')
            "#,
            network
        )
        .fetch_all(pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.address).collect())
    }

    pub async fn update_status(
        pool: &sqlx::PgPool,
        id: Uuid,
        org_id: Uuid,
        active: bool,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "UPDATE subscriptions SET active = $1, updated_at = NOW() WHERE id = $2 AND organization_id = $3",
            active,
            id,
            org_id
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn delete(
        pool: &sqlx::PgPool,
        id: Uuid,
        org_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "UPDATE subscriptions SET active = false, updated_at = NOW() WHERE id = $1 AND organization_id = $2",
            id,
            org_id
        )
        .execute(pool)
        .await?;
        Ok(())
    }
}

impl Filter {
    pub async fn create_batch(
        pool: &sqlx::PgPool,
        subscription_id: Uuid,
        filters: Vec<FilterInput>,
    ) -> Result<Vec<Self>, sqlx::Error> {
        let mut created_filters = Vec::new();

        for filter in filters {
            let f = sqlx::query_as!(
                Filter,
                r#"
                INSERT INTO filters (subscription_id, filter_type, filter_value)
                VALUES ($1, $2, $3)
                RETURNING id, subscription_id, filter_type, filter_value, active
                "#,
                subscription_id,
                filter.filter_type,
                filter.value
            )
            .fetch_one(pool)
            .await?;

            created_filters.push(f);
        }

        Ok(created_filters)
    }

    pub async fn get_by_subscription(
        pool: &sqlx::PgPool,
        subscription_id: Uuid,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            Filter,
            "SELECT * FROM filters WHERE subscription_id = $1 AND active = true",
            subscription_id
        )
        .fetch_all(pool)
        .await
    }
}
