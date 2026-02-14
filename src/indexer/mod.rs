pub mod websocket;
pub mod block_processor;
pub mod event_indexer;

use crate::state::AppState;
use std::sync::Arc;

pub async fn start_mainnet_indexer(state: Arc<AppState>) -> anyhow::Result<()> {
    tracing::info!("Starting mainnet indexer");
    
    let ws_url = state.config.tempo_mainnet_ws.clone();
    let http_url = state.config.tempo_mainnet_http.clone();
    let network = "mainnet".to_string();
    
    start_network_indexer(state, ws_url, http_url, network).await
}

pub async fn start_testnet_indexer(state: Arc<AppState>) -> anyhow::Result<()> {
    tracing::info!("Starting testnet indexer");
    
    let ws_url = state.config.tempo_testnet_ws.clone();
    let http_url = state.config.tempo_testnet_http.clone();
    let network = "testnet".to_string();
    
    start_network_indexer(state, ws_url, http_url, network).await
}

async fn start_network_indexer(
    state: Arc<AppState>,
    ws_url: String,
    http_url: String,
    network: String,
) -> anyhow::Result<()> {
    let ws_manager = websocket::WebSocketManager::new(ws_url, http_url, network.clone());
    
    let processor = block_processor::BlockProcessor::new(
        state.db.clone(),
        state.config.confirmation_blocks,
        network.clone(),
        state.clone(),
    );
    
    let indexer = event_indexer::EventIndexer::new(
        state.db.clone(),
        network.clone(),
    );

    ws_manager.start(processor, indexer).await
}