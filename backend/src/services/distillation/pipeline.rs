use anyhow::Result;
use sqlx::{Pool, Sqlite};
use tracing::{error, info, instrument};

use crate::db::stm::{STMRepository, SessionMessage};
use crate::tenant::TenantId;

use super::l1_dedup::L1Deduplicator;
use super::l1_extractor::L1Extractor;
use super::l2_consolidator::L2Consolidator;
use super::l3_persona::L3PersonaGenerator;
use super::repository::DistillationRepository;
use super::types::*;

pub struct DistillationConfig {
    pub enabled: bool,
    pub llm_base_url: String,
    pub l1_model: String,
    pub l2_model: String,
    pub l3_model: String,
    pub l1_trigger_after_messages: usize,
    pub l2_trigger_after_atoms: usize,
    pub l3_trigger_after_scenes: usize,
    pub max_atoms_per_extraction: usize,
    pub dedup_enabled: bool,
    pub dedup_top_k: usize,
    pub max_scenes: usize,
}

impl Default for DistillationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            llm_base_url: "http://localhost:11434".to_string(),
            l1_model: "llama3.2".to_string(),
            l2_model: "llama3.2".to_string(),
            l3_model: "llama3.2".to_string(),
            l1_trigger_after_messages: 20,
            l2_trigger_after_atoms: 50,
            l3_trigger_after_scenes: 5,
            max_atoms_per_extraction: 20,
            dedup_enabled: true,
            dedup_top_k: 5,
            max_scenes: 15,
        }
    }
}

pub struct DistillationPipeline {
    config: DistillationConfig,
    extractor: L1Extractor,
    deduplicator: L1Deduplicator,
    consolidator: L2Consolidator,
    persona_generator: L3PersonaGenerator,
}

impl DistillationPipeline {
    pub fn new(config: DistillationConfig) -> Self {
        let extractor = L1Extractor::new(&config.llm_base_url, &config.l1_model);
        let deduplicator = L1Deduplicator::new(
            &config.llm_base_url,
            &config.l1_model,
            config.dedup_top_k,
        );
        let consolidator = L2Consolidator::new(
            &config.llm_base_url,
            &config.l2_model,
            config.max_scenes,
        );
        let persona_generator = L3PersonaGenerator::new(
            &config.llm_base_url,
            &config.l3_model,
        );

        Self { config, extractor, deduplicator, consolidator, persona_generator }
    }

