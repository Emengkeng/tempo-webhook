-- Enable required extensions
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- Add webhook_secret to organizations table
ALTER TABLE organizations 
ADD COLUMN webhook_secret TEXT;

-- Update existing organizations with a webhook secret
UPDATE organizations 
SET webhook_secret = 'whsec_' || encode(gen_random_bytes(32), 'hex')
WHERE webhook_secret IS NULL;

-- Make it NOT NULL after populating
ALTER TABLE organizations 
ALTER COLUMN webhook_secret SET NOT NULL;

-- Remove webhook_secret from subscriptions table (it's redundant now)
ALTER TABLE subscriptions 
DROP COLUMN webhook_secret;