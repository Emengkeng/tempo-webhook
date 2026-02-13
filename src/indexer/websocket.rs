use alloy::{
    primitives::BlockNumber,
    providers::{Provider, ProviderBuilder},
    transports::ws::WsConnect,
    rpc::types::Header,
};
use futures_util::StreamExt;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info, warn};

use super::block_processor::BlockProcessor;
use super::event_indexer::EventIndexer;

pub struct WebSocketManager {
    ws_url: String,
    http_url: String,
    network: String,
    reconnect_delay: Duration,
    max_reconnect_delay: Duration,
}

impl WebSocketManager {
    pub fn new(ws_url: String, http_url: String, network: String) -> Self {
        Self {
            ws_url,
            http_url,
            network,
            reconnect_delay: Duration::from_secs(1),
            max_reconnect_delay: Duration::from_secs(60),
        }
    }

    pub async fn start(
        mut self,
        mut processor: BlockProcessor,
        indexer: EventIndexer,
    ) -> anyhow::Result<()> {
        loop {
            match self.run_subscription(&mut processor, &indexer).await {
                Ok(_) => {
                    warn!("WebSocket stream ended for network: {}", self.network);
                }
                Err(e) => {
                    error!("WebSocket error for {}: {}", self.network, e);
                }
            }

            sleep(self.reconnect_delay).await;
            self.reconnect_delay = std::cmp::min(
                self.reconnect_delay * 2,
                self.max_reconnect_delay,
            );
            warn!(
                "Reconnecting to {} in {:?}...",
                self.network, self.reconnect_delay
            );
        }
    }

    async fn run_subscription(
        &mut self,
        processor: &mut BlockProcessor,
        indexer: &EventIndexer,
    ) -> anyhow::Result<()> {
        info!("Connecting to WebSocket: {}", self.ws_url);

        let ws = WsConnect::new(&self.ws_url);
        let provider = ProviderBuilder::new().on_ws(ws).await?;

        info!("WebSocket connected to {}", self.network);
        self.reconnect_delay = Duration::from_secs(1);

        let sub = provider.subscribe_blocks().await?;
        let mut stream = sub.into_stream();

        info!("Subscribed to new blocks for {}", self.network);

        while let Some(header) = stream.next().await {
            if let Err(e) = self.handle_block(header, processor, indexer).await {
                error!("Error handling block: {}", e);
            }
        }

        Ok(())
    }

    async fn handle_block(
        &self,
        header: Header,
        processor: &mut BlockProcessor,
        indexer: &EventIndexer,
    ) -> anyhow::Result<()> {
        let block_number = header.number;
        let block_hash = format!("{:?}", header.hash);
        let timestamp = header.timestamp as i64;

        info!(
            "[{}] Received block #{} (hash: {})",
            self.network, block_number, &block_hash[..10]
        );

        processor
            .process_block(block_number, block_hash, timestamp, indexer)
            .await?;

        Ok(())
    }
}