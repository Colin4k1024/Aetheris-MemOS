use crate::db::memory::MemoryConfigRepository;
use crate::models::*;
use crate::services::agent::{MemoryAgent, TaskContextBundle};
use crate::services::*;
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
        resource_constraints: &ResourceConstraints,
        preferences: &TaskPreferences,
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
        let mut adjustment_reasons = crate::models::AdjustmentReasons {
            stm: "Primary memory, always enabled".to_string(),
            ltm: String::new(),
            kg: String::new(),
            mm: String::new(),
        };
        apply_preferences(&mut memory_config, preferences, &mut adjustment_reasons);

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
        let (adjusted_weights, strategy_reasons) = self
            .weight_adjuster
            .adjust_memory_weights(
                &characteristics,
                cost_benefit_ratio,
                Some(&memory_config.memory_weights),
                Some(&task_context.task_id),
            )
            .await?;

        memory_config.memory_weights = adjusted_weights;
        merge_adjustment_reasons(&mut adjustment_reasons, &strategy_reasons);
        apply_preferences(&mut memory_config, preferences, &mut adjustment_reasons);
        let resource_requirements = enforce_resource_constraints(
            &mut memory_config,
            resource_constraints,
            &mut adjustment_reasons,
        )?;

        // 7. 重新计算性能预测（基于调整后的权重）
        debug!("步骤7: 重新计算性能预测");
        let (final_prediction, _, _, _) = self.predictor.predict_memory_performance(&memory_config);
        debug!("步骤8: 资源约束校验通过");

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
                                tokio::time::sleep(tokio::time::Duration::from_secs(
                                    2u64.pow(attempt as u32),
                                ))
                                .await;
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
        resource_constraints: &ResourceConstraints,
        preferences: &TaskPreferences,
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
            reasoning_depth: memory_strategy.reasoning_depth.clone(),
            enable_multimodal: memory_strategy.enable_multimodal,
        };
        let mut adjustment_reasons = crate::models::AdjustmentReasons {
            stm: "Primary memory, always enabled".to_string(),
            ltm: String::new(),
            kg: String::new(),
            mm: String::new(),
        };
        apply_preferences(&mut memory_config, preferences, &mut adjustment_reasons);

        let (performance_prediction, synergy_factor, decay_factor, performance_breakdown) =
            self.predictor.predict_memory_performance(&memory_config);
        let cost_benefit_ratio = self
            .monitor
            .calculate_cost_benefit_ratio(&performance_prediction, &resource_status.current_status);

        let initial_memory_config = memory_config.clone();
        let (adjusted_weights, strategy_reasons) = self
            .weight_adjuster
            .adjust_memory_weights(
                &characteristics,
                cost_benefit_ratio,
                Some(&memory_config.memory_weights),
                None,
            )
            .await?;

        memory_config.memory_weights = adjusted_weights.clone();
        merge_adjustment_reasons(&mut adjustment_reasons, &strategy_reasons);
        apply_preferences(&mut memory_config, preferences, &mut adjustment_reasons);
        let resource_requirements = enforce_resource_constraints(
            &mut memory_config,
            resource_constraints,
            &mut adjustment_reasons,
        )?;
        let adjusted_weights = memory_config.memory_weights.clone();
        let (final_prediction, _, _, _) = self.predictor.predict_memory_performance(&memory_config);

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

fn estimate_resource_requirements(weights: &MemoryWeights) -> ResourceRequirements {
    ResourceRequirements {
        estimated_memory_mb: (weights.stm * 256.0
            + weights.ltm * 512.0
            + weights.kg * 256.0
            + weights.mm * 512.0) as u64,
        estimated_cpu_percent: ((weights.stm * 20.0
            + weights.ltm * 30.0
            + weights.kg * 25.0
            + weights.mm * 35.0) as u8)
            .min(95),
        estimated_response_time_ms: (500.0
            + weights.ltm * 300.0
            + weights.kg * 200.0
            + weights.mm * 400.0) as u64,
    }
}

fn estimated_storage_usage_percent(weights: &MemoryWeights) -> u8 {
    ((10.0 + weights.ltm * 45.0 + weights.kg * 25.0 + weights.mm * 35.0) as u8).min(100)
}

fn append_reason(reason: &mut String, msg: &str) {
    if reason.is_empty() {
        reason.push_str(msg);
    } else if !reason.contains(msg) {
        reason.push_str("; ");
        reason.push_str(msg);
    }
}

fn remove_secondary(memory_config: &mut MemoryConfig, memory_type: MemoryType) {
    memory_config
        .secondary_memory
        .retain(|m| match memory_type {
            MemoryType::Ltm => !matches!(m, MemoryType::Ltm),
            MemoryType::Kg => !matches!(m, MemoryType::Kg),
            MemoryType::Mm => !matches!(m, MemoryType::Mm),
            MemoryType::Stm => true,
        });
}

