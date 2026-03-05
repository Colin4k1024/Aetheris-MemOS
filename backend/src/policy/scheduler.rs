//! Policy Scheduler - Adaptive Memory Selection
//!
//! This module implements the adaptive memory scheduling logic.

use crate::kernel::types::*;
use crate::kernel::error::{MemoryError, MemoryResult};
use crate::policy::cost_model::CostModel;
use crate::policy::MemoryPolicy;
use crate::policy::{TaskCharacteristics, TaskType, Modality, ReasoningDepth};

/// Policy-based memory scheduler.
/// 
/// This scheduler decides which memory layers to use based on task characteristics.
pub struct PolicyScheduler {
    cost_model: CostModel,
}

impl PolicyScheduler {
    pub fn new() -> Self {
        Self {
            cost_model: CostModel::new(),
        }
    }

    /// Select optimal memory policy for a given task.
    pub async fn select_policy(
        &self,
        task: &TaskCharacteristics,
        resource_constraints: Option<&ResourceConstraints>,
    ) -> MemoryResult<MemoryPolicy> {
        // Start with task-based default
        let mut policy = MemoryPolicy::default_for_task(task);
        
        // Adjust based on complexity
        if task.complexity > 0.7 {
            // High complexity: use more layers
            policy.secondary_layers.push(LayerType::Kg);
            policy.weights.kg = 0.2;
        }
        
        // Adjust based on modality
        if task.modality == Modality::Multimodal 
            || task.modality == Modality::Image 
            || task.modality == Modality::Video {
            policy.secondary_layers.push(LayerType::Mm);
            policy.weights.mm = 0.2;
            policy.primary_layer = LayerType::Ltm;
        }
        
        // Apply resource constraints if provided
        if let Some(constraints) = resource_constraints {
            if let Some(max_memory_mb) = constraints.max_memory_mb {
                // Limit memory usage
                if max_memory_mb < 100 {
                    // Low memory: prefer STM only
                    policy.secondary_layers.clear();
                    policy.weights = MemoryWeights {
                        stm: 0.9,
                        ltm: 0.1,
                        kg: 0.0,
                        mm: 0.0,
                    };
                }
            }
            
            if let Some(max_latency_ms) = constraints.max_latency_ms {
                // Low latency requirement: prefer STM
                if max_latency_ms < 100 {
                    policy.primary_layer = LayerType::Stm;
                    policy.reasoning_depth = ReasoningDepth::Shallow;
                }
            }
        }
        
        // Optimize weights using cost model
        let optimized = self.cost_model.optimize_weights(&policy, task).await;
        policy.weights = optimized;
        
        Ok(policy)
    }

    /// Determine if memory should be evicted based on policy.
    pub async fn should_evict(
        &self,
        entry: &MemoryEntry,
        current_size: usize,
        max_size: usize,
    ) -> bool {
        // Evict if over capacity
        if current_size >= max_size {
            return true;
        }
        
        // Evict old entries with low importance
        if entry.metadata.importance < 0.3 {
            if let Some(expires_at) = entry.metadata.expires_at {
                let now = chrono::Utc::now().timestamp();
                if now > expires_at {
                    return true;
                }
            }
        }
        
        // Evict rarely accessed entries when near capacity
        if current_size >= max_size * 9 / 10 {
            return entry.metadata.access_count < 2;
        }
        
        false
    }
}

impl Default for PolicyScheduler {
    fn default() -> Self {
        Self::new()
    }
}

/// Resource constraints for memory selection.
#[derive(Debug, Clone)]
pub struct ResourceConstraints {
    pub max_memory_mb: Option<usize>,
    pub max_latency_ms: Option<u64>,
    pub max_cost_usd: Option<f64>,
}

impl Default for ResourceConstraints {
    fn default() -> Self {
        Self {
            max_memory_mb: None,
            max_latency_ms: None,
            max_cost_usd: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_query_policy() {
        let scheduler = PolicyScheduler::new();
        
        let task = TaskCharacteristics {
            task_type: TaskType::Query,
            complexity: 0.3,
            modality: Modality::Text,
            context_requirements: Default::default(),
            latency_sla_ms: None,
            budget_usd: None,
        };
        
        let policy = scheduler.select_policy(&task, None).await.unwrap();
        
        assert_eq!(policy.primary_layer, LayerType::Stm);
    }

    #[tokio::test]
    async fn test_reasoning_policy() {
        let scheduler = PolicyScheduler::new();
        
        let task = TaskCharacteristics {
            task_type: TaskType::Reasoning,
            complexity: 0.8,
            modality: Modality::Text,
            context_requirements: Default::default(),
            latency_sla_ms: None,
            budget_usd: None,
        };
        
        let policy = scheduler.select_policy(&task, None).await.unwrap();
        
        assert!(policy.secondary_layers.contains(&LayerType::Kg));
    }
}
