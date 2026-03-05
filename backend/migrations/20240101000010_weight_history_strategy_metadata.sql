-- Add strategy_metadata to weight_adjustment_history

ALTER TABLE weight_adjustment_history ADD COLUMN IF NOT EXISTS strategy_metadata TEXT;
