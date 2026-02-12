use crate::models::{Filter, Subscription, TransferEvent};
use regex::Regex;
use sqlx::PgPool;
use uuid::Uuid;

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

    let mut matched = Vec::new();

    for event in events {
        let subscriptions = get_matching_subscriptions(pool, &event, network).await?;

        for (sub, filters) in subscriptions {
            if apply_filters(&filters, &event) {
                matched.push(MatchedWebhook {
                    subscription_id: sub.id,
                    organization_id: sub.organization_id,
                    webhook_url: sub.webhook_url.clone(),
                    webhook_secret: sub.webhook_secret.clone(),
                    event: event.clone(),
                });
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
            address, webhook_url, webhook_secret, active, 
            confirmation_blocks, created_at, updated_at
        FROM subscriptions
        WHERE active = true
        AND network = $1
        AND address = $2
        AND type IN ('TRANSFER', 'TRANSFER_WITH_MEMO')
        "#,
        network,
        event.token_address
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
    for filter in filters {
        match filter.filter_type.as_str() {
            "amount_min" => {
                let min: f64 = filter.filter_value.parse().unwrap_or(0.0);
                let amount: f64 = event.amount.parse().unwrap_or(0.0);
                if amount < min {
                    return false;
                }
            }
            "amount_max" => {
                let max: f64 = filter.filter_value.parse().unwrap_or(f64::MAX);
                let amount: f64 = event.amount.parse().unwrap_or(0.0);
                if amount > max {
                    return false;
                }
            }
            "from_address" => {
                if event.from_address.to_lowercase() != filter.filter_value.to_lowercase() {
                    return false;
                }
            }
            "to_address" => {
                if event.to_address.to_lowercase() != filter.filter_value.to_lowercase() {
                    return false;
                }
            }
            "memo_pattern" => {
                if let Some(memo) = &event.memo {
                    if let Ok(re) = Regex::new(&filter.filter_value) {
                        if !re.is_match(memo) {
                            return false;
                        }
                    }
                } else {
                    return false;
                }
            }
            _ => {}
        }
    }

    true
}
