//! Memory Kernel Module
//!
//! This module provides the core abstractions for the Adaptive Memory System.
//! It defines the unified interface for memory operations across different layers.

pub mod error;
pub mod types;
pub mod traits;

pub use error::{MemoryError, MemoryResult};
pub use types::*;
pub use traits::*;
