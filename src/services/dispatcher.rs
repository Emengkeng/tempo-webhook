use crate::models::WebhookLog;
use crate::services::matcher;
use crate::state::AppState;
use crate::utils::crypto::generate_webhook_signature;
use backoff::{backoff::Backoff, ExponentialBackoff};
use futures::StreamExt;
use serde_json::json;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{error, info, warn};

const WEBHOOK_QUEUE_SUBJECT: &str = "webhooks.deliver";

pub async fn start_dispatcher(state: Arc<AppState>) -> anyhow::Result<()> {
    info!("Webhook dispatcher started");

    // Subscribe to webhook delivery queue
    let mut subscriber = state.nats.subscribe(WEBHOOK_QUEUE_SUBJECT).await?;

    while let Some(msg) = subscriber.next().await {
        let payload = String::from_utf8_lossy(&msg.payload).to_string();
        
        match serde_json::from_str::<WebhookDeliveryJob>(&payload) {
            Ok(job) => {
                let state_clone = state.clone();
                tokio::spawn(async move {
                    if let Err(e) = deliver_webhook(state_clone, job).await {
                        error!("Webhook delivery error: {}", e);
                    }
                });
            }
            Err(e) => {
                error!("Failed to parse webhook job: {}", e);
            }
        }
    }

    Ok(())
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct WebhookDeliveryJob {
    webhook_log_id: uuid::Uuid,
    subscription_id: uuid::Uuid,
    organization_id: uuid::Uuid,
    webhook_url: String,
    webhook_secret: String,
    payload: serde_json::Value,
}

async fn deliver_webhook(state: Arc<AppState>, job: WebhookDeliveryJob) -> anyhow::Result<()> {
    let start_time = Instant::now();
    
    let mut backoff = ExponentialBackoff {
        current_interval: Duration::from_secs(1),
        initial_interval: Duration::from_secs(1),
        max_interval: Duration::from_secs(1800), // 30 minutes
        max_elapsed_time: Some(Duration::from_secs(7200)), // 2 hours total
        ..Default::default()
    };

    loop {
        match attempt_delivery(&state, &job).await {
            Ok((status_code, latency_ms)) => {
                // Success!
                WebhookLog::update_delivery_status(
                    &state.db,
                    job.webhook_log_id,
                    "delivered".to_string(),
                    Some(status_code),
                    None,
                    Some(latency_ms),
                )
                .await?;

                info!(
                    "Webhook delivered successfully (status: {}, latency: {}ms)",
                    status_code, latency_ms
                );

                return Ok(());
            }
            Err(e) => {
                let should_retry = is_retryable_error(&e);
                
                if should_retry {
                    if let Some(duration) = backoff.next_backoff() {
                        warn!(
                            "Webhook delivery failed, retrying in {:?}: {}",
                            duration, e
                        );

                        // Update status to retrying
                        WebhookLog::update_delivery_status(
                            &state.db,
                            job.webhook_log_id,
                            "retrying".to_string(),
                            None,
                            Some(e.to_string()),
                            Some(start_time.elapsed().as_millis() as i64),
                        )
                        .await?;

                        tokio::time::sleep(duration).await;
                        continue;
                    }
                }

                // Failed permanently
                error!("Webhook delivery failed permanently: {}", e);

                WebhookLog::update_delivery_status(
                    &state.db,
                    job.webhook_log_id,
                    "failed".to_string(),
                    None,
                    Some(e.to_string()),
                    Some(start_time.elapsed().as_millis() as i64),
                )
                .await?;

                return Err(e.into());
            }
        }
    }
}

async fn attempt_delivery(
    state: &AppState,
    job: &WebhookDeliveryJob,
) -> Result<(i32, i64), Box<dyn std::error::Error>> {
    let start = Instant::now();
    
    // Generate HMAC signature
    let timestamp = chrono::Utc::now().timestamp();
    let payload_str = serde_json::to_string(&job.payload)?;
    let signature = generate_webhook_signature(&payload_str, &job.webhook_secret, timestamp);

    // Send webhook
    let response = state
        .http_client
        .post(&job.webhook_url)
        .header("Content-Type", "application/json")
        .header("X-Tempo-Signature", signature)
        .header("User-Agent", "Tempo-Webhook/1.0")
        .body(payload_str)
        .timeout(Duration::from_secs(30))
        .send()
        .await?;

    let status_code = response.status().as_u16() as i32;
    let latency_ms = start.elapsed().as_millis() as i64;

    if response.status().is_success() {
        Ok((status_code, latency_ms))
    } else {
        let error_body = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        Err(format!("HTTP {}: {}", status_code, error_body).into())
    }
}

fn is_retryable_error(error: &Box<dyn std::error::Error>) -> bool {
    let error_str = error.to_string().to_lowercase();
    
    // Retry on network errors and 5xx status codes
    error_str.contains("500")
        || error_str.contains("502")
        || error_str.contains("503")
        || error_str.contains("504")
        || error_str.contains("timeout")
        || error_str.contains("connection")
        || error_str.contains("network")
}

pub async fn enqueue_webhook(state: &AppState, job: WebhookDeliveryJob) -> anyhow::Result<()> {
    let payload = serde_json::to_vec(&job)?;
    state.nats.publish(WEBHOOK_QUEUE_SUBJECT, payload.into()).await?;
    Ok(())
}

// Process matched events and enqueue webhooks
pub async fn process_block_webhooks(
    state: Arc<AppState>,
    block_number: i64,
    network: &str,
) -> anyhow::Result<()> {
    let matches = matcher::match_transfer_events(&state.db, block_number, network).await?;

    for matched in matches {
        // Create webhook log
        let payload = matched.event.to_webhook_payload(network);
        let payload_json = serde_json::to_value(&payload)?;

        let webhook_log = WebhookLog::create(
            &state.db,
            matched.subscription_id,
            matched.organization_id,
            matched.event.tx_hash.clone(),
            matched.event.block_number as i32,
            payload_json.clone(),
        )
        .await?;

        // Enqueue for delivery
        let job = WebhookDeliveryJob {
            webhook_log_id: webhook_log.id,
            subscription_id: matched.subscription_id,
            organization_id: matched.organization_id,
            webhook_url: matched.webhook_url,
            webhook_secret: matched.webhook_secret,
            payload: payload_json,
        };

        enqueue_webhook(state.as_ref(), job).await?;
    }

    Ok(())
}
