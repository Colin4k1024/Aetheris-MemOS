//! WebAssembly sandbox isolation for MCP tool execution (Plane B).
//!
//! Provides zero-trust execution environment for untrusted MCP tools using
//! WebAssembly runtime (wasmtime) with capability-based security.
//!
//! The wasm module must export:
//! - `memory` — WebAssembly memory instance
//! - `alloc(size: i32) -> i32` — allocate `size` bytes, return pointer
//! - `execute(ptr: i32, len: i32) -> i32` — execute with JSON input at `ptr:len`,
//!   return pointer to null-terminated JSON output string

use crate::AppError;
use serde_json::Value as JsonValue;
use std::collections::HashSet;
use thiserror::Error;
use tracing::{info, warn};
use wasmtime::{Engine, Linker, Memory, Module, Store};

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
/// with capability-based isolation and resource limits.
pub struct WasmSandbox {
    engine: Engine,
}

const MAX_FUEL: u64 = 10_000_000;

impl Default for WasmSandbox {
    fn default() -> Self {
        Self::new()
    }
}

impl WasmSandbox {
    pub fn new() -> Self {
        let mut config = wasmtime::Config::new();
        config.consume_fuel(true);

        let engine = Engine::new(&config).expect("failed to create wasmtime engine");

        Self { engine }
    }

    pub fn execute_wasm(
        &self,
        wasm_bytes: &[u8],
        input: JsonValue,
        policy: &CapabilityPolicy,
    ) -> Result<JsonValue, SandboxError> {
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

        let module = Module::new(&self.engine, wasm_bytes).map_err(|e| {
            warn!("invalid wasm module: {}", e);
            SandboxError::InvalidModule(e.to_string())
        })?;

        let mut store = Store::new(&self.engine, ());
        store
            .set_fuel(MAX_FUEL)
            .map_err(|e| SandboxError::RuntimeError(format!("failed to set fuel: {}", e)))?;

        let linker = Linker::new(&self.engine);
        let instance = linker.instantiate(&mut store, &module).map_err(|e| {
            warn!("wasm instantiation failed: {}", e);
            SandboxError::WasmExecutionFailed(e.to_string())
        })?;

        let memory = instance.get_memory(&mut store, "memory").ok_or_else(|| {
            SandboxError::InvalidModule("wasm module must export 'memory'".into())
        })?;

        let alloc = instance
            .get_typed_func::<i32, i32>(&mut store, "alloc")
            .map_err(|_| SandboxError::InvalidModule("wasm module must export 'alloc'".into()))?;

        let execute = instance
            .get_typed_func::<(i32, i32), i32>(&mut store, "execute")
            .map_err(|_| SandboxError::InvalidModule("wasm module must export 'execute'".into()))?;

        let input_json = serde_json::to_string(&input)
            .map_err(|e| SandboxError::RuntimeError(format!("failed to serialize input: {}", e)))?;
        let input_bytes = input_json.as_bytes();

        let input_ptr = alloc
            .call(&mut store, input_bytes.len() as i32)
            .map_err(|e| SandboxError::WasmExecutionFailed(format!("alloc failed: {}", e)))?;

        memory
            .write(&mut store, input_ptr as usize, input_bytes)
            .map_err(|e| {
                SandboxError::WasmExecutionFailed(format!("memory write failed: {}", e))
            })?;

        let output_ptr = execute
            .call(&mut store, (input_ptr, input_bytes.len() as i32))
            .map_err(|e| {
                let msg = e.to_string();
                if msg.contains("fuel") {
                    SandboxError::RuntimeError(format!("resource limit exceeded: {}", msg))
                } else {
                    SandboxError::WasmExecutionFailed(msg)
                }
            })?;

        let output_json =
            read_cstr_from_memory(&memory, &store, output_ptr as usize).map_err(|e| {
                SandboxError::WasmExecutionFailed(format!("failed to read output: {}", e))
            })?;

        let output: JsonValue = serde_json::from_str(&output_json)
            .map_err(|e| SandboxError::RuntimeError(format!("invalid output JSON: {}", e)))?;

        info!("wasm sandbox execution completed successfully");
        Ok(output)
    }
}

/// Reads a null-terminated C string from wasm memory at the given pointer.
fn read_cstr_from_memory(
    memory: &Memory,
    store: &Store<()>,
    ptr: usize,
) -> Result<String, SandboxError> {
    let mut bytes = Vec::new();
    let mut offset = 0;
    loop {
        let mut buf = [0u8; 1];
        memory
            .read(store, ptr + offset, &mut buf)
            .map_err(|e| SandboxError::WasmExecutionFailed(format!("memory read: {}", e)))?;
        if buf[0] == 0 {
            break;
        }
        bytes.push(buf[0]);
        offset += 1;
        if offset > 1024 * 1024 {
            return Err(SandboxError::RuntimeError(
                "output string exceeds 1 MiB limit".into(),
            ));
        }
    }
    String::from_utf8(bytes)
        .map_err(|e| SandboxError::RuntimeError(format!("invalid UTF-8 in output: {}", e)))
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