    #[instrument(skip(self, pool))]
    pub async fn run_l1_extraction(
        &self,
        pool: &Pool<Sqlite>,
        messages: &[SessionMessage],
        session_id: &str,
        user_id: &str,
        agent_id: &str,
        tenant_id: &str,
    ) -> Result<L1ExtractionResult> {
        if !self.config.enabled {
            return Ok(L1ExtractionResult {
                success: false,
                extracted_count: 0,
                stored_count: 0,
                scene_names: vec![],
                atom_ids: vec![],
            });
        }

        info!("Starting L1 extraction for session={}", session_id);

        let extraction_results = self.extractor.extract_from_messages(
            messages,
            session_id,
            user_id,
            agent_id,
            tenant_id,
            None,
        ).await?;

        let mut all_atoms: Vec<MemoryAtom> = extraction_results.iter()
            .flat_map(|(_, atoms)| atoms.clone())
            .collect();

        let extracted_count = all_atoms.len();
        let scene_names: Vec<String> = extraction_results.iter()
            .map(|(scene, _)| scene.scene_name.clone())
            .collect();

        if self.config.dedup_enabled && !all_atoms.is_empty() {
            let existing = DistillationRepository::get_atoms_by_user(
                pool, tenant_id, user_id, (self.config.dedup_top_k * 5) as i64,
            ).await.unwrap_or_default();

            if !existing.is_empty() {
                let dedup_results = self.deduplicator
                    .deduplicate(&all_atoms, &existing)
                    .await
                    .unwrap_or_else(|e| {
                        error!("Dedup failed, keeping all: {}", e);
                        all_atoms.iter().map(|_| DedupResult {
                            decision: DedupDecision::Keep,
                            existing_id: None,
                            merged_content: None,
                        }).collect()
                    });

                let mut kept_atoms = Vec::new();
                for (atom, result) in all_atoms.iter().zip(dedup_results.iter()) {
                    match result.decision {
                        DedupDecision::Keep => kept_atoms.push(atom.clone()),
                        DedupDecision::Merge => {
                            if let Some(ref content) = result.merged_content {
                                let mut merged = atom.clone();
                                merged.content = content.clone();
                                kept_atoms.push(merged);
                            }
                            if let Some(ref existing_id) = result.existing_id {
                                let _ = DistillationRepository::update_atom_version(
                                    pool, existing_id, "(superseded)", atom.version + 1,
                                ).await;
                            }
                        }
                        DedupDecision::Supersede => {
                            kept_atoms.push(atom.clone());
                            if let Some(ref existing_id) = result.existing_id {
                                let _ = DistillationRepository::update_atom_version(
                                    pool, existing_id, "(superseded)", atom.version + 1,
                                ).await;
                            }
                        }
                        DedupDecision::Discard => {}
                    }
                }
                all_atoms = kept_atoms;
            }
        }

        let mut atom_ids = Vec::new();
        for atom in &all_atoms {
            if let Err(e) = DistillationRepository::insert_atom(pool, atom).await {
                error!("Failed to insert atom {}: {}", atom.id, e);
            } else {
                atom_ids.push(atom.id.clone());
            }
        }

        let stored_count = atom_ids.len();
        info!(
            "L1 extraction complete: extracted={}, stored={}, scenes={}",
            extracted_count, stored_count, scene_names.len()
        );

        Ok(L1ExtractionResult {
            success: true,
            extracted_count,
            stored_count,
            scene_names,
            atom_ids,
        })
    }

    #[instrument(skip(self, pool))]
    pub async fn run_l2_consolidation(
        &self,
        pool: &Pool<Sqlite>,
        user_id: &str,
        tenant_id: &str,
    ) -> Result<L2ConsolidationResult> {
        if !self.config.enabled {
            return Ok(L2ConsolidationResult {
                success: false,
                scenes_created: 0,
                scenes_updated: 0,
                atoms_processed: 0,
            });
        }

        let recent_atoms = DistillationRepository::get_atoms_by_user(
            pool, tenant_id, user_id, self.config.l2_trigger_after_atoms as i64,
        ).await?;

        let existing_scenes = DistillationRepository::get_scenes_by_user(
            pool, tenant_id, user_id,
        ).await?;

        let result = self.consolidator.consolidate(
            &recent_atoms,
            &existing_scenes,
            user_id,
            tenant_id,
        ).await?;

        Ok(result)
    }

    #[instrument(skip(self, pool))]
    pub async fn run_l3_persona(
        &self,
        pool: &Pool<Sqlite>,
        user_id: &str,
        agent_id: Option<&str>,
        tenant_id: &str,
    ) -> Result<L3PersonaResult> {
        if !self.config.enabled {
            return Ok(L3PersonaResult {
                success: false,
                persona_id: String::new(),
                version: 0,
            });
        }

        let existing_persona = DistillationRepository::get_persona(
            pool, tenant_id, user_id, agent_id,
        ).await?;

        let scenes = DistillationRepository::get_scenes_by_user(
            pool, tenant_id, user_id,
        ).await?;

        if scenes.is_empty() {
            info!("No scenes available for persona generation, skipping");
            return Ok(L3PersonaResult {
                success: false,
                persona_id: String::new(),
                version: 0,
            });
        }

        let persona = self.persona_generator.generate(
            existing_persona.as_ref(),
            &scenes,
            user_id,
            agent_id,
            tenant_id,
        ).await?;

        DistillationRepository::upsert_persona(pool, &persona).await?;

        Ok(L3PersonaResult {
            success: true,
            persona_id: persona.id,
            version: persona.version,
        })
    }
}
