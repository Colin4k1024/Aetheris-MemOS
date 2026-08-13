use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::Instant;
use tracing::{info, warn};

use crate::services::distillation::repository::DistillationRepository;
use crate::services::distillation::types::{MemoryAtom, MemoryAtomType, Persona, SceneBlock};

use super::formatter::RecallFormatter;
use super::strategy::{RecallSource, RecallStrategy};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecallRequest {
    pub query: String,
    pub user_id: String,
    pub agent_id: Option<String>,
    pub tenant_id: String,
    pub strategy: Option<RecallStrategy>,
    pub max_results: Option<usize>,
    pub max_tokens: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecallResult {
    pub context_memories: Vec<RecalledMemory>,
    pub system_context: Option<String>,
    pub strategy_used: RecallStrategy,
    pub latency_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecalledMemory {
    pub content: String,
    pub atom_type: Option<MemoryAtomType>,
    pub score: f64,
    pub source: RecallSource,
}

impl RecalledMemory {
    pub fn atom_type_str(&self) -> &str {
        self.atom_type.map(|t| t.as_str()).unwrap_or("memory")
    }
}

pub struct AutoRecallService {
    timeout_ms: u64,
    max_l1_results: usize,
    max_recall_tokens: usize,
    inject_persona: bool,
    inject_scene_nav: bool,
}

impl AutoRecallService {
    pub fn new(
        timeout_ms: u64,
        max_l1_results: usize,
        max_recall_tokens: usize,
        inject_persona: bool,
        inject_scene_nav: bool,
    ) -> Self {
        Self {
            timeout_ms,
            max_l1_results,
            max_recall_tokens,
            inject_persona,
            inject_scene_nav,
        }
    }

    pub async fn recall(
        &self,
        pool: &sqlx::Pool<sqlx::Sqlite>,
        request: &RecallRequest,
    ) -> Result<RecallResult> {
        let start = Instant::now();
        let strategy = request.strategy.unwrap_or(RecallStrategy::Hybrid);
        let max_results = request.max_results.unwrap_or(self.max_l1_results);
        let max_tokens = request.max_tokens.unwrap_or(self.max_recall_tokens);

        let timeout = tokio::time::Duration::from_millis(self.timeout_ms);

        let result = tokio::time::timeout(timeout, self.recall_inner(
            pool, request, strategy, max_results, max_tokens,
        )).await;

        match result {
            Ok(Ok(mut recall_result)) => {
                recall_result.latency_ms = start.elapsed().as_millis() as u64;
                Ok(recall_result)
            }
            Ok(Err(e)) => {
                warn!("Recall failed: {}", e);
                Ok(RecallResult {
                    context_memories: vec![],
                    system_context: None,
                    strategy_used: strategy,
                    latency_ms: start.elapsed().as_millis() as u64,
                })
            }
            Err(_) => {
                warn!("Recall timed out after {}ms", self.timeout_ms);
                Ok(RecallResult {
                    context_memories: vec![],
                    system_context: None,
                    strategy_used: strategy,
                    latency_ms: self.timeout_ms,
                })
            }
        }
    }

    async fn recall_inner(
        &self,
        pool: &sqlx::Pool<sqlx::Sqlite>,
        request: &RecallRequest,
        strategy: RecallStrategy,
        max_results: usize,
        max_tokens: usize,
    ) -> Result<RecallResult> {
        let mut context_memories = Vec::new();
        let mut system_parts = Vec::new();

        // 1. Search L1 atoms by keyword (BM25-like)
        let atoms = DistillationRepository::search_atoms_by_content(
            pool,
            &request.tenant_id,
            &request.user_id,
            &request.query,
            max_results as i64,
        ).await.unwrap_or_default();

        for (i, atom) in atoms.iter().enumerate() {
            let score = 1.0 - (i as f64 * 0.1);
            context_memories.push(RecalledMemory {
                content: atom.content.clone(),
                atom_type: Some(atom.atom_type),
                score,
                source: RecallSource::L1Atom,
            });
        }

        // 2. Load L3 persona (stable context)
        if self.inject_persona {
            if let Ok(Some(persona)) = DistillationRepository::get_persona(
                pool,
                &request.tenant_id,
                &request.user_id,
                request.agent_id.as_deref(),
            ).await {
                let persona_ctx = RecallFormatter::format_persona_context(&persona.content);
                system_parts.push(persona_ctx);
            }
        }

        // 3. Load L2 scene navigation
        if self.inject_scene_nav {
            let scenes = DistillationRepository::get_scenes_by_user(
                pool, &request.tenant_id, &request.user_id,
            ).await.unwrap_or_default();

            if !scenes.is_empty() {
                let nav: Vec<(String, String)> = scenes.iter()
                    .take(10)
                    .map(|s| (s.name.clone(), s.summary.clone()))
                    .collect();
                let nav_ctx = RecallFormatter::format_scene_navigation(&nav);
                system_parts.push(nav_ctx);
            }
        }

        let system_context = if system_parts.is_empty() {
            None
        } else {
            Some(system_parts.join("\n\n"))
        };

        info!(
            "Recall complete: {} memories, persona={}, scenes={}",
            context_memories.len(),
            system_context.is_some(),
            self.inject_scene_nav
        );

        Ok(RecallResult {
            context_memories,
            system_context,
            strategy_used: strategy,
            latency_ms: 0,
        })
    }
}
