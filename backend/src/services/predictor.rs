use crate::models::*;
use crate::services::agent::{MemoryAgent, PredictorDecision};

pub struct PerformancePredictionModel {
    performance_baselines: PerformanceBaselines,
    marginal_decay_factors: MarginalDecayFactors,
}

impl PerformancePredictionModel {
    pub fn new() -> Self {
        Self {
            performance_baselines: PerformanceBaselines {
                stm: PerformanceBaseline {
                    efficiency_gain: 0.2473,
                    coherence_gain: 0.5447,
                    resource_cost: 0.2,
                },
                ltm: PerformanceBaseline {
                    efficiency_gain: 0.3698,
                    coherence_gain: 1.3751,
                    resource_cost: 0.4,
                },
                kg: PerformanceBaseline {
                    efficiency_gain: 0.4273,
                    coherence_gain: 1.5970,
                    resource_cost: 0.6,
                },
                mm: PerformanceBaseline {
                    efficiency_gain: 0.4314,
                    coherence_gain: 1.9312,
                    resource_cost: 0.8,
                },
            },
            marginal_decay_factors: MarginalDecayFactors {
                stm_to_ltm: 0.495,
                ltm_to_kg: 0.470,
                kg_to_mm: 0.071,
            },
        }
    }

    pub fn get_baselines(&self) -> &PerformanceBaselines {
        &self.performance_baselines
    }

    pub fn get_marginal_decay_factors(&self) -> &MarginalDecayFactors {
        &self.marginal_decay_factors
    }

    pub fn predict_memory_performance(
        &self,
        memory_config: &MemoryConfig,
    ) -> (PerformancePrediction, f64, f64, PerformanceBreakdown) {
        // 获取基础性能
        let base_performance = match memory_config.primary_memory {
            MemoryType::Stm => &self.performance_baselines.stm,
            MemoryType::Ltm => &self.performance_baselines.ltm,
            MemoryType::Kg => &self.performance_baselines.kg,
            MemoryType::Mm => &self.performance_baselines.mm,
        };

        // 计算协同效应
        let synergy_factor = self.calculate_synergy_factor(memory_config);

        // 应用边际效益递减
        let decay_factor = self.calculate_decay_factor(memory_config);

        // 计算最终性能
        let efficiency = base_performance.efficiency_gain * synergy_factor * decay_factor;
        let coherence = base_performance.coherence_gain * synergy_factor * decay_factor;

        // 估算资源成本
        let resource_cost = self.estimate_resource_cost(memory_config);

        // 计算成本效益比
        let performance_score = efficiency * 0.6 + coherence * 0.4;
        let cost_benefit_ratio = if resource_cost > 0.0 {
            Some(performance_score / resource_cost)
        } else {
            None
        };

        let prediction = PerformancePrediction {
            efficiency_gain: efficiency,
            coherence_gain: coherence,
            resource_cost,
            cost_benefit_ratio,
            confidence_score: Some(0.88),
        };

        // 计算性能分解
        let breakdown = self.calculate_performance_breakdown(memory_config);

        (prediction, synergy_factor, decay_factor, breakdown)
    }
}

impl MemoryAgent for PerformancePredictionModel {
    type Context = MemoryConfig;
    type Observation = MemoryConfig;
    type Decision = PredictorDecision;
    type Action = PredictorDecision;

    fn observe(
        &self,
        context: &Self::Context,
    ) -> impl std::future::Future<Output = Self::Observation> + Send {
        let config = context.clone();
        std::future::ready(config)
    }

    fn decide(
        &self,
        observation: &Self::Observation,
    ) -> impl std::future::Future<Output = Self::Decision> + Send {
        let (prediction, synergy_factor, decay_factor, performance_breakdown) =
            self.predict_memory_performance(observation);
        std::future::ready(PredictorDecision {
            prediction,
            synergy_factor,
            decay_factor,
            performance_breakdown,
        })
    }

    fn act(
        &self,
        decision: &Self::Decision,
    ) -> impl std::future::Future<Output = Result<Self::Action, crate::AppError>> + Send {
        let d = decision.clone();
        std::future::ready(Ok(d))
    }
}

