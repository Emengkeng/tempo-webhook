-- Add direction column to transfer_events table
ALTER TABLE transfer_events 
ADD COLUMN direction TEXT;

-- Optionally add a check constraint for valid direction values
ALTER TABLE transfer_events
ADD CONSTRAINT transfer_events_direction_check
CHECK(direction IS NULL OR direction IN ('incoming', 'outgoing', 'internal', 'unknown'));

-- Create an index if you plan to filter by direction
CREATE INDEX idx_transfers_direction ON transfer_events(direction) WHERE direction IS NOT NULL;