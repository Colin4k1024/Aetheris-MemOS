//! Reconciliation run/item repository (W1.1).
//!
//! PostgreSQL-only. Records drift between DB knowledge entries and Qdrant points,
//! and the actions taken to repair them. Used by the reconciliation scanner
//! [`crate::services::vector_reconciliation`].
//!
//! Uses the runtime `sqlx::query` API (not the compile-time macros) so this module
//! compiles offline without a `.sqlx` cache, matching the rest of `db/`.

use serde::{Deserialize, Serialize};
use tracing::error;
use ulid::Ulid;

use crate::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReconciliationMode {
    DryRun,
    Repair,
}

impl ReconciliationMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DryRun => "dry_run",
            Self::Repair => "repair",
        }
    }

    pub fn parse(s: &str) -> Result<Self, AppError> {
        match s {
            "dry_run" => Ok(Self::DryRun),
            "repair" => Ok(Self::Repair),
            other => Err(AppError::Internal(format!(
                "unknown reconciliation mode: {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReconciliationStatus {
    Running,
    Completed,
    Failed,
}

impl ReconciliationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftType {
    Missing,
    Orphan,
    TenantMismatch,
    ContentHashMismatch,
}

impl DriftType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Orphan => "orphan",
            Self::TenantMismatch => "tenant_mismatch",
            Self::ContentHashMismatch => "content_hash_mismatch",
        }
    }

    pub fn parse(s: &str) -> Result<Self, AppError> {
        match s {
            "missing" => Ok(Self::Missing),
            "orphan" => Ok(Self::Orphan),
            "tenant_mismatch" => Ok(Self::TenantMismatch),
            "content_hash_mismatch" => Ok(Self::ContentHashMismatch),
            other => Err(AppError::Internal(format!(
                "unknown drift type: {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReconciliationAction {
    Report,
    Upsert,
    Delete,
    RewritePayload,
    Readonly,
}

impl ReconciliationAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Report => "report",
            Self::Upsert => "upsert",
            Self::Delete => "delete",
            Self::RewritePayload => "rewrite_payload",
            Self::Readonly => "readonly",
        }
    }

    pub fn parse(s: &str) -> Result<Self, AppError> {
        match s {
            "report" => Ok(Self::Report),
            "upsert" => Ok(Self::Upsert),
            "delete" => Ok(Self::Delete),
            "rewrite_payload" => Ok(Self::RewritePayload),
            "readonly" => Ok(Self::Readonly),
            other => Err(AppError::Internal(format!(
                "unknown reconciliation action: {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconciliationRun {
    pub run_id: String,
    pub mode: String,
    pub status: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub summary_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconciliationItem {
    pub run_id: String,
    pub drift_type: String,
    pub entry_id: Option<String>,
    pub action: String,
    pub resolved: bool,
}

pub struct ReconciliationRepository;

impl ReconciliationRepository {
    pub async fn create_run(mode: &str) -> Result<String, AppError> {
        let mode_enum = ReconciliationMode::parse(mode)?;
        let run_id = Ulid::new().to_string();
        let pool = crate::db::pool();

        sqlx::query(
            r#"
            INSERT INTO memory_vector_reconciliation_runs (run_id, mode, status)
            VALUES ($1, $2, 'running')
            "#,
        )
        .bind(&run_id)
        .bind(mode_enum.as_str())
        .execute(pool)
        .await
        .map_err(|e| {
            error!("Failed to create reconciliation run: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        Ok(run_id)
    }

    pub async fn add_item(
        run_id: &str,
        drift_type: &str,
        entry_id: Option<&str>,
        action: &str,
    ) -> Result<String, AppError> {
        let drift_enum = DriftType::parse(drift_type)?;
        let action_enum = ReconciliationAction::parse(action)?;
        let item_id = Ulid::new().to_string();
        let pool = crate::db::pool();

        sqlx::query(
            r#"
            INSERT INTO memory_vector_reconciliation_items (
                item_id, run_id, entry_id, drift_type, action
            ) VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(&item_id)
        .bind(run_id)
        .bind(entry_id)
        .bind(drift_enum.as_str())
        .bind(action_enum.as_str())
        .execute(pool)
        .await
        .map_err(|e| {
            error!("Failed to add reconciliation item: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        Ok(item_id)
    }

    pub async fn complete_run(run_id: &str, summary_json: &str) -> Result<(), AppError> {
        let pool = crate::db::pool();

        sqlx::query(
            r#"
            UPDATE memory_vector_reconciliation_runs
            SET status = 'completed',
                completed_at = CURRENT_TIMESTAMP,
                summary_json = $2
            WHERE run_id = $1
            "#,
        )
        .bind(run_id)
        .bind(summary_json)
        .execute(pool)
        .await
        .map_err(|e| {
            error!("Failed to complete reconciliation run: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        Ok(())
    }

    pub async fn fail_run(run_id: &str, error_message: &str) -> Result<(), AppError> {
        let pool = crate::db::pool();

        sqlx::query(
            r#"
            UPDATE memory_vector_reconciliation_runs
            SET status = 'failed',
                completed_at = CURRENT_TIMESTAMP,
                error_message = $2
            WHERE run_id = $1
            "#,
        )
        .bind(run_id)
        .bind(error_message)
        .execute(pool)
        .await
        .map_err(|e| {
            error!("Failed to mark reconciliation run as failed: {}", e);
            AppError::Internal(format!("Database error: {}", e))
        })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_roundtrip() {
        assert_eq!(
            ReconciliationMode::parse("dry_run").unwrap(),
            ReconciliationMode::DryRun
        );
        assert_eq!(
            ReconciliationMode::parse("repair").unwrap(),
            ReconciliationMode::Repair
        );
        assert!(ReconciliationMode::parse("nope").is_err());
        assert_eq!(ReconciliationMode::DryRun.as_str(), "dry_run");
        assert_eq!(ReconciliationMode::Repair.as_str(), "repair");
    }

    #[test]
    fn drift_type_roundtrip() {
        assert_eq!(DriftType::parse("missing").unwrap(), DriftType::Missing);
        assert_eq!(DriftType::parse("orphan").unwrap(), DriftType::Orphan);
        assert_eq!(
            DriftType::parse("tenant_mismatch").unwrap(),
            DriftType::TenantMismatch
        );
        assert_eq!(
            DriftType::parse("content_hash_mismatch").unwrap(),
            DriftType::ContentHashMismatch
        );
        assert!(DriftType::parse("unknown").is_err());
        assert_eq!(DriftType::ContentHashMismatch.as_str(), "content_hash_mismatch");
    }

    #[test]
    fn action_roundtrip() {
        assert_eq!(
            ReconciliationAction::parse("report").unwrap(),
            ReconciliationAction::Report
        );
        assert_eq!(
            ReconciliationAction::parse("rewrite_payload").unwrap(),
            ReconciliationAction::RewritePayload
        );
        assert!(ReconciliationAction::parse("invalid").is_err());
        assert_eq!(ReconciliationAction::RewritePayload.as_str(), "rewrite_payload");
    }

    #[test]
    fn status_as_str() {
        assert_eq!(ReconciliationStatus::Running.as_str(), "running");
        assert_eq!(ReconciliationStatus::Completed.as_str(), "completed");
        assert_eq!(ReconciliationStatus::Failed.as_str(), "failed");
    }
}
