pub mod types;
pub mod repository;
pub mod prompts;
pub mod l1_extractor;
pub mod l1_dedup;
pub mod l2_consolidator;
pub mod l3_persona;
pub mod pipeline;
pub mod worker;

pub use types::*;
pub use pipeline::DistillationPipeline;
pub use worker::{DistillationService, init_distillation_service};
