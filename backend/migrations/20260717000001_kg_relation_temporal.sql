-- KG Relation Temporal Columns (expand phase, W3.6)
--
-- Adds bi-temporal support to knowledge_relations, matching the existing
-- valid_from/valid_to columns on knowledge_entries. This is the expand
-- step of the expand-contract pattern: columns are nullable, existing
-- rows are backfilled from created_at, and NOT NULL enforcement is
-- deferred to a later migration after data validation.

ALTER TABLE knowledge_relations
ADD COLUMN IF NOT EXISTS valid_from TIMESTAMPTZ;

ALTER TABLE knowledge_relations
ADD COLUMN IF NOT EXISTS valid_to TIMESTAMPTZ;

-- Backfill: set valid_from to created_at for existing rows.
UPDATE knowledge_relations
SET valid_from = created_at
WHERE valid_from IS NULL;