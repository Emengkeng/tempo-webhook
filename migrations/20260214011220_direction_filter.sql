-- Update the filters table constraint
ALTER TABLE filters DROP CONSTRAINT IF EXISTS filters_filter_type_check;

ALTER TABLE filters 
ADD CONSTRAINT filters_filter_type_check 
CHECK(filter_type IN (
    'amount_min', 'amount_max', 'token_address', 
    'memo_pattern', 'from_address', 'to_address', 'direction'
));