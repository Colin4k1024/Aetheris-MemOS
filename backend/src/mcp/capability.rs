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
}
