//! Canonical enums for DB `CHECK`-constrained memory fields.
//!
//! ⚠️ SINGLE SOURCE OF TRUTH (backlog D-a):
//! The valid-value sets here MUST stay in lockstep with the
//! `CHECK (<col> IN (...))` clauses in `backend/migrations/`. Before this module
//! existed the legal values lived ONLY in the migration `CHECK` clause, so a
//! caller had no way to learn them and an invalid value surfaced as a DB error
//! (HTTP 500). The write paths now validate against these enums at the service
//! boundary and return a 400 that lists the valid values.
//!
//! To guarantee the two copies never drift, `anti_drift_*` tests parse the
//! migration text (`include_str!`) and assert the parsed `IN (...)` set equals
//! the enum's [`ALL`](SessionType::ALL) set — adding a value in the DB without
//! updating the enum (or vice versa) fails the build, not production.

/// Build the caller-facing 400 message for an invalid enum value.
///
/// Deliberately lists every valid value so an integrator can fix the request
/// from the error alone (this is the whole point of D-a).
fn invalid_value_message(field: &str, got: &str, valid: &[&str]) -> String {
    format!(
        "invalid {field}: '{got}' is not a valid value; valid values are: {}",
        valid.join(", ")
    )
}

/// STM `context_sessions.session_type`.
///
/// Source of truth: `migrations/20240101000002_short_term_memory.sql`
/// (`CHECK (session_type IN ('conversation', 'task', 'query'))`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionType {
    Conversation,
    Task,
    Query,
}

impl SessionType {
    /// All valid values, ordered as in the migration `CHECK` clause.
    pub const ALL: &'static [&'static str] = &["conversation", "task", "query"];

    /// Canonical string persisted to the DB.
    pub fn as_str(self) -> &'static str {
        match self {
            SessionType::Conversation => "conversation",
            SessionType::Task => "task",
            SessionType::Query => "query",
        }
    }

    /// Exact-match parse; `Err` is the caller-facing 400 message.
    ///
    /// No case-folding or trimming: the DB `CHECK` is exact, so quietly accepting
    /// `"Conversation"` or `" task "` here would make the app enforce a *different*
    /// contract than the DB — the same "same name, different meaning across a
    /// boundary" defect class this fix exists to remove. Reject and let the
    /// message tell the caller the exact accepted spelling.
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "conversation" => Ok(SessionType::Conversation),
            "task" => Ok(SessionType::Task),
            "query" => Ok(SessionType::Query),
            other => Err(invalid_value_message("sessionType", other, Self::ALL)),
        }
    }
}

/// LTM `knowledge_entries.source_type`.
///
/// Source of truth: `migrations/20240101000003_long_term_memory.sql`
/// (`CHECK (source_type IN ('document', 'api', 'database', 'web', 'user_input'))`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceType {
    Document,
    Api,
    Database,
    Web,
    UserInput,
}

