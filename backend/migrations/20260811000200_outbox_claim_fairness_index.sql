-- Supporting index for the tenant-fair outbox claim (backlog C-2).
--
-- WHY THIS IS NOT OPTIONAL
--
-- C-2 replaced the claim's flat `ORDER BY created_at LIMIT n` with a window
-- ranking:
--
--   ROW_NUMBER() OVER (PARTITION BY tenant_id ORDER BY created_at, event_id)
--
-- over the eligible set (`status IN ('pending','failed')` and retry-due), plus a
-- `COUNT(DISTINCT tenant_id)` over the same set to size the per-tenant cap.
--
-- The pre-existing `idx_memory_vector_outbox_status_retry (status,
-- next_retry_at, created_at)` served the old query well: the planner could walk
-- it in `created_at` order and stop after `LIMIT n` rows. It does **not** serve
-- the window, because the ranking needs rows grouped by `tenant_id` — so without
-- a matching index the planner scans and sorts the entire eligible set on every
-- claim cycle (every 2s), and the DISTINCT count scans it again.
--
-- That would make C-2 *slower* precisely under a large backlog, which is the
-- scenario C-2 exists to improve. Hence this index ships with it.
--
-- COLUMN ORDER
--
-- `(tenant_id, created_at, event_id)` under a partial predicate:
--   - the partial `WHERE` replaces a leading `status` column, keeping the index
--     small (the eligible set is a shrinking minority of a healthy outbox —
--     `applied` rows dominate and are irrelevant here);
--   - `tenant_id` first matches `PARTITION BY`, so the ranking can stream
--     per-tenant groups instead of sorting;
--   - `created_at, event_id` matches the window's `ORDER BY` exactly, including
--     the `event_id` tiebreaker that makes the ranking deterministic;
--   - `COUNT(DISTINCT tenant_id)` over the same predicate can use it too.
--
-- `next_retry_at` is deliberately NOT in the key: it is a range predicate
-- (`IS NULL OR <= now()`), so placing it before `created_at` would break the
-- ordering benefit, and placing it after contributes nothing. It is included as
-- a payload column so the retry-due filter can be applied without a heap fetch.
--
-- The older `status_retry` index is left in place: `reclaim_stale` and
-- `count_pending` still use `status` as a leading equality column.

CREATE INDEX IF NOT EXISTS idx_memory_vector_outbox_claim_fairness
ON memory_vector_outbox (tenant_id, created_at, event_id)
INCLUDE (next_retry_at)
WHERE status IN ('pending', 'failed');
