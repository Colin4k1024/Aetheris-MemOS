//! Persistent audit repository (P1 子项 b).
//!
//! Writes structured audit events to the `memory_audit_events` table created by
//! `migrations/20260706000100_memory_storage_tenant_foundation.sql`. Two entry
//! points are provided:
//!
//! - [`insert_event`] — write one event on the global pool (used by the async
//!   [`crate::services::audit_writer`] background writer for best-effort audit).
//! - [`insert_tx`] — write one event inside an existing transaction, so a mutation
//!   and its audit record commit atomically (used by 子项 a's single-transaction LTM
//!   write path when strong audit consistency is required).
//!
//! Uses the runtime `sqlx::query` API (not the compile-time macros) so this module
//! compiles offline without a `.sqlx` cache, matching the rest of `db/`.

use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use tracing::error;
use ulid::Ulid;

use crate::AppError;

/// A single audit event, mirroring the nullable/NOT NULL shape of
/// `memory_audit_events`. `created_at` is intentionally omitted — the column has a
/// `DEFAULT CURRENT_TIMESTAMP`, so the database stamps it on insert.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    /// ULID primary key (`event_id`).
    pub event_id: String,
    /// Owning tenant, if the request is tenant-scoped (`tenant_id`, nullable).
    pub tenant_id: Option<String>,
    /// Acting principal — user/agent id (`actor_id`, nullable).
    pub actor_id: Option<String>,
    /// Event class, e.g. `memory.write` / `memory.search` (`event_type`, NOT NULL).
    pub event_type: String,
    /// Affected resource kind, e.g. `ltm_entry` / `kg_entity` (`resource_type`, NOT NULL).
    pub resource_type: String,
    /// Affected resource id, if known (`resource_id`, nullable).
    pub resource_id: Option<String>,
    /// Correlation id threading a request across audit records (`correlation_id`, nullable).
    pub correlation_id: Option<String>,
    /// Serialized JSON string stored verbatim in `metadata_json` (NOT NULL, defaults to `{}`).
    pub metadata_json: String,
}

impl AuditEvent {
    /// Build a new event with a generated ULID `event_id`, empty metadata, and no
    /// optional fields set. Chain the builder setters to populate the rest.
    pub fn new(event_type: impl Into<String>, resource_type: impl Into<String>) -> Self {
        Self {
            event_id: Ulid::new().to_string(),
            tenant_id: None,
            actor_id: None,
            event_type: event_type.into(),
            resource_type: resource_type.into(),
            resource_id: None,
            correlation_id: None,
            metadata_json: "{}".to_string(),
        }
    }

    /// Set the owning tenant id.
    pub fn tenant(mut self, tenant_id: impl Into<String>) -> Self {
        self.tenant_id = Some(tenant_id.into());
        self
    }

    /// Set the acting principal id.
    pub fn actor(mut self, actor_id: impl Into<String>) -> Self {
        self.actor_id = Some(actor_id.into());
        self
    }

    /// Set the affected resource id.
    pub fn resource_id(mut self, resource_id: impl Into<String>) -> Self {
        self.resource_id = Some(resource_id.into());
        self
    }

    /// Set the correlation id used to thread audit records for one request.
    pub fn correlation_id(mut self, correlation_id: impl Into<String>) -> Self {
        self.correlation_id = Some(correlation_id.into());
        self
    }

    /// Set `metadata_json` from a raw, already-serialized JSON string.
    pub fn metadata_json(mut self, metadata_json: impl Into<String>) -> Self {
        self.metadata_json = metadata_json.into();
        self
    }

    /// Set `metadata_json` by serializing any value. Falls back to `{}` (with an
    /// `error!` log) if serialization fails, so audit construction never panics.
    pub fn with_metadata<T: Serialize>(mut self, value: &T) -> Self {
        match serde_json::to_string(value) {
            Ok(json) => self.metadata_json = json,
            Err(e) => {
                error!(
                    "Failed to serialize audit metadata: {}; using empty object",
                    e
                );
                self.metadata_json = "{}".to_string();
            }
        }
        self
    }
}

