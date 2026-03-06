use crate::db::memory::MemoryConfigRepository;
use crate::models::*;
use crate::services::*;
use crate::services::agent::{MemoryAgent, TaskContextBundle};
use tracing::{debug, error, info, instrument, warn};

pub struct AdaptiveMemoryScheduler {
    analyzer: TaskCharacteristicAnalyzer,
    predictor: PerformancePredictionModel,
    monitor: ResourceMonitor,
    weight_adjuster: DynamicWeightAdjuster,
}

impl AdaptiveMemoryScheduler {
    pub fn new() -> Self {
        Self {
            analyzer: TaskCharacteristicAnalyzer::new(),
            predictor: PerformancePredictionModel::new(),
            monitor: ResourceMonitor::new(),
            weight_adjuster: DynamicWeightAdjuster::new(),
        }
    }

    #[instrument(skip(self), fields(task_id = %task_context.task_id))]
    pub async fn adaptive_memory_selection(
        &self,
        task_context: &TaskContext,
        _resource_constraints: &ResourceConstraints,
        _preferences: &TaskPreferences,
    ) -> Result<MemorySelectionResult, crate::AppError> {
        info!("开始自适应记忆选择，任务ID: {}", task_context.task_id);
        // 1. 任务特征分析
        debug!("步骤1: 分析任务特征");

        // 构建更丰富的任务上下文输入
        let task_input = TaskContextInput {
            content: format!(
                "Task: {}
Type: {:?}
Complexity: {}
Modality: {:?}
Temporal Scope: {:?}
Reasoning Depth: {:?}
User ID: {}
Agent ID: {}",
                task_context.task_id,
                task_context.task_type,
                task_context.complexity,
                task_context.modality_requirements,
                task_context.temporal_scope,
                task_context.reasoning_depth,
                task_context.user_id,
                task_context.agent_id
            ),
            modality: task_context
                .modality_requirements
                .iter()
                .map(|m| format!("{:?}", m).to_lowercase())
                .collect(),
            context_history: Vec::new(),
            task_metadata: None,
        };

        let (characteristics, memory_strategy, _) =
            self.analyzer.analyze_task_characteristics(&task_input);

        // 2. 资源状态评估
        debug!("步骤2: 评估资源状态");
        let resource_status = self.monitor.get_current_status().await;

        // 3. 构建初始记忆配置
        debug!("步骤3: 构建初始记忆配置");
        let mut memory_config = MemoryConfig {
            primary_memory: MemoryType::Stm,
            secondary_memory: memory_strategy
                .secondary_memory
                .iter()
                .map(|s| match s.as_str() {
                    "ltm" => MemoryType::Ltm,
                    "kg" => MemoryType::Kg,
                    "mm" => MemoryType::Mm,
                    _ => MemoryType::Stm,
                })
                .collect(),
            memory_weights: MemoryWeights {
                stm: 1.0,
                ltm: if memory_strategy
                    .secondary_memory
                    .contains(&"ltm".to_string())
                {
                    0.8
                } else {
                    0.0
                },
                kg: if memory_strategy.secondary_memory.contains(&"kg".to_string()) {
                    0.7
                } else {
                    0.0
                },
                mm: if memory_strategy.enable_multimodal {
                    0.6
                } else {
                    0.0
                },
            },
            reasoning_depth: memory_strategy.reasoning_depth,
            enable_multimodal: memory_strategy.enable_multimodal,
        };

        // 4. 性能收益预测
        debug!("步骤4: 预测性能收益");
        let (performance_prediction, synergy_factor, decay_factor, performance_breakdown) =
            self.predictor.predict_memory_performance(&memory_config);

        // 5. 成本效益分析
        debug!("步骤5: 分析成本效益");
        let cost_benefit_ratio = self
            .monitor
            .calculate_cost_benefit_ratio(&performance_prediction, &resource_status.current_status);
        debug!("成本效益比: {:.2}", cost_benefit_ratio);

        // 6. 动态权重调整
        debug!("步骤6: 动态调整权重");
        let (adjusted_weights, adjustment_reasons) = self
            .weight_adjuster
            .adjust_memory_weights(
                &characteristics,
                cost_benefit_ratio,
                Some(&memory_config.memory_weights),
                Some(&task_context.task_id),
            )
            .await?;

        memory_config.memory_weights = adjusted_weights;

        // 7. 重新计算性能预测（基于调整后的权重）
        debug!("步骤7: 重新计算性能预测");
        let (final_prediction, _, _, _) = self.predictor.predict_memory_performance(&memory_config);

        // 8. 估算资源需求
        debug!("步骤8: 估算资源需求");
        let resource_requirements = ResourceRequirements {
            estimated_memory_mb: (memory_config.memory_weights.stm * 256.0
                + memory_config.memory_weights.ltm * 512.0
                + memory_config.memory_weights.kg * 256.0
                + memory_config.memory_weights.mm * 512.0) as u64,
            estimated_cpu_percent: ((memory_config.memory_weights.stm * 20.0
                + memory_config.memory_weights.ltm * 30.0
                + memory_config.memory_weights.kg * 25.0
                + memory_config.memory_weights.mm * 35.0)
                as u8)
                .min(80),
            estimated_response_time_ms: (500.0
                + memory_config.memory_weights.ltm * 300.0
                + memory_config.memory_weights.kg * 200.0
                + memory_config.memory_weights.mm * 400.0)
                as u64,
        };

        // 9. 保存配置到数据库
        debug!("步骤9: 保存配置到数据库");
        let config_id = MemoryConfigRepository::create(
            &task_context.user_id,
            &task_context.agent_id,
            &format!("Config for task {}", task_context.task_id),
            "optimized",
            &memory_config,
        )
        .await?;

        info!(
            config_id = %config_id,
            task_id = %task_context.task_id,
            "记忆配置已保存到数据库"
        );

        // 10. 如果选择了长期记忆，自动存储任务内容
        if memory_config.memory_weights.ltm > 0.0 {
            debug!("步骤10: 检测到长期记忆，开始存储任务内容");

            // 构建任务内容摘要
            let task_content = format!(
                "Task ID: {}\nTask Type: {:?}\nComplexity: {:.2}\nModality: {:?}\nTemporal Scope: {:?}\nReasoning Depth: {:?}",
                task_context.task_id,
                task_context.task_type,
                characteristics.complexity,
                task_context.modality_requirements,
                task_context.temporal_scope,
                task_context.reasoning_depth
            );

            // 异步存储长期记忆（不阻塞主流程），带重试机制
            let task_id = task_context.task_id.clone();
            let user_id = task_context.user_id.clone();
            let task_content = task_content.clone();
            tokio::spawn(async move {
                let _ = user_id; // 保留用于未来扩展（如用户隔离的日志）

                // 重试机制：最多重试3次
                let max_retries = 3;
                let mut last_error = None;

                for attempt in 1..=max_retries {
                    match MemoryStorageService::store_ltm(
                        &task_id,
                        "task",
                        &task_content,
                        Some(&format!("Task {}", task_id)),
                    )
                    .await
                    {
                        Ok(entry_id) => {
                            info!(
                                task_id = %task_id,
                                entry_id = %entry_id,
                                "任务内容已存储为长期记忆"
                            );
                            return; // 成功，直接返回
                        }
                        Err(e) => {
                            let error_msg = format!("{}", e);
                            last_error = Some(e);
                            if attempt < max_retries {
                                warn!(
                                    task_id = %task_id,
                                    attempt = %attempt,
                                    error = %error_msg,
                                    "存储长期记忆失败，尝试重试..."
                                );
                                tokio::time::sleep(tokio::time::Duration::from_secs(2u64.pow(attempt as u32))).await;
                            }
                        }
                    }
                }

                // 所有重试都失败
                if let Some(e) = last_error {
                    error!(
                        task_id = %task_id,
                        error = %e,
                        "存储长期记忆失败，已达最大重试次数"
                    );
                    // TODO: 可以添加错误上报到监控系统
                }
            });
        }

        Ok(MemorySelectionResult {
            memory_config,
            performance_prediction: final_prediction,
            resource_requirements,
            synergy_factor,
            decay_factor,
            performance_breakdown,
            adjustment_reasons,
        })
    }

