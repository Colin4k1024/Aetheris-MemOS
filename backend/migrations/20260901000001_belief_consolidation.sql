-- #129: belief consolidation support + the single-cardinality constraint fix.
--
-- Two things this migration does:
--
-- 1. CONSOLIDATION SUPPORT:
--    * `last_confirmed_at` — when an authority last re-vouched for an edge.
--      Stale scans key off THIS (not valid_from): an SoR reconfirmation
--      resets the aging clock without rewriting the belief's history.
--      Backfilled from recorded_at (the only confirmation signal that
--      existed before this column).
--    * Partial index for the stale scan (open active edges per predicate).
--
-- 2. SINGLE-CARDINALITY CONSTRAINT FIX (a real #127 defect this issue's
--    multi-active scan surfaced): the exclusion constraint
--    `beliefs_single_open_edge_per_subject` had no cardinality condition, so
--    MULTI-valued predicates (prefers, owner_of, member_of…) could never hold
--    more than one open edge — the write gate masked it by force-superseding
--    equal-rank claims, silently destroying multi-value semantics. The
--    constraint is rebuilt to apply ONLY to single-valued edges, denormalized
--    into `memory_beliefs.single_valued` (maintained by the gate from the
--    catalog at write time) because EXCLUDE cannot join the policies table.

-- ── Consolidation columns ────────────────────────────────────────────────────

ALTER TABLE memory_beliefs
    ADD COLUMN IF NOT EXISTS last_confirmed_at TIMESTAMPTZ;

UPDATE memory_beliefs
SET last_confirmed_at = recorded_at
WHERE last_confirmed_at IS NULL;

ALTER TABLE memory_beliefs
    ALTER COLUMN last_confirmed_at SET NOT NULL;

-- ── Cardinality flag + constraint rebuild ────────────────────────────────────

ALTER TABLE memory_beliefs
    ADD COLUMN IF NOT EXISTS single_valued BOOLEAN NOT NULL DEFAULT TRUE;

-- Backfill from the policy catalog; single stays TRUE (the safe default:
-- wrongly-single only blocks a coexistence, wrongly-multi would ALLOW an
-- invariant violation).
UPDATE memory_beliefs b
SET single_valued = (p.cardinality = 'single')
FROM memory_predicate_policies p
WHERE b.predicate = p.name
  AND p.cardinality = 'multi';

ALTER TABLE memory_beliefs
    DROP CONSTRAINT IF EXISTS beliefs_single_open_edge_per_subject;

ALTER TABLE memory_beliefs ADD CONSTRAINT beliefs_single_open_edge_per_subject
EXCLUDE USING gist (
    tenant_id WITH =,
    subject WITH =,
    predicate WITH =,
    tstzrange(valid_from, COALESCE(valid_to, 'infinity'), '[)') WITH &&
) WHERE (single_valued AND status IN ('active', 'needs_confirm'));

COMMENT ON CONSTRAINT beliefs_single_open_edge_per_subject ON memory_beliefs IS
    'At most one open edge per SINGLE-cardinality (tenant, subject, predicate); multi-valued predicates are exempt via the single_valued flag maintained by the write gate from the catalog.';

-- ── Scan support index ───────────────────────────────────────────────────────

CREATE INDEX IF NOT EXISTS idx_beliefs_consolidation_scan
ON memory_beliefs (tenant_id, predicate, last_confirmed_at)
WHERE valid_to IS NULL AND status = 'active';
