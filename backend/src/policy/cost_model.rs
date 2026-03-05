//! Cost Model - Memory Cost Optimization
//!
//! This module provides cost models for memory layer selection and optimization.

use crate::kernel::types::*;
use crate::policy::MemoryPolicy;
use crate::policy::TaskCharacteristics;

/// Cost model for memory operations.
/// 
/// This model estimates the cost (latency, memory, accuracy) of different memory configurations.
pub struct CostModel {
    // Layer cost parameters (latency in ms per operation)
    stm_latency_ms: f64,
    ltm_latency_ms: f64,
    kg_latency_ms: f64,
    mm_latency_ms: f64,
    
    // Memory usage (MB per 1000 entries)
    stm_memory_mb: f64,
    ltm_memory_mb: f64,
    kg_memory_mb: f64,
    mm_memory_mb: f64,
    
    // Accuracy multipliers (how much each layer contributes to accuracy)
    stm_accuracy: f64,
    ltm_accuracy: f64,
    kg_accuracy: f64,
    mm_accuracy: f64,
}

impl CostModel {
    pub fn new() -> Self {
        // Default cost parameters (empirical estimates)
        Self {
            stm_latency_ms: 1.0,
            ltm_latency_ms: 10.0,
            kg_latency_ms: 15.0,
            mm_latency_ms: 20.0,
            
            stm_memory_mb: 1.0,
            ltm_memory_mb: 5.0,
            kg_memory_mb: 8.0,
            mm_memory_mb: 20.0,
            
            stm_accuracy: 0.6,
            ltm_accuracy: 0.8,
            kg_accuracy: 0.9,
            mm_accuracy: 0.85,
        }
    }

    /// Estimate total latency for a given policy.
    pub fn estimate_latency(&self, policy: &MemoryPolicy) -> f64 {
        let mut latency = match policy.primary_layer {
            LayerType::Stm => self.stm_latency_ms,
            LayerType::Ltm => self.ltm_latency_ms,
            LayerType::Kg => self.kg_latency_ms,
            LayerType::Mm => self.mm_latency_ms,
        };
        
        // Add latency for secondary layers
        for layer in &policy.secondary_layers {
            let layer_latency = match layer {
                LayerType::Stm => self.stm_latency_ms,
                LayerType::Ltm => self.ltm_latency_ms,
                LayerType::Kg => self.kg_latency_ms,
                LayerType::Mm => self.mm_latency_ms,
            };
            latency += layer_latency * 0.5; // Secondary layers add partial latency
        }
        
        // Add reasoning overhead
        latency *= match policy.reasoning_depth {
            crate::policy::ReasoningDepth::Shallow => 1.0,
            crate::policy::ReasoningDepth::Medium => 1.5,
            crate::policy::ReasoningDepth::Deep => 2.0,
        };
        
        latency
    }

    /// Estimate memory usage for a given policy.
    pub fn estimate_memory(&self, policy: &MemoryPolicy) -> f64 {
        let mut memory = match policy.primary_layer {
            LayerType::Stm => self.stm_memory_mb,
            LayerType::Ltm => self.ltm_memory_mb,
            LayerType::Kg => self.kg_memory_mb,
            LayerType::Mm => self.mm_memory_mb,
        };
        
        for layer in &policy.secondary_layers {
            let layer_memory = match layer {
                LayerType::Stm => self.stm_memory_mb,
                LayerType::Ltm => self.ltm_memory_mb,
                LayerType::Kg => self.kg_memory_mb,
                LayerType::Mm => self.mm_memory_mb,
            };
            memory += layer_memory * 0.5;
        }
        
        memory
    }

    /// Estimate accuracy for a given policy.
    pub fn estimate_accuracy(&self, policy: &MemoryPolicy) -> f64 {
        let mut accuracy = match policy.primary_layer {
            LayerType::Stm => self.stm_accuracy,
            LayerType::Ltm => self.ltm_accuracy,
            LayerType::Kg => self.kg_accuracy,
            LayerType::Mm => self.mm_accuracy,
        };
        
        // Secondary layers add accuracy boost
        for layer in &policy.secondary_layers {
            let layer_accuracy = match layer {
                LayerType::Stm => self.stm_accuracy,
                LayerType::Ltm => self.ltm_accuracy,
                LayerType::Kg => self.kg_accuracy,
                LayerType::Mm => self.mm_accuracy,
            };
            accuracy += layer_accuracy * 0.2;
        }
        
        // Cap at 1.0
        accuracy.min(1.0)
    }