/// Column list / placeholders shared by both insert paths so they can never drift.
const INSERT_AUDIT_SQL: &str = r#"
    INSERT INTO memory_audit_events (
        event_id, tenant_id, actor_id, event_type, resource_type,
        resource_id, correlation_id, metadata_json
    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
"#;

/// Bind and execute the insert against any Postgres executor (`&PgPool` or a
/// transaction connection), keeping the bind order in exactly one place.
async fn exec_insert<'e, E>(executor: E, event: &AuditEvent) -> Result<(), AppError>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    sqlx::query(INSERT_AUDIT_SQL)
        .bind(&event.event_id)
        .bind(event.tenant_id.as_deref())
        .bind(event.actor_id.as_deref())
        .bind(&event.event_type)
        .bind(&event.resource_type)
        .bind(event.resource_id.as_deref())
        .bind(event.correlation_id.as_deref())
        .bind(&event.metadata_json)
        .execute(executor)
        .await
        .map_err(|e| {
            error!("Failed to insert audit event {}: {}", event.event_id, e);
            AppError::Internal(format!("Database error: {}", e))
        })?;
    Ok(())
}

/// Persist one audit event using the global connection pool.
pub async fn insert_event(pool: &PgPool, event: &AuditEvent) -> Result<(), AppError> {
    exec_insert(pool, event).await
}

/// Persist one audit event inside an existing transaction, so the event commits (or
/// rolls back) atomically with the surrounding mutation. Used by 子项 a's LTM write.
pub async fn insert_tx(
    tx: &mut Transaction<'_, Postgres>,
    event: &AuditEvent,
) -> Result<(), AppError> {
    exec_insert(&mut **tx, event).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_event_has_ulid_and_empty_metadata() {
        let ev = AuditEvent::new("memory.write", "ltm_entry");
        // ULID canonical form is 26 characters (Crockford base32).
        assert_eq!(ev.event_id.len(), 26, "event_id should be a 26-char ULID");
        assert_eq!(ev.event_type, "memory.write");
        assert_eq!(ev.resource_type, "ltm_entry");
        assert_eq!(ev.metadata_json, "{}");
        assert!(ev.tenant_id.is_none());
        assert!(ev.actor_id.is_none());
        assert!(ev.resource_id.is_none());
        assert!(ev.correlation_id.is_none());
    }

    #[test]
    fn builder_sets_optional_fields() {
        let ev = AuditEvent::new("memory.search", "kg_entity")
            .tenant("t-1")
            .actor("user-9")
            .resource_id("entry-42")
            .correlation_id("corr-abc");
        assert_eq!(ev.tenant_id.as_deref(), Some("t-1"));
        assert_eq!(ev.actor_id.as_deref(), Some("user-9"));
        assert_eq!(ev.resource_id.as_deref(), Some("entry-42"));
        assert_eq!(ev.correlation_id.as_deref(), Some("corr-abc"));
    }

    #[test]
    fn with_metadata_serialises_to_json_string() {
        let ev = AuditEvent::new("memory.write", "ltm_entry")
            .with_metadata(&serde_json::json!({"decision": "allow", "count": 3}));
        let parsed: serde_json::Value = serde_json::from_str(&ev.metadata_json).unwrap();
        assert_eq!(parsed["decision"], "allow");
        assert_eq!(parsed["count"], 3);
    }

    #[test]
    fn two_new_events_have_distinct_ids() {
        let a = AuditEvent::new("x", "y");
        let b = AuditEvent::new("x", "y");
        assert_ne!(a.event_id, b.event_id, "ULIDs must be unique");
    }

    #[test]
    fn serde_round_trip_preserves_fields() {
        let ev = AuditEvent::new("memory.delete", "mm_entry")
            .tenant("t-7")
            .with_metadata(&serde_json::json!({"k": "v"}));
        let json = serde_json::to_string(&ev).unwrap();
        let back: AuditEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back.event_id, ev.event_id);
        assert_eq!(back.tenant_id, ev.tenant_id);
        assert_eq!(back.event_type, ev.event_type);
        assert_eq!(back.resource_type, ev.resource_type);
        assert_eq!(back.metadata_json, ev.metadata_json);
    }
}
