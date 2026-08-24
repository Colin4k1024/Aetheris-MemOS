// The SQLite skill store/matcher (services/skill/store.rs + matcher.rs) were
// dead after the skill router (#103) used the PG db/skill.rs repo — deleted
// (#90 unify). Live: types.rs (SkillCreateRequest, the extractor's output
// type) + extractor.rs (LLM-based, used by POST /v1/skills/extract).
pub mod types;
pub mod extractor;

pub use types::*;
