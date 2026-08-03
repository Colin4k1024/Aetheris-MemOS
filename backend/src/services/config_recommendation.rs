//! P3-lite: Configuration recommendation engine.
//!
//! Recommends the best memory configuration for a given task based on:
//! 1. Task characteristics (complexity, modality, reasoning depth)
//! 2. Resource constraints (CPU, memory, response time)
//! 3. Historical performance data (from training_samples)
//!
//! This is a rule-based + historical data approach, replacing the full ML
//! adaptive learning system (P3-full) with a simpler, more explainable solution.

use tracing::{debug, info};

use crate::db::config_archetypes::{ArchetypeConfig, ConfigArchetype, ConfigArchetypeRepository};
use crate::models::{MemoryConfig, MemoryType, MemoryWeights, ResourceConstraints, TaskContext, TaskPreferences};
use crate::AppError;

/// Configuration recommendation result.
#[derive(Debug, Clone)]
pub struct ConfigRecommendation {
    /// Recommended memory configuration.
    pub config: MemoryConfig,
    /// Archetype ID that was selected.
    pub archetype_id: String,
    /// Confidence score (0.0–1.0) based on historical performance.
    pub confidence: f64,
    /// Explanation of why this configuration was recommended.
    pub explanation: String,
}

/// Configuration recommendation engine.
pub struct ConfigRecommendationEngine;

impl ConfigRecommendationEngine {
    /// Recommend a memory configuration for the given task.
    pub async fn recommend(
        task_context: &TaskContext,
        resource_constraints: &ResourceConstraints,
        preferences: &TaskPreferences,
    ) -> Result<ConfigRecommendation, AppError> {
        info!("Recommending config for task: {}", task_context.task_id);

        // 1. Load active archetypes
        let archetypes = ConfigArchetypeRepository::list_active().await?;
        if archetypes.is_empty() {
            return Err(AppError::Internal("No active config archetypes found".to_string()));
        }

        // 2. Filter archetypes by resource constraints
        let feasible_archetypes = Self::filter_by_constraints(&archetypes, resource_constraints);
        if feasible_archetypes.is_empty() {
            debug!("No archetypes meet resource constraints, using fallback");
            return Self::fallback_recommendation(task_context, &archetypes);
        }

        // 3. Score each archetype based on task characteristics
        let mut scored_archetypes: Vec<(ConfigArchetype, f64, String)> = feasible_archetypes
            .iter()
            .map(|archetype| {
                let (score, reason) = Self::score_archetype(archetype, task_context, preferences);
                (archetype.clone(), score, reason)
            })
            .collect();

        // 4. Sort by score (descending)
        scored_archetypes.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // 5. Select the best archetype
        let (best_archetype, score, reason) = scored_archetypes.first().unwrap();

        // 6. Parse archetype config and convert to MemoryConfig
        let archetype_config = best_archetype.parse_config()?;
        let memory_config = Self::archetype_to_memory_config(&archetype_config);

        let recommendation = ConfigRecommendation {
            config: memory_config,
            archetype_id: best_archetype.archetype_id.clone(),
            confidence: *score,
            explanation: format!(
                "Selected '{}' (score: {:.2}): {}",
                best_archetype.name, score, reason
            ),
        };

        info!(
            "Recommended config: {} (confidence: {:.2})",
            recommendation.archetype_id, recommendation.confidence
        );

        Ok(recommendation)
    }

    /// Filter archetypes by resource constraints.
    fn filter_by_constraints(
        archetypes: &[ConfigArchetype],
        constraints: &ResourceConstraints,
    ) -> Vec<ConfigArchetype> {
        archetypes
            .iter()
            .filter(|archetype| {
                if let Ok(config) = archetype.parse_config() {
                    // Estimate resource cost based on config
                    let estimated_cost = Self::estimate_resource_cost(&config);

                    // Check if estimated cost is within constraints
                    // Use a simple heuristic: more layers = more cost
                    let max_allowed_cost = constraints.max_memory_usage_mb as f64 / 100.0;
                    estimated_cost <= max_allowed_cost
                } else {
                    false
                }
            })
            .cloned()
            .collect()
    }

