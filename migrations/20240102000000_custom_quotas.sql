-- Create custom_quotas table for enterprise customer overrides
CREATE TABLE custom_quotas (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    organization_id UUID NOT NULL UNIQUE REFERENCES organizations(id) ON DELETE CASCADE,
    
    -- Custom limits (NULL means use plan default)
    max_subscriptions BIGINT,
    max_webhook_deliveries BIGINT,
    max_api_requests_per_minute BIGINT,
    can_use_mainnet BOOLEAN,
    can_use_testnet BOOLEAN,
    max_filters_per_subscription BIGINT,
    
    -- Custom features as JSON
    custom_features JSONB,
    
    -- Notes for internal tracking
    notes TEXT,
    
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    updated_at TIMESTAMPTZ DEFAULT NOW() NOT NULL
);

CREATE INDEX idx_custom_quotas_org ON custom_quotas(organization_id);

-- Add comments for documentation
COMMENT ON TABLE custom_quotas IS 'Custom quota overrides for enterprise customers';
COMMENT ON COLUMN custom_quotas.max_subscriptions IS 'Override max subscriptions, NULL uses plan default';
COMMENT ON COLUMN custom_quotas.max_webhook_deliveries IS 'Override max webhook deliveries per month, NULL uses plan default';
COMMENT ON COLUMN custom_quotas.max_api_requests_per_minute IS 'Override API rate limit, NULL uses plan default';
COMMENT ON COLUMN custom_quotas.custom_features IS 'JSON object for custom feature flags';
COMMENT ON COLUMN custom_quotas.notes IS 'Internal notes about this enterprise customer';
