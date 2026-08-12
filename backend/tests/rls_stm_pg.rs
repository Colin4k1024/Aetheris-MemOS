//! RLS penetration test — proves context_sessions + session_messages (STM) tenant
//! isolation is enforced at the PostgreSQL layer, not just by application-layer
//! user_id prefix checks.
//!
//! Reports as `ignored` when `DATABASE_URL` is unset (no false-green pass). The
//! env-var guard inside the body still skips the test when `--include-ignored` is
//! used without DATABASE_URL. Run as the CI gate against a live PG:
//!
//!   DATABASE_URL=postgres://memory:memory@localhost:5432/memory \
//!     cargo test --test rls_stm_pg -- --nocapture
//!
//! WHY A DEDICATED ROLE: RLS (even FORCE) is bypassed by superusers and roles with
//! BYPASSRLS. The stock dev image connects as `memory`, which is the table owner and
//! a superuser — under it the policy is a NO-OP. This test provisions a restricted
//! role (`aetheris_rls_probe`, NOSUPERUSER NOBYPASSRLS), points the global pool at
//! it, and drives the real `STMRepository` code paths through it, so the assertions
//! actually exercise the policy. It also proves the app-path methods and raw
//! `begin_tenant_tx` queries agree, for BOTH the session table and the messages
//! table (which is only reachable through its parent session's tenant).

use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::Row;
use std::path::Path;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use backend::db::stm::STMRepository;
use backend::db::tenant_scope::begin_tenant_tx;
use backend::tenant::TenantId;

