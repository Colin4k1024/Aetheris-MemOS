//! RLS penetration test — proves multimodal_entries + modality_relations (MM) tenant
//! isolation is enforced at the PostgreSQL layer, not just by the application-layer
//! content_metadata JSON tenant filter.
//!
//! Skips (does not fail) when `DATABASE_URL` is unset, so the offline `cargo test`
//! stays green. Run as the CI gate against a live PG:
//!
//!   DATABASE_URL=postgres://memory:memory@localhost:5432/memory \
//!     cargo test --test rls_mm_pg -- --nocapture
//!
//! WHY A DEDICATED ROLE: RLS (even FORCE) is bypassed by superusers and roles with
//! BYPASSRLS. The stock dev image connects as `memory`, which is the table owner and
//! a superuser — under it the policy is a NO-OP. This test provisions a restricted
//! role (`aetheris_rls_probe`, NOSUPERUSER NOBYPASSRLS), points the global pool at
//! it, and drives the real `MMRepository` code paths through it, so the assertions
//! actually exercise the policy. It covers the MM slice's two hardest properties:
//!   1. multimodal_entries isolation (repository + raw begin_tenant_tx + fail-closed).
//!      MM stores tenant in a content_metadata JSON field, NOT an id prefix, so this
//!      proves the physical tenant_id column + policy isolate — independent of the JSON.
//!   2. modality_relations isolation — relations have no tenant of their own; the
//!      physical tenant_id column (written by create_relation) is what RLS keys on.

use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::Row;
use std::path::Path;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use backend::db::mm::MMRepository;
use backend::db::tenant_scope::begin_tenant_tx;
use backend::tenant::TenantId;

const PROBE_ROLE: &str = "aetheris_rls_probe";
const PROBE_PASSWORD: &str = "aetheris_rls_probe_pw";

