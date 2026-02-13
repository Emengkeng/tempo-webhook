-- Custom Plan Limits table for enterprise and custom plans
CREATE TABLE custom_plan_limits (
    organization_id UUID PRIMARY KEY REFERENCES organizations(id) ON DELETE CASCADE,
    limits JSONB NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    updated_at TIMESTAMPTZ DEFAULT NOW() NOT NULL
);

CREATE INDEX idx_custom_plan_limits_org ON custom_plan_limits(organization_id);

-- Add comment
COMMENT ON TABLE custom_plan_limits IS 'Custom quota limits for enterprise and custom plan organizations';
COMMENT ON COLUMN custom_plan_limits.limits IS 'JSON object containing custom limits: max_subscriptions, max_webhook_deliveries, max_api_requests_per_minute, can_use_mainnet, can_use_testnet, max_filters_per_subscription';
