-- Add strategy_metadata to weight_adjustment_history (v0.3 experiment tracking)
ALTER TABLE weight_adjustment_history ADD COLUMN strategy_metadata TEXT;
