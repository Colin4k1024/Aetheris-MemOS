//! Memory event stream models (#126).
//!
//! Every conversation turn, agent reply, tool result, system note and external
//! CRM/HR record lands in `memory_events` **first** — the append-only log is the
//! source of truth for debugging, compliance and the later distillation stages
//! (#127+#). Rows are never updated or deleted by application code: corrections
//! are new compensation events, and the migration revokes UPDATE/DELETE from the
//! hardened app role.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Build the caller-facing message for an invalid enum value.
fn invalid_value_message(field: &str, got: &str, valid: &[&str]) -> String {
    format!(
        "invalid {field}: '{got}' is not a valid value; valid values are: {}",
        valid.join(", ")
    )
}

/// What happened, at the coarsest useful grain.
///
/// Source of truth: `migrations/20260828000001_memory_event_stream_and_principals.sql`
/// (`CHECK (event_type IN (...))`) — kept in lockstep by the anti-drift test below.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryEventType {
    /// A message authored by the user (conversation).
    UserMessage,
    /// A reply produced by the agent (conversation).
    AgentReply,
    /// Output returned by a tool invocation.
    ToolResult,
    /// Internal lifecycle fact recorded for audit/reconciliation (e.g. a
    /// principal merge/unmerge was performed).
    SystemEvent,
    /// A record pushed from a system of record (CRM contact update, HR
    /// status change). Carries its external origin in metadata.
    ExternalRecord,
}

impl MemoryEventType {
    /// All valid values, ordered as in the migration CHECK clause.
    pub const ALL: &'static [&'static str] = &[
        "user_message",
        "agent_reply",
        "tool_result",
        "system_event",
        "external_record",
    ];

    /// Canonical string persisted to the DB.
    pub fn as_str(self) -> &'static str {
        match self {
            MemoryEventType::UserMessage => "user_message",
            MemoryEventType::AgentReply => "agent_reply",
            MemoryEventType::ToolResult => "tool_result",
            MemoryEventType::SystemEvent => "system_event",
            MemoryEventType::ExternalRecord => "external_record",
        }
    }

    /// Exact-match parse; `Err` lists every valid value.
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "user_message" => Ok(MemoryEventType::UserMessage),
            "agent_reply" => Ok(MemoryEventType::AgentReply),
            "tool_result" => Ok(MemoryEventType::ToolResult),
            "system_event" => Ok(MemoryEventType::SystemEvent),
            "external_record" => Ok(MemoryEventType::ExternalRecord),
            other => Err(invalid_value_message("event type", other, Self::ALL)),
        }
    }
}

/// One row of `memory_events`.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct MemoryEvent {
    pub id: String,
    pub tenant_id: String,
    pub principal_id: String,
    pub session_id: Option<String>,
    pub event_type: String,
    pub actor: Option<String>,
    pub content_hash: String,
    pub payload_json: String,
    pub occurred_at: String,
    pub recorded_at: String,
}

impl MemoryEvent {
    /// Parsed [`MemoryEventType`].
    pub fn event_type(&self) -> Result<MemoryEventType, String> {
        MemoryEventType::parse(&self.event_type)
    }
}

/// Request payload for [`crate::db::memory_event::MemoryEventRepository::append`].
///
/// `occurred_at` is RFC 3339 (world time — when it actually happened; callers
/// replaying an outbox may backdate) and defaults to the DB clock when `None`.
/// `idempotency_key` makes replays collapse onto one row per tenant.
#[derive(Debug, Clone)]
pub struct AppendMemoryEventRequest {
    pub principal_id: String,
    pub session_id: Option<String>,
    pub event_type: MemoryEventType,
    pub actor: Option<String>,
    /// Serialized JSON stored verbatim in `payload_json`.
    pub payload_json: serde_json::Value,
    /// Override for world-time (`occurred_at`). Must be RFC 3339.
    pub occurred_at: Option<String>,
    /// Replay guard; unique per tenant when present.
    pub idempotency_key: Option<String>,
    /// Precomputed SHA-256 hex over the canonical content. Computed from
    /// `payload_json` when absent.
    pub content_hash: Option<String>,
}

impl AppendMemoryEventRequest {
    pub fn new(principal_id: impl Into<String>, event_type: MemoryEventType) -> Self {
        Self {
            principal_id: principal_id.into(),
            session_id: None,
            event_type,
            actor: None,
            payload_json: serde_json::json!({}),
            occurred_at: None,
            idempotency_key: None,
            content_hash: None,
        }
    }

    pub fn session_id(mut self, v: impl Into<String>) -> Self {
        self.session_id = Some(v.into());
        self
    }

    pub fn actor(mut self, v: impl Into<String>) -> Self {
        self.actor = Some(v.into());
        self
    }

    pub fn payload(mut self, v: serde_json::Value) -> Self {
        self.payload_json = v;
        self
    }

    pub fn occurred_at(mut self, rfc3339: impl Into<String>) -> Self {
        self.occurred_at = Some(rfc3339.into());
        self
    }

    pub fn idempotency_key(mut self, v: impl Into<String>) -> Self {
        self.idempotency_key = Some(v.into());
        self
    }

    pub fn content_hash(mut self, sha256_hex: impl Into<String>) -> Self {
        self.content_hash = Some(sha256_hex.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_types_parse_and_round_trip() {
        for v in MemoryEventType::ALL {
            assert_eq!(MemoryEventType::parse(v).unwrap().as_str(), *v);
        }
        assert!(MemoryEventType::parse("memory_write").is_err());
        assert!(MemoryEventType::parse("USER_MESSAGE").is_err());
        let err = MemoryEventType::parse("tool_call").unwrap_err();
        assert!(err.contains("tool_result"), "{err}");
    }

    #[test]
    fn request_builder_sets_all_fields() {
        let req = AppendMemoryEventRequest::new("p1", MemoryEventType::UserMessage)
            .session_id("sess-9")
            .actor("u_lisa")
            .payload(serde_json::json!({"text": "hi"}))
            .occurred_at("2026-08-28T10:00:00Z")
            .idempotency_key("k1");
        assert_eq!(req.principal_id, "p1");
        assert_eq!(req.session_id.as_deref(), Some("sess-9"));
        assert_eq!(req.actor.as_deref(), Some("u_lisa"));
        assert_eq!(req.payload_json["text"], "hi");
        assert_eq!(req.occurred_at.as_deref(), Some("2026-08-28T10:00:00Z"));
        assert_eq!(req.idempotency_key.as_deref(), Some("k1"));
        assert!(req.content_hash.is_none());
    }

    // --- Anti-drift: enum ⇄ migration CHECK clause -------------------------- //

    #[test]
    fn anti_drift_event_type_matches_migration_check() {
        let sql =
            include_str!("../../migrations/20260828000001_memory_event_stream_and_principals.sql");
        let anchor = "event_type IN (";
        let start = sql.find(anchor).expect("event_type CHECK present") + anchor.len();
        let end = sql[start..].find(')').expect("unterminated IN") + start;
        let migration_values: Vec<String> = sql[start..end]
            .split(',')
            .map(|part| part.trim().trim_matches('\'').to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let enum_values: Vec<String> = MemoryEventType::ALL.iter().map(|s| s.to_string()).collect();
        assert_eq!(
            migration_values, enum_values,
            "MemoryEventType::ALL drifted from the migration CHECK clause"
        );
    }
}
