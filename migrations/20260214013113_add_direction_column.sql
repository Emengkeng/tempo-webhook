-- Add direction column only if it doesn't exist
DO $$ 
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'transfer_events' 
        AND column_name = 'direction'
    ) THEN
        ALTER TABLE transfer_events ADD COLUMN direction TEXT;
    END IF;
END $$;

-- Add constraint
ALTER TABLE transfer_events DROP CONSTRAINT IF EXISTS transfer_events_direction_check;
ALTER TABLE transfer_events
ADD CONSTRAINT transfer_events_direction_check
CHECK(direction IS NULL OR direction IN ('incoming', 'outgoing', 'internal', 'unknown'));

-- Add index
DROP INDEX IF EXISTS idx_transfers_direction;
CREATE INDEX idx_transfers_direction ON transfer_events(direction) WHERE direction IS NOT NULL;