    /// Runs the same pipeline as adaptive_memory_selection but returns full decision trace (no DB persist, no LTM store).
    #[instrument(skip(self), fields(task_id = %task_context.task_id))]
    pub async fn adaptive_memory_selection_trace(
        &self,
        task_context: &TaskContext,
        _resource_constraints: &ResourceConstraints,
        _preferences: &TaskPreferences,
    ) -> Result<DecisionTrace, crate::AppError> {
        let task_input = TaskContextInput {
            content: format!(
                "Task: {} Type: {:?} Complexity: {} Modality: {:?}",
                task_context.task_id,
                task_context.task_type,
                task_context.complexity,
                task_context.modality_requirements
            ),
            modality: task_context
                .modality_requirements
                .iter()
                .map(|m| format!("{:?}", m).to_lowercase())
                .collect(),
            context_history: Vec::new(),
            task_metadata: None,
        };

        let (characteristics, memory_strategy, confidence_score) =
            self.analyzer.analyze_task_characteristics(&task_input);
        let resource_status = self.monitor.get_current_status().await;

        let mut memory_config = MemoryConfig {
            primary_memory: MemoryType::Stm,
            secondary_memory: memory_strategy
                .secondary_memory
                .iter()
                .map(|s| match s.as_str() {
                    "ltm" => MemoryType::Ltm,
                    "kg" => MemoryType::Kg,
                    "mm" => MemoryType::Mm,
                    _ => MemoryType::Stm,
                })
                .collect(),
            memory_weights: MemoryWeights {
                stm: 1.0,
                ltm: if memory_strategy.secondary_memory.contains(&"ltm".to_string()) {
                    0.8
                } else {
                    0.0
                },
                kg: if memory_strategy.secondary_memory.contains(&"kg".to_string()) {
                    0.7
                } else {
                    0.0
                },
                mm: if memory_strategy.enable_multimodal {
                    0.6
                } else {
                    0.0
                },
            },
            reasoning_depth: memory_strategy.reasoning_depth.clone(),
            enable_multimodal: memory_strategy.enable_multimodal,
        };

        let (performance_prediction, synergy_factor, decay_factor, performance_breakdown) =
            self.predictor.predict_memory_performance(&memory_config);
        let cost_benefit_ratio = self.monitor.calculate_cost_benefit_ratio(
            &performance_prediction,
            &resource_status.current_status,
        );

        let initial_memory_config = memory_config.clone();
        let (adjusted_weights, adjustment_reasons) = self
            .weight_adjuster
            .adjust_memory_weights(
                &characteristics,
                cost_benefit_ratio,
                Some(&memory_config.memory_weights),
                None,
            )
            .await?;

        memory_config.memory_weights = adjusted_weights.clone();
        let (final_prediction, _, _, _) = self.predictor.predict_memory_performance(&memory_config);
        let resource_requirements = ResourceRequirements {
            estimated_memory_mb: (memory_config.memory_weights.stm * 256.0
                + memory_config.memory_weights.ltm * 512.0
                + memory_config.memory_weights.kg * 256.0
                + memory_config.memory_weights.mm * 512.0) as u64,
            estimated_cpu_percent: ((memory_config.memory_weights.stm * 20.0
                + memory_config.memory_weights.ltm * 30.0
                + memory_config.memory_weights.kg * 25.0
                + memory_config.memory_weights.mm * 35.0)
                as u8)
                .min(80),
            estimated_response_time_ms: (500.0
                + memory_config.memory_weights.ltm * 300.0
                + memory_config.memory_weights.kg * 200.0
                + memory_config.memory_weights.mm * 400.0)
                as u64,
        };

        let final_result = MemorySelectionResult {
            memory_config: memory_config.clone(),
            performance_prediction: final_prediction,
            resource_requirements,
            synergy_factor,
            decay_factor,
            performance_breakdown: performance_breakdown.clone(),
            adjustment_reasons: adjustment_reasons.clone(),
        };

        let memory_contributions = vec![
            MemoryTypeContribution {
                memory_type: "stm".to_string(),
                weight: memory_config.memory_weights.stm,
                reason: adjustment_reasons.stm.clone(),
            },
            MemoryTypeContribution {
                memory_type: "ltm".to_string(),
                weight: memory_config.memory_weights.ltm,
                reason: adjustment_reasons.ltm.clone(),
            },
            MemoryTypeContribution {
                memory_type: "kg".to_string(),
                weight: memory_config.memory_weights.kg,
                reason: adjustment_reasons.kg.clone(),
            },
            MemoryTypeContribution {
                memory_type: "mm".to_string(),
                weight: memory_config.memory_weights.mm,
                reason: adjustment_reasons.mm.clone(),
            },
        ];

        Ok(DecisionTrace {
            task_id: task_context.task_id.clone(),
            analyzer: AnalyzerTraceStep {
                task_characteristics: characteristics,
                memory_strategy,
                confidence_score,
            },
            resource_status,
            initial_memory_config,
            predictor: PredictorTraceStep {
                performance_prediction,
                synergy_factor,
                decay_factor,
                performance_breakdown,
            },
            cost_benefit_ratio,
            weight_adjustment: WeightAdjustmentTraceStep {
                adjusted_weights,
                adjustment_reasons,
            },
            memory_contributions,
            final_result,
        })
    }
}

