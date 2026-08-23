use serde::Deserialize;

#[derive(Deserialize, Clone, Debug)]
pub struct DistillationConfig {
    #[serde(default = "default_distillation_enabled")]
    pub enabled: bool,
    #[serde(default = "default_worker_poll_interval")]
    pub worker_poll_interval_seconds: u64,
    #[serde(default = "default_l1_extraction_timeout")]
    pub l1_extraction_timeout_seconds: u64,
    #[serde(default = "default_l2_consolidation_timeout")]
    pub l2_consolidation_timeout_seconds: u64,
    #[serde(default = "default_l3_persona_timeout")]
    pub l3_persona_timeout_seconds: u64,
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    #[serde(default = "default_scene_atom_threshold")]
    pub scene_consolidation_atom_threshold: u32,
    #[serde(default = "default_persona_scene_threshold")]
    pub persona_rebuild_scene_threshold: u32,
    #[serde(default = "default_max_atoms_per_session")]
    pub max_atoms_per_session: u32,
    #[serde(default = "default_min_message_count")]
    pub min_message_count: u32,
}

impl Default for DistillationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            worker_poll_interval_seconds: 30,
            l1_extraction_timeout_seconds: 30,
            l2_consolidation_timeout_seconds: 60,
            l3_persona_timeout_seconds: 60,
            max_retries: 3,
            scene_consolidation_atom_threshold: 5,
            persona_rebuild_scene_threshold: 2,
            max_atoms_per_session: 20,
            min_message_count: 3,
        }
    }
}

fn default_distillation_enabled() -> bool {
    true
}
fn default_worker_poll_interval() -> u64 {
    30
}
fn default_l1_extraction_timeout() -> u64 {
    30
}
fn default_l2_consolidation_timeout() -> u64 {
    60
}
fn default_l3_persona_timeout() -> u64 {
    60
}
fn default_max_retries() -> u32 {
    3
}
fn default_scene_atom_threshold() -> u32 {
    5
}
fn default_persona_scene_threshold() -> u32 {
    2
}
fn default_max_atoms_per_session() -> u32 {
    20
}
fn default_min_message_count() -> u32 {
    3
}

#[derive(Deserialize, Clone, Debug)]
pub struct RecallConfig {
    #[serde(default = "default_recall_enabled")]
    pub enabled: bool,
    #[serde(default = "default_recall_timeout")]
    pub timeout_ms: u64,
    #[serde(default = "default_max_l1_results")]
    pub max_l1_results: usize,
    #[serde(default = "default_max_l2_results")]
    pub max_l2_results: usize,
    #[serde(default = "default_inject_persona")]
    pub inject_l3_persona: bool,
    #[serde(default = "default_search_strategy")]
    pub search_strategy: String,
}

impl Default for RecallConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            timeout_ms: 5000,
            max_l1_results: 10,
            max_l2_results: 3,
            inject_l3_persona: true,
            search_strategy: "hybrid".to_string(),
        }
    }
}

fn default_recall_enabled() -> bool {
    true
}
fn default_recall_timeout() -> u64 {
    5000
}
fn default_max_l1_results() -> usize {
    10
}
fn default_max_l2_results() -> usize {
    3
}
fn default_inject_persona() -> bool {
    true
}
fn default_search_strategy() -> String {
    "hybrid".to_string()
}

#[derive(Deserialize, Clone, Debug)]
pub struct SkillsConfig {
    #[serde(default = "default_skills_enabled")]
    pub enabled: bool,
    #[serde(default = "default_auto_extract")]
    pub auto_extract_on_session_complete: bool,
    #[serde(default = "default_max_skills_per_agent")]
    pub max_skills_per_agent: u32,
}

impl Default for SkillsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_extract_on_session_complete: true,
            max_skills_per_agent: 100,
        }
    }
}

fn default_skills_enabled() -> bool {
    true
}
fn default_auto_extract() -> bool {
    true
}
fn default_max_skills_per_agent() -> u32 {
    100
}
