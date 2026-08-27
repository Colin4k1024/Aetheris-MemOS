//! Principal identity graph models (#126).
//!
//! A **principal** is WHO a piece of memory belongs to — a person, a service
//! account, a device, or an anonymous session-owner — and is the unit memory
//! attaches to (`#124` Epic: "记忆挂在 principal 上，不挂在 session 上").
//! [`PrincipalAlias`] rows are the identity keys (jwt_sub / email / device_id…)
//! that resolve to one.
//!
//! Merge model: an anonymous principal merged into a person keeps its own row;
//! `status = 'merged'` + `merged_into_id = <person>` form a one-hop redirect
//! that is explicitly reversible. Both directions are audited by the identity
//! service (see `services::identity`). Device aliases resolve only to
//! device-kind principals, so sharing a device never chains two people.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Build the caller-facing message for an invalid enum value.
fn invalid_value_message(field: &str, got: &str, valid: &[&str]) -> String {
    format!(
        "invalid {field}: '{got}' is not a valid value; valid values are: {}",
        valid.join(", ")
    )
}

// ============================================================================
// Principal kind
// ============================================================================

/// What class of actor this principal represents.
///
/// Source of truth: `migrations/20260828000001_memory_event_stream_and_principals.sql`
/// (`CHECK (kind IN (...))`) — kept in lockstep by the anti-drift test below.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrincipalKind {
    /// A human user (mapped from JWT auth, or an anonymous promoted by merge).
    Person,
    /// A non-human workload principal (agent-side automation, integrations).
    ServiceAccount,
    /// A physical/shared endpoint (kiosk tablet, shop floor terminal). Never
    /// auto-linked to a person; two users on one device stay separate persons.
    Device,
    /// Pre-authentication bucket ("anonymous checkout", shared-link visitor).
    /// Carries events until an explicit, audited merge attaches it to a person.
    Anonymous,
}

impl PrincipalKind {
    /// All valid values, ordered as in the migration CHECK clause.
    pub const ALL: &'static [&'static str] = &["person", "service_account", "device", "anonymous"];

    /// Canonical string persisted to the DB.
    pub fn as_str(self) -> &'static str {
        match self {
            PrincipalKind::Person => "person",
            PrincipalKind::ServiceAccount => "service_account",
            PrincipalKind::Device => "device",
            PrincipalKind::Anonymous => "anonymous",
        }
    }

    /// Exact-match parse; `Err` lists every valid value.
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "person" => Ok(PrincipalKind::Person),
            "service_account" => Ok(PrincipalKind::ServiceAccount),
            "device" => Ok(PrincipalKind::Device),
            "anonymous" => Ok(PrincipalKind::Anonymous),
            other => Err(invalid_value_message("principal kind", other, Self::ALL)),
        }
    }
}

// ============================================================================
// Principal status
// ============================================================================

/// Lifecycle state of a principal row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrincipalStatus {
    /// Current owner of its aliases and events.
    Active,
    /// Redirected into another principal via an explicit merge; resolution
    /// follows [`crate::models::principal::MemoryPrincipal::merged_into_id`].
    Merged,
    /// Deactivated (e.g. offboarding); reads keep working for audit, new writes
    /// should not target it.
    Deactivated,
}

impl PrincipalStatus {
    /// All valid values, ordered as in the migration CHECK clause.
    pub const ALL: &'static [&'static str] = &["active", "merged", "deactivated"];

    /// Canonical string persisted to the DB.
    pub fn as_str(self) -> &'static str {
        match self {
            PrincipalStatus::Active => "active",
            PrincipalStatus::Merged => "merged",
            PrincipalStatus::Deactivated => "deactivated",
        }
    }

    /// Exact-match parse; `Err` lists every valid value.
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "active" => Ok(PrincipalStatus::Active),
            "merged" => Ok(PrincipalStatus::Merged),
            "deactivated" => Ok(PrincipalStatus::Deactivated),
            other => Err(invalid_value_message("principal status", other, Self::ALL)),
        }
    }
}

// ============================================================================
// Alias type
// ============================================================================

/// Which identity namespace an alias value belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrincipalAliasType {
    /// The JWT `uid` claim — how authenticated requests map to a person.
    JwtSub,
    /// Application username.
    Username,
    /// Email address (from profile or IdP).
    Email,
    /// Hardware/browser device fingerprint or managed-device id. Belongs ONLY to
    /// device-kind principals; that is what keeps shared devices from chaining
    /// two users together.
    DeviceId,
    /// External system identifier (CRM contact id, HR employee number). The
    /// SoR reconciliation of #129 will assert through this namespace.
    ExternalId,
}

