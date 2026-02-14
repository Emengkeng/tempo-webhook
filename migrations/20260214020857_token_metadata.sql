-- Migration: Add token metadata caching for accurate decimal formatting
-- This is an optional enhancement to support automatic token decimal detection

-- Token metadata cache table
CREATE TABLE IF NOT EXISTS token_metadata (
    address TEXT PRIMARY KEY,
    network TEXT NOT NULL CHECK(network IN ('mainnet', 'testnet')),
    symbol TEXT NOT NULL,
    name TEXT NOT NULL,
    decimals INTEGER NOT NULL CHECK(decimals >= 0 AND decimals <= 77),
    total_supply TEXT,
    
    -- Metadata
    logo_url TEXT,
    verified BOOLEAN DEFAULT false NOT NULL,
    is_native_token BOOLEAN DEFAULT false NOT NULL,
    
    -- Tracking
    fetched_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    last_updated TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    fetch_failed_at TIMESTAMPTZ,
    fetch_error TEXT,
    
    UNIQUE(address, network)
);

-- Indexes for fast lookups
CREATE INDEX IF NOT EXISTS idx_token_metadata_network_address 
ON token_metadata(network, LOWER(address));

CREATE INDEX IF NOT EXISTS idx_token_metadata_symbol 
ON token_metadata(symbol) 
WHERE verified = true;

-- Add comment
COMMENT ON TABLE token_metadata IS 
'Cached token metadata for accurate amount formatting. Reduces blockchain queries.';

COMMENT ON COLUMN token_metadata.decimals IS 
'Number of decimal places for this token. All Tempo TIP-20 stablecoins use 6 decimals (USDC/USDT standard)';

COMMENT ON COLUMN token_metadata.verified IS 
'Whether this token has been verified as legitimate (helps prevent scam tokens)';

COMMENT ON COLUMN token_metadata.is_native_token IS 
'Whether this is the native blockchain token (e.g., TEMPO)';

-- Insert known Tempo tokens (all use 6 decimals)
-- Native Tempo tokens
INSERT INTO token_metadata (address, network, symbol, name, decimals, is_native_token, verified)
VALUES 
    -- Testnet tokens
    ('0x20c0000000000000000000000000000000000000', 'testnet', 'pathUSD', 'pathUSD', 6, true, true)
ON CONFLICT (address, network) DO NOTHING;

-- Note: Additional tokens can be automatically fetched from tokenlist.tempo.xyz
-- All Tempo stablecoins use 6 decimals per TIP-20 standard


-- Optional: Add a function to automatically update last_updated timestamp
CREATE OR REPLACE FUNCTION update_token_metadata_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.last_updated = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_update_token_metadata_timestamp
    BEFORE UPDATE ON token_metadata
    FOR EACH ROW
    EXECUTE FUNCTION update_token_metadata_timestamp();


-- Optional: Add a view for easy querying of valid tokens
CREATE OR REPLACE VIEW valid_tokens AS
SELECT 
    address,
    network,
    symbol,
    name,
    decimals,
    logo_url,
    is_native_token,
    fetched_at
FROM token_metadata
WHERE verified = true
  AND fetch_error IS NULL
ORDER BY network, symbol;

COMMENT ON VIEW valid_tokens IS 
'Quick access to verified tokens with successfully fetched metadata';


-- Usage statistics table (optional - track which tokens are most queried)
CREATE TABLE IF NOT EXISTS token_usage_stats (
    token_address TEXT NOT NULL,
    network TEXT NOT NULL,
    date DATE NOT NULL,
    transfer_count INTEGER DEFAULT 0 NOT NULL,
    unique_wallets INTEGER DEFAULT 0 NOT NULL,
    
    PRIMARY KEY (token_address, network, date)
);

CREATE INDEX IF NOT EXISTS idx_token_usage_date 
ON token_usage_stats(date DESC);

COMMENT ON TABLE token_usage_stats IS 
'Daily aggregated statistics for token usage. Helps identify which tokens need decimal metadata most urgently.';