impl SourceType {
    /// All valid values, ordered as in the migration `CHECK` clause.
    pub const ALL: &'static [&'static str] = &["document", "api", "database", "web", "user_input"];

    /// Canonical string persisted to the DB.
    pub fn as_str(self) -> &'static str {
        match self {
            SourceType::Document => "document",
            SourceType::Api => "api",
            SourceType::Database => "database",
            SourceType::Web => "web",
            SourceType::UserInput => "user_input",
        }
    }

    /// Exact-match parse; `Err` is the caller-facing 400 message.
    ///
    /// Replaces the previous silent "unknown → user_input" remap in the LTM store
    /// path: mapping a caller's typo to a real value stored garbage under a valid
    /// label with no signal to anyone. Rejecting is the honest behavior.
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "document" => Ok(SourceType::Document),
            "api" => Ok(SourceType::Api),
            "database" => Ok(SourceType::Database),
            "web" => Ok(SourceType::Web),
            "user_input" => Ok(SourceType::UserInput),
            other => Err(invalid_value_message("sourceType", other, Self::ALL)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_type_parse_accepts_all_valid_values() {
        assert_eq!(
            SessionType::parse("conversation").unwrap(),
            SessionType::Conversation
        );
        assert_eq!(SessionType::parse("task").unwrap(), SessionType::Task);
        assert_eq!(SessionType::parse("query").unwrap(), SessionType::Query);
    }

    #[test]
    fn source_type_parse_accepts_all_valid_values() {
        assert_eq!(SourceType::parse("document").unwrap(), SourceType::Document);
        assert_eq!(SourceType::parse("api").unwrap(), SourceType::Api);
        assert_eq!(SourceType::parse("database").unwrap(), SourceType::Database);
        assert_eq!(SourceType::parse("web").unwrap(), SourceType::Web);
        assert_eq!(
            SourceType::parse("user_input").unwrap(),
            SourceType::UserInput
        );
    }

    #[test]
    fn as_str_round_trips_through_parse() {
        for v in SessionType::ALL {
            assert_eq!(SessionType::parse(v).unwrap().as_str(), *v);
        }
        for v in SourceType::ALL {
            assert_eq!(SourceType::parse(v).unwrap().as_str(), *v);
        }
    }

    #[test]
    fn invalid_session_type_error_lists_all_valid_values() {
        let err = SessionType::parse("chat").unwrap_err();
        assert!(
            err.contains("chat"),
            "message must echo the bad value: {err}"
        );
        for v in SessionType::ALL {
            assert!(
                err.contains(v),
                "message must list valid value '{v}': {err}"
            );
        }
    }

    #[test]
    fn invalid_source_type_error_lists_all_valid_values() {
        let err = SourceType::parse("documentation").unwrap_err();
        assert!(
            err.contains("documentation"),
            "message must echo the bad value: {err}"
        );
        for v in SourceType::ALL {
            assert!(
                err.contains(v),
                "message must list valid value '{v}': {err}"
            );
        }
    }

    #[test]
    fn parse_is_exact_match_no_case_fold_or_trim() {
        // Guards the deliberate decision to NOT silently normalize: these must be
        // rejected, not coerced, so app and DB enforce the identical contract.
        assert!(SessionType::parse("Conversation").is_err());
        assert!(SessionType::parse(" task ").is_err());
        assert!(SessionType::parse("TASK").is_err());
        assert!(SourceType::parse("Document").is_err());
        assert!(SourceType::parse(" web ").is_err());
        assert!(SourceType::parse("USER_INPUT").is_err());
    }

    #[test]
    fn empty_value_is_rejected() {
        assert!(SessionType::parse("").is_err());
        assert!(SourceType::parse("").is_err());
    }

    // --- Anti-drift: enum ⇄ migration CHECK clause -------------------------- //

    /// Extract the values inside `<column> IN ('a', 'b', ...)` from migration SQL.
    ///
    /// Kept intentionally small: finds the `<column> IN (` anchor, reads up to the
    /// next `)`, and pulls each single-quoted literal. If the migration format
    /// changes shape, the returned set changes and the assertion below fails —
    /// which is the point.
    fn parse_check_in_values(sql: &str, column: &str) -> Vec<String> {
        let anchor = format!("{column} IN (");
        let start = sql
            .find(&anchor)
            .unwrap_or_else(|| panic!("`{anchor}` not found in migration"))
            + anchor.len();
        let end = sql[start..]
            .find(')')
            .unwrap_or_else(|| panic!("unterminated `IN (` for column `{column}`"))
            + start;
        let list = &sql[start..end];
        list.split(',')
            .map(|part| part.trim().trim_matches('\'').to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }

    #[test]
    fn anti_drift_session_type_matches_migration_check() {
        // include_str! resolves at compile time relative to THIS file
        // (backend/src/models/), so the test needs no runtime filesystem access
        // and runs under `cargo test --lib`.
        let sql = include_str!("../../migrations/20240101000002_short_term_memory.sql");
        let migration_values = parse_check_in_values(sql, "session_type");
        let enum_values: Vec<String> = SessionType::ALL.iter().map(|s| s.to_string()).collect();
        assert_eq!(
            migration_values, enum_values,
            "SessionType::ALL drifted from the migration CHECK clause; \
             update whichever is wrong so both agree"
        );
    }

    #[test]
    fn anti_drift_source_type_matches_migration_check() {
        let sql = include_str!("../../migrations/20240101000003_long_term_memory.sql");
        let migration_values = parse_check_in_values(sql, "source_type");
        let enum_values: Vec<String> = SourceType::ALL.iter().map(|s| s.to_string()).collect();
        assert_eq!(
            migration_values, enum_values,
            "SourceType::ALL drifted from the migration CHECK clause; \
             update whichever is wrong so both agree"
        );
    }
}