    /// Optimize weights for a given task.
    pub async fn optimize_weights(
        &self,
        _policy: &MemoryPolicy,
        task: &TaskCharacteristics,
    ) -> MemoryWeights {
        // Simple weight optimization based on task type
        let base = MemoryWeights {
            stm: 0.25,
            ltm: 0.25,
            kg: 0.25,
            mm: 0.25,
        };
        
        // Adjust based on task characteristics
        match task.task_type {
            crate::policy::TaskType::Query => MemoryWeights {
                stm: 0.7,
                ltm: 0.15,
                kg: 0.15,
                mm: 0.0,
            },
            crate::policy::TaskType::Retrieval => MemoryWeights {
                stm: 0.2,
                ltm: 0.5,
                kg: 0.3,
                mm: 0.0,
            },
            crate::policy::TaskType::Reasoning => MemoryWeights {
                stm: 0.25,
                ltm: 0.35,
                kg: 0.4,
                mm: 0.0,
            },
            crate::policy::TaskType::Generation => MemoryWeights {
                stm: 0.5,
                ltm: 0.4,
                kg: 0.1,
                mm: 0.0,
            },
            crate::policy::TaskType::Classification => MemoryWeights {
                stm: 0.15,
                ltm: 0.55,
                kg: 0.3,
                mm: 0.0,
            },
            crate::policy::TaskType::Planning => MemoryWeights {
                stm: 0.2,
                ltm: 0.3,
                kg: 0.5,
                mm: 0.0,
            },
        }
    }

    /// Calculate overall cost score (lower is better).
    pub fn calculate_cost(
        &self,
        policy: &MemoryPolicy,
        task: &TaskCharacteristics,
    ) -> f64 {
        let latency = self.estimate_latency(policy);
        let memory = self.estimate_memory(policy);
        let accuracy = self.estimate_accuracy(policy);
        
        // Weighted cost: latency + memory - accuracy
        // Normalize to similar scales
        let latency_cost = latency / 100.0; // Normalize to 0-1 range
        let memory_cost = memory / 50.0; // Normalize to 0-1 range
        let accuracy_bonus = 1.0 - accuracy; // Invert: lower is better
        
        // Apply task-specific weights
        let latency_weight = if task.latency_sla_ms.is_some() { 0.5 } else { 0.2 };
        let memory_weight = if task.budget_usd.is_some() { 0.4 } else { 0.2 };
        let accuracy_weight = 0.6;
        
        latency_cost * latency_weight + memory_cost * memory_weight + accuracy_bonus * accuracy_weight
    }
}

impl Default for CostModel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cost_model_latency() {
        let model = CostModel::new();
        
        let policy = MemoryPolicy {
            primary_layer: LayerType::Stm,
            secondary_layers: vec![LayerType::Ltm],
            weights: MemoryWeights { stm: 0.5, ltm: 0.5, kg: 0.0, mm: 0.0 },
            reasoning_depth: crate::policy::ReasoningDepth::Shallow,
            reasoning_strategy: crate::policy::ReasoningStrategy::Sequential,
        };
        
        let latency = model.estimate_latency(&policy);
        assert!(latency > 0.0);
    }

    #[test]
    fn test_cost_model_accuracy() {
        let model = CostModel::new();
        
        let policy = MemoryPolicy {
            primary_layer: LayerType::Ltm,
            secondary_layers: vec![LayerType::Kg],
            weights: MemoryWeights { stm: 0.2, ltm: 0.4, kg: 0.4, mm: 0.0 },
            reasoning_depth: crate::policy::ReasoningDepth::Deep,
            reasoning_strategy: crate::policy::ReasoningStrategy::Recursive,
        };
        
        let accuracy = model.estimate_accuracy(&policy);
        assert!(accuracy > 0.8);
    }
}
