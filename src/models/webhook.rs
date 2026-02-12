use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct WebhookLog {
    pub id: Uuid,
    pub subscription_id: Uuid,
    pub organization_id: Uuid,
    pub transaction_hash: String,
    pub block_number: i32,
    pub payload: serde_json::Value,
    pub attempt_count: i32,
    pub status: String, // pending, delivered, failed, retrying
    pub http_status_code: Option<i32>,
    pub error_message: Option<String>,
    pub first_attempt: DateTime<Utc>,
    pub last_attempt: Option<DateTime<Utc>>,
    pub delivered_at: Option<DateTime<Utc>>,
    pub latency_ms: Option<i64>,
    pub billable: bool,
}

#[derive(Debug, Serialize)]
pub struct WebhookLogResponse {
    pub id: Uuid,
    pub subscription_id: Uuid,
    pub transaction_hash: String,
    pub block_number: i32,
    pub status: String,
    pub http_status_code: Option<i32>,
    pub attempt_count: i32,
    pub latency_ms: Option<i64>,
    pub delivered_at: Option<DateTime<Utc>>,
}

impl WebhookLog {
    pub async fn create(
        pool: &sqlx::PgPool,
        subscription_id: Uuid,
        organization_id: Uuid,
        transaction_hash: String,
        block_number: i32,
        payload: serde_json::Value,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
            WebhookLog,
            r#"
            INSERT INTO webhook_logs (
                subscription_id, organization_id, transaction_hash, 
                block_number, payload, status
            )
            VALUES ($1, $2, $3, $4, $5, 'pending')
            RETURNING 
                id, subscription_id, organization_id, transaction_hash, 
                block_number, payload, attempt_count, status, 
                http_status_code, error_message, first_attempt, 
                last_attempt, delivered_at, latency_ms, billable
            "#,
            subscription_id,
            organization_id,
            transaction_hash,
            block_number,
            payload
        )
        .fetch_one(pool)
        .await
    }

    pub async fn update_delivery_status(
        pool: &sqlx::PgPool,
        id: Uuid,
        status: String,
        http_status_code: Option<i32>,
        error_message: Option<String>,
        latency_ms: Option<i64>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"
            UPDATE webhook_logs 
            SET 
                status = $2,
                http_status_code = $3,
                error_message = $4,
                last_attempt = NOW(),
                attempt_count = attempt_count + 1,
                latency_ms = $5,
                delivered_at = CASE WHEN $2 = 'delivered' THEN NOW() ELSE delivered_at END
            WHERE id = $1
            "#,
            id,
            status,
            http_status_code,
            error_message,
            latency_ms
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn list_by_subscription(
        pool: &sqlx::PgPool,
        subscription_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            WebhookLog,
            r#"
            SELECT 
                id, subscription_id, organization_id, transaction_hash, 
                block_number, payload, attempt_count, status, 
                http_status_code, error_message, first_attempt, 
                last_attempt, delivered_at, latency_ms, billable
            FROM webhook_logs 
            WHERE subscription_id = $1
            ORDER BY first_attempt DESC
            LIMIT $2 OFFSET $3
            "#,
            subscription_id,
            limit,
            offset
        )
        .fetch_all(pool)
        .await
    }

    pub async fn list_by_organization(
        pool: &sqlx::PgPool,
        organization_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            WebhookLog,
            r#"
            SELECT 
                id, subscription_id, organization_id, transaction_hash, 
                block_number, payload, attempt_count, status, 
                http_status_code, error_message, first_attempt, 
                last_attempt, delivered_at, latency_ms, billable
            FROM webhook_logs 
            WHERE organization_id = $1
            ORDER BY first_attempt DESC
            LIMIT $2 OFFSET $3
            "#,
            organization_id,
            limit,
            offset
        )
        .fetch_all(pool)
        .await
    }

    pub async fn cancel_by_block(
        pool: &sqlx::PgPool,
        block_number: i32,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"
            UPDATE webhook_logs 
            SET status = 'cancelled', error_message = 'Reorg detected'
            WHERE block_number = $1 AND status IN ('pending', 'retrying')
            "#,
            block_number
        )
        .execute(pool)
        .await?;
        Ok(())
    }
}

impl From<WebhookLog> for WebhookLogResponse {
    fn from(log: WebhookLog) -> Self {
        Self {
            id: log.id,
            subscription_id: log.subscription_id,
            transaction_hash: log.transaction_hash,
            block_number: log.block_number,
            status: log.status,
            http_status_code: log.http_status_code,
            attempt_count: log.attempt_count,
            latency_ms: log.latency_ms,
            delivered_at: log.delivered_at,
        }
    }
}