const PROBE_ROLE: &str = "aetheris_rls_probe";
const PROBE_PASSWORD: &str = "aetheris_rls_probe_pw";

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn stm_rls_blocks_cross_tenant_reads_via_real_repository() {
    let Ok(admin_url) = std::env::var("DATABASE_URL") else {
        eprintln!("SKIP rls_stm_pg: DATABASE_URL not set");
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

    // Verify RLS is enabled+forced on BOTH tables; otherwise assertions pass vacuously.
    for table in ["context_sessions", "session_messages"] {
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
    //    STMRepository methods (which call db::pool()) go through RLS.
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

    // 3. Tenant A creates a session + message through the real repository. Both writes
    //    exercise the RLS WITH CHECK path on the physical tenant_id column.
    let session_id = STMRepository::create_session(
        &tenant_a,
        "rls-probe-user",
        "rls-probe-agent",
        "conversation",
        4096,
        24,
    )
    .await
    .expect("tenant A create_session must succeed under its own GUC");

    let message_id = STMRepository::add_message(
        &probe_pool,
        &tenant_a,
        &session_id,
        "user",
        "secret tenant A message",
        Some(5),
        Some(0.9),
    )
    .await
    .expect("tenant A add_message must succeed under its own GUC");

    // 4a. Tenant B reads the session via the repository → must not see tenant A's row.
    let seen_by_b = STMRepository::get_session(&probe_pool, &tenant_b, &session_id)
        .await
        .expect("tenant B get_session returns Ok");
    assert!(
        seen_by_b.is_none(),
        "RLS breach: tenant B saw tenant A's session via repository"
    );

    // 4b. Tenant B reads the messages → must get an empty list (session invisible).
    let msgs_seen_by_b =
        STMRepository::get_session_messages(&probe_pool, &tenant_b, &session_id, None)
            .await
            .expect("tenant B get_session_messages returns Ok");
    assert!(
        msgs_seen_by_b.is_empty(),
        "RLS breach: tenant B saw tenant A's messages via repository"
    );

    // 4c. Tenant A reads its own session + messages → must succeed.
    let seen_by_a = STMRepository::get_session(&probe_pool, &tenant_a, &session_id)
        .await
        .expect("tenant A get_session returns Ok");
    assert!(
        seen_by_a.is_some(),
        "tenant A must be able to read its own session"
    );
    let msgs_seen_by_a =
        STMRepository::get_session_messages(&probe_pool, &tenant_a, &session_id, None)
            .await
            .expect("tenant A get_session_messages returns Ok");
    assert_eq!(
        msgs_seen_by_a.len(),
        1,
        "tenant A must be able to read its own message"
    );

    // 5. Pure RLS proof (no application-layer prefix check): raw queries inside
    //    begin_tenant_tx(B) must return 0 rows on BOTH tables; tenant A sees its own.
    assert_eq!(
        raw_count(
            &probe_pool,
            &tenant_b,
            "context_sessions",
            "session_id",
            &session_id
        )
        .await,
        0,
        "RLS breach: raw begin_tenant_tx(B) saw tenant A's session row"
    );
    assert_eq!(
        raw_count(
            &probe_pool,
            &tenant_a,
            "context_sessions",
            "session_id",
            &session_id
        )
        .await,
        1,
        "tenant A raw query must see its own session row"
    );
    assert_eq!(
        raw_count(
            &probe_pool,
            &tenant_b,
            "session_messages",
            "message_id",
            &message_id
        )
        .await,
        0,
        "RLS breach: raw begin_tenant_tx(B) saw tenant A's message row"
    );
    assert_eq!(
        raw_count(
            &probe_pool,
            &tenant_a,
            "session_messages",
            "message_id",
            &message_id
        )
        .await,
        1,
        "tenant A raw query must see its own message row"
    );

    // 6. Fail-closed: queries with NO tenant GUC set must return 0 rows, not all rows.
    let sessions_no_guc: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM context_sessions WHERE session_id = $1")
            .bind(&session_id)
            .fetch_one(&probe_pool)
            .await
            .expect("fail-closed session count query");
    assert_eq!(
        sessions_no_guc, 0,
        "RLS must fail closed: no GUC set should hide context_sessions rows"
    );
    let messages_no_guc: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM session_messages WHERE message_id = $1")
            .bind(&message_id)
            .fetch_one(&probe_pool)
            .await
            .expect("fail-closed message count query");
    assert_eq!(
        messages_no_guc, 0,
        "RLS must fail closed: no GUC set should hide session_messages rows"
    );

    // 7. Cross-tenant WRITE is refused: tenant B trying to add a message to tenant A's
    //    session must not succeed (the session is invisible to B → NotFound, never a
    //    silent cross-tenant insert).
    let cross_write = STMRepository::add_message(
        &probe_pool,
        &tenant_b,
        &session_id,
        "user",
        "tenant B injection attempt",
        Some(1),
        Some(0.1),
    )
    .await;
    assert!(
        cross_write.is_err(),
        "RLS breach: tenant B was able to write a message into tenant A's session"
    );

    // Best-effort cleanup via the admin (superuser bypasses RLS).
    let _ = sqlx::query("DELETE FROM session_messages WHERE tenant_id = $1 OR tenant_id = $2")
        .bind(tenant_a.as_str())
        .bind(tenant_b.as_str())
        .execute(&admin_pool)
        .await;
    let _ = sqlx::query("DELETE FROM context_sessions WHERE tenant_id = $1 OR tenant_id = $2")
        .bind(tenant_a.as_str())
        .bind(tenant_b.as_str())
        .execute(&admin_pool)
        .await;
}

/// Count rows matching `id_col = id` inside a tenant-scoped transaction — the raw RLS
/// probe that bypasses any application-layer tenant filtering. `table` and `id_col`
/// are fixed string literals from the caller (never user input).
async fn raw_count(
    pool: &sqlx::PgPool,
    tenant: &TenantId,
    table: &str,
    id_col: &str,
    id: &str,
) -> i64 {
    let mut tx = begin_tenant_tx(pool, tenant)
        .await
        .expect("begin tenant tx for raw count");
    let sql = format!("SELECT COUNT(*) AS c FROM {table} WHERE {id_col} = $1");
    let row = sqlx::query(&sql)
        .bind(id)
        .fetch_one(&mut *tx)
        .await
        .expect("raw count query");
    let count: i64 = row.get("c");
    tx.commit().await.ok();
    count
}

/// Create the restricted probe role (idempotent) and grant it just enough to
/// read/write the STM tables. NOSUPERUSER + NOBYPASSRLS so the policy applies.
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
        format!("GRANT SELECT, INSERT, UPDATE, DELETE ON context_sessions TO {PROBE_ROLE}"),
        format!("GRANT SELECT, INSERT, UPDATE, DELETE ON session_messages TO {PROBE_ROLE}"),
    ] {
        sqlx::raw_sql(&stmt)
            .execute(admin_pool)
            .await
            .unwrap_or_else(|e| panic!("grant failed ({stmt}): {e}"));
    }
}
