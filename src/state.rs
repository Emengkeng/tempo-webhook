use std::sync::Arc;

use redis::Client as RedisClient;
use sqlx::PgPool;

use crate::{config::Config, services::EmailService};

pub struct AppState {
    pub db: PgPool,
    pub redis: RedisClient,
    pub email: Arc<EmailService>,
    pub nats: async_nats::Client,
    pub http_client: reqwest::Client,
    pub config: Config,
}
