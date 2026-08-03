//! P3a: Feature pipeline — extracts training features from task context, resource
//! status, and memory config for the adaptive learning system (ADR-0008 §2).
//!
//! This module converts raw telemetry into a normalized feature vector that the
//! offline batch trainer consumes. Features are deterministic and reproducible
//! (same input → same vector), which is required for eval harness correctness.

use serde::{Deserialize, Serialize};
use tracing::info;

use crate::models::{MemoryConfig, ResourceStatus, TaskCharacteristics, TaskContext};
use crate::tenant::TenantId;

/// Normalized feature vector for a single (task, config) observation.
///
/// All values are f64 for compatibility with linear/GLM/GBDT trainers.
/// Feature names are stable strings (used as column headers in training data).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureVector {
    /// Task complexity (0.0–1.0), from analyzer.
    pub complexity: f64,
    /// Number of modality requirements (0–4).
    pub modality_count: f64,
    /// Temporal scope: 0.0=short, 0.5=medium, 1.0=long.
    pub temporal_scope: f64,
    /// Reasoning depth (0.0–1.0), from analyzer.
    pub reasoning_depth: f64,
    /// Context dependency (0.0–1.0), from task context.
    pub context_dependency: f64,
    /// CPU usage percent (0–100), from sysinfo.
    pub resource_cpu: f64,
    /// Memory usage percent (0–100), from sysinfo.
    pub resource_memory: f64,
    /// Response time p50 in ms (from performance_metrics, 0 if unknown).
    pub response_time_ms: f64,
    /// STM weight from config.
    pub config_stm_weight: f64,
    /// LTM weight from config.
    pub config_ltm_weight: f64,
    /// KG weight from config.
    pub config_kg_weight: f64,
    /// MM weight from config.
    pub config_mm_weight: f64,
    /// Multimodal enabled (0.0 or 1.0).
    pub config_multimodal: f64,
}

impl FeatureVector {
    /// Ordered feature names for training data CSV/JSON serialization.
    /// Must match field order for deterministic column mapping.
    pub const FEATURE_NAMES: &'static [&'static str] = &[
        "complexity",
        "modality_count",
        "temporal_scope",
        "reasoning_depth",
        "context_dependency",
        "resource_cpu",
        "resource_memory",
        "response_time_ms",
        "config_stm_weight",
        "config_ltm_weight",
        "config_kg_weight",
        "config_mm_weight",
        "config_multimodal",
    ];

    /// Convert to a flat f64 array (for training pipeline consumption).
    pub fn to_vec(&self) -> Vec<f64> {
        vec![
            self.complexity,
            self.modality_count,
            self.temporal_scope,
            self.reasoning_depth,
            self.context_dependency,
            self.resource_cpu,
            self.resource_memory,
            self.response_time_ms,
            self.config_stm_weight,
            self.config_ltm_weight,
            self.config_kg_weight,
            self.config_mm_weight,
            self.config_multimodal,
        ]
    }
}

/// Extract features from task context, resource status, and memory config.
///
/// This is a pure function (no I/O) so it can be unit-tested offline.
pub fn extract_features(
    characteristics: &TaskCharacteristics,
    resource_status: &ResourceStatus,
    config: &MemoryConfig,
    response_time_ms: Option<u64>,
) -> FeatureVector {
    FeatureVector {
        complexity: characteristics.complexity as f64,
        modality_count: characteristics.modality_count as f64,
        temporal_scope: match characteristics.temporal_scope.as_str() {
            "short" => 0.0,
            "medium" => 0.5,
            "long" => 1.0,
            _ => 0.5,
        },
        reasoning_depth: characteristics.reasoning_depth as f64,
        context_dependency: characteristics.context_dependency as f64,
        resource_cpu: resource_status.cpu_usage_percent as f64,
        resource_memory: resource_status.memory_usage_percent as f64,
        response_time_ms: response_time_ms.unwrap_or(0) as f64,
        config_stm_weight: config.memory_weights.stm as f64,
        config_ltm_weight: config.memory_weights.ltm as f64,
        config_kg_weight: config.memory_weights.kg as f64,
        config_mm_weight: config.memory_weights.mm as f64,
        config_multimodal: if config.enable_multimodal { 1.0 } else { 0.0 },
    }
}

/// Label vector from independent measurement (NOT from predictor output).
///
/// Labels are recorded after the task completes, using external signals:
/// - accuracy/coherence: from LLM-judge or task success oracle
/// - latency: from OTel request timing
/// - cost: from resource usage during task execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabelVector {
    /// Task completion quality (0.0–1.0), from LLM-judge or oracle.
    pub accuracy: Option<f64>,
    /// Response coherence (0.0–1.0), from LLM-judge.
    pub coherence: Option<f64>,
    /// Actual latency in ms.
    pub latency_ms: Option<f64>,
    /// Resource cost (0.0–1.0), computed from CPU/memory/duration.
    pub cost: Option<f64>,
    /// Optional user satisfaction signal.
    pub user_satisfaction: Option<f64>,
}

/// Complete training sample: features + labels + metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingSample {
    pub sample_id: String,
    pub tenant_id: String,
    pub task_id: String,
    pub config_id: String,
    pub features: FeatureVector,
    pub labels: LabelVector,
    pub policy_tag: PolicyTag,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PolicyTag {
    Normal,
    Exploration,
}

impl std::fmt::Display for PolicyTag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PolicyTag::Normal => write!(f, "normal"),
            PolicyTag::Exploration => write!(f, "exploration"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{MemoryWeights, TaskCharacteristics};

    fn default_characteristics() -> TaskCharacteristics {
        TaskCharacteristics {
            complexity: 0.5,
            modality_count: 1,
            temporal_scope: "medium".to_string(),
            reasoning_depth: 0.3,
            context_dependency: 0.4,
        }
    }

    fn default_resource_status() -> ResourceStatus {
        ResourceStatus {
            memory_usage_mb: 512,
            memory_usage_percent: 50,
            cpu_usage_percent: 30,
            response_time_ms: 200,
            storage_usage_percent: 40,
        }
    }

    fn default_config() -> MemoryConfig {
        MemoryConfig {
            primary_memory: crate::models::MemoryType::Stm,
            secondary_memory: vec![crate::models::MemoryType::Ltm],
            memory_weights: MemoryWeights {
                stm: 0.5,
                ltm: 0.5,
                kg: 0.0,
                mm: 0.0,
            },
            reasoning_depth: "medium".to_string(),
            enable_multimodal: false,
        }
    }

    #[test]
    fn feature_extraction_is_deterministic() {
        let chars = default_characteristics();
        let res = default_resource_status();
        let cfg = default_config();

        let f1 = extract_features(&chars, &res, &cfg, Some(200));
        let f2 = extract_features(&chars, &res, &cfg, Some(200));

        assert_eq!(f1.to_vec(), f2.to_vec());
    }

    #[test]
    fn feature_names_match_field_count() {
        assert_eq!(FeatureVector::FEATURE_NAMES.len(), 13);
    }

    #[test]
    fn feature_values_in_expected_range() {
        let chars = default_characteristics();
        let res = default_resource_status();
        let cfg = default_config();
        let features = extract_features(&chars, &res, &cfg, Some(150));

        assert!(features.complexity >= 0.0 && features.complexity <= 1.0);
        assert!(features.resource_cpu >= 0.0 && features.resource_cpu <= 100.0);
        assert!(features.config_stm_weight >= 0.0 && features.config_stm_weight <= 1.0);
        assert_eq!(features.config_multimodal, 0.0);
    }
}