impl PrincipalAliasType {
    /// All valid values, ordered as in the migration CHECK clause.
    pub const ALL: &'static [&'static str] =
        &["jwt_sub", "username", "email", "device_id", "external_id"];

    /// Canonical string persisted to the DB.
    pub fn as_str(self) -> &'static str {
        match self {
            PrincipalAliasType::JwtSub => "jwt_sub",
            PrincipalAliasType::Username => "username",
            PrincipalAliasType::Email => "email",
            PrincipalAliasType::DeviceId => "device_id",
            PrincipalAliasType::ExternalId => "external_id",
        }
    }

    /// Exact-match parse; `Err` lists every valid value.
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "jwt_sub" => Ok(PrincipalAliasType::JwtSub),
            "username" => Ok(PrincipalAliasType::Username),
            "email" => Ok(PrincipalAliasType::Email),
            "device_id" => Ok(PrincipalAliasType::DeviceId),
            "external_id" => Ok(PrincipalAliasType::ExternalId),
            other => Err(invalid_value_message("alias type", other, Self::ALL)),
        }
    }

    /// Whether an alias of this type may attach to a principal of `kind`.
    ///
    /// Structural rule behind "#126: 共享设备默认不与个人主体自动合并": a
    /// device identity never personalizes a person account. jwt/username/email
    /// are human namespaces and never describe a kiosk tablet either.
    pub fn may_attach_to_kind(self, kind: PrincipalKind) -> bool {
        match self {
            PrincipalAliasType::JwtSub
            | PrincipalAliasType::Username
            | PrincipalAliasType::Email => matches!(kind, PrincipalKind::Person),
            PrincipalAliasType::ExternalId => {
                matches!(kind, PrincipalKind::Person | PrincipalKind::ServiceAccount)
            }
            PrincipalAliasType::DeviceId => matches!(kind, PrincipalKind::Device),
        }
    }
}

// ============================================================================
// Row structs
// ============================================================================

/// One row of `memory_principals`.
///
/// Enum-ish columns are stored as their canonical strings (same convention as
/// the rest of `db/`, where TIMESTAMPTZ columns arrive as `::text` casts);
/// use the typed accessors instead of pattern-matching on raw strings.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct MemoryPrincipal {
    pub id: String,
    pub tenant_id: String,
    pub kind: String,
    pub display_name: Option<String>,
    pub status: String,
    pub merged_into_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl MemoryPrincipal {
    /// Parsed [`PrincipalKind`]; errors name the offending raw value.
    pub fn kind(&self) -> Result<PrincipalKind, String> {
        PrincipalKind::parse(&self.kind)
    }

    /// Parsed [`PrincipalStatus`]; errors name the offending raw value.
    pub fn status(&self) -> Result<PrincipalStatus, String> {
        PrincipalStatus::parse(&self.status)
    }

    /// Whether resolution must continue through the merge redirect.
    pub fn is_redirecting(&self) -> bool {
        self.status == PrincipalStatus::Merged.as_str() && self.merged_into_id.is_some()
    }
}

/// One row of `principal_aliases`.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct PrincipalAlias {
    pub id: String,
    pub tenant_id: String,
    pub principal_id: String,
    pub alias_type: String,
    pub alias_value: String,
    pub created_at: String,
}