#[tokio::test]
async fn mm_rls_blocks_cross_tenant_access_via_real_repository() {
    let Ok(admin_url) = std::env::var("DATABASE_URL") else {
        eprintln!("SKIP rls_mm_pg: DATABASE_URL not set");
        return;
    };

    // 1. Admin/owner connection — provisions the restricted role and runs migrations.
    let admin_pool = sqlx::PgPool::connect(&admin_url)
        .await
        .expect("connect to postgres as admin/owner");

    // Ensure this slice's migration (and all prior) are applied. Idempotent.
    let migrations_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let migrator = sqlx::migrate::Migrator::new(migrations_path)
        .await
        .expect("build migrator");
    migrator.run(&admin_pool).await.expect("run migrations");

    // Verify RLS is actually enabled+forced on BOTH tables; otherwise assertions
    // below would pass vacuously.
    for table in ["multimodal_entries", "modality_relations"] {
        let (enabled, forced): (bool, bool) = sqlx::query_as(
            "SELECT relrowsecurity, relforcerowsecurity FROM pg_class WHERE relname = $1",
        )
        .bind(table)
        .fetch_one(&admin_pool)
        .await
        .unwrap_or_else(|e| panic!("read pg_class rls flags for {table}: {e}"));
        assert!(enabled, "{table} must have ROW LEVEL SECURITY enabled");
        assert!(forced, "{table} must FORCE ROW LEVEL SECURITY");
    }

    // Provision the restricted probe role (idempotent) and grant table access.
    provision_probe_role(&admin_pool).await;

    // 2. Build a restricted-role pool and install it as the global pool, so the real
    //    MMRepository methods (which call db::pool()) go through RLS.
    let probe_opts = PgConnectOptions::from_str(&admin_url)
        .expect("parse DATABASE_URL")
        .username(PROBE_ROLE)
        .password(PROBE_PASSWORD);
    let probe_pool = PgPoolOptions::new()
        .max_connections(4)
        .connect_with(probe_opts)
        .await
        .expect("connect as restricted probe role");

    backend::db::DATABASE_POOL
        .set(backend::db::DatabasePool::Postgres(probe_pool.clone()))
        .map_err(|_| ())
        .expect("install probe pool as global (test binary owns the OnceLock)");

    // Unique tenants per run so scans do not collide with prior runs.
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let tenant_a = TenantId::from_string(format!("rls_a_{suffix}"));
    let tenant_b = TenantId::from_string(format!("rls_b_{suffix}"));

    // 3. Tenant A creates two entries + a relation between them through the real
    //    repository (RLS WITH CHECK path). create_entry returns the bare ULID entry_id
    //    (the PK); the physical tenant_id column is double-written from tenant A.
    let src_id = MMRepository::create_entry(
        Some("rls-probe-session"),
        "rls-probe-source",
        "text",
        "{}",
        Some("secret tenant A multimodal content"),
        None,
        None,
        None,
        Some(tenant_a.as_str()),
    )
    .await
    .expect("tenant A create source entry must succeed under its own GUC");
    let dst_id = MMRepository::create_entry(
        Some("rls-probe-session"),
        "rls-probe-target",
        "text",
        "{}",
        Some("tenant A target multimodal content"),
        None,
        None,
        None,
        Some(tenant_a.as_str()),
    )
    .await
    .expect("tenant A create target entry must succeed");

    let relation_id = MMRepository::create_relation(
        &src_id,
        &dst_id,
        "semantic",
        1.0,
        1.0,
        None,
        Some(tenant_a.as_str()),
    )
    .await
    .expect("tenant A create relation must succeed under its own GUC");

    // ── multimodal_entries isolation ─────────────────────────────────────────

    // 4a. Tenant B reads the entry via the repository → must not see tenant A's row.
    let seen_by_b = MMRepository::get_entry_by_id(&src_id, Some(tenant_b.as_str()))
        .await
        .expect("tenant B entry read returns Ok");
    assert!(
        seen_by_b.is_none(),
        "RLS breach: tenant B saw tenant A's multimodal entry via repository"
    );

    // 4b. Tenant A reads via the repository → must see its own row.
    let seen_by_a = MMRepository::get_entry_by_id(&src_id, Some(tenant_a.as_str()))
        .await
        .expect("tenant A entry read returns Ok");
    assert!(
        seen_by_a.is_some(),
        "tenant A must be able to read its own multimodal entry"
    );

    // 4c. list_entries is RLS-scoped; tenant B sees zero, tenant A sees its two.
    let list_b = MMRepository::list_entries(None, Some(50), Some(0), Some(tenant_b.as_str()))
        .await
        .expect("tenant B list_entries returns Ok");
    assert_eq!(
        list_b.total, 0,
        "RLS breach: tenant B's multimodal entry list is non-empty"
    );
    let list_a = MMRepository::list_entries(None, Some(50), Some(0), Some(tenant_a.as_str()))
        .await
        .expect("tenant A list_entries returns Ok");
    assert!(
        list_a.total >= 2,
        "tenant A must see its own two entries (got {})",
        list_a.total
    );

    // 5. Pure RLS proof for entries (no application-layer JSON filter): a raw query
    //    inside begin_tenant_tx(B) must return 0 rows.
    let entry_count_b = raw_count(
        &probe_pool,
        &tenant_b,
        "multimodal_entries",
        "entry_id",
        &src_id,
    )
    .await;
    assert_eq!(
        entry_count_b, 0,
        "RLS breach: raw begin_tenant_tx(B) query saw tenant A's entry"
    );
    let entry_count_a = raw_count(
        &probe_pool,
        &tenant_a,
        "multimodal_entries",
        "entry_id",
        &src_id,
    )
    .await;
    assert_eq!(
        entry_count_a, 1,
        "tenant A raw begin_tenant_tx query must see its own entry"
    );

    // 6. Fail-closed for entries: NO tenant GUC set must return 0 rows, not all rows.
    let entry_count_no_guc: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM multimodal_entries WHERE entry_id = $1")
            .bind(&src_id)
            .fetch_one(&probe_pool)
            .await
            .expect("fail-closed entry count query");
    assert_eq!(
        entry_count_no_guc, 0,
        "RLS must fail closed on multimodal_entries: no GUC set should hide all rows"
    );

    // ── modality_relations isolation ─────────────────────────────────────────
    // relations carry no tenant field; RLS keys purely on the physical tenant_id
    // column written by create_relation. These assertions prove that column + policy
    // isolate, independent of the entries' content_metadata JSON.

    // 7. Raw RLS proof: relation is visible to A, invisible to B.
    let relation_count_b = raw_count(
        &probe_pool,
        &tenant_b,
        "modality_relations",
        "relation_id",
        &relation_id,
    )
    .await;
    assert_eq!(
        relation_count_b, 0,
        "RLS breach: tenant B saw tenant A's relation (physical tenant_id not enforced)"
    );
    let relation_count_a = raw_count(
        &probe_pool,
        &tenant_a,
        "modality_relations",
        "relation_id",
        &relation_id,
    )
    .await;
    assert_eq!(
        relation_count_a, 1,
        "tenant A must see its own relation via begin_tenant_tx"
    );

    // 8. Fail-closed for relations: NO tenant GUC set must return 0 rows.
    let relation_count_no_guc: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM modality_relations WHERE relation_id = $1")
            .bind(&relation_id)
            .fetch_one(&probe_pool)
            .await
            .expect("fail-closed relation count query");
    assert_eq!(
        relation_count_no_guc, 0,
        "RLS must fail closed on modality_relations: no GUC set should hide all rows"
    );

    // 9. get_related_entries via the repository: tenant B gets nothing, tenant A
    //    resolves the relation + its target entry (proves the path is scoped, not
    //    fail-closed to empty, for the owning tenant).
    let related_b = MMRepository::get_related_entries(&src_id, Some(10), Some(tenant_b.as_str()))
        .await
        .expect("tenant B get_related_entries returns Ok");
    assert!(
        related_b.is_empty(),
        "RLS breach: tenant B resolved tenant A's relations"
    );
    let related_a = MMRepository::get_related_entries(&src_id, Some(10), Some(tenant_a.as_str()))
        .await
        .expect("tenant A get_related_entries returns Ok");
    assert!(
        !related_a.is_empty(),
        "tenant A must resolve its own relation + target entry"
    );

    // Best-effort cleanup via the admin (superuser bypasses RLS). Delete relations
    // first (FK), then entries.
    let _ = sqlx::query("DELETE FROM modality_relations WHERE tenant_id = $1 OR tenant_id = $2")
        .bind(tenant_a.as_str())
        .bind(tenant_b.as_str())
        .execute(&admin_pool)
        .await;
    let _ = sqlx::query("DELETE FROM multimodal_entries WHERE tenant_id = $1 OR tenant_id = $2")
        .bind(tenant_a.as_str())
        .bind(tenant_b.as_str())
        .execute(&admin_pool)
        .await;
}

