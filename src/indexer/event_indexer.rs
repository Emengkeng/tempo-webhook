use crate::models::{Subscription, TransferEvent};
use alloy::{
    primitives::{Address, FixedBytes, U256},
    providers::{Provider, ProviderBuilder},
    rpc::types::{Filter, Log},
    transports::http::{Client, Http},
};
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

        let http = Http::<Client>::new(http_url.parse().unwrap());
        let provider = ProviderBuilder::new().on_http(http);

        Self {
            db,
            provider,
            network,
        }
    }

    pub async fn index_block(&self, block_number: u64) -> anyhow::Result<usize> {
        let monitored_tokens =
            Subscription::get_active_monitored_addresses(&self.db, &self.network).await?;

        if monitored_tokens.is_empty() {
            return Ok(0);
        }

        info!(
            "[{}] Indexing {} tokens in block #{}",
            self.network,
            monitored_tokens.len(),
            block_number
        );

        let addresses: Vec<Address> = monitored_tokens
            .iter()
            .filter_map(|addr| Address::from_str(addr).ok())
            .collect();

        if addresses.is_empty() {
            return Ok(0);
        }

        let filter = Filter::new()
            .from_block(block_number)
            .to_block(block_number)
            .address(addresses)
            .event_signature(TRANSFER_SIGNATURE);

        let logs = self.provider.get_logs(&filter).await?;

        for log in &logs {
            self.store_transfer_event(log, block_number).await?;
        }

        Ok(logs.len())
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
