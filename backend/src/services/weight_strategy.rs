//! Weight strategy plugin abstraction: each strategy produces a weight suggestion from metrics.
//! Built-in strategies: complexity/modality/reasoning (marginal benefit), cost-benefit decay, synergy.

#![allow(dead_code)]

use crate::models::*;

/// Input to a weight strategy: task profile, cost-benefit ratio, and current (base) weights.
#[derive(Debug, Clone)]
pub struct WeightStrategyMetrics<'a> {
    pub task_profile: &'a TaskCharacteristics,
    pub cost_benefit_ratio: f64,
    pub base_weights: &'a MemoryWeights,
}

/// Result of a strategy: suggested weights and reasons for this step.
#[derive(Debug, Clone)]
pub struct WeightDelta {
    pub weights: MemoryWeights,
    pub reasons: crate::models::AdjustmentReasons,
}

/// Strategy that evaluates metrics and returns a weight delta (suggested weights + reasons).
pub trait WeightStrategy: Send + Sync {
    fn evaluate(&self, metrics: &WeightStrategyMetrics<'_>) -> WeightDelta;
    /// Identifier for experiment tracking (strategy chain order is preserved in history).
    fn name(&self) -> &'static str;
}

/// LTM from complexity, MM from modality count, KG from reasoning depth (marginal benefit).
#[derive(Debug, Default, Clone)]
pub struct MarginalBenefitStrategy;

impl WeightStrategy for MarginalBenefitStrategy {
    fn name(&self) -> &'static str {
        "MarginalBenefit"
    }
    fn evaluate(&self, metrics: &WeightStrategyMetrics<'_>) -> WeightDelta {
        let mut weights = metrics.base_weights.clone();
        let mut reasons = crate::models::AdjustmentReasons {
            stm: "Primary memory, always enabled".to_string(),
            ltm: String::new(),
            kg: String::new(),
            mm: String::new(),
        };

        if metrics.task_profile.complexity > 0.5 {
            weights.ltm = (metrics.task_profile.complexity * 0.8).min(0.8);
            reasons.ltm = format!(
                "High complexity task ({:.2}) requires long-term memory",
                metrics.task_profile.complexity
            );
        }
        if metrics.task_profile.modality_count > 1 {
            weights.mm = (metrics.task_profile.modality_count as f64 * 0.3).min(0.6);
            reasons.mm = format!(
                "Multi-modal task detected ({} modalities), enabling multimodal memory",
                metrics.task_profile.modality_count
            );
        }
        if metrics.task_profile.reasoning_depth > 0.7 {
            weights.kg = (metrics.task_profile.reasoning_depth * 0.7).min(0.7);
            reasons.kg = format!(
                "Deep reasoning required (depth: {:.2}), enabling knowledge graph",
                metrics.task_profile.reasoning_depth
            );
        }

        WeightDelta { weights, reasons }
    }
}

/// Linear decay: scale down LTM/KG/MM when cost-benefit ratio is below 1.0.
#[derive(Debug, Default, Clone)]
pub struct LinearDecayStrategy;

impl WeightStrategy for LinearDecayStrategy {
    fn name(&self) -> &'static str {
        "LinearDecay"
    }
    fn evaluate(&self, metrics: &WeightStrategyMetrics<'_>) -> WeightDelta {
        let mut weights = metrics.base_weights.clone();
        let mut reasons = crate::models::AdjustmentReasons {
            stm: "Primary memory, always enabled".to_string(),
            ltm: String::new(),
            kg: String::new(),
            mm: String::new(),
        };

        if metrics.cost_benefit_ratio < 1.0 {
            weights.ltm *= 0.5;
            weights.kg *= 0.5;
            weights.mm *= 0.5;
            reasons.ltm = "Reduced due to low cost-benefit ratio".to_string();
            reasons.kg = "Reduced due to low cost-benefit ratio".to_string();
            reasons.mm = "Reduced due to low cost-benefit ratio".to_string();
        }

        WeightDelta { weights, reasons }
    }
}

/// Synergy-aware: small boost when multiple secondary layers are active (optional second pass).
#[derive(Debug, Default, Clone)]
pub struct SynergyAwareStrategy;

impl WeightStrategy for SynergyAwareStrategy {
    fn name(&self) -> &'static str {
        "SynergyAware"
    }
    fn evaluate(&self, metrics: &WeightStrategyMetrics<'_>) -> WeightDelta {
        let weights = metrics.base_weights.clone();
        let active_secondary = (weights.ltm > 0.0) as i32
            + (weights.kg > 0.0) as i32
            + (weights.mm > 0.0) as i32;
        let mut reasons = crate::models::AdjustmentReasons {
            stm: "Primary memory, always enabled".to_string(),
            ltm: String::new(),
            kg: String::new(),
            mm: String::new(),
        };

        if active_secondary >= 2 {
            let boost = 1.0 + (active_secondary as f64) * 0.05;
            let mut w = weights;
            w.ltm = (w.ltm * boost).min(1.0);
            w.kg = (w.kg * boost).min(1.0);
            w.mm = (w.mm * boost).min(1.0);
            reasons.ltm = "Synergy boost (multiple layers active)".to_string();
            reasons.kg = "Synergy boost (multiple layers active)".to_string();
            reasons.mm = "Synergy boost (multiple layers active)".to_string();
            return WeightDelta { weights: w, reasons };
        }

        WeightDelta { weights, reasons }
    }
}
