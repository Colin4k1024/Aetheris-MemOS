//! Policy Engine - Memory Scheduling and Cost Model
//!
//! This module provides the core policy engine for memory selection and optimization.

pub mod scheduler;
pub mod cost_model;

pub use scheduler::PolicyScheduler;
pub use cost_model::CostModel;

use crate::kernel::types::*;
use crate::kernel::error::MemoryResult;

/// Memory policy decision.
#[derive(Debug, Clone)]
pub struct MemoryPolicy {
    pub primary_layer: LayerType,
    pub secondary_layers: Vec<LayerType>,
    pub weights: MemoryWeights,
    pub reasoning_depth: ReasoningDepth,
    pub reasoning_strategy: ReasoningStrategy,
}

#[derive(Debug, Clone, Copy)]
pub enum ReasoningDepth {
    Shallow,
    Medium,
    Deep,
}

#[derive(Debug, Clone, Copy)]
pub enum ReasoningStrategy {
    Sequential,
    Parallel,
    Recursive,
    Hybrid,
}

/// Task characteristics for policy decision.
#[derive(Debug, Clone)]
pub struct TaskCharacteristics {
    pub task_type: TaskType,
    pub complexity: f64,
    pub modality: Modality,
    pub context_requirements: ContextRequirements,
    pub latency_sla_ms: Option<u64>,
    pub budget_usd: Option<f64>,
}

#[derive(Debug, Clone, Copy)]
pub enum TaskType {
    Query,
    Reasoning,
    Generation,
    Classification,
    Retrieval,
    Planning,
}

#[derive(Debug, Clone, Copy)]
pub enum Modality {
    Text,
    Image,
    Audio,
    Video,
    Multimodal,
}

#[derive(Debug, Clone)]
pub struct ContextRequirements {
    pub max_tokens: Option<usize>,
    pub relevance_threshold: f64,
    pub diversity_requirement: f64,
}

/// Default memory policy based on task type.
impl MemoryPolicy {
    pub fn default_for_task(task: &TaskCharacteristics) -> Self {
        match task.task_type {
            TaskType::Query => Self {
                primary_layer: LayerType::Stm,
                secondary_layers: vec![],
                weights: MemoryWeights {
                    stm: 0.8,
                    ltm: 0.1,
                    kg: 0.1,
                    mm: 0.0,
                },
                reasoning_depth: ReasoningDepth::Shallow,
                reasoning_strategy: ReasoningStrategy::Sequential,
            },
            TaskType::Retrieval => Self {
                primary_layer: LayerType::Ltm,
                secondary_layers: vec![LayerType::Kg],
                weights: MemoryWeights {
                    stm: 0.2,
                    ltm: 0.5,
                    kg: 0.3,
                    mm: 0.0,
                },
                reasoning_depth: ReasoningDepth::Medium,
                reasoning_strategy: ReasoningStrategy::Parallel,
            },
            TaskType::Reasoning | TaskType::Planning => Self {
                primary_layer: LayerType::Ltm,
                secondary_layers: vec![LayerType::Stm, LayerType::Kg],
                weights: MemoryWeights {
                    stm: 0.3,
                    ltm: 0.4,
                    kg: 0.3,
                    mm: 0.0,
                },
                reasoning_depth: ReasoningDepth::Deep,
                reasoning_strategy: ReasoningStrategy::Recursive,
            },
            TaskType::Generation => Self {
                primary_layer: LayerType::Stm,
                secondary_layers: vec![LayerType::Ltm],
                weights: MemoryWeights {
                    stm: 0.5,
                    ltm: 0.4,
                    kg: 0.1,
                    mm: 0.0,
                },
                reasoning_depth: ReasoningDepth::Medium,
                reasoning_strategy: ReasoningStrategy::Hybrid,
            },
            TaskType::Classification => Self {
                primary_layer: LayerType::Ltm,
                secondary_layers: vec![],
                weights: MemoryWeights {
                    stm: 0.2,
                    ltm: 0.6,
                    kg: 0.2,
                    mm: 0.0,
                },
                reasoning_depth: ReasoningDepth::Shallow,
                reasoning_strategy: ReasoningStrategy::Sequential,
            },
        }
    }
}

impl Default for MemoryPolicy {
    fn default() -> Self {
        Self::default_for_task(&TaskCharacteristics {
            task_type: TaskType::Query,
            complexity: 0.5,
            modality: Modality::Text,
            context_requirements: ContextRequirements {
                max_tokens: None,
                relevance_threshold: 0.7,
                diversity_requirement: 0.3,
            },
            latency_sla_ms: None,
            budget_usd: None,
        })
    }
}
