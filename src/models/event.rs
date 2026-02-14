use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::config::Config;

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
    pub direction: Option<String>,
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
    #[serde(rename = "formattedAmount")]
    pub formatted_amount: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_info: Option<TokenInfo>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct TokenInfo {
    pub symbol: String,
    pub name: String,
    pub decimals: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logo_url: Option<String>,
    pub verified: bool,
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
        direction: Option<String>,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
            TransferEvent,
            r#"
            INSERT INTO transfer_events (
                block_number, tx_hash, log_index, token_address, 
                from_address, to_address, direction, amount, memo, timestamp
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (tx_hash, log_index) DO NOTHING
            RETURNING id, block_number, tx_hash, log_index, token_address, 
                      from_address, to_address, direction, amount, memo, timestamp
            "#,
            block_number,
            tx_hash,
            log_index,
            token_address,
            from_address,
            to_address,
            direction,
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

    /// Convert to webhook payload with database token lookup
    pub async fn to_webhook_payload_with_db(
        &self,
        network: &str,
        monitored_wallet: &str,
        db: &sqlx::PgPool,
    ) -> WebhookPayload {
        // Try to fetch token metadata from database cache
        let token_metadata = Self::get_token_metadata(db, &self.token_address, network).await;
        
        let decimals = token_metadata
            .as_ref()
            .map(|m| m.decimals as u32)
            .unwrap_or(6);  // Default to 6 for Tempo stablecoins
        
        let formatted_amount = Self::format_amount_with_decimals(&self.amount, decimals);
        
        let token_info = token_metadata.map(|m| TokenInfo {
            symbol: m.symbol,
            name: m.name,
            decimals: m.decimals as u32,
            logo_url: m.logo_url,
            verified: m.verified,
        });

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
            formatted_amount,
            memo: self.memo.clone(),
            direction: Some(self.determine_direction(monitored_wallet)),
            token_info,
            metadata: serde_json::json!({
                "logIndex": self.log_index
            }),
        }
    }

    /// Synchronous version without database lookup (uses 6 decimals default)
    pub fn to_webhook_payload(&self, network: &str, monitored_wallet: &str) -> WebhookPayload {
        // All Tempo stablecoins use 6 decimals
        let formatted_amount = Self::format_amount_with_decimals(&self.amount, 6);

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
            formatted_amount,
            memo: self.memo.clone(),
            direction: Some(self.determine_direction(monitored_wallet)),
            token_info: None,  // Not available without DB lookup
            metadata: serde_json::json!({
                "logIndex": self.log_index
            }),
        }
    }

    /// Fetch token metadata from database cache
    async fn get_token_metadata(
        db: &sqlx::PgPool,
        token_address: &str,
        network: &str,
    ) -> Option<TokenMetadata> {
        sqlx::query_as!(
            TokenMetadata,
            r#"
            SELECT address, network, symbol, name, decimals, logo_url, verified, fetched_at
            FROM token_metadata
            WHERE LOWER(address) = LOWER($1)
            AND network = $2
            "#,
            token_address,
            network
        )
        .fetch_optional(db)
        .await
        .ok()
        .flatten()
    }

    /// Format amount with specified decimals
    fn format_amount_with_decimals(raw_amount: &str, decimals: u32) -> String {
        let amount_u128 = match raw_amount.parse::<u128>() {
            Ok(val) => val,
            Err(_) => return raw_amount.to_string(),
        };

        let divisor = 10_u128.pow(decimals);
        let whole = amount_u128 / divisor;
        let fraction = amount_u128 % divisor;
        
        if fraction == 0 {
            format!("{}", whole)
        } else {
            let fraction_str = format!("{:0width$}", fraction, width = decimals as usize);
            let trimmed = fraction_str.trim_end_matches('0');
            format!("{}.{}", whole, trimmed)
        }
    }

    pub fn determine_direction(&self, monitored_wallet: &str) -> String {
        let wallet_lower = monitored_wallet.to_lowercase();
        let from_lower = self.from_address.to_lowercase();
        let to_lower = self.to_address.to_lowercase();

        if from_lower == wallet_lower && to_lower == wallet_lower {
            "internal".to_string()  // Self-transfer
        } else if to_lower == wallet_lower {
            "incoming".to_string()  // Receiving
        } else if from_lower == wallet_lower {
            "outgoing".to_string()  // Sending
        } else {
            "unknown".to_string()   // Shouldn't happen
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
