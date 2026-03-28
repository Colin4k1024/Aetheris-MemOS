//! MCP Module
//!
//! Model Context Protocol implementation for adaptive memory access.

pub mod signing;

pub use signing::{
    verify_component, verify_unsigned, ComponentSignature, SigningError, TrustedKeyBundle,
};
