use anyhow::Result;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tokio::signal;
use tracing::{info, error};

mod config;
mod error;
mod indexer;
mod models;
mod routes;
mod services;
mod state;
mod utils;

use config::Config;
use state::AppState;

use crate::services::EmailService;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tempo_webhooks=debug".into()),
        )
        .json()
        .init();

    info!("Starting Tempo Webhook Service");

    // Load configuration
    let config = Config::from_env()?;
    info!("Configuration loaded successfully");

    // Initialize Sentry if configured
    let _sentry_guard = config.sentry_dsn.as_ref().map(|dsn| {
        sentry::init((
            dsn.as_str(),
            sentry::ClientOptions {
                release: sentry::release_name!(),
                environment: Some("production".into()),
                ..Default::default()
            },
        ))
    });

    // Create database pool
    info!("Connecting to database: {}", mask_connection_string(&config.database_url));
    let db_pool = PgPoolOptions::new()
        .max_connections(config.database_max_connections)
        .connect(&config.database_url)
        .await?;
    
    info!("Database connected successfully");

    // Run migrations
    info!("Running database migrations");
    sqlx::migrate!("./migrations")
        .run(&db_pool)
        .await?;
    info!("Migrations completed");

    // Create Redis client
    info!("Connecting to Redis");
    let redis_client = redis::Client::open(config.redis_url.clone())?;
    let redis_conn = redis_client.get_multiplexed_async_connection().await?;
    info!("Redis connected successfully");

    info!("Initializing email service");
    let email_service = Arc::new(EmailService::new(
        config.smtp_host.clone(),
        config.smtp_port,
        config.smtp_username.clone(),
        config.smtp_password.clone(),
        config.from_email.clone(),
        config.base_url.clone(),
    )?);
    info!("Email service initialized");

    // Create NATS client
    info!("Connecting to NATS");
    let nats_client = async_nats::connect(&config.nats_url).await?;
    info!("NATS connected successfully");

    // Create HTTP client for webhooks
    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    // Create application state
    let state = Arc::new(AppState {
        db: db_pool,
        redis: redis_client,
        email: email_service,
        nats: nats_client,
        http_client,
        config: config.clone(),
    });

    // Start background services
    let indexer_state = state.clone();
    tokio::spawn(async move {
        if let Err(e) = start_indexer_services(indexer_state).await {
            error!("Indexer service error: {}", e);
        }
    });

    // Start webhook dispatcher
    let dispatcher_state = state.clone();
    tokio::spawn(async move {
        if let Err(e) = start_webhook_dispatcher(dispatcher_state).await {
            error!("Webhook dispatcher error: {}", e);
        }
    });

    // Start webhook processor (processes matched events)
    let processor_state = state.clone();
    tokio::spawn(async move {
        if let Err(e) = start_webhook_processor(processor_state).await {
            error!("Webhook processor error: {}", e);
        }
    });

    // Build API server
    let app = routes::create_router(state.clone());

    // Start server
    let addr = format!("{}:{}", config.host, config.port);
    info!("Starting server on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn start_indexer_services(state: Arc<AppState>) -> Result<()> {
    info!("Starting blockchain indexer services");
    
    // Start mainnet indexer if enabled
    if state.config.enable_mainnet {
        let mainnet_state = state.clone();
        tokio::spawn(async move {
            if let Err(e) = indexer::start_mainnet_indexer(mainnet_state).await {
                error!("Mainnet indexer error: {}", e);
            }
        });
    }

    // Start testnet indexer if enabled
    if state.config.enable_testnet {
        let testnet_state = state.clone();
        tokio::spawn(async move {
            if let Err(e) = indexer::start_testnet_indexer(testnet_state).await {
                error!("Testnet indexer error: {}", e);
            }
        });
    }

    Ok(())
}

async fn start_webhook_dispatcher(state: Arc<AppState>) -> Result<()> {
    info!("Starting webhook dispatcher");
    services::dispatcher::start_dispatcher(state).await
}

async fn start_webhook_processor(state: Arc<AppState>) -> Result<()> {
    info!("Starting webhook processor");
    
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
    
    loop {
        interval.tick().await;
        
        // Get the latest indexed block for each network
        if state.config.enable_mainnet {
            if let Ok(Some(latest_block)) = crate::models::IndexedBlock::get_latest(&state.db).await {
                let _ = services::dispatcher::process_block_webhooks(
                    state.clone(),
                    latest_block.block_number,
                    "mainnet",
                )
                .await;
            }
        }
        
        if state.config.enable_testnet {
            if let Ok(Some(latest_block)) = crate::models::IndexedBlock::get_latest(&state.db).await {
                let _ = services::dispatcher::process_block_webhooks(
                    state.clone(),
                    latest_block.block_number,
                    "testnet",
                )
                .await;
            }
        }
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("Shutdown signal received, starting graceful shutdown");
}

fn mask_connection_string(url: &str) -> String {
    if let Some(at_pos) = url.find('@') {
        if let Some(protocol_end) = url.find("://") {
            format!("{}://***@{}", &url[..protocol_end], &url[at_pos + 1..])
        } else {
            "***".to_string()
        }
    } else {
        url.to_string()
    }
}