impl MemoryAgent for AdaptiveMemoryScheduler {
    type Context = TaskContextBundle;
    type Observation = ();
    type Decision = ();
    type Action = MemorySelectionResult;

    fn observe(&self, _context: &Self::Context) -> impl std::future::Future<Output = Self::Observation> + Send {
        std::future::ready(())
    }

    fn decide(&self, _observation: &Self::Observation) -> impl std::future::Future<Output = Self::Decision> + Send {
        std::future::ready(())
    }

    fn act(&self, _decision: &Self::Decision) -> impl std::future::Future<Output = Result<Self::Action, crate::AppError>> + Send {
        // Scheduler runs the full pipeline in adaptive_memory_selection(); use that with TaskContextBundle.
        std::future::ready(Err(crate::AppError::Internal(
            "SchedulerAgent: use adaptive_memory_selection(task_context, resource_constraints, preferences) instead of act()".into(),
        )))
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, salvo::oapi::ToSchema)]
pub struct MemorySelectionResult {
    #[serde(rename = "memory_config")]
    pub memory_config: MemoryConfig,
    #[serde(rename = "performance_prediction")]
    pub performance_prediction: PerformancePrediction,
    #[serde(rename = "resource_requirements")]
    pub resource_requirements: ResourceRequirements,
    #[serde(rename = "synergy_factor")]
    pub synergy_factor: f64,
    #[serde(rename = "decay_factor")]
    pub decay_factor: f64,
    #[serde(rename = "performance_breakdown")]
    pub performance_breakdown: PerformanceBreakdown,
    #[serde(rename = "adjustment_reasons")]
    pub adjustment_reasons: crate::models::AdjustmentReasons,
}

