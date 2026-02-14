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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TempoTokenInfo {
    pub name: String,
    pub symbol: String,
    pub decimals: i32,
    #[serde(rename = "chainId")]
    pub chain_id: i32,
    pub address: String,
    #[serde(rename = "logoURI")]
    pub logo_uri: Option<String>,
    pub extensions: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TokenMetadata {
    pub address: String,
    pub network: String,
    pub symbol: String,
    pub name: String,
    pub decimals: i32,
    pub logo_url: Option<String>,
    pub verified: bool,
    pub fetched_at: DateTime<Utc>,
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

/// Service for fetching and caching token metadata from Tempo Tokenlist API
pub struct TempoTokenlistService {
    db: sqlx::PgPool,
    http_client: reqwest::Client,
    api_base_url: String,
}

impl TempoTokenlistService {
    pub fn new(db: sqlx::PgPool, config: &Config) -> Self {
        Self {
            db,
            http_client: reqwest::Client::new(),
            api_base_url: config.tempo_tokenlist_url.clone(),
        }
    }

    /// Get chain ID for network name
    fn get_chain_id(network: &str, config: &Config) -> &str {
        match network {
            "mainnet" => config.tempo_mainnet_chain_id,
            "testnet" => config.tempo_testnet_chain_id,
            _ => config.tempo_testnet_chain_id,
        }
    }

    /// Get or fetch token metadata with automatic caching
    pub async fn get_or_fetch_metadata(
        &self,
        token_address: &str,
        network: &str,
        config: &Config,
    ) -> anyhow::Result<TokenMetadata> {
        // First, try to get from cache
        if let Some(metadata) = self.get_from_cache(token_address, network).await? {
            // Check if cache is fresh (less than 7 days old)
            let age = chrono::Utc::now() - metadata.fetched_at;
            if age.num_days() < 7 {
                return Ok(metadata);
            }
        }

        // Cache miss or stale - fetch from Tempo Tokenlist API
        self.fetch_and_cache(token_address, network, config).await
    }

    async fn get_from_cache(
        &self,
        token_address: &str,
        network: &str,
    ) -> anyhow::Result<Option<TokenMetadata>> {
        let result = sqlx::query_as!(
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
        .fetch_optional(&self.db)
        .await?;

        Ok(result)
    }

    async fn fetch_and_cache(
        &self,
        token_address: &str,
        network: &str,
        config: &Config,
    ) -> anyhow::Result<TokenMetadata> {
        let chain_id = Self::get_chain_id(network, config);
        
        // Fetch from Tempo Tokenlist API
        // Try by address: /asset/{chain_id}/{address}
        let url = format!("{}/asset/{}/{}", self.api_base_url, chain_id, token_address);
        
        let response = self.http_client
            .get(&url)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Failed to fetch token metadata: HTTP {}",
                response.status()
            ));
        }

        let token_info: TempoTokenInfo = response.json().await?;

        // Verify the token is actually on Tempo (all should have 6 decimals)
        let verified = token_info.decimals == 6;

        // Store in database cache
        sqlx::query!(
            r#"
            INSERT INTO token_metadata (address, network, symbol, name, decimals, logo_url, verified)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (address, network) 
            DO UPDATE SET 
                symbol = EXCLUDED.symbol,
                name = EXCLUDED.name,
                decimals = EXCLUDED.decimals,
                logo_url = EXCLUDED.logo_url,
                verified = EXCLUDED.verified,
                fetched_at = NOW()
            "#,
            token_address.to_lowercase(),
            network,
            token_info.symbol,
            token_info.name,
            token_info.decimals,
            token_info.logo_uri,
            verified
        )
        .execute(&self.db)
        .await?;

        Ok(TokenMetadata {
            address: token_address.to_string(),
            network: network.to_string(),
            symbol: token_info.symbol,
            name: token_info.name,
            decimals: token_info.decimals,
            logo_url: token_info.logo_uri,
            verified,
            fetched_at: chrono::Utc::now(),
        })
    }

    /// Prefetch and cache all tokens for a network
    pub async fn prefetch_all_tokens(&self, network: &str, config: &Config,) -> anyhow::Result<usize> {
        let chain_id = Self::get_chain_id(network, config);
        
        // Fetch complete token list
        let url = format!("{}/list/{}", self.api_base_url, chain_id);
        
        #[derive(Deserialize)]
        struct TokenList {
            tokens: Vec<TempoTokenInfo>,
        }

        let response = self.http_client
            .get(&url)
            .send()
            .await?;

        let token_list: TokenList = response.json().await?;

        let mut count = 0;
        for token in token_list.tokens {
            // Store each token in cache
            let verified = token.decimals == 6;
            
            sqlx::query!(
                r#"
                INSERT INTO token_metadata (address, network, symbol, name, decimals, logo_url, verified)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                ON CONFLICT (address, network) 
                DO UPDATE SET 
                    symbol = EXCLUDED.symbol,
                    name = EXCLUDED.name,
                    decimals = EXCLUDED.decimals,
                    logo_url = EXCLUDED.logo_url,
                    verified = EXCLUDED.verified,
                    fetched_at = NOW()
                "#,
                token.address.to_lowercase(),
                network,
                token.symbol,
                token.name,
                token.decimals,
                token.logo_uri,
                verified
            )
            .execute(&self.db)
            .await?;
            
            count += 1;
        }

        tracing::info!("Prefetched {} tokens for {}", count, network);
        Ok(count)
    }
}