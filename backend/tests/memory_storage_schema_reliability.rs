//! Schema-level checks for the memory storage tenant foundation migration.
//!
//! These tests inspect migration SQL instead of requiring a live PostgreSQL instance. They protect
//! the expand-phase contract before later RLS, backfill, and repository changes are implemented.

use std::fs;
use std::path::PathBuf;

const TENANT_FOUNDATION_MIGRATION: &str =
    "migrations/20260706000100_memory_storage_tenant_foundation.sql";

const RLS_LTM_MIGRATION: &str = "migrations/20260716000100_rls_ltm.sql";

const TENANT_SCOPED_TABLES: &[&str] = &[
    "context_sessions",
    "context_messages",
    "session_messages",
    "knowledge_entries",
    "knowledge_relations",
    "knowledge_entry_versions",
    "entities",
    "relations",
    "reasoning_paths",
    "entity_versions",
    "multimodal_entries",
    "modality_relations",
];

const FOUNDATION_TABLES: &[&str] = &[
    "memory_tenant_readonly_isolation",
    "memory_vector_outbox",
    "memory_vector_reconciliation_runs",
    "memory_vector_reconciliation_items",
    "memory_audit_events",
];

fn migration_sql() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(TENANT_FOUNDATION_MIGRATION);
    fs::read_to_string(path).expect("tenant foundation migration must be readable")
}

fn rls_ltm_sql() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(RLS_LTM_MIGRATION);
    fs::read_to_string(path).expect("rls ltm migration must be readable")
}

#[test]
fn memory_owned_tables_get_tenant_id_columns() {
    let sql = migration_sql();

    for table in TENANT_SCOPED_TABLES {
        let statement = format!("ALTER TABLE {table}\nADD COLUMN IF NOT EXISTS tenant_id TEXT;");
        assert!(
            sql.contains(&statement),
            "tenant foundation migration must add tenant_id to {table}"
        );
    }
}

#[test]
fn memory_foundation_tables_are_created() {
    let sql = migration_sql();

    for table in FOUNDATION_TABLES {
        let statement = format!("CREATE TABLE IF NOT EXISTS {table}");
        assert!(
            sql.contains(&statement),
            "tenant foundation migration must create {table}"
        );
    }
}

#[test]
fn vector_outbox_has_required_reliability_fields() {
    let sql = migration_sql();

    for field in [
        "tenant_id TEXT NOT NULL",
        "entry_id TEXT NOT NULL",
        "operation TEXT NOT NULL CHECK (operation IN ('upsert', 'delete'))",
        "payload_hash TEXT NOT NULL",
        "idempotency_key TEXT NOT NULL",
        "attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0)",
        "UNIQUE (tenant_id, idempotency_key)",
    ] {
        assert!(
            sql.contains(field),
            "memory_vector_outbox must include required field or constraint: {field}"
        );
    }

    for status in ["pending", "processing", "applied", "failed", "dead_letter"] {
        assert!(
            sql.contains(status),
            "memory_vector_outbox must include status {status}"
        );
    }
}

#[test]
fn readonly_isolation_uses_readonly_status_not_delete_semantics() {
    let sql = migration_sql();

    assert!(sql.contains("memory_tenant_readonly_isolation"));
    assert!(sql.contains("status TEXT NOT NULL DEFAULT 'readonly'"));
    assert!(sql.contains("CHECK (status IN ('readonly', 'resolved'))"));
    assert!(
        !sql.contains("ON DELETE") || sql.contains("ON DELETE CASCADE"),
        "readonly isolation must not encode destructive cleanup semantics"
    );
}

#[test]
fn reconciliation_tracks_expected_drift_types() {
    let sql = migration_sql();

    for drift_type in [
        "missing",
        "orphan",
        "tenant_mismatch",
        "content_hash_mismatch",
    ] {
        assert!(
            sql.contains(drift_type),
            "reconciliation items must track drift type {drift_type}"
        );
    }

    for action in ["report", "upsert", "delete", "rewrite_payload", "readonly"] {
        assert!(
            sql.contains(action),
            "reconciliation items must track action {action}"
        );
    }
}

