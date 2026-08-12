//! PG-gated proof that RBAC now reads real membership rows (C-3 / ADR-0009 方案 A).
//!
//! Reports as `ignored` when `DATABASE_URL` is unset (no false-green pass). The
//! env-var guard inside the body still skips when `--include-ignored` is used
//! without a database. CI provides PostgreSQL and runs this for real.
//!
//! WHY A DEDICATED ROLE: `tenant_members` and `tenants` are RLS-protected, and RLS
//! — even FORCE — is bypassed by superusers and BYPASSRLS roles. The stock dev
//! image connects as `memory`, which is the table owner *and* a superuser; under
//! it every policy is a NO-OP and these assertions would pass vacuously. This test
//! provisions `aetheris_members_probe` (NOSUPERUSER NOBYPASSRLS), installs it as
//! the global pool, and drives the real `RbacService` / repository code through
//! it — same approach as `rls_isolation_pg.rs`.
//!
//! ## Why a single test function
//!
//! Separate `#[tokio::test]` functions each create their own runtime. The global
//! `DATABASE_POOL` OnceLock is shared across all of them, but sqlx pool handles
//! are runtime-bound — connections acquired in one runtime's context panic or
//! timeout when polled from another. A shared `OnceLock<Runtime>` + `block_on`
//! triggers "cannot `block_on` from within a runtime" panics because the test
//! harness's own implicit runtime context leaks in.
//!
//! The cost of one big function is negligible (~1s total wall time) and avoids an
//! entire class of runtime-boundary flakiness that is not the system under test.

use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::path::Path;
use std::str::FromStr;

use backend::services::rbac::{Permission, RbacService, Role};
use backend::tenant::TenantId;

const PROBE_ROLE: &str = "aetheris_members_probe";
const PROBE_PASSWORD: &str = "aetheris_members_probe_pw";

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn rbac_reads_tenant_members_under_rls() {
    let Ok(admin_url) = std::env::var("DATABASE_URL") else {
        eprintln!("SKIP tenant_members_pg: DATABASE_URL not set");
        return;
    };

    // ── Bootstrap: migrate, verify RLS, provision restricted role ──

    let admin_pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&admin_url)
        .await
        .expect("connect as admin/owner");

    let migrations_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    sqlx::migrate::Migrator::new(migrations_path)
        .await
        .expect("build migrator")
        .run(&admin_pool)
        .await
        .expect("run migrations");

    for table in ["tenants", "tenant_members"] {
        let (enabled, forced): (bool, bool) = sqlx::query_as(
            "SELECT relrowsecurity, relforcerowsecurity FROM pg_class WHERE relname = $1",
        )
        .bind(table)
        .fetch_one(&admin_pool)
        .await
        .unwrap_or_else(|e| panic!("read pg_class flags for {table}: {e}"));
        assert!(enabled, "{table} must have ROW LEVEL SECURITY enabled");
        assert!(forced, "{table} must FORCE ROW LEVEL SECURITY");
    }

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
        .execute(&admin_pool)
        .await
        .expect("create probe role");
    for stmt in [
        format!("GRANT USAGE ON SCHEMA public TO {PROBE_ROLE}"),
        format!("GRANT SELECT, INSERT, UPDATE, DELETE ON tenants TO {PROBE_ROLE}"),
        format!("GRANT SELECT, INSERT, UPDATE, DELETE ON tenant_members TO {PROBE_ROLE}"),
        format!("GRANT SELECT, INSERT, UPDATE, DELETE ON users TO {PROBE_ROLE}"),
    ] {
        sqlx::raw_sql(&stmt)
            .execute(&admin_pool)
            .await
            .unwrap_or_else(|e| panic!("grant failed ({stmt}): {e}"));
    }

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
        .set(backend::db::DatabasePool::Postgres(probe_pool))
        .map_err(|_| ())
        .expect("install probe pool as global (test binary owns the OnceLock)");

    // ── Seed: one org, an owner, a reader, and a non-member ──

    let org = format!("org_c3_{}", std::process::id());
    let owner = format!("u_owner_{}", std::process::id());
    let reader = format!("u_reader_{}", std::process::id());
    let outsider = format!("u_outsider_{}", std::process::id());

    for u in [&owner, &reader, &outsider] {
        sqlx::query("INSERT INTO users (id, username, password) VALUES ($1, $1, 'x')")
            .bind(u)
            .execute(&admin_pool)
            .await
            .expect("seed user");
    }
    sqlx::query("INSERT INTO tenants (tenant_id, name) VALUES ($1, $1)")
        .bind(&org)
        .execute(&admin_pool)
        .await
        .expect("seed org");
    for (u, role) in [(&owner, "owner"), (&reader, "reader")] {
        sqlx::query("INSERT INTO tenant_members (tenant_id, user_id, role) VALUES ($1, $2, $3)")
            .bind(&org)
            .bind(u)
            .bind(role)
            .execute(&admin_pool)
            .await
            .expect("seed membership");
    }

    let rbac = RbacService::new();

    // ── Scenario 1: two members of the SAME org get DIFFERENT answers ──
    // This is the headline C-3 result. Before this change every caller was Owner.

    assert_eq!(rbac.get_role(&org, &owner).await, Some(Role::Owner));
    assert_eq!(rbac.get_role(&org, &reader).await, Some(Role::Reader));

    assert!(rbac.has_permission(&org, &owner, Permission::Write).await);
    assert!(
        !rbac.has_permission(&org, &reader, Permission::Write).await,
        "a Reader must not hold Write — this is the distinction that did not exist before C-3"
    );
    assert!(rbac.has_permission(&org, &reader, Permission::Read).await);

    // ── Scenario 2: no membership row → denied ──

    assert_eq!(rbac.get_role(&org, &outsider).await, None);
    assert!(!rbac.has_permission(&org, &outsider, Permission::Read).await);

    // ── Scenario 3: membership does not leak across orgs ──

    let other_org = format!("{org}_other");
    assert_eq!(rbac.get_role(&other_org, &owner).await, None);

    // ── Scenario 4: assign_role persists and is readable back ──

    rbac.assign_role(&org, &reader, Role::Admin, &owner)
        .await
        .expect("assign role");

    let fresh = RbacService::new();
    assert_eq!(fresh.get_role(&org, &reader).await, Some(Role::Admin));

    let listed = fresh.list_roles(&org).await.expect("list roles");
    assert!(
        listed
            .iter()
            .any(|r| r.user_id == reader && r.role == Role::Admin),
        "the reassignment must appear in the member listing"
    );

    // ── Scenario 5: cross-org write refused by the database ──

    let foreign_org = TenantId::from_string(format!("{org}_foreign"));
    let result =
        backend::db::tenant_members::upsert_member(&foreign_org, &outsider, Role::Owner, &owner)
            .await;
    assert!(
        result.is_err(),
        "writing into an org the caller is not scoped to must fail at the RLS WITH CHECK"
    );
}
