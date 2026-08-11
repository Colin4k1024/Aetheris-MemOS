//! MCP Plane A capability authorization model.
//!
//! Implements the **capability 授权** decision described in
//! `docs/adr/ADR-0004-mcp-sandbox-execution-model.md` (Plane A — 可信第一方工具平面).
//!
//! This module is intentionally **pure logic**: it maps each first-party memory
//! tool to its minimal required capabilities and decides, given the caller's
//! granted capabilities, whether a `call_tool` dispatch is authorized. It has no
//! database, network, or I/O dependencies, so it can be verified offline.
//!
//! Wiring this decision into `routers/mcp.rs::call_tool` (together with signing
//! verification and structured audit) is deferred to the P1 execution step,
//! which owns the DB-backed audit trail.

use thiserror::Error;

use crate::protocol::mcp::{
    TOOL_MEMORY_FORGET, TOOL_MEMORY_LIST, TOOL_MEMORY_RECALL, TOOL_MEMORY_SEARCH, TOOL_MEMORY_WRITE,
};
use crate::services::rbac::{Permission, Role};

/// Minimal capability scope required to invoke a first-party memory tool (Plane A).
///
/// Distinct from [`crate::mcp::sandbox::Capability`], which models WASM host
/// capabilities (network / filesystem / env vars) for the untrusted Plane B
/// sandbox. `MemoryCapability` models the *authorization scope* a caller must
/// hold to invoke a trusted first-party memory tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemoryCapability {
    /// Read memories (search, recall, list).
    Read,
    /// Write / create memories.
    Write,
    /// Delete / forget memories.
    Delete,
}

impl std::fmt::Display for MemoryCapability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemoryCapability::Read => write!(f, "Read"),
            MemoryCapability::Write => write!(f, "Write"),
            MemoryCapability::Delete => write!(f, "Delete"),
        }
    }
}

/// Errors produced by capability authorization.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AuthzError {
    /// The requested tool is not a known first-party memory tool.
    ///
    /// Authorization is deny-by-default: unknown tools are always rejected.
    #[error("unknown tool: {0}")]
    UnknownTool(String),

    /// The caller's granted capabilities do not cover the tool's requirements.
    #[error("tool '{tool}' requires additional capabilities: {missing:?}")]
    MissingCapabilities {
        /// The tool that was requested.
        tool: String,
        /// Required capabilities the caller was not granted.
        missing: Vec<MemoryCapability>,
    },
}

const CAPS_READ: &[MemoryCapability] = &[MemoryCapability::Read];
const CAPS_WRITE: &[MemoryCapability] = &[MemoryCapability::Write];
const CAPS_DELETE: &[MemoryCapability] = &[MemoryCapability::Delete];

/// Returns the minimal required capabilities for a known memory tool, or `None`
/// for any unknown tool. This `Option` is the basis for deny-by-default in
/// [`authorize`]: it distinguishes "known tool that requires nothing" from
/// "unknown tool".
fn capabilities_for_tool(tool_name: &str) -> Option<&'static [MemoryCapability]> {
    match tool_name {
        TOOL_MEMORY_WRITE => Some(CAPS_WRITE),
        TOOL_MEMORY_SEARCH | TOOL_MEMORY_RECALL | TOOL_MEMORY_LIST => Some(CAPS_READ),
        TOOL_MEMORY_FORGET => Some(CAPS_DELETE),
        _ => None,
    }
}

/// Returns the minimal capabilities required to invoke `tool_name`.
///
/// Known first-party memory tools map to their least-privilege scope; any
/// unknown tool maps to an empty slice. Callers enforcing access control should
/// prefer [`authorize`], which rejects unknown tools deny-by-default rather than
/// treating them as "requires nothing".
pub fn required_capabilities(tool_name: &str) -> &'static [MemoryCapability] {
    capabilities_for_tool(tool_name).unwrap_or(&[])
}

/// Derive the capability set granted to a caller holding `role`.
///
/// Single source of truth: each `MemoryCapability` is granted iff the role holds
/// the corresponding [`Permission`]. Deriving rather than hardcoding means a
/// change to [`Role::has_permission`] cannot silently desynchronise the MCP
/// plane from the REST plane.
///
/// Today's effect, stated plainly: `Reader` is the only role that loses
/// anything — it gets `[Read]` and can therefore no longer invoke
/// `memory_write` / `memory_forget` over MCP. `Owner`, `Admin` and `Member` all
/// hold Read+Write+Delete, so for them this is identical to the previous
/// hardcoded grant. And because every user is presently the `Owner` of their own
/// single-user tenant (see backlog C-3 — `tenant_id` is not yet decoupled from
/// `user_id`), the observable behaviour is unchanged until a real org-level
/// tenant model lands. What changes now is that the grant is *derived from the
/// subject* instead of being a constant, so role separation takes effect the
/// moment roles can actually differ.
pub fn capabilities_for_role(role: Role) -> Vec<MemoryCapability> {
    [
        (Permission::Read, MemoryCapability::Read),
        (Permission::Write, MemoryCapability::Write),
        (Permission::Delete, MemoryCapability::Delete),
    ]
    .into_iter()
    .filter(|(permission, _)| role.has_permission(permission))
    .map(|(_, capability)| capability)
    .collect()
}