#[test]
fn tenant_scoped_indexes_cover_core_access_patterns() {
    let sql = migration_sql();

    for index_name in [
        "idx_context_sessions_tenant_user_status",
        "idx_session_messages_tenant_session",
        "idx_knowledge_entries_tenant_status_created",
        "idx_knowledge_relations_tenant_source",
        "idx_entities_tenant_name_type",
        "idx_relations_tenant_source",
        "idx_multimodal_entries_tenant_source",
        "idx_modality_relations_tenant_source",
        "idx_memory_vector_outbox_tenant_entry",
        "idx_memory_audit_events_tenant_created",
    ] {
        assert!(
            sql.contains(index_name),
            "tenant foundation migration must define index {index_name}"
        );
    }
}

// ── RLS LTM slice (migrations/20260716000100_rls_ltm.sql) — plan §6.1 ──

#[test]
fn rls_ltm_migration_backfills_then_enforces_then_enables_rls() {
    let sql = rls_ltm_sql();

    // M1 backfill from the transitional prefix, with sentinel for unattributable rows.
    assert!(
        sql.contains("split_part(source_id, ':', 2)"),
        "must backfill tenant_id from the source_id prefix"
    );
    assert!(
        sql.contains("source_id LIKE 't:%:%'"),
        "backfill must guard on the well-formed prefix shape"
    );
    assert!(
        sql.contains("memory_tenant_readonly_isolation"),
        "unattributable rows must be registered read-only"
    );
    assert!(
        sql.contains("'__unattributed__'"),
        "unattributable rows must fall back to the sentinel tenant"
    );

    // M2 enforce NOT NULL.
    assert!(
        sql.contains("ALTER COLUMN tenant_id SET NOT NULL"),
        "tenant_id must be enforced NOT NULL"
    );

    // M3 enable + FORCE RLS. FORCE is mandatory because the app role owns the table.
    assert!(
        sql.contains("ENABLE ROW LEVEL SECURITY"),
        "must enable row level security"
    );
    assert!(
        sql.contains("FORCE ROW LEVEL SECURITY"),
        "must FORCE row level security (owner is subject to the policy)"
    );
    assert!(
        sql.contains("CREATE POLICY knowledge_entries_tenant_isolation"),
        "must create the tenant isolation policy"
    );
}

#[test]
fn rls_ltm_policy_is_fail_closed_and_checks_writes() {
    let sql = rls_ltm_sql();

    // Reads the transaction-local GUC with missing_ok=true so an unset GUC yields NULL.
    assert!(
        sql.contains("current_setting('aetheris.tenant_id', true)"),
        "policy must read the transaction-local tenant GUC"
    );
    // Fail-closed: an unset GUC (NULL) must hide all rows, never expose them.
    assert!(
        sql.contains("current_setting('aetheris.tenant_id', true) IS NOT NULL"),
        "policy must fail closed when no tenant GUC is set"
    );
    // WITH CHECK prevents writing rows attributed to another tenant.
    assert!(
        sql.contains("WITH CHECK"),
        "policy must constrain writes with WITH CHECK"
    );
}

/// Guards the RLS-completeness invariants against source regressions (plan §6.1):
/// LTM writes must dual-write the physical tenant_id column, and the tenant GUC
/// must only ever be set transaction-locally (never session-level, which would
/// leak across pooled connections).
#[test]
fn ltm_writes_dual_write_physical_tenant_id_column() {
    let ltm = fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/db/ltm.rs"))
        .expect("db/ltm.rs must be readable");

    // Both INSERT INTO knowledge_entries statements must include the tenant_id column.
    let insert_count = ltm.matches("INSERT INTO knowledge_entries").count();
    assert_eq!(
        insert_count, 2,
        "expected exactly the create + supersede INSERTs into knowledge_entries"
    );
    // The column list of each INSERT must carry tenant_id (dual-write with source_id prefix).
    let tenant_col_inserts = ltm.matches("valid_from, superseded_by, tenant_id").count()
        + ltm.matches("quality_score, status, tenant_id").count();
    assert_eq!(
        tenant_col_inserts, 2,
        "both knowledge_entries INSERTs must dual-write the physical tenant_id column"
    );
}

#[test]
fn tenant_guc_is_only_set_transaction_local() {
    let scope = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/db/tenant_scope.rs"),
    )
    .expect("db/tenant_scope.rs must be readable");

    // set_config's third argument (is_local) must be true so the GUC never leaks to
    // the next request sharing the pooled connection.
    assert!(
        scope.contains("set_config($1, $2, true)"),
        "tenant GUC must be set transaction-locally (is_local = true)"
    );
    // No session-level SET of the tenant GUC anywhere in the executor.
    assert!(
        !scope.contains("SET aetheris.tenant_id"),
        "tenant GUC must never be set session-level (pooled-connection leak risk)"
    );
}