impl PrincipalAlias {
    /// Parsed [`PrincipalAliasType`].
    pub fn alias_type(&self) -> Result<PrincipalAliasType, String> {
        PrincipalAliasType::parse(&self.alias_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_parse_and_round_trip() {
        for v in PrincipalKind::ALL {
            assert_eq!(PrincipalKind::parse(v).unwrap().as_str(), *v);
        }
        assert!(PrincipalKind::parse("human").is_err());
        assert!(PrincipalKind::parse("Person").is_err());
        let err = PrincipalKind::parse("org").unwrap_err();
        assert!(err.contains("person") && err.contains("anonymous"), "{err}");
    }

    #[test]
    fn statuses_parse_and_round_trip() {
        for v in PrincipalStatus::ALL {
            assert_eq!(PrincipalStatus::parse(v).unwrap().as_str(), *v);
        }
        assert!(PrincipalStatus::parse("archived").is_err());
        assert!(PrincipalStatus::parse("ACTIVE").is_err());
    }

    #[test]
    fn alias_types_parse_and_round_trip() {
        for v in PrincipalAliasType::ALL {
            assert_eq!(PrincipalAliasType::parse(v).unwrap().as_str(), *v);
        }
        assert!(PrincipalAliasType::parse("phone").is_err());
        assert!(PrincipalAliasType::parse("").is_err());
    }

    #[test]
    fn human_aliases_never_attach_to_devices_or_anonymous() {
        // Structural enforcement of the no-auto-merge rule: a device_id can
        // only describe a device, and person namespaces can only describe
        // persons. Without this the service layer could be talked into wiring
        // a kiosk tablet to Lisa's account.
        for t in [
            PrincipalAliasType::JwtSub,
            PrincipalAliasType::Username,
            PrincipalAliasType::Email,
        ] {
            assert!(t.may_attach_to_kind(PrincipalKind::Person));
            assert!(!t.may_attach_to_kind(PrincipalKind::Device));
            assert!(!t.may_attach_to_kind(PrincipalKind::Anonymous));
        }
        // external_id may describe a person OR a service account workload,
        // but still never a device or a throwaway anonymous bucket.
        assert!(PrincipalAliasType::ExternalId.may_attach_to_kind(PrincipalKind::Person));
        assert!(PrincipalAliasType::ExternalId.may_attach_to_kind(PrincipalKind::ServiceAccount));
        assert!(!PrincipalAliasType::ExternalId.may_attach_to_kind(PrincipalKind::Device));
        // device_id belongs to devices alone.
        assert!(PrincipalAliasType::DeviceId.may_attach_to_kind(PrincipalKind::Device));
        assert!(!PrincipalAliasType::DeviceId.may_attach_to_kind(PrincipalKind::Person));
    }

    #[test]
    fn redirect_flag_requires_both_status_and_pointer() {
        let mut p = sample();
        p.status = "merged".to_string();
        p.merged_into_id = Some("p_person".to_string());
        assert!(p.is_redirecting());

        p.merged_into_id = None;
        assert!(!p.is_redirecting(), "merged without pointer is corrupt");

        p.status = "active".to_string();
        p.merged_into_id = Some("p_person".to_string());
        assert!(!p.is_redirecting());
    }

    #[test]
    fn typed_accessors_surface_bad_rows() {
        let mut p = sample();
        p.kind = "alien".to_string();
        assert!(p.kind().is_err());
        p.kind = "person".to_string();
        assert_eq!(p.kind().unwrap(), PrincipalKind::Person);
        p.status = "quantum".to_string();
        assert!(p.status().is_err());
    }

    fn sample() -> MemoryPrincipal {
        MemoryPrincipal {
            id: "01J..".to_string(),
            tenant_id: "tenant-a".to_string(),
            kind: "person".to_string(),
            display_name: None,
            status: "active".to_string(),
            merged_into_id: None,
            created_at: "2026-08-28T00:00:00Z".to_string(),
            updated_at: "2026-08-28T00:00:00Z".to_string(),
        }
    }

    // --- Anti-drift: enums ⇄ migration CHECK clauses ------------------------ //

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
        sql[start..end]
            .split(',')
            .map(|part| part.trim().trim_matches('\'').to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }

    fn as_enum_values(all: &[&str]) -> Vec<String> {
        all.iter().map(|v| v.to_string()).collect()
    }

    #[test]
    fn anti_drift_principal_kind_matches_migration_check() {
        let sql =
            include_str!("../../migrations/20260828000001_memory_event_stream_and_principals.sql");
        assert_eq!(
            parse_check_in_values(sql, "kind"),
            as_enum_values(PrincipalKind::ALL),
            "PrincipalKind::ALL drifted from the migration CHECK clause"
        );
    }

    #[test]
    fn anti_drift_principal_status_matches_migration_check() {
        let sql =
            include_str!("../../migrations/20260828000001_memory_event_stream_and_principals.sql");
        assert_eq!(
            parse_check_in_values(sql, "status"),
            as_enum_values(PrincipalStatus::ALL),
            "PrincipalStatus::ALL drifted from the migration CHECK clause"
        );
    }

    #[test]
    fn anti_drift_alias_type_matches_migration_check() {
        let sql =
            include_str!("../../migrations/20260828000001_memory_event_stream_and_principals.sql");
        assert_eq!(
            parse_check_in_values(sql, "alias_type"),
            as_enum_values(PrincipalAliasType::ALL),
            "PrincipalAliasType::ALL drifted from the migration CHECK clause"
        );
    }
}
