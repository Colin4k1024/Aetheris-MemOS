use crate::db::weights::WeightHistoryRepository;
use crate::models::*;
use crate::services::weight_strategy::{LinearDecayStrategy, MarginalBenefitStrategy};
use crate::services::weight_strategy::{WeightDelta, WeightStrategy, WeightStrategyMetrics};
use tracing::info;

pub struct DynamicWeightAdjuster {
    strategies: Vec<Box<dyn WeightStrategy>>,
}

impl Default for DynamicWeightAdjuster {
    fn default() -> Self {
        Self::new()
    }
}

impl DynamicWeightAdjuster {
    pub fn new() -> Self {
        Self {
            strategies: vec![
                Box::new(MarginalBenefitStrategy),
                Box::new(LinearDecayStrategy),
            ],
        }
    }

    /// Build an adjuster with a custom strategy chain (for plugins / tests).
    pub fn with_strategies(strategies: Vec<Box<dyn WeightStrategy>>) -> Self {
        Self { strategies }
    }

    pub async fn adjust_memory_weights(
        &self,
        task_profile: &TaskCharacteristics,
        cost_benefit_ratio: f64,
        current_weights: Option<&MemoryWeights>,
        task_id: Option<&str>,
    ) -> Result<(MemoryWeights, AdjustmentReasons), crate::AppError> {
        let base_weights = current_weights.cloned().unwrap_or_else(|| MemoryWeights {
            stm: 1.0,
            ltm: 0.0,
            kg: 0.0,
            mm: 0.0,
        });

        let mut weights = base_weights.clone();
        let mut reasons = crate::models::AdjustmentReasons {
            stm: "Primary memory, always enabled".to_string(),
            ltm: String::new(),
            kg: String::new(),
            mm: String::new(),
        };
        let mut strategy_names: Vec<&'static str> = Vec::with_capacity(self.strategies.len());

        for strategy in &self.strategies {
            strategy_names.push(strategy.name());
            let metrics = WeightStrategyMetrics {
                task_profile,
                cost_benefit_ratio,
                base_weights: &weights,
            };
            let delta: WeightDelta = strategy.evaluate(&metrics);
            weights = delta.weights;
            merge_reasons(&mut reasons, &delta.reasons);
        }

        if let Some(task_id) = task_id {
            let performance_impact = (cost_benefit_ratio - 1.0) * 0.1;
            let strategy_metadata = serde_json::to_string(&strategy_names).ok();
            let _ = WeightHistoryRepository::create(
                task_id,
                &base_weights,
                &weights,
                &reasons,
                performance_impact as f32,
                strategy_metadata.as_deref(),
            )
            .await;
            info!("Saved weight adjustment history for task: {}", task_id);
        }

        Ok((weights, reasons))
    }
}

/// Re-export for API and DB use.
pub use crate::models::AdjustmentReasons;

fn merge_reasons(
    acc: &mut crate::models::AdjustmentReasons,
    next: &crate::models::AdjustmentReasons,
) {
    if !next.ltm.is_empty() {
        if !acc.ltm.is_empty() {
            acc.ltm.push_str("; ");
        }
        acc.ltm.push_str(&next.ltm);
    }
    if !next.kg.is_empty() {
        if !acc.kg.is_empty() {
            acc.kg.push_str("; ");
        }
        acc.kg.push_str(&next.kg);
    }
    if !next.mm.is_empty() {
        if !acc.mm.is_empty() {
            acc.mm.push_str("; ");
        }
        acc.mm.push_str(&next.mm);
    }
}