fn apply_preferences(
    memory_config: &mut MemoryConfig,
    preferences: &TaskPreferences,
    reasons: &mut crate::models::AdjustmentReasons,
) {
    if !preferences.enable_multimodal {
        memory_config.memory_weights.mm = 0.0;
        memory_config.enable_multimodal = false;
        remove_secondary(memory_config, MemoryType::Mm);
        append_reason(&mut reasons.mm, "Disabled by user preference");
    }

    if !preferences.enable_reasoning {
        memory_config.memory_weights.kg = 0.0;
        memory_config.reasoning_depth = "shallow".to_string();
        remove_secondary(memory_config, MemoryType::Kg);
        append_reason(&mut reasons.kg, "Disabled by user preference");
    }

    if preferences.prioritize_efficiency && !preferences.prioritize_coherence {
        memory_config.memory_weights.ltm *= 0.85;
        memory_config.memory_weights.kg *= 0.85;
        memory_config.memory_weights.mm *= 0.80;
        append_reason(
            &mut reasons.stm,
            "Efficiency preference: reduce secondary memory usage",
        );
    } else if preferences.prioritize_coherence && !preferences.prioritize_efficiency {
        memory_config.memory_weights.ltm = (memory_config.memory_weights.ltm * 1.10).min(1.0);
        memory_config.memory_weights.kg = (memory_config.memory_weights.kg * 1.15).min(1.0);
        if preferences.enable_multimodal {
            memory_config.memory_weights.mm = (memory_config.memory_weights.mm * 1.05).min(1.0);
        }
        append_reason(
            &mut reasons.stm,
            "Coherence preference: boost secondary memory usage",
        );
    }

    if memory_config.memory_weights.ltm <= 0.0 {
        remove_secondary(memory_config, MemoryType::Ltm);
    }
    if memory_config.memory_weights.kg <= 0.0 {
        remove_secondary(memory_config, MemoryType::Kg);
    }
    if memory_config.memory_weights.mm <= 0.0 {
        remove_secondary(memory_config, MemoryType::Mm);
    }
}

fn enforce_resource_constraints(
    memory_config: &mut MemoryConfig,
    resource_constraints: &ResourceConstraints,
    reasons: &mut crate::models::AdjustmentReasons,
) -> Result<ResourceRequirements, crate::AppError> {
    // STM baseline cost from current estimation model.
    if resource_constraints.max_memory_usage_mb < 256
        || resource_constraints.max_cpu_usage_percent < 20
        || resource_constraints.max_response_time_ms < 500
        || resource_constraints.storage_quota_percent < 10
    {
        return Err(crate::AppError::BadRequest(
            "Resource constraints are lower than STM baseline requirements".to_string(),
        ));
    }

    let mut requirements = estimate_resource_requirements(&memory_config.memory_weights);
    let mut storage_usage = estimated_storage_usage_percent(&memory_config.memory_weights);

    let mut retries = 0;
    while (requirements.estimated_memory_mb > resource_constraints.max_memory_usage_mb
        || requirements.estimated_cpu_percent > resource_constraints.max_cpu_usage_percent
        || requirements.estimated_response_time_ms > resource_constraints.max_response_time_ms
        || storage_usage > resource_constraints.storage_quota_percent)
        && retries < 12
    {
        memory_config.memory_weights.ltm *= 0.85;
        memory_config.memory_weights.kg *= 0.85;
        memory_config.memory_weights.mm *= 0.80;
        retries += 1;
        requirements = estimate_resource_requirements(&memory_config.memory_weights);
        storage_usage = estimated_storage_usage_percent(&memory_config.memory_weights);
    }

    if requirements.estimated_memory_mb > resource_constraints.max_memory_usage_mb
        || requirements.estimated_cpu_percent > resource_constraints.max_cpu_usage_percent
        || requirements.estimated_response_time_ms > resource_constraints.max_response_time_ms
        || storage_usage > resource_constraints.storage_quota_percent
    {
        return Err(crate::AppError::BadRequest(
            "Unable to satisfy resource constraints with current memory policy".to_string(),
        ));
    }

    if retries > 0 {
        append_reason(
            &mut reasons.stm,
            "Resource constraints applied: reduced secondary memory weights",
        );
        append_reason(&mut reasons.ltm, "Reduced by resource constraints");
        append_reason(&mut reasons.kg, "Reduced by resource constraints");
        append_reason(&mut reasons.mm, "Reduced by resource constraints");
    }

    if memory_config.memory_weights.ltm <= 0.01 {
        memory_config.memory_weights.ltm = 0.0;
        remove_secondary(memory_config, MemoryType::Ltm);
    }
    if memory_config.memory_weights.kg <= 0.01 {
        memory_config.memory_weights.kg = 0.0;
        remove_secondary(memory_config, MemoryType::Kg);
    }
    if memory_config.memory_weights.mm <= 0.01 {
        memory_config.memory_weights.mm = 0.0;
        memory_config.enable_multimodal = false;
        remove_secondary(memory_config, MemoryType::Mm);
    }

    Ok(requirements)
}