impl PerformancePredictionModel {
    fn calculate_synergy_factor(&self, memory_config: &MemoryConfig) -> f64 {
        let active_layers = if memory_config.memory_weights.stm > 0.0 {
            1
        } else {
            0
        } + if memory_config.memory_weights.ltm > 0.0 {
            1
        } else {
            0
        } + if memory_config.memory_weights.kg > 0.0 {
            1
        } else {
            0
        } + if memory_config.memory_weights.mm > 0.0 {
            1
        } else {
            0
        };

        1.0 + ((active_layers - 1) as f64) * 0.1
    }

    fn calculate_decay_factor(&self, memory_config: &MemoryConfig) -> f64 {
        let mut decay = 1.0;

        // 如果启用了 LTM，应用 STM 到 LTM 的衰减
        if memory_config.memory_weights.ltm > 0.0 {
            decay *= 1.0 - self.marginal_decay_factors.stm_to_ltm * 0.5;
        }

        // 如果启用了 KG，应用 LTM 到 KG 的衰减
        if memory_config.memory_weights.kg > 0.0 {
            decay *= 1.0 - self.marginal_decay_factors.ltm_to_kg * 0.5;
        }

        // 如果启用了 MM，应用 KG 到 MM 的衰减
        if memory_config.memory_weights.mm > 0.0 {
            decay *= 1.0 - self.marginal_decay_factors.kg_to_mm * 0.5;
        }

        decay.max(0.5) // 最小衰减到 50%
    }

    fn estimate_resource_cost(&self, memory_config: &MemoryConfig) -> f64 {
        let mut cost = 0.0;

        cost += memory_config.memory_weights.stm * self.performance_baselines.stm.resource_cost;
        cost += memory_config.memory_weights.ltm * self.performance_baselines.ltm.resource_cost;
        cost += memory_config.memory_weights.kg * self.performance_baselines.kg.resource_cost;
        cost += memory_config.memory_weights.mm * self.performance_baselines.mm.resource_cost;

        cost.min(1.0)
    }

    fn calculate_performance_breakdown(
        &self,
        memory_config: &MemoryConfig,
    ) -> PerformanceBreakdown {
        PerformanceBreakdown {
            stm_contribution: memory_config.memory_weights.stm
                * self.performance_baselines.stm.efficiency_gain,
            ltm_contribution: memory_config.memory_weights.ltm
                * self.performance_baselines.ltm.efficiency_gain
                * 0.48, // 考虑边际递减
            kg_contribution: memory_config.memory_weights.kg
                * self.performance_baselines.kg.efficiency_gain
                * 0.23, // 考虑边际递减
            mm_contribution: memory_config.memory_weights.mm
                * self.performance_baselines.mm.efficiency_gain
                * 0.016, // 考虑边际递减
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_predictor_creation() {
        let predictor = PerformancePredictionModel::new();
        assert_eq!(predictor.performance_baselines.stm.efficiency_gain, 0.2473);
        assert_eq!(predictor.performance_baselines.ltm.efficiency_gain, 0.3698);
    }

    #[test]
    fn test_predict_memory_performance() {
        let predictor = PerformancePredictionModel::new();
        let memory_config = MemoryConfig {
            primary_memory: MemoryType::Stm,
            secondary_memory: vec![MemoryType::Ltm, MemoryType::Kg],
            memory_weights: MemoryWeights {
                stm: 1.0,
                ltm: 0.8,
                kg: 0.7,
                mm: 0.0,
            },
            reasoning_depth: "deep".to_string(),
            enable_multimodal: false,
        };

        let (prediction, synergy, _decay, _breakdown) =
            predictor.predict_memory_performance(&memory_config);
        assert!(prediction.efficiency_gain > 0.0);
        assert!(prediction.coherence_gain > 0.0);
        assert!(synergy >= 1.0);
    }

    #[test]
    fn test_calculate_synergy_factor() {
        let predictor = PerformancePredictionModel::new();
        let memory_config = MemoryConfig {
            primary_memory: MemoryType::Stm,
            secondary_memory: vec![MemoryType::Ltm, MemoryType::Kg],
            memory_weights: MemoryWeights {
                stm: 1.0,
                ltm: 0.8,
                kg: 0.7,
                mm: 0.0,
            },
            reasoning_depth: "deep".to_string(),
            enable_multimodal: false,
        };

        let synergy = predictor.calculate_synergy_factor(&memory_config);
        assert!(synergy >= 1.0);
    }
}
