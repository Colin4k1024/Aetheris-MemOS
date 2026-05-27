//! WebAssembly sandbox isolation for MCP tool execution.
//!
//! Provides zero-trust execution environment for untrusted MCP tools using
//! WebAssembly runtime (wasmtime) with capability-based security.

use serde_json::Value as JsonValue;
use std::collections::HashSet;
use thiserror::Error;
use tracing::warn;

/// Capability types that can be granted or denied to sandboxed tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Capability {
    /// Allows network access (HTTP requests, etc.)
    NetworkAccess,
    /// Allows reading from the filesystem
    FilesystemRead,
    /// Allows writing to the filesystem
    FilesystemWrite,
    /// Allows access to environment variables
    EnvVars,
}

impl std::fmt::Display for Capability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Capability::NetworkAccess => write!(f, "NetworkAccess"),
            Capability::FilesystemRead => write!(f, "FilesystemRead"),
            Capability::FilesystemWrite => write!(f, "FilesystemWrite"),
            Capability::EnvVars => write!(f, "EnvVars"),
        }
    }
}

/// Error types for sandbox operations.
#[derive(Debug, Error)]
pub enum SandboxError {
    #[error("capability denied: {0}")]
    CapabilityDenied(Capability),

    #[error("wasm execution failed: {0}")]
    WasmExecutionFailed(String),

    #[error("invalid wasm module: {0}")]
    InvalidModule(String),

    #[error("runtime error: {0}")]
    RuntimeError(String),
}

/// Policy that defines which capabilities are allowed or denied.
#[derive(Debug, Clone, Default)]
pub struct CapabilityPolicy {
    /// Capabilities that are explicitly allowed.
    pub allowed: HashSet<Capability>,
    /// Capabilities that are explicitly denied.
    pub denied: HashSet<Capability>,
}

impl CapabilityPolicy {
    /// Creates a new empty policy (no capabilities allowed or denied).
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a policy that allows the given capabilities.
    pub fn allow(capabilities: impl IntoIterator<Item = Capability>) -> Self {
        Self {
            allowed: capabilities.into_iter().collect(),
            denied: HashSet::new(),
        }
    }

    /// Creates a policy that denies the given capabilities.
    pub fn deny(capabilities: impl IntoIterator<Item = Capability>) -> Self {
        Self {
            allowed: HashSet::new(),
            denied: capabilities.into_iter().collect(),
        }
    }

    /// Checks if a capability is permitted under this policy.
    pub fn is_permitted(&self, capability: Capability) -> bool {
        if self.denied.contains(&capability) {
            return false;
        }
        self.allowed.contains(&capability)
    }

    /// Records a capability denial in logs.
    fn log_capability_denied(&self, capability: Capability) {
        warn!(
            capability = ?capability,
            allowed = ?self.allowed,
            denied = ?self.denied,
            "sandbox capability denied"
        );
    }
}

/// Trait for tools that can be executed inside a sandbox.
pub trait SandboxedTool: Send + Sync {
    /// Executes the tool with the given input.
    fn execute(&self, input: JsonValue) -> Result<JsonValue, SandboxError>;
}

/// WasmSandbox wraps a wasmtime runtime for executing WebAssembly modules
/// with capability-based isolation.
pub struct WasmSandbox<T> {
    _engine: wasmtime::Engine,
    _store: wasmtime::Store<T>,
    _linker: wasmtime::Linker<T>,
}

impl<T> WasmSandbox<T> {
    /// Creates a new WasmSandbox with the given state.
    pub fn new(state: T) -> Result<Self, SandboxError>
    where
        T: Send + Sync,
    {
        let engine = wasmtime::Engine::default();
        let store = wasmtime::Store::new(&engine, state);
        let linker = wasmtime::Linker::new(&engine);

        Ok(Self {
            _engine: engine,
            _store: store,
            _linker: linker,
        })
    }

    /// Executes a WebAssembly module with the given input and policy.
    ///
    /// Returns an error if the capability is denied or wasm execution fails.
    pub fn execute_wasm(
        &self,
        wasm_bytes: &[u8],
        input: JsonValue,
        policy: &CapabilityPolicy,
    ) -> Result<JsonValue, SandboxError> {
        // For MVP, we validate the policy but don't fully instantiate a real wasm runtime.
        // This is a mock implementation that validates the policy framework.

        // Validate all required capabilities are permitted
        for capability in [
            Capability::NetworkAccess,
            Capability::FilesystemRead,
            Capability::FilesystemWrite,
            Capability::EnvVars,
        ] {
            if !policy.is_permitted(capability) {
                policy.log_capability_denied(capability);
                return Err(SandboxError::CapabilityDenied(capability));
            }
        }

        // Mock: In a real implementation, this would:
        // 1. Instantiate the wasm module with the linker
        // 2. Call the wasm entry point with the JSON input
        // 3. Return the wasm output as JSON

        // For MVP, we just return the input as output to demonstrate the framework works
        Ok(input)
    }
}

impl<T> Default for WasmSandbox<T>
where
    T: Default + Send + Sync,
{
    fn default() -> Self {
        Self::new(T::default()).expect("failed to create default WasmSandbox")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capability_policy_denies_forbidden() {
        let policy = CapabilityPolicy::deny([Capability::NetworkAccess, Capability::EnvVars]);

        assert!(!policy.is_permitted(Capability::NetworkAccess));
        assert!(!policy.is_permitted(Capability::EnvVars));
        // Not in allowed set either, so still denied
        assert!(!policy.is_permitted(Capability::FilesystemRead));
        assert!(!policy.is_permitted(Capability::FilesystemWrite));
    }

    #[test]
    fn test_capability_policy_allows_permitted() {
        let policy = CapabilityPolicy::allow([Capability::FilesystemRead]);

        assert!(policy.is_permitted(Capability::FilesystemRead));
        assert!(!policy.is_permitted(Capability::NetworkAccess));
        assert!(!policy.is_permitted(Capability::EnvVars));
        assert!(!policy.is_permitted(Capability::FilesystemWrite));
    }

    #[test]
    fn test_empty_policy_denies_all() {
        let policy = CapabilityPolicy::new();

        assert!(!policy.is_permitted(Capability::NetworkAccess));
        assert!(!policy.is_permitted(Capability::FilesystemRead));
        assert!(!policy.is_permitted(Capability::FilesystemWrite));
        assert!(!policy.is_permitted(Capability::EnvVars));
    }

    #[test]
    fn test_deny_takes_precedence() {
        let policy = CapabilityPolicy {
            allowed: [Capability::NetworkAccess].into_iter().collect(),
            denied: [Capability::NetworkAccess].into_iter().collect(),
        };

        assert!(!policy.is_permitted(Capability::NetworkAccess));
    }
}
