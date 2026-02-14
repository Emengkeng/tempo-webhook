use crate::models::{IndexedBlock, WebhookLog};
use crate::services::{matcher, dispatcher};
use crate::state::AppState;
use sqlx::PgPool;
use std::collections::VecDeque;
use std::sync::Arc;
use tracing::{error, info, warn};

use super::event_indexer::EventIndexer;

#[derive(Clone)]
struct PendingBlock {
    number: u64,
    hash: String,
    timestamp: i64,
}

pub struct BlockProcessor {
    db: PgPool,
    confirmation_blocks: u64,
    pending_blocks: VecDeque<PendingBlock>,
    network: String,
    state: Arc<AppState>,
}

impl BlockProcessor {
    pub fn new(db: PgPool, confirmation_blocks: u64, network: String, state: Arc<AppState>) -> Self {
        Self {
            db,
            confirmation_blocks,
            pending_blocks: VecDeque::new(),
            network,
            state,
        }
    }

    pub async fn process_block(
        &mut self,
        block_number: u64,
        block_hash: String,
        timestamp: i64,
        indexer: &EventIndexer,
    ) -> anyhow::Result<()> {
        self.pending_blocks.push_back(PendingBlock {
            number: block_number,
            hash: block_hash.clone(),
            timestamp,
        });

        if (self.pending_blocks.len() as u64) > self.confirmation_blocks {
            let confirmed_block = self.pending_blocks.pop_front().unwrap();

            match self.verify_block(&confirmed_block).await? {
                true => {
                    self.index_confirmed_block(confirmed_block, indexer).await?;
                }
                false => {
                    self.handle_reorg(confirmed_block).await?;
                }
            }
        }

        Ok(())
    }

    async fn verify_block(&self, block: &PendingBlock) -> anyhow::Result<bool> {
        let current_hash = IndexedBlock::get_by_number(&self.db, block.number as i64)
            .await?
            .map(|b| b.block_hash);

        match current_hash {
            Some(hash) => Ok(hash == block.hash),
            None => Ok(true),
        }
    }

    async fn index_confirmed_block(
        &self,
        block: PendingBlock,
        indexer: &EventIndexer,
    ) -> anyhow::Result<()> {
        info!(
            "[{}] Indexing confirmed block #{}",
            self.network, block.number
        );

        IndexedBlock::create(&self.db, block.number as i64, block.hash, block.timestamp).await?;

        let event_count = indexer.index_block(block.number).await?;

        if event_count > 0 {
            info!(
                "[{}] Indexed {} events from block #{}",
                self.network, event_count, block.number
            );
            
            self.process_webhooks(block.number as i64).await?;
        }

        Ok(())
    }

    async fn process_webhooks(&self, block_number: i64) -> anyhow::Result<()> {
        info!("[{}] Processing webhooks for block #{}", self.network, block_number);
        
        // Match events to subscriptions
        let matches = matcher::match_transfer_events(
            &self.db,
            block_number,
            &self.network,
        ).await?;

        if matches.is_empty() {
            info!("[{}] No webhook matches found for block #{}", self.network, block_number);
            return Ok(());
        }

        info!(
            "[{}] Found {} webhook matches for block #{}",
            self.network,
            matches.len(),
            block_number
        );

        // Enqueue webhooks for delivery
        for matched in matches {
            info!(
                "[{}] Match found - Subscription: {} | Token: {} | Amount: {}",
                self.network,
                matched.subscription_id,
                matched.event.token_address,
                matched.event.amount
            );
            // Create webhook log
            let payload = matched.event.to_webhook_payload(&self.network);
            let payload_json = serde_json::to_value(&payload)?;

            let webhook_log = WebhookLog::create(
                &self.db,
                matched.subscription_id,
                matched.organization_id,
                matched.event.tx_hash.clone(),
                matched.event.block_number as i32,
                payload_json.clone(),
            )
            .await?;

            // Enqueue for delivery
            let job = dispatcher::WebhookDeliveryJob {
                webhook_log_id: webhook_log.id,
                subscription_id: matched.subscription_id,
                organization_id: matched.organization_id,
                webhook_url: matched.webhook_url,
                webhook_secret: matched.webhook_secret,
                payload: payload_json,
            };

            dispatcher::enqueue_webhook(&self.state, job).await?;
            
            info!(
                "[{}] Enqueued webhook for subscription {} (tx: {})",
                self.network,
                matched.subscription_id,
                &matched.event.tx_hash[..10]
            );
        }

        Ok(())
    }

    async fn handle_reorg(&self, block: PendingBlock) -> anyhow::Result<()> {
        warn!(
            "[{}] Reorg detected at block #{}",
            self.network, block.number
        );

        WebhookLog::cancel_by_block(&self.db, block.number as i32).await?;
        crate::models::TransferEvent::delete_by_block(&self.db, block.number as i64).await?;

        Ok(())
    }
}