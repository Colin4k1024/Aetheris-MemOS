//! Per-request tenant-scoped transaction executor — the RLS keystone (P1 PR-1).
//!
//! PostgreSQL Row-Level Security enforces tenant isolation via a
//! transaction-local GUC, `aetheris.tenant_id`, which policies read with
//! `current_setting('aetheris.tenant_id', true)`. This module opens a transaction
//! and sets that GUC so every RLS-protected query executed inside it is scoped to
//! the tenant at the *database* layer — instead of relying on application-layer
//! `t:{tenant}:` prefix checks that any missed query path can bypass.
//!
//! See `docs/artifacts/2026-07-16-enterprise-productionization/p1-execution-runbook.md`
//! (PR-1) and ADR-0001. Repositories are being migrated to take an executor from
//! here instead of the global `pool()`. Uses the runtime `sqlx::query` API, so it
//! does not depend on the compile-time `.sqlx` cache.

use sqlx::{PgPool, Postgres, Transaction};

use crate::error::AppError;
use crate::tenant::TenantId;

/// GUC key RLS policies read via `current_setting(TENANT_GUC, true)`.
///
/// Must be dotted/namespaced so PostgreSQL accepts `set_config` on a custom
/// parameter without it being declared in `postgresql.conf`.
pub const TENANT_GUC: &str = "aetheris.tenant_id";

/// Begin a transaction with the tenant GUC set transaction-locally.
///
/// Every RLS-protected statement executed on the returned transaction is scoped to
/// `tenant_id`. The caller runs queries on `&mut *tx` and must `commit()` on
/// success (dropping the transaction rolls back). The GUC is set with
/// `is_local = true`, so it is automatically cleared when the transaction ends and
/// never leaks to other requests sharing the pooled connection.
pub async fn begin_tenant_tx<'a>(
    pool: &'a PgPool,
    tenant_id: &TenantId,
) -> Result<Transaction<'a, Postgres>, AppError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to begin tenant transaction: {e}")))?;

    sqlx::query("SELECT set_config($1, $2, true)")
        .bind(TENANT_GUC)
        .bind(tenant_id.as_str())
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to set tenant GUC: {e}")))?;

    Ok(tx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::Row;

    #[test]
    fn tenant_guc_key_is_namespaced() {
        // A custom GUC must be dotted, else Postgres rejects set_config on it.
        assert!(TENANT_GUC.contains('.'), "GUC key must be namespaced");
        assert_eq!(TENANT_GUC, "aetheris.tenant_id");
    }

    /// Helper: get a test pool (requires DATABASE_URL env var).
    async fn test_pool() -> PgPool {
        let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
            "postgres://postgres:postgres@localhost:5432/aetheris_test".to_string()
        });
        PgPool::connect(&url)
            .await
            .expect("Failed to connect to test database")
    }

    #[tokio::test]
    async fn begin_tenant_tx_sets_guc() {
        let pool = test_pool().await;
        let tenant_id = TenantId::from_string("test-tenant-1");

        let mut tx = begin_tenant_tx(&pool, &tenant_id)
            .await
            .expect("Failed to begin tenant transaction");

        // Verify GUC is set correctly within the transaction
        let row = sqlx::query("SELECT current_setting($1, true) as val")
            .bind(TENANT_GUC)
            .fetch_one(&mut *tx)
            .await
            .expect("Failed to query GUC");

        let val: Option<String> = row.get("val");
        assert_eq!(val, Some("test-tenant-1".to_string()));

        tx.commit().await.expect("Failed to commit transaction");
    }

    #[tokio::test]
    async fn tenant_tx_isolation() {
        let pool = test_pool().await;
        let tenant_a = TenantId::from_string("tenant-a");
        let tenant_b = TenantId::from_string("tenant-b");

        // Start two concurrent transactions
        let mut tx_a = begin_tenant_tx(&pool, &tenant_a)
            .await
            .expect("Failed to begin tenant A transaction");

        let mut tx_b = begin_tenant_tx(&pool, &tenant_b)
            .await
            .expect("Failed to begin tenant B transaction");

        // Verify each transaction sees its own GUC
        let row_a = sqlx::query("SELECT current_setting($1, true) as val")
            .bind(TENANT_GUC)
            .fetch_one(&mut *tx_a)
            .await
            .expect("Failed to query GUC in tx_a");

        let row_b = sqlx::query("SELECT current_setting($1, true) as val")
            .bind(TENANT_GUC)
            .fetch_one(&mut *tx_b)
            .await
            .expect("Failed to query GUC in tx_b");

        let val_a: Option<String> = row_a.get("val");
        let val_b: Option<String> = row_b.get("val");

        assert_eq!(val_a, Some("tenant-a".to_string()));
        assert_eq!(val_b, Some("tenant-b".to_string()));

        tx_a.commit().await.expect("Failed to commit tx_a");
        tx_b.commit().await.expect("Failed to commit tx_b");
    }

    #[tokio::test]
    async fn tenant_tx_rollback_clears_guc() {
        let pool = test_pool().await;
        let tenant_id = TenantId::from_string("test-tenant-rollback");

        // Begin transaction and set GUC
        let tx = begin_tenant_tx(&pool, &tenant_id)
            .await
            .expect("Failed to begin tenant transaction");

        // Drop transaction (rollback)
        drop(tx);

        // Verify GUC is cleared after rollback
        let mut conn = pool.acquire().await.expect("Failed to acquire connection");
        let row = sqlx::query("SELECT current_setting($1, true) as val")
            .bind(TENANT_GUC)
            .fetch_one(&mut *conn)
            .await
            .expect("Failed to query GUC after rollback");

        let val: Option<String> = row.get("val");
        // After rollback, GUC should be empty or NULL
        assert!(val.is_none() || val == Some("".to_string()));
    }
}
