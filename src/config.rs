use serde::Deserialize;
use std::env;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    // Database
    #[serde(default = "default_database_url")]
    pub database_url: String,
    
    #[serde(default = "default_database_max_connections")]
    pub database_max_connections: u32,

    // Redis
    #[serde(default = "default_redis_url")]
    pub redis_url: String,

    // NATS
    #[serde(default = "default_nats_url")]
    pub nats_url: String,

    // Tempo RPC endpoints
    pub tempo_mainnet_ws: String,
    pub tempo_mainnet_http: String,
    pub tempo_testnet_ws: String,
    pub tempo_testnet_http: String,
    pub tempo_tokenlist_url: String,
    pub tempo_mainnet_chain_id: i32,
    pub tempo_testnet_chain_id: i32,

    //SMTP
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_username: String,
    pub smtp_password: String,
    pub from_email: String,
    pub base_url: String,

    // Server
    #[serde(default = "default_port")]
    pub port: u16,
    
    #[serde(default = "default_host")]
    pub host: String,

    // Security
    pub jwt_secret: String,
    pub api_key_encryption_key: String,
    pub session_secret: String, 

    // Polar (billing)
    pub polar_secret_key: String,
    pub polar_webhook_secret: String,

    // Monitoring
    pub sentry_dsn: Option<String>,

    // Features
    #[serde(default = "default_true")]
    pub enable_mainnet: bool,
    
    #[serde(default = "default_true")]
    pub enable_testnet: bool,
    
    #[serde(default = "default_confirmation_blocks")]
    pub confirmation_blocks: u64,

    // Rate limiting
    #[serde(default = "default_rate_limit_per_minute")]
    pub rate_limit_per_minute: u32,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        dotenv::dotenv().ok();
        
        Ok(Self {
            database_url: env::var("DATABASE_URL")?,
            database_max_connections: env::var("DATABASE_MAX_CONNECTIONS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(10),
            redis_url: env::var("REDIS_URL").unwrap_or_else(|_| default_redis_url()),
            nats_url: env::var("NATS_URL").unwrap_or_else(|_| default_nats_url()),
            tempo_mainnet_ws: env::var("TEMPO_MAINNET_WS")?,
            tempo_mainnet_http: env::var("TEMPO_MAINNET_HTTP")?,
            tempo_testnet_ws: env::var("TEMPO_TESTNET_WS")?,
            tempo_testnet_http: env::var("TEMPO_TESTNET_HTTP")?,
            tempo_tokenlist_url: env::var("TEMPO_TOKENLIST_URL")?,
            tempo_mainnet_chain_id: env::var("TEMPO_MAINNET_CHAIN_ID")?
                .parse()
                .unwrap_or(42429),
            tempo_testnet_chain_id: env::var("TEMPO_TESTNET_CHAIN_ID")?
                .parse()
                .unwrap_or(42431),
            smtp_host: env::var("SMTP_HOST")?,
            smtp_port: env::var("SMTP_PORT")?.parse()?,
            smtp_username: env::var("SMTP_USERNAME")?,
            smtp_password: env::var("SMTP_PASSWORD")?,
            from_email: env::var("FROM_EMAIL")?,
            base_url: env::var("BASE_URL")?,
            port: env::var("PORT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(8080),
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            jwt_secret: env::var("JWT_SECRET")?,
            api_key_encryption_key: env::var("API_KEY_ENCRYPTION_KEY")?,
            session_secret: env::var("SESSION_SECRET")?,
            polar_secret_key: env::var("POLAR_SECRET_KEY")?,
            polar_webhook_secret: env::var("POLAR_WEBHOOK_SECRET")?,
            sentry_dsn: env::var("SENTRY_DSN").ok(),
            enable_mainnet: env::var("ENABLE_MAINNET")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(true),
            enable_testnet: env::var("ENABLE_TESTNET")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(true),
            confirmation_blocks: env::var("CONFIRMATION_BLOCKS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(1),
            rate_limit_per_minute: env::var("RATE_LIMIT_PER_MINUTE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(100),
        })
    }
}

fn default_database_url() -> String {
    "postgres://postgres:postgres@localhost/tempo_webhooks".to_string()
}

fn default_database_max_connections() -> u32 {
    10
}

fn default_redis_url() -> String {
    "redis://localhost:6379".to_string()
}

fn default_nats_url() -> String {
    "nats://localhost:4222".to_string()
}

fn default_port() -> u16 {
    8080
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_true() -> bool {
    true
}

fn default_confirmation_blocks() -> u64 {
    1
}

fn default_rate_limit_per_minute() -> u32 {
    100
}