fn merge_adjustment_reasons(
    target: &mut crate::models::AdjustmentReasons,
    source: &crate::models::AdjustmentReasons,
) {
    append_reason(&mut target.stm, &source.stm);
    append_reason(&mut target.ltm, &source.ltm);
    append_reason(&mut target.kg, &source.kg);
    append_reason(&mut target.mm, &source.mm);
}

impl MemoryAgent for AdaptiveMemoryScheduler {
    type Context = TaskContextBundle;
    type Observation = ();
    type Decision = ();
    type Action = MemorySelectionResult;

    fn observe(
        &self,
        _context: &Self::Context,
    ) -> impl std::future::Future<Output = Self::Observation> + Send {
        std::future::ready(())
    }

    fn decide(
        &self,
        _observation: &Self::Observation,
    ) -> impl std::future::Future<Output = Self::Decision> + Send {
        std::future::ready(())
    }

    fn act(
        &self,
        _decision: &Self::Decision,
    ) -> impl std::future::Future<Output = Result<Self::Action, crate::AppError>> + Send {
        // Scheduler runs the full pipeline in adaptive_memory_selection(); use that with TaskContextBundle.
        std::future::ready(Err(crate::AppError::Internal(
            "SchedulerAgent: use adaptive_memory_selection(task_context, resource_constraints, preferences) instead of act()".into(),
        )))
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct MemoryTypeContribution {
    #[serde(rename = "memory_type")]
    pub memory_type: String,
    #[serde(rename = "weight")]
    pub weight: f64,
    #[serde(rename = "reason")]
    pub reason: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct AnalyzerTraceStep {
    #[serde(rename = "task_characteristics")]
    pub task_characteristics: TaskCharacteristics,
    #[serde(rename = "memory_strategy")]
    pub memory_strategy: MemoryStrategy,
    #[serde(rename = "confidence_score")]
    pub confidence_score: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct WeightAdjustmentTraceStep {
    #[serde(rename = "adjusted_weights")]
    pub adjusted_weights: MemoryWeights,
    #[serde(rename = "adjustment_reasons")]
    pub adjustment_reasons: crate::models::AdjustmentReasons,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        ReasoningDepth, ResourceConstraints, TaskContext, TaskPreferences, TaskType, TemporalScope,
    };

    #[tokio::test]
    async fn test_scheduler_creation() {
        let _scheduler = AdaptiveMemoryScheduler::new();
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

    #[test]
    fn test_apply_preferences_disables_mm_and_kg() {
        let mut config = MemoryConfig {
            primary_memory: MemoryType::Stm,
            secondary_memory: vec![MemoryType::Ltm, MemoryType::Kg, MemoryType::Mm],
            memory_weights: MemoryWeights {
                stm: 1.0,
                ltm: 0.8,
                kg: 0.7,
                mm: 0.6,
            },
            reasoning_depth: "deep".to_string(),
            enable_multimodal: true,
        };
        let prefs = TaskPreferences {
            prioritize_efficiency: true,
            prioritize_coherence: false,
            enable_multimodal: false,
            enable_reasoning: false,
        };
        let mut reasons = crate::models::AdjustmentReasons {
            stm: String::new(),
            ltm: String::new(),
            kg: String::new(),
            mm: String::new(),
        };

        apply_preferences(&mut config, &prefs, &mut reasons);

        assert_eq!(config.memory_weights.mm, 0.0);
        assert_eq!(config.memory_weights.kg, 0.0);
        assert!(!config.enable_multimodal);
        assert!(!config
            .secondary_memory
            .iter()
            .any(|m| matches!(m, MemoryType::Mm)));
        assert!(!config
            .secondary_memory
            .iter()
            .any(|m| matches!(m, MemoryType::Kg)));
    }

    #[test]
    fn test_enforce_resource_constraints_reduces_secondary_weights() {
        let mut config = MemoryConfig {
            primary_memory: MemoryType::Stm,
            secondary_memory: vec![MemoryType::Ltm, MemoryType::Kg, MemoryType::Mm],
            memory_weights: MemoryWeights {
                stm: 1.0,
                ltm: 1.0,
                kg: 1.0,
                mm: 1.0,
            },
            reasoning_depth: "deep".to_string(),
            enable_multimodal: true,
        };
        let constraints = ResourceConstraints {
            max_memory_usage_mb: 700,
            max_cpu_usage_percent: 50,
            max_response_time_ms: 1000,
            storage_quota_percent: 55,
        };
        let mut reasons = crate::models::AdjustmentReasons {
            stm: String::new(),
            ltm: String::new(),
            kg: String::new(),
            mm: String::new(),
        };

        let req = enforce_resource_constraints(&mut config, &constraints, &mut reasons)
            .expect("constraints should be satisfiable");

        assert!(req.estimated_memory_mb <= constraints.max_memory_usage_mb);
        assert!(req.estimated_cpu_percent <= constraints.max_cpu_usage_percent);
        assert!(req.estimated_response_time_ms <= constraints.max_response_time_ms);
        assert!(config.memory_weights.ltm < 1.0 || config.memory_weights.kg < 1.0);
    }
}
