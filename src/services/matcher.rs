use crate::models::{Filter, Subscription, TransferEvent};
use regex::Regex;
use sqlx::PgPool;
use uuid::Uuid;
use tracing::info;

#[derive(Debug, Clone)]
pub struct MatchedWebhook {
    pub subscription_id: Uuid,
    pub organization_id: Uuid,
    pub webhook_url: String,
    pub webhook_secret: String,
    pub event: TransferEvent,
}

pub async fn match_transfer_events(
    pool: &PgPool,
    block_number: i64,
    network: &str,
) -> anyhow::Result<Vec<MatchedWebhook>> {
    let events = TransferEvent::get_by_block(pool, block_number).await?;

    // info!("[{}] 🔎 Matcher: Found {} events in block #{}", network, events.len(), block_number);

    let mut matched = Vec::new();

    for event in events {
        info!(
            "[{}] 🔎 Matching event: Token {} | From {} → To {} | Amount: {}",
            network,
            &event.token_address[..10],
            &event.from_address[..10],
            &event.to_address[..10],
            event.amount
        );
        
        let subscriptions = get_matching_subscriptions(pool, &event, network).await?;

        info!(
            "[{}] 🔎 Found {} potential subscriptions",
            network,
            subscriptions.len()
        );

        for (sub, filters) in subscriptions {
            info!(
                "[{}] 🔎 Checking subscription {} (wallet: {}) with {} filters",
                network,
                sub.id,
                &sub.address[..10],
                filters.len()
            );
            
            if apply_filters(&filters, &event) {
                info!("[{}] ✅ Match! Wallet {} is involved in this transfer", network, &sub.address[..10]);
                matched.push(MatchedWebhook {
                    subscription_id: sub.id,
                    organization_id: sub.organization_id,
                    webhook_url: sub.webhook_url.clone(),
                    webhook_secret: sub.webhook_secret.clone(),
                    event: event.clone(),
                });
            } else {
                info!("[{}] ❌ Filters rejected this event", network);
            }
        }
    }

    Ok(matched)
}

async fn get_matching_subscriptions(
    pool: &PgPool,
    event: &TransferEvent,
    network: &str,
) -> anyhow::Result<Vec<(Subscription, Vec<Filter>)>> {
    let subscriptions = sqlx::query_as!(
        Subscription,
        r#"
        SELECT 
            id, organization_id, user_id, network, type as event_type, 
            address, webhook_url, webhook_secret, subscription_type, active, 
            confirmation_blocks, created_at, updated_at
        FROM subscriptions
        WHERE active = true
        AND network = $1
        AND (
            LOWER(address) = LOWER($2)
            OR LOWER(address) = LOWER($3)
        )
        "#,
        network,
        event.from_address,
        event.to_address
    )
    .fetch_all(pool)
    .await?;

    let mut result = Vec::new();
    for sub in subscriptions {
        let filters = Filter::get_by_subscription(pool, sub.id).await?;
        result.push((sub, filters));
    }

    Ok(result)
}

fn apply_filters(filters: &[Filter], event: &TransferEvent) -> bool {
    if filters.is_empty() {
        info!("No filters to apply, accepting event");
        return true;
    }
    
    for filter in filters {
        match filter.filter_type.as_str() {
            "amount_min" => {
                let min: f64 = filter.filter_value.parse().unwrap_or(0.0);
                let amount: f64 = event.amount.parse().unwrap_or(0.0);
                info!("Filter amount_min: {} >= {} = {}", amount, min, amount >= min);
                if amount < min {
                    return false;
                }
            }
            "amount_max" => {
                let max: f64 = filter.filter_value.parse().unwrap_or(f64::MAX);
                let amount: f64 = event.amount.parse().unwrap_or(0.0);
                info!("Filter amount_max: {} <= {} = {}", amount, max, amount <= max);
                if amount > max {
                    return false;
                }
            }
            "token_address" => {
                let matches = event.token_address.to_lowercase() == filter.filter_value.to_lowercase();
                info!("Filter token_address: {} == {} = {}", 
                    event.token_address, filter.filter_value, matches);
                if !matches {
                    return false;
                }
            }
            "memo_pattern" => {
                if let Some(memo) = &event.memo {
                    if let Ok(re) = Regex::new(&filter.filter_value) {
                        let matches = re.is_match(memo);
                        info!("Filter memo_pattern: '{}' matches '{}' = {}", 
                            memo, filter.filter_value, matches);
                        if !matches {
                            return false;
                        }
                    }
                } else {
                    info!("Filter memo_pattern: No memo in event, filter fails");
                    return false;
                }
            }
            _ => {}
        }
    }

    true
}