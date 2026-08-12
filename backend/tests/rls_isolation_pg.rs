//! RLS penetration test — proves knowledge_entries (LTM) tenant isolation is
//! enforced at the PostgreSQL layer, not just by application-layer prefix checks.
//!
//! Reports as `ignored` when `DATABASE_URL` is unset (no false-green pass). The
//! env-var guard inside the body still skips the test when `--include-ignored` is
//! used without DATABASE_URL. Run as the CI gate against a live PG:
//!
//!   DATABASE_URL=postgres://memory:memory@localhost:5432/memory \
//!     cargo test --test rls_isolation_pg -- --nocapture
//!
//! WHY A DEDICATED ROLE: RLS (even FORCE) is bypassed by superusers and roles with
//! BYPASSRLS. The stock dev image connects as `memory`, which is the table owner and
//! a superuser — under it the policy is a NO-OP. This test provisions a restricted
//! role (`aetheris_rls_probe`, NOSUPERUSER NOBYPASSRLS), points the global pool at
//! it, and drives the real `LTMRepository` code paths through it, so the assertions
//! actually exercise the policy. It also proves the app-path methods and a raw
//! `begin_tenant_tx` query agree.

use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::Row;
use std::path::Path;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use backend::db::ltm::LTMRepository;
use backend::db::tenant_scope::begin_tenant_tx;
use backend::tenant::TenantId;

const PROBE_ROLE: &str = "aetheris_rls_probe";
const PROBE_PASSWORD: &str = "aetheris_rls_probe_pw";

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn ltm_rls_blocks_cross_tenant_reads_via_real_repository() {
    let Ok(admin_url) = std::env::var("DATABASE_URL") else {
        eprintln!("SKIP rls_isolation_pg: DATABASE_URL not set");
        return;
    };

    // 1. Admin/owner connection — provisions the restricted role and runs migrations.
    let admin_pool = sqlx::PgPool::connect(&admin_url)
        .await
        .expect("connect to postgres as admin/owner");

    // Ensure this slice's migration (and all prior) are applied. Idempotent: a no-op
    // if the controller already migrated the live DB.
    let migrations_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let migrator = sqlx::migrate::Migrator::new(migrations_path)
        .await
        .expect("build migrator");
    migrator.run(&admin_pool).await.expect("run migrations");

    // Verify RLS is actually enabled+forced on the table; otherwise the test below
    // would pass vacuously.
    let (enabled, forced): (bool, bool) = sqlx::query_as(
        "SELECT relrowsecurity, relforcerowsecurity FROM pg_class WHERE relname = 'knowledge_entries'",
    )
    .fetch_one(&admin_pool)
    .await
    .expect("read pg_class rls flags");
    assert!(
        enabled,
        "knowledge_entries must have ROW LEVEL SECURITY enabled"
    );
    assert!(forced, "knowledge_entries must FORCE ROW LEVEL SECURITY");

    // Provision the restricted probe role (idempotent) and grant table access.
    provision_probe_role(&admin_pool).await;

    // 2. Build a restricted-role pool and install it as the global pool, so the real
    //    LTMRepository write methods (which call db::pool()) go through RLS.
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

    // Unique tenants per run so prefix scans do not collide with prior runs.
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let tenant_a = TenantId::from_string(format!("rls_a_{suffix}"));
    let tenant_b = TenantId::from_string(format!("rls_b_{suffix}"));

    // 3. Tenant A writes one entry through the real repository (RLS WITH CHECK path).
    let dummy_vec = vec![0.0_f32; 4];
    let entry_id = LTMRepository::create_knowledge_entry(
        &tenant_a,
        "rls-probe-source",
        "document",
        Some("rls probe"),
        "secret tenant A content",
        "text",
        &dummy_vec,
        "test-model",
        4,
        Some(0.9),
    )
    .await
    .expect("tenant A create must succeed under its own GUC");

    // 4a. Tenant B reads via the repository → must not see tenant A's row.
    let seen_by_b = LTMRepository::get_entry_by_id(&probe_pool, &tenant_b, &entry_id)
        .await
        .expect("tenant B read returns Ok");
    assert!(
        seen_by_b.is_none(),
        "RLS breach: tenant B saw tenant A's entry via repository"
    );

    // 4b. Tenant A reads via the repository → must see its own row.
    let seen_by_a = LTMRepository::get_entry_by_id(&probe_pool, &tenant_a, &entry_id)
        .await
        .expect("tenant A read returns Ok");
    assert!(
        seen_by_a.is_some(),
        "tenant A must be able to read its own entry"
    );

    // 5. Pure RLS proof (no application-layer starts_with): a raw query on
    //    knowledge_entries inside begin_tenant_tx(B) must return 0 rows.
    let count_as_b = raw_count_by_id(&probe_pool, &tenant_b, &entry_id).await;
    assert_eq!(
        count_as_b, 0,
        "RLS breach: raw begin_tenant_tx(B) query saw tenant A's row"
    );
    let count_as_a = raw_count_by_id(&probe_pool, &tenant_a, &entry_id).await;
    assert_eq!(
        count_as_a, 1,
        "tenant A raw begin_tenant_tx query must see its own row"
    );

    // 6. Fail-closed: a query with NO tenant GUC set must return 0 rows, not all rows.
    let count_no_guc: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM knowledge_entries WHERE entry_id = $1")
            .bind(&entry_id)
            .fetch_one(&probe_pool)
            .await
            .expect("fail-closed count query");
    assert_eq!(
        count_no_guc, 0,
        "RLS must fail closed: no GUC set should hide all rows, not expose them"
    );

    // Best-effort cleanup via the admin (superuser bypasses RLS).
    let _ = sqlx::query("DELETE FROM knowledge_entries WHERE tenant_id = $1 OR tenant_id = $2")
        .bind(tenant_a.as_str())
        .bind(tenant_b.as_str())
        .execute(&admin_pool)
        .await;
}

/// Count rows matching `entry_id` inside a tenant-scoped transaction — the raw RLS
/// probe that bypasses any application-layer tenant filtering.
async fn raw_count_by_id(pool: &sqlx::PgPool, tenant: &TenantId, entry_id: &str) -> i64 {
    let mut tx = begin_tenant_tx(pool, tenant)
        .await
        .expect("begin tenant tx for raw count");
    let row = sqlx::query("SELECT COUNT(*) AS c FROM knowledge_entries WHERE entry_id = $1")
        .bind(entry_id)
        .fetch_one(&mut *tx)
        .await
        .expect("raw count query");
    let count: i64 = row.get("c");
    tx.commit().await.ok();
    count
}

/// Create the restricted probe role (idempotent) and grant it just enough to
/// read/write knowledge_entries. NOSUPERUSER + NOBYPASSRLS so the policy applies.
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
        format!("GRANT SELECT, INSERT, UPDATE, DELETE ON knowledge_entries TO {PROBE_ROLE}"),
    ] {
        sqlx::raw_sql(&stmt)
            .execute(admin_pool)
            .await
            .unwrap_or_else(|e| panic!("grant failed ({stmt}): {e}"));
    }
}
