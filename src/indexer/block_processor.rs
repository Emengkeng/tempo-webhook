use crate::models::{IndexedBlock, WebhookLog};
use sqlx::PgPool;
use std::collections::VecDeque;
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
}

impl BlockProcessor {
    pub fn new(db: PgPool, confirmation_blocks: u64, network: String) -> Self {
        Self {
            db,
            confirmation_blocks,
            pending_blocks: VecDeque::new(),
            network,
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
