-- Enable UUID extension
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Organizations
CREATE TABLE organizations (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name TEXT NOT NULL,
    slug TEXT UNIQUE NOT NULL,
    org_type TEXT CHECK(org_type IN ('individual', 'team', 'enterprise')) NOT NULL,
    owner_id UUID NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    active BOOLEAN DEFAULT true NOT NULL
);

CREATE INDEX idx_organizations_slug ON organizations(slug);
CREATE INDEX idx_organizations_active ON organizations(active);

-- Users
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    organization_id UUID NOT NULL REFERENCES organizations(id),
    email TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    full_name TEXT,
    role TEXT CHECK(role IN ('owner', 'admin', 'developer', 'viewer')) NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    active BOOLEAN DEFAULT true NOT NULL
);

CREATE INDEX idx_users_email ON users(email);
CREATE INDEX idx_users_org ON users(organization_id);

-- Subscription Plans (Polar integration)
CREATE TABLE subscription_plans (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    organization_id UUID NOT NULL UNIQUE REFERENCES organizations(id),
    plan_tier TEXT CHECK(plan_tier IN ('free', 'starter', 'pro', 'enterprise')) NOT NULL,
    billing_cycle TEXT CHECK(billing_cycle IN ('monthly', 'yearly')),
    price_usd NUMERIC(10, 2),
    status TEXT CHECK(status IN ('active', 'cancelled', 'past_due', 'trialing')) NOT NULL,
    current_period_start TIMESTAMPTZ,
    current_period_end TIMESTAMPTZ,
    polar_subscription_id TEXT UNIQUE,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL
);

CREATE INDEX idx_subscription_plans_org ON subscription_plans(organization_id);
CREATE INDEX idx_subscription_plans_status ON subscription_plans(status);

-- API Keys
CREATE TABLE api_keys (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    organization_id UUID NOT NULL REFERENCES organizations(id),
    user_id UUID NOT NULL REFERENCES users(id),
    key_hash TEXT UNIQUE NOT NULL,
    key_prefix TEXT NOT NULL,
    name TEXT,
    network TEXT CHECK(network IN ('mainnet', 'testnet', 'both')) NOT NULL,
    active BOOLEAN DEFAULT true NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    expires_at TIMESTAMPTZ,
    last_used TIMESTAMPTZ,
    request_count BIGINT DEFAULT 0 NOT NULL
);

CREATE INDEX idx_api_keys_hash ON api_keys(key_hash) WHERE active = true;
CREATE INDEX idx_api_keys_org ON api_keys(organization_id);

-- Subscriptions (webhook subscriptions)
CREATE TABLE subscriptions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    organization_id UUID NOT NULL REFERENCES organizations(id),
    user_id UUID NOT NULL REFERENCES users(id),
    network TEXT NOT NULL CHECK(network IN ('mainnet', 'testnet')),
    type TEXT NOT NULL,
    address TEXT NOT NULL,
    webhook_url TEXT NOT NULL,
    webhook_secret TEXT NOT NULL,
    active BOOLEAN DEFAULT true NOT NULL,
    confirmation_blocks INTEGER DEFAULT 1 NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    updated_at TIMESTAMPTZ DEFAULT NOW() NOT NULL
);

CREATE INDEX idx_subscriptions_address ON subscriptions(address, active) WHERE active = true;
CREATE INDEX idx_subscriptions_org ON subscriptions(organization_id, active);
CREATE INDEX idx_subscriptions_network ON subscriptions(network, active) WHERE active = true;

-- Filters
CREATE TABLE filters (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    subscription_id UUID NOT NULL REFERENCES subscriptions(id) ON DELETE CASCADE,
    filter_type TEXT NOT NULL CHECK(filter_type IN (
        'amount_min', 'amount_max', 'token_address', 
        'memo_pattern', 'from_address', 'to_address'
    )),
    filter_value TEXT NOT NULL,
    active BOOLEAN DEFAULT true NOT NULL
);

CREATE INDEX idx_filters_subscription ON filters(subscription_id);

-- Indexed Blocks
CREATE TABLE indexed_blocks (
    block_number BIGINT PRIMARY KEY,
    block_hash TEXT NOT NULL,
    timestamp BIGINT NOT NULL,
    processed_at TIMESTAMPTZ DEFAULT NOW() NOT NULL
);

CREATE INDEX idx_indexed_blocks_timestamp ON indexed_blocks(timestamp DESC);

-- Transfer Events
CREATE TABLE transfer_events (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    block_number BIGINT NOT NULL,
    tx_hash TEXT NOT NULL,
    log_index INTEGER NOT NULL,
    token_address TEXT NOT NULL,
    from_address TEXT NOT NULL,
    to_address TEXT NOT NULL,
    amount TEXT NOT NULL,
    memo TEXT,
    timestamp BIGINT NOT NULL,
    UNIQUE(tx_hash, log_index)
);

CREATE INDEX idx_transfers_token ON transfer_events(token_address, block_number DESC);
CREATE INDEX idx_transfers_from ON transfer_events(from_address, block_number DESC);
CREATE INDEX idx_transfers_to ON transfer_events(to_address, block_number DESC);
CREATE INDEX idx_transfers_block ON transfer_events(block_number DESC);

-- Webhook Logs
CREATE TABLE webhook_logs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    subscription_id UUID NOT NULL REFERENCES subscriptions(id),
    organization_id UUID NOT NULL REFERENCES organizations(id),
    transaction_hash TEXT NOT NULL,
    block_number INTEGER NOT NULL,
    payload JSONB NOT NULL,
    attempt_count INTEGER DEFAULT 1 NOT NULL,
    status TEXT CHECK(status IN ('pending', 'delivered', 'failed', 'retrying', 'cancelled')) NOT NULL,
    http_status_code INTEGER,
    error_message TEXT,
    first_attempt TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    last_attempt TIMESTAMPTZ,
    delivered_at TIMESTAMPTZ,
    latency_ms BIGINT,
    billable BOOLEAN DEFAULT true NOT NULL
);

CREATE INDEX idx_webhook_logs_sub ON webhook_logs(subscription_id, first_attempt DESC);
CREATE INDEX idx_webhook_logs_org ON webhook_logs(organization_id, first_attempt DESC);
CREATE INDEX idx_webhook_logs_status ON webhook_logs(status) WHERE status IN ('pending', 'retrying');

-- Usage Records
CREATE TABLE usage_records (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    organization_id UUID NOT NULL REFERENCES organizations(id),
    usage_date DATE NOT NULL,
    api_requests INTEGER DEFAULT 0 NOT NULL,
    webhook_deliveries INTEGER DEFAULT 0 NOT NULL,
    active_subscriptions INTEGER DEFAULT 0 NOT NULL,
    estimated_cost NUMERIC(10, 2),
    recorded_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    UNIQUE(organization_id, usage_date)
);

CREATE INDEX idx_usage_org_date ON usage_records(organization_id, usage_date DESC);
