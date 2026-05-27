//! MCP Module
//!
//! Model Context Protocol implementation for adaptive memory access.

pub mod sandbox;
pub mod sandbox_proxy;
pub mod signing;

pub use sandbox::{Capability, CapabilityPolicy, SandboxError, SandboxedTool, WasmSandbox};
pub use sandbox_proxy::{ProxyError, SandboxProxy, ToolExecutionLog};

pub use signing::{
    verify_component, verify_unsigned, ComponentSignature, SigningError, TrustedKeyBundle,
};
