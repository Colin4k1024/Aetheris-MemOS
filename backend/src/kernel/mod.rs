//! Memory Kernel Module
//!
//! This module provides the core abstractions for the Adaptive Memory System.
//! It defines the unified interface for memory operations across different layers.

#![allow(unused_imports)]

pub mod error;
pub mod traits;
pub mod types;

pub use error::{MemoryError, MemoryResult};
pub use traits::*;
pub use types::*;
