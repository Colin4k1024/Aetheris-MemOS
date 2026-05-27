//! Memory Layers - Unified Layer Trait Implementations
//!
//! This module provides trait implementations that bridge the existing
//! repository implementations with the Memory Kernel interface.

pub mod kg_layer;
pub mod ltm_layer;
pub mod mm_layer;
pub mod procedural_layer;
pub mod stm_layer;

pub use kg_layer::KgMemoryLayer;
pub use ltm_layer::LtmMemoryLayer;
pub use mm_layer::MmMemoryLayer;
pub use procedural_layer::ProceduralMemoryLayer;
pub use stm_layer::StmMemoryLayer;

use crate::kernel::error::MemoryResult;
use crate::kernel::types::*;

/// Get all memory layers as a vector.
pub fn create_layers() -> Vec<Box<dyn crate::kernel::MemoryLayer + Send + Sync>> {
    vec![
        Box::new(StmMemoryLayer::new()),
        Box::new(LtmMemoryLayer::new()),
        Box::new(KgMemoryLayer::new()),
        Box::new(MmMemoryLayer::new()),
        Box::new(ProceduralMemoryLayer::new()),
    ]
}
