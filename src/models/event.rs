use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TransferEvent {
    pub id: Uuid,
    pub block_number: i64,
    pub tx_hash: String,
    pub log_index: i32,
    pub token_address: String,
    pub from_address: String,
    pub to_address: String,
    pub amount: String,
    pub memo: Option<String>,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct IndexedBlock {
    pub block_number: i64,
    pub block_hash: String,
    pub timestamp: i64,
    pub processed_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct WebhookPayload {
    #[serde(rename = "type")]
    pub event_type: String,
    pub network: String,
    #[serde(rename = "blockNumber")]
    pub block_number: String,
    #[serde(rename = "transactionHash")]
    pub transaction_hash: String,
    pub timestamp: i64,
    pub from: String,
    pub to: String,
    pub token: String,
    pub amount: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memo: Option<String>,
    pub metadata: serde_json::Value,
}

impl TransferEvent {
    pub async fn create(
        pool: &sqlx::PgPool,
        block_number: i64,
        tx_hash: String,
        log_index: i32,
        token_address: String,
        from_address: String,
        to_address: String,
        amount: String,
        memo: Option<String>,
        timestamp: i64,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
            TransferEvent,
            r#"
            INSERT INTO transfer_events (
                block_number, tx_hash, log_index, token_address, 
                from_address, to_address, amount, memo, timestamp
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (tx_hash, log_index) DO NOTHING
            RETURNING id, block_number, tx_hash, log_index, token_address, 
                      from_address, to_address, amount, memo, timestamp
            "#,
            block_number,
            tx_hash,
            log_index,
            token_address,
            from_address,
            to_address,
            amount,
            memo,
            timestamp
        )
        .fetch_one(pool)
        .await
    }

    pub async fn get_by_block(
        pool: &sqlx::PgPool,
        block_number: i64,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            TransferEvent,
            "SELECT * FROM transfer_events WHERE block_number = $1 ORDER BY log_index",
            block_number
        )
        .fetch_all(pool)
        .await
    }

    pub async fn delete_by_block(
        pool: &sqlx::PgPool,
        block_number: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "DELETE FROM transfer_events WHERE block_number = $1",
            block_number
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub fn to_webhook_payload(&self, network: &str) -> WebhookPayload {
        WebhookPayload {
            event_type: if self.memo.is_some() {
                "transfer_with_memo".to_string()
            } else {
                "transfer".to_string()
            },
            network: network.to_string(),
            block_number: self.block_number.to_string(),
            transaction_hash: self.tx_hash.clone(),
            timestamp: self.timestamp,
            from: self.from_address.clone(),
            to: self.to_address.clone(),
            token: self.token_address.clone(),
            amount: self.amount.clone(),
            memo: self.memo.clone(),
            metadata: serde_json::json!({
                "logIndex": self.log_index
            }),
        }
    }
}

impl IndexedBlock {
    pub async fn create(
        pool: &sqlx::PgPool,
        block_number: i64,
        block_hash: String,
        timestamp: i64,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
            IndexedBlock,
            r#"
            INSERT INTO indexed_blocks (block_number, block_hash, timestamp)
            VALUES ($1, $2, $3)
            ON CONFLICT (block_number) DO UPDATE
            SET block_hash = $2, timestamp = $3
            RETURNING block_number, block_hash, timestamp, processed_at
            "#,
            block_number,
            block_hash,
            timestamp
        )
        .fetch_one(pool)
        .await
    }

    pub async fn get_by_number(
        pool: &sqlx::PgPool,
        block_number: i64,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            IndexedBlock,
            "SELECT * FROM indexed_blocks WHERE block_number = $1",
            block_number
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn get_latest(pool: &sqlx::PgPool) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            IndexedBlock,
            "SELECT * FROM indexed_blocks ORDER BY block_number DESC LIMIT 1"
        )
        .fetch_optional(pool)
        .await
    }
}