    /// Score an archetype based on task characteristics.
    fn score_archetype(
        archetype: &ConfigArchetype,
        task_context: &TaskContext,
        preferences: &TaskPreferences,
    ) -> (f64, String) {
        let config = match archetype.parse_config() {
            Ok(c) => c,
            Err(_) => return (0.0, "Invalid config".to_string()),
        };

        let mut score: f64 = 0.0;
        let mut reasons = Vec::new();

        // 1. Complexity matching
        let complexity_score = match task_context.complexity {
            0.0..=0.3 => {
                if config.reasoning_depth == "shallow" {
                    0.3
                } else {
                    0.1
                }
            }
            0.3..=0.7 => {
                if config.reasoning_depth == "medium" {
                    0.3
                } else {
                    0.15
                }
            }
            _ => {
                if config.reasoning_depth == "deep" {
                    0.3
                } else {
                    0.1
                }
            }
        };
        score += complexity_score;
        reasons.push(format!("complexity match: {:.2}", complexity_score));

        // 2. Modality matching
        let modality_count = task_context.modality_requirements.len();
        let modality_score = if modality_count > 1 && config.enable_multimodal {
            0.25
        } else if modality_count <= 1 && !config.enable_multimodal {
            0.15
        } else {
            0.05
        };
        score += modality_score;
        reasons.push(format!("modality match: {:.2}", modality_score));

        // 3. Temporal scope matching
        let temporal_score = match task_context.temporal_scope {
            crate::models::TemporalScope::Short => {
                if config.stm_weight >= 0.5 {
                    0.2
                } else {
                    0.1
                }
            }
            crate::models::TemporalScope::Medium => {
                if config.ltm_weight >= 0.3 {
                    0.2
                } else {
                    0.1
                }
            }
            crate::models::TemporalScope::Long => {
                if config.kg_weight >= 0.2 {
                    0.2
                } else {
                    0.1
                }
            }
        };
        score += temporal_score;
        reasons.push(format!("temporal match: {:.2}", temporal_score));

        // 4. Preference matching
        let preference_score = if preferences.prioritize_efficiency && config.stm_weight >= 0.5 {
            0.15
        } else if preferences.prioritize_coherence && config.ltm_weight >= 0.4 {
            0.15
        } else if preferences.enable_multimodal && config.enable_multimodal {
            0.15
        } else {
            0.05
        };
        score += preference_score;
        reasons.push(format!("preference match: {:.2}", preference_score));

        // Normalize score to 0.0–1.0
        let normalized_score: f64 = score.min(1.0);

        (normalized_score, reasons.join(", "))
    }

    /// Estimate resource cost for a config.
    fn estimate_resource_cost(config: &ArchetypeConfig) -> f64 {
        let base_cost = 0.1;
        let stm_cost = config.stm_weight * 0.1;
        let ltm_cost = config.ltm_weight * 0.2;
        let kg_cost = config.kg_weight * 0.3;
        let mm_cost = config.mm_weight * 0.4;

        base_cost + stm_cost + ltm_cost + kg_cost + mm_cost
    }

    /// Convert archetype config to MemoryConfig.
    fn archetype_to_memory_config(config: &ArchetypeConfig) -> MemoryConfig {
        let primary_memory = match config.primary_memory.as_str() {
            "stm" => MemoryType::Stm,
            "ltm" => MemoryType::Ltm,
            "kg" => MemoryType::Kg,
            "mm" => MemoryType::Mm,
            _ => MemoryType::Stm,
        };

        let secondary_memory = config
            .secondary_memory
            .iter()
            .map(|s| match s.as_str() {
                "stm" => MemoryType::Stm,
                "ltm" => MemoryType::Ltm,
                "kg" => MemoryType::Kg,
                "mm" => MemoryType::Mm,
                _ => MemoryType::Stm,
            })
            .collect();

        MemoryConfig {
            primary_memory,
            secondary_memory,
            memory_weights: MemoryWeights {
                stm: config.stm_weight,
                ltm: config.ltm_weight,
                kg: config.kg_weight,
                mm: config.mm_weight,
            },
            reasoning_depth: config.reasoning_depth.clone(),
            enable_multimodal: config.enable_multimodal,
        }
    }

    /// Fallback recommendation when no archetypes meet constraints.
    fn fallback_recommendation(
        task_context: &TaskContext,
        archetypes: &[ConfigArchetype],
    ) -> Result<ConfigRecommendation, AppError> {
        // Use the simplest archetype as fallback
        let fallback = archetypes
            .iter()
            .find(|a| a.archetype_id == "stm-only")
            .or_else(|| archetypes.first())
            .ok_or_else(|| AppError::Internal("No archetypes available".to_string()))?;

        let config = fallback.parse_config()?;
        let memory_config = Self::archetype_to_memory_config(&config);

        Ok(ConfigRecommendation {
            config: memory_config,
            archetype_id: fallback.archetype_id.clone(),
            confidence: 0.3,
            explanation: "Fallback: no archetypes meet resource constraints".to_string(),
        })
    }
}
