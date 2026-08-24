// The SQLite distillation island (pipeline/repository/types/l1_extractor/
// l1_dedup/l3_persona + the L2Consolidator struct) was dead after the recall
// port (#108) moved AutoRecallService to PG — deleted (#88 unify). The PG path
// is canonical: worker.rs (DistillationService) + prompts.rs + the parse
// helpers in l2_consolidator.rs.
pub mod prompts;
pub mod l2_consolidator;
pub mod worker;

pub use worker::{DistillationService, init_distillation_service};
