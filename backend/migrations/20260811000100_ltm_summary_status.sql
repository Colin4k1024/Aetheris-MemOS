-- Long-Term Memory summary degradation marker (backlog D-e).
--
-- Additive, nullable column. When the LLM backend is unavailable at write time
-- (the "Ollama 不可达" case), the LTM entry is stored with an empty summary and
-- this column set to 'pending', so the write is NOT blocked and the entry can
-- be located later for summary backfill. Normal writes set 'complete'.
-- Pre-existing rows keep NULL (treated as complete / legacy); NULL passes the
-- CHECK below.

ALTER TABLE knowledge_entries
ADD COLUMN IF NOT EXISTS summary_status TEXT
    CHECK (summary_status IN ('complete', 'pending'));

-- Partial index so the backfill lookup ("entries whose LLM summary is still
-- deferred") stays cheap, without adding write/space overhead to the common
-- 'complete' / legacy-NULL rows.
CREATE INDEX IF NOT EXISTS idx_knowledge_entries_summary_status_pending
ON knowledge_entries (tenant_id, created_at)
WHERE summary_status = 'pending';
