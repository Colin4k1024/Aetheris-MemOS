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

/// Multimodal `multimodal_entries.modality_type`.
///
/// Source of truth: `migrations/20240101000005_multimodal_memory.sql`
/// (`CHECK (modality_type IN ('text', 'image', 'audio', 'video', 'mixed'))`).
///
/// Before this enum the write path (`MMRepository::create_entry`) bound the
/// caller's raw string straight into the INSERT, so an invalid modality
/// surfaced as a DB `CHECK` violation (HTTP 500). The HTTP write boundary now
/// validates here and returns a 400 that lists the valid values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalityType {
    Text,
    Image,
    Audio,
    Video,
    Mixed,
}

impl ModalityType {
    /// All valid values, ordered as in the migration `CHECK` clause.
    pub const ALL: &'static [&'static str] = &["text", "image", "audio", "video", "mixed"];

    /// Canonical string persisted to the DB.
    pub fn as_str(self) -> &'static str {
        match self {
            ModalityType::Text => "text",
            ModalityType::Image => "image",
            ModalityType::Audio => "audio",
            ModalityType::Video => "video",
            ModalityType::Mixed => "mixed",
        }
    }

    /// Exact-match parse; `Err` is the caller-facing 400 message.
    ///
    /// No case-folding or trimming — same rationale as [`SessionType::parse`]:
    /// the DB `CHECK` is exact, so the app must enforce the identical contract.
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "text" => Ok(ModalityType::Text),
            "image" => Ok(ModalityType::Image),
            "audio" => Ok(ModalityType::Audio),
            "video" => Ok(ModalityType::Video),
            "mixed" => Ok(ModalityType::Mixed),
            other => Err(invalid_value_message("modalityType", other, Self::ALL)),
        }
    }
}

/// Memory config `memory_configurations.config_type`.
///
/// Source of truth: `migrations/20240101000006_memory_management.sql`
/// (`CHECK (config_type IN ('default', 'custom', 'optimized'))`).
///
/// The create/update HTTP handlers bound the caller's raw string straight into
/// the INSERT/UPDATE, so an invalid `configType` surfaced as a DB `CHECK`
/// violation (HTTP 500). Those handlers now validate here and return a 400 that
/// lists the valid values. (Internal callers — scheduler / orchestrator — pass
/// the fixed literal `"optimized"`, so they never depended on this.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigType {
    Default,
    Custom,
    Optimized,
}

impl ConfigType {
    /// All valid values, ordered as in the migration `CHECK` clause.
    pub const ALL: &'static [&'static str] = &["default", "custom", "optimized"];

    /// Canonical string persisted to the DB.
    pub fn as_str(self) -> &'static str {
        match self {
            ConfigType::Default => "default",
            ConfigType::Custom => "custom",
            ConfigType::Optimized => "optimized",
        }
    }

    /// Exact-match parse; `Err` is the caller-facing 400 message.
    ///
    /// No case-folding or trimming — same rationale as [`SessionType::parse`].
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "default" => Ok(ConfigType::Default),
            "custom" => Ok(ConfigType::Custom),
            "optimized" => Ok(ConfigType::Optimized),
            other => Err(invalid_value_message("configType", other, Self::ALL)),
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
    fn modality_type_parse_accepts_all_valid_values() {
        assert_eq!(ModalityType::parse("text").unwrap(), ModalityType::Text);
        assert_eq!(ModalityType::parse("image").unwrap(), ModalityType::Image);
        assert_eq!(ModalityType::parse("audio").unwrap(), ModalityType::Audio);
        assert_eq!(ModalityType::parse("video").unwrap(), ModalityType::Video);
        assert_eq!(ModalityType::parse("mixed").unwrap(), ModalityType::Mixed);
    }

    #[test]
    fn config_type_parse_accepts_all_valid_values() {
        assert_eq!(ConfigType::parse("default").unwrap(), ConfigType::Default);
        assert_eq!(ConfigType::parse("custom").unwrap(), ConfigType::Custom);
        assert_eq!(
            ConfigType::parse("optimized").unwrap(),
            ConfigType::Optimized
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
        for v in ModalityType::ALL {
            assert_eq!(ModalityType::parse(v).unwrap().as_str(), *v);
        }
        for v in ConfigType::ALL {
            assert_eq!(ConfigType::parse(v).unwrap().as_str(), *v);
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
    fn invalid_modality_type_error_lists_all_valid_values() {
        let err = ModalityType::parse("gif").unwrap_err();
        assert!(
            err.contains("gif"),
            "message must echo the bad value: {err}"
        );
        for v in ModalityType::ALL {
            assert!(
                err.contains(v),
                "message must list valid value '{v}': {err}"
            );
        }
    }

    #[test]
    fn invalid_config_type_error_lists_all_valid_values() {
        let err = ConfigType::parse("tuned").unwrap_err();
        assert!(
            err.contains("tuned"),
            "message must echo the bad value: {err}"
        );
        for v in ConfigType::ALL {
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
        assert!(ModalityType::parse("Image").is_err());
        assert!(ModalityType::parse(" audio ").is_err());
        assert!(ModalityType::parse("VIDEO").is_err());
        assert!(ConfigType::parse("Default").is_err());
        assert!(ConfigType::parse(" custom ").is_err());
        assert!(ConfigType::parse("OPTIMIZED").is_err());
    }

    #[test]
    fn empty_value_is_rejected() {
        assert!(SessionType::parse("").is_err());
        assert!(SourceType::parse("").is_err());
        assert!(ModalityType::parse("").is_err());
        assert!(ConfigType::parse("").is_err());
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

    #[test]
    fn anti_drift_modality_type_matches_migration_check() {
        let sql = include_str!("../../migrations/20240101000005_multimodal_memory.sql");
        let migration_values = parse_check_in_values(sql, "modality_type");
        let enum_values: Vec<String> = ModalityType::ALL.iter().map(|s| s.to_string()).collect();
        assert_eq!(
            migration_values, enum_values,
            "ModalityType::ALL drifted from the migration CHECK clause; \
             update whichever is wrong so both agree"
        );
    }

    #[test]
    fn anti_drift_config_type_matches_migration_check() {
        let sql = include_str!("../../migrations/20240101000006_memory_management.sql");
        let migration_values = parse_check_in_values(sql, "config_type");
        let enum_values: Vec<String> = ConfigType::ALL.iter().map(|s| s.to_string()).collect();
        assert_eq!(
            migration_values, enum_values,
            "ConfigType::ALL drifted from the migration CHECK clause; \
             update whichever is wrong so both agree"
        );
    }
}