/// Full decision pipeline trace (no DB persist or LTM store).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, salvo::oapi::ToSchema)]
pub struct DecisionTrace {
    #[serde(rename = "task_id")]
    pub task_id: String,
    #[serde(rename = "analyzer")]
    pub analyzer: AnalyzerTraceStep,
    #[serde(rename = "resource_status")]
    pub resource_status: crate::models::CurrentResourceStatus,
    #[serde(rename = "initial_memory_config")]
    pub initial_memory_config: MemoryConfig,
    #[serde(rename = "predictor")]
    pub predictor: PredictorTraceStep,
    #[serde(rename = "cost_benefit_ratio")]
    pub cost_benefit_ratio: f64,
    #[serde(rename = "weight_adjustment")]
    pub weight_adjustment: WeightAdjustmentTraceStep,
    #[serde(rename = "final_result")]
    pub final_result: MemorySelectionResult,
    /// Per-memory-type contribution: why each type was selected and at what weight.
    #[serde(rename = "memory_contributions")]
    pub memory_contributions: Vec<MemoryTypeContribution>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, salvo::oapi::ToSchema)]
pub struct MemoryTypeContribution {
    #[serde(rename = "memory_type")]
    pub memory_type: String,
    #[serde(rename = "weight")]
    pub weight: f64,
    #[serde(rename = "reason")]
    pub reason: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, salvo::oapi::ToSchema)]
