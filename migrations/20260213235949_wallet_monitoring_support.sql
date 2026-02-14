-- Migration: Add subscription_type enum and optimize indexes for wallet monitoring

-- Add subscription_type column as NOT NULL with default
ALTER TABLE subscriptions 
ADD COLUMN IF NOT EXISTS subscription_type TEXT 
DEFAULT 'WALLET'
NOT NULL
CHECK(subscription_type IN ('WALLET', 'TOKEN', 'CONTRACT'));

-- Update existing rows (just in case)
UPDATE subscriptions 
SET subscription_type = 'WALLET' 
WHERE subscription_type IS NULL;

COMMENT ON COLUMN subscriptions.subscription_type IS 
'Type of monitoring: WALLET = monitor wallet address, TOKEN = monitor token contract, CONTRACT = monitor smart contract';

-- Add performance indexes for wallet matching
-- This speeds up queries that check: WHERE from_address = X OR to_address = X
CREATE INDEX IF NOT EXISTS idx_transfers_from 
ON transfer_events(from_address) 
WHERE from_address IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_transfers_to 
ON transfer_events(to_address) 
WHERE to_address IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_transfers_wallet_block 
ON transfer_events(from_address, to_address, block_number DESC);

CREATE INDEX IF NOT EXISTS idx_subscriptions_address_lower 
ON subscriptions(LOWER(address)) 
WHERE active = true;

-- DO NOT add direction column (struct doesn't support it yet)
-- Uncomment this later if you add direction field to TransferEvent struct:
-- ALTER TABLE transfer_events 
-- ADD COLUMN IF NOT EXISTS direction TEXT 
-- CHECK(direction IN ('incoming', 'outgoing', 'internal'));
--
-- COMMENT ON COLUMN transfer_events.direction IS 
-- 'Direction relative to monitored wallet: incoming = received, outgoing = sent, internal = between own wallets';