/// Count rows matching `id_column = id_value` inside a tenant-scoped transaction —
/// the raw RLS probe that bypasses any application-layer tenant filtering.
async fn raw_count(
    pool: &sqlx::PgPool,
    tenant: &TenantId,
    table: &str,
    id_column: &str,
    id_value: &str,
) -> i64 {
    let mut tx = begin_tenant_tx(pool, tenant)
        .await
        .expect("begin tenant tx for raw count");
    // table/id_column are test-controlled constants, not user input.
    let sql = format!("SELECT COUNT(*) AS c FROM {table} WHERE {id_column} = $1");
    let row = sqlx::query(&sql)
        .bind(id_value)
        .fetch_one(&mut *tx)
        .await
        .expect("raw count query");
    let count: i64 = row.get("c");
    tx.commit().await.ok();
    count
}

/// Create the restricted probe role (idempotent) and grant it just enough to
/// read/write multimodal_entries + modality_relations. NOSUPERUSER + NOBYPASSRLS so
/// the policy applies.
async fn provision_probe_role(admin_pool: &sqlx::PgPool) {
    let create_role = format!(
        r#"
        DO $$
        BEGIN
            IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = '{role}') THEN
                CREATE ROLE {role} LOGIN PASSWORD '{pw}' NOSUPERUSER NOBYPASSRLS;
            END IF;
        END
        $$;
        "#,
        role = PROBE_ROLE,
        pw = PROBE_PASSWORD,
    );
    sqlx::raw_sql(&create_role)
        .execute(admin_pool)
        .await
        .expect("create probe role");

    // Schema usage + table DML (all idempotent).
    for stmt in [
        format!("GRANT USAGE ON SCHEMA public TO {PROBE_ROLE}"),
        format!("GRANT SELECT, INSERT, UPDATE, DELETE ON multimodal_entries TO {PROBE_ROLE}"),
        format!("GRANT SELECT, INSERT, UPDATE, DELETE ON modality_relations TO {PROBE_ROLE}"),
    ] {
        sqlx::raw_sql(&stmt)
            .execute(admin_pool)
            .await
            .unwrap_or_else(|e| panic!("grant failed ({stmt}): {e}"));
    }
}