pub struct AnalyzerTraceStep {
    #[serde(rename = "task_characteristics")]
    pub task_characteristics: TaskCharacteristics,
    #[serde(rename = "memory_strategy")]
    pub memory_strategy: MemoryStrategy,
    #[serde(rename = "confidence_score")]
    pub confidence_score: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, salvo::oapi::ToSchema)]
pub struct PredictorTraceStep {
    #[serde(rename = "performance_prediction")]
    pub performance_prediction: PerformancePrediction,
    #[serde(rename = "synergy_factor")]
    pub synergy_factor: f64,
    #[serde(rename = "decay_factor")]
    pub decay_factor: f64,
    #[serde(rename = "performance_breakdown")]
    pub performance_breakdown: PerformanceBreakdown,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, salvo::oapi::ToSchema)]
pub struct WeightAdjustmentTraceStep {
    #[serde(rename = "adjusted_weights")]
    pub adjusted_weights: MemoryWeights,
    #[serde(rename = "adjustment_reasons")]
    pub adjustment_reasons: crate::models::AdjustmentReasons,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{TaskContext, TaskType, TemporalScope, ReasoningDepth, ResourceConstraints, TaskPreferences};

    #[tokio::test]
    async fn test_scheduler_creation() {
        let scheduler = AdaptiveMemoryScheduler::new();
        assert!(true);
    }

    #[test]
    fn test_task_context_default() {
        let context = TaskContext {
            task_id: "test_001".to_string(),
            task_type: TaskType::Query,
            complexity: 0.5,
            modality_requirements: vec![Modality::Text],
            temporal_scope: TemporalScope::Short,
            reasoning_depth: ReasoningDepth::Shallow,
            context_dependency: 0.5,
            user_id: "user_1".to_string(),
            agent_id: "agent_1".to_string(),
        };

        assert_eq!(context.task_id, "test_001");
        assert_eq!(context.task_type, TaskType::Query);
        assert_eq!(context.complexity, 0.5);
    }

    #[test]
    fn test_resource_constraints_default() {
        let constraints = ResourceConstraints {
            max_memory_usage_mb: 1024,
            max_cpu_usage_percent: 80,
            max_response_time_ms: 2000,
            storage_quota_percent: 90,
        };

        assert_eq!(constraints.max_memory_usage_mb, 1024);
        assert_eq!(constraints.max_cpu_usage_percent, 80);
    }

    #[test]
    fn test_task_preferences_default() {
        let preferences = TaskPreferences {
            prioritize_efficiency: true,
            prioritize_coherence: false,
            enable_multimodal: true,
            enable_reasoning: true,
        };

        assert!(preferences.prioritize_efficiency);
        assert!(!preferences.prioritize_coherence);
        assert!(preferences.enable_multimodal);
        assert!(preferences.enable_reasoning);
    }
}