/// Authorizes a `call_tool` dispatch under Plane A.
///
/// Returns `Ok(())` only when `tool_name` is a known first-party memory tool and
/// every capability it requires is present in `granted`. Otherwise returns an
/// [`AuthzError`] identifying either the unknown tool or the missing capabilities.
///
/// Deny-by-default: an unknown tool is always rejected, and an empty `granted`
/// set can never satisfy a tool that requires any capability.
pub fn authorize(granted: &[MemoryCapability], tool_name: &str) -> Result<(), AuthzError> {
    let required = capabilities_for_tool(tool_name)
        .ok_or_else(|| AuthzError::UnknownTool(tool_name.to_string()))?;

    let missing: Vec<MemoryCapability> = required
        .iter()
        .copied()
        .filter(|cap| !granted.contains(cap))
        .collect();

    if missing.is_empty() {
        Ok(())
    } else {
        Err(AuthzError::MissingCapabilities {
            tool: tool_name.to_string(),
            missing,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_tool_requires_only_write() {
        assert_eq!(
            required_capabilities(TOOL_MEMORY_WRITE),
            &[MemoryCapability::Write]
        );
    }

    #[test]
    fn read_tools_require_only_read() {
        for tool in [TOOL_MEMORY_SEARCH, TOOL_MEMORY_RECALL, TOOL_MEMORY_LIST] {
            assert_eq!(
                required_capabilities(tool),
                &[MemoryCapability::Read],
                "tool: {tool}"
            );
        }
    }

    #[test]
    fn forget_tool_requires_only_delete() {
        assert_eq!(
            required_capabilities(TOOL_MEMORY_FORGET),
            &[MemoryCapability::Delete]
        );
    }

    #[test]
    fn unknown_tool_lists_no_required_capabilities() {
        assert!(required_capabilities("memory_teleport").is_empty());
    }

    #[test]
    fn authorizes_when_granted_matches_exactly() {
        assert_eq!(
            authorize(&[MemoryCapability::Write], TOOL_MEMORY_WRITE),
            Ok(())
        );
        assert_eq!(
            authorize(&[MemoryCapability::Read], TOOL_MEMORY_SEARCH),
            Ok(())
        );
        assert_eq!(
            authorize(&[MemoryCapability::Delete], TOOL_MEMORY_FORGET),
            Ok(())
        );
    }

    #[test]
    fn authorizes_when_granted_is_superset() {
        let granted = [
            MemoryCapability::Read,
            MemoryCapability::Write,
            MemoryCapability::Delete,
        ];
        assert_eq!(authorize(&granted, TOOL_MEMORY_WRITE), Ok(()));
        assert_eq!(authorize(&granted, TOOL_MEMORY_LIST), Ok(()));
        assert_eq!(authorize(&granted, TOOL_MEMORY_FORGET), Ok(()));
    }

    #[test]
    fn denies_when_required_capability_missing() {
        // Read does not grant Write.
        let err = authorize(&[MemoryCapability::Read], TOOL_MEMORY_WRITE).unwrap_err();
        assert_eq!(
            err,
            AuthzError::MissingCapabilities {
                tool: TOOL_MEMORY_WRITE.to_string(),
                missing: vec![MemoryCapability::Write],
            }
        );
    }

    #[test]
    fn denies_wrong_capability_for_forget() {
        // Holding Write does not grant Delete.
        let err = authorize(&[MemoryCapability::Write], TOOL_MEMORY_FORGET).unwrap_err();
        assert_eq!(
            err,
            AuthzError::MissingCapabilities {
                tool: TOOL_MEMORY_FORGET.to_string(),
                missing: vec![MemoryCapability::Delete],
            }
        );
    }

    #[test]
    fn denies_empty_granted_for_known_tool() {
        let err = authorize(&[], TOOL_MEMORY_SEARCH).unwrap_err();
        assert_eq!(
            err,
            AuthzError::MissingCapabilities {
                tool: TOOL_MEMORY_SEARCH.to_string(),
                missing: vec![MemoryCapability::Read],
            }
        );
    }

    #[test]
    fn denies_unknown_tool_by_default() {
        // Deny-by-default: even a fully-privileged caller cannot invoke an unknown tool.
        let granted = [
            MemoryCapability::Read,
            MemoryCapability::Write,
            MemoryCapability::Delete,
        ];
        let err = authorize(&granted, "memory_exfiltrate").unwrap_err();
        assert_eq!(
            err,
            AuthzError::UnknownTool("memory_exfiltrate".to_string())
        );
    }

    #[test]
    fn denies_unknown_tool_even_with_empty_granted() {
        let err = authorize(&[], "").unwrap_err();
        assert_eq!(err, AuthzError::UnknownTool(String::new()));
    }

    #[test]
    fn capability_display_is_stable() {
        assert_eq!(MemoryCapability::Read.to_string(), "Read");
        assert_eq!(MemoryCapability::Write.to_string(), "Write");
        assert_eq!(MemoryCapability::Delete.to_string(), "Delete");
    }

    // --- Subject-derived grants ------------------------------------------ //

    /// The point of deriving grants from the subject: a `Reader` must not be
    /// able to mutate memory over MCP. Previously `granted` was hardcoded to
    /// `[Read, Write, Delete]` for every caller, so this was reachable.
    #[test]
    fn reader_cannot_write_or_forget() {
        let granted = capabilities_for_role(Role::Reader);
        assert_eq!(granted, vec![MemoryCapability::Read]);

        assert!(authorize(&granted, TOOL_MEMORY_SEARCH).is_ok());
        assert!(authorize(&granted, TOOL_MEMORY_RECALL).is_ok());
        assert!(authorize(&granted, TOOL_MEMORY_LIST).is_ok());

        assert_eq!(
            authorize(&granted, TOOL_MEMORY_WRITE).unwrap_err(),
            AuthzError::MissingCapabilities {
                tool: TOOL_MEMORY_WRITE.to_string(),
                missing: vec![MemoryCapability::Write],
            }
        );
        assert_eq!(
            authorize(&granted, TOOL_MEMORY_FORGET).unwrap_err(),
            AuthzError::MissingCapabilities {
                tool: TOOL_MEMORY_FORGET.to_string(),
                missing: vec![MemoryCapability::Delete],
            }
        );
    }

    #[test]
    fn owner_admin_member_hold_all_memory_capabilities() {
        for role in [Role::Owner, Role::Admin, Role::Member] {
            let granted = capabilities_for_role(role);
            assert_eq!(
                granted,
                vec![
                    MemoryCapability::Read,
                    MemoryCapability::Write,
                    MemoryCapability::Delete
                ],
                "role {role:?} should hold all three memory capabilities"
            );
            for tool in [
                TOOL_MEMORY_SEARCH,
                TOOL_MEMORY_RECALL,
                TOOL_MEMORY_LIST,
                TOOL_MEMORY_WRITE,
                TOOL_MEMORY_FORGET,
            ] {
                assert!(
                    authorize(&granted, tool).is_ok(),
                    "role {role:?} should be able to invoke {tool}"
                );
            }
        }
    }

    /// Anti-drift guard: the MCP capability grant is *derived* from
    /// `Role::has_permission`, so the two planes cannot diverge. If someone
    /// changes the role→permission table, this test pins that the MCP grant
    /// follows rather than silently keeping a stale copy.
    #[test]
    fn grant_tracks_role_permission_table() {
        for role in [Role::Owner, Role::Admin, Role::Member, Role::Reader] {
            let granted = capabilities_for_role(role);
            assert_eq!(
                granted.contains(&MemoryCapability::Read),
                role.has_permission(&Permission::Read),
                "Read grant must track Permission::Read for {role:?}"
            );
            assert_eq!(
                granted.contains(&MemoryCapability::Write),
                role.has_permission(&Permission::Write),
                "Write grant must track Permission::Write for {role:?}"
            );
            assert_eq!(
                granted.contains(&MemoryCapability::Delete),
                role.has_permission(&Permission::Delete),
                "Delete grant must track Permission::Delete for {role:?}"
            );
        }
    }

    /// Deny-by-default must survive the subject-derived path too: an unknown
    /// tool is rejected even for the most privileged role.
    #[test]
    fn unknown_tool_denied_for_every_role() {
        for role in [Role::Owner, Role::Admin, Role::Member, Role::Reader] {
            let granted = capabilities_for_role(role);
            assert!(
                authorize(&granted, "memory_exfiltrate").is_err(),
                "unknown tool must be denied for {role:?}"
            );
        }
    }
}
