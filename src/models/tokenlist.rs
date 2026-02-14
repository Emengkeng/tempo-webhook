use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::config::Config;

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
    fn get_chain_id(network: &str, config: &Config) -> i32 {
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