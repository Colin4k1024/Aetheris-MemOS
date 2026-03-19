//! Selectable memory types (STM/LTM/KG/MM) as explainable units with cost/value/priority.
//! Used to attach "why this memory type was selected" to the decision trace.

use crate::models::*;

/// Context for evaluating a memory type (constraints and current config).
#[derive(Debug, Clone)]
pub struct MemoryTypeContext<'a> {
    pub weights: &'a MemoryWeights,
    pub constraints: &'a ResourceConstraints,
}

/// Result of evaluating one memory type: cost, value, and selection priority (higher = more likely to be selected).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MemoryTypeEvaluation {
    pub name: String,
    pub cost_estimate: f64,
    pub value_estimate: f64,
    pub selection_priority: f64,
}

/// A memory type that can be selected by the scheduler (STM, LTM, KG, MM).
/// Provides cost/value estimates and selection priority for explainability.
pub trait SelectableMemory: Send + Sync {
    fn name(&self) -> &'static str;
    fn evaluate(&self, ctx: &MemoryTypeContext<'_>) -> MemoryTypeEvaluation;
}

fn default_priority(weight: f64, _ctx: &MemoryTypeContext<'_>) -> f64 {
    weight
}

pub struct StmMemory;
impl SelectableMemory for StmMemory {
    fn name(&self) -> &'static str {
        "stm"
    }
    fn evaluate(&self, ctx: &MemoryTypeContext<'_>) -> MemoryTypeEvaluation {
        let w = ctx.weights.stm;
        MemoryTypeEvaluation {
            name: "stm".to_string(),
            cost_estimate: w * 256.0 / (ctx.constraints.max_memory_usage_mb as f64).max(1.0),
            value_estimate: w * 1.0,
            selection_priority: default_priority(w, ctx),
        }
    }
}

pub struct LtmMemory;
impl SelectableMemory for LtmMemory {
    fn name(&self) -> &'static str {
        "ltm"
    }
    fn evaluate(&self, ctx: &MemoryTypeContext<'_>) -> MemoryTypeEvaluation {
        let w = ctx.weights.ltm;
        MemoryTypeEvaluation {
            name: "ltm".to_string(),
            cost_estimate: w * 512.0 / (ctx.constraints.max_memory_usage_mb as f64).max(1.0),
            value_estimate: w * 0.8,
            selection_priority: default_priority(w, ctx),
        }
    }
}

pub struct KgMemory;
impl SelectableMemory for KgMemory {
    fn name(&self) -> &'static str {
        "kg"
    }
    fn evaluate(&self, ctx: &MemoryTypeContext<'_>) -> MemoryTypeEvaluation {
        let w = ctx.weights.kg;
        MemoryTypeEvaluation {
            name: "kg".to_string(),
            cost_estimate: w * 256.0 / (ctx.constraints.max_memory_usage_mb as f64).max(1.0),
            value_estimate: w * 0.7,
            selection_priority: default_priority(w, ctx),
        }
    }
}

pub struct MmMemory;
impl SelectableMemory for MmMemory {
    fn name(&self) -> &'static str {
        "mm"
    }
    fn evaluate(&self, ctx: &MemoryTypeContext<'_>) -> MemoryTypeEvaluation {
        let w = ctx.weights.mm;
        MemoryTypeEvaluation {
            name: "mm".to_string(),
            cost_estimate: w * 512.0 / (ctx.constraints.max_memory_usage_mb as f64).max(1.0),
            value_estimate: w * 0.6,
            selection_priority: default_priority(w, ctx),
        }
    }
}
