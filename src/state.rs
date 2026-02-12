use redis::Client as RedisClient;
use sqlx::PgPool;

use crate::config::Config;

pub struct AppState {
    pub db: PgPool,
    pub redis: RedisClient,
    pub nats: async_nats::Client,
    pub http_client: reqwest::Client,
    pub config: Config,
}
