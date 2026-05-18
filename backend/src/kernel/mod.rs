//! Memory Kernel Module
//!
//! This module provides the core abstractions for the Adaptive Memory System.
//! It defines the unified interface for memory operations across different layers.

#![allow(unused_imports)]

pub mod approval_node;
pub mod error;
pub mod hybrid;
pub mod provider;
pub mod traits;
pub mod types;

pub use approval_node::*;
pub use error::{MemoryError, MemoryResult};
pub use hybrid::*;
pub use provider::*;
pub use traits::*;
pub use types::*;
