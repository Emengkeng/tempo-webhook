use crate::models::{Subscription, TransferEvent};
use alloy::{
    primitives::{Address, FixedBytes, U256},
    providers::{Provider, ProviderBuilder},
    rpc::types::{Filter, Log},
    transports::http::Http,
};
use reqwest::Client;
use sqlx::PgPool;
use std::str::FromStr;
use tracing::info;

const TRANSFER_SIGNATURE: FixedBytes<32> = FixedBytes::new([
    0xdd, 0xf2, 0x52, 0xad, 0x1b, 0xe2, 0xc8, 0x9b, 0x69, 0xc2, 0xb0, 0x68, 0xfc, 0x37, 0x8d,
    0xaa, 0x95, 0x2b, 0xa7, 0xf1, 0x63, 0xc4, 0xa1, 0x16, 0x28, 0xf5, 0x5a, 0x4d, 0xf5, 0x23,
    0xb3, 0xef,
]);

pub struct EventIndexer {
    db: PgPool,
    provider: alloy::providers::RootProvider<Http<Client>>,
    network: String,
}

impl EventIndexer {
    pub fn new(db: PgPool, network: String) -> Self {
        let http_url = if network == "mainnet" {
            std::env::var("TEMPO_MAINNET_HTTP").unwrap()
        } else {
            std::env::var("TEMPO_TESTNET_HTTP").unwrap()
        };

        let url = http_url.parse().expect("Invalid HTTP URL");
        let provider = ProviderBuilder::new().on_http(url);

        Self {
            db,
            provider,
            network,
        }
    }

    pub async fn index_block(&self, block_number: u64) -> anyhow::Result<usize> {
        // Get monitored WALLETS, not tokens
        let monitored_wallets =
            Subscription::get_active_monitored_addresses(&self.db, &self.network).await?;

        if monitored_wallets.is_empty() {
            return Ok(0);
        }

        info!(
            "[{}] 👁️ Monitoring {} wallets in block #{}",
            self.network,
            monitored_wallets.len(),
            block_number
        );

        info!(
            "[{}] 👛 Monitored wallets: {:?}",
            self.network,
            monitored_wallets.iter().map(|w| &w[..10]).collect::<Vec<_>>()
        );

        //  Get ALL transfers in this block, not just from specific addresses
        let filter = Filter::new()
            .from_block(block_number)
            .to_block(block_number)
            .event_signature(TRANSFER_SIGNATURE);

        info!("[{}] 🔍 Fetching ALL transfers in block #{}", self.network, block_number);
        
        let logs = self.provider.get_logs(&filter).await?;

        info!(
            "[{}] 📦 Block #{} has {} total transfer events",
            self.network, block_number, logs.len()
        );

        // Filter for only transfers involving monitored wallets
        let mut relevant_events = 0;
        
        for log in &logs {
            if log.topics().len() < 3 {
                continue;
            }

            // Extract from and to addresses from topics
            let from_address = format!("0x{}", hex::encode(&log.topics()[1].as_slice()[12..]));
            let to_address = format!("0x{}", hex::encode(&log.topics()[2].as_slice()[12..]));

            // Check if this transfer involves any monitored wallet
            let is_relevant = monitored_wallets.iter().any(|wallet| {
                wallet.eq_ignore_ascii_case(&from_address) || 
                wallet.eq_ignore_ascii_case(&to_address)
            });

            if is_relevant {
                info!(
                    "[{}] ✅ RELEVANT: {:?} | {} → {} | Token: {:?}",
                    self.network,
                    log.transaction_hash,
                    &from_address[..10],
                    &to_address[..10],
                    log.address()
                );
                
                self.store_transfer_event(log, block_number).await?;
                relevant_events += 1;
            }
        }

        if relevant_events == 0 {
            info!(
                "[{}] 😴 No transfers for monitored wallets in block #{}",
                self.network, block_number
            );
        } else {
            info!(
                "[{}] 🎯 Found {} relevant transfers in block #{}",
                self.network, relevant_events, block_number
            );
        }

        Ok(relevant_events)
    }

    async fn store_transfer_event(&self, log: &Log, block_number: u64) -> anyhow::Result<()> {
        let from_address = format!("0x{}", hex::encode(&log.topics()[1].as_slice()[12..]));
        let to_address = format!("0x{}", hex::encode(&log.topics()[2].as_slice()[12..]));

        let amount_bytes: [u8; 32] = log.data().data[0..32]
            .try_into()
            .map_err(|_| anyhow::anyhow!("Invalid amount data"))?;
        let amount = U256::from_be_bytes(amount_bytes);

        let memo = if log.data().data.len() > 64 {
            let memo_length_bytes: [u8; 32] = log.data().data[32..64]
                .try_into()
                .map_err(|_| anyhow::anyhow!("Invalid memo length"))?;
            let memo_length = U256::from_be_bytes(memo_length_bytes).to::<usize>();

            if log.data().data.len() >= 64 + memo_length {
                let memo_bytes = &log.data().data[64..64 + memo_length];
                Some(String::from_utf8_lossy(memo_bytes).to_string())
            } else {
                None
            }
        } else {
            None
        };

        info!(
            "[{}] 💰 Storing: {} → {} | Amount: {} | Token: {:?} | Memo: {:?}",
            self.network,
            &from_address[..10],
            &to_address[..10],
            amount,
            log.address(),
            memo
        );

        TransferEvent::create(
            &self.db,
            block_number as i64,
            format!("{:?}", log.transaction_hash.unwrap()),
            log.log_index.unwrap_or(0) as i32,
            format!("{:?}", log.address()),
            from_address,
            to_address,
            amount.to_string(),
            memo,
            chrono::Utc::now().timestamp(),
        )
        .await?;

        Ok(())
    }
}