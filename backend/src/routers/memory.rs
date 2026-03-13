use axum::Json;
use axum::extract::{Path, Query};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

use crate::db::{
    decision_trace::DecisionTraceRepository, memory::MemoryConfigRepository,
    performance::PerformanceMetricsRepository, weights::WeightHistoryRepository,
};
use crate::models::*;
use crate::services::*;
use crate::{JsonResult, json_ok};

static SCHEDULER: Lazy<Arc<AdaptiveMemoryScheduler>> =
    Lazy::new(|| Arc::new(AdaptiveMemoryScheduler::new()));

static ANALYZER: Lazy<Arc<TaskCharacteristicAnalyzer>> =
    Lazy::new(|| Arc::new(TaskCharacteristicAnalyzer::new()));

static PREDICTOR: Lazy<Arc<PerformancePredictionModel>> =
    Lazy::new(|| Arc::new(PerformancePredictionModel::new()));

static MONITOR: Lazy<Arc<ResourceMonitor>> = Lazy::new(|| Arc::new(ResourceMonitor::new()));

static WEIGHT_ADJUSTER: Lazy<Arc<DynamicWeightAdjuster>> =
    Lazy::new(|| Arc::new(DynamicWeightAdjuster::new()));

// ========== 自适应记忆调度器 API ==========

#[derive(Deserialize, ToSchema)]
pub struct SelectMemoryRequest {
    #[serde(rename = "task_context")]
    pub task_context: TaskContext,
    #[serde(rename = "resource_constraints")]
    pub resource_constraints: ResourceConstraints,
    pub preferences: TaskPreferences,
    /// When true, response includes full decision trace (explainability).
    #[serde(default)]
    pub explain: Option<bool>,
    /// When true, no config/weight history/LTM is persisted; result and optional trace only.
    #[serde(default)]
    pub dry_run: Option<bool>,
    /// When true and a trace is returned, persist the trace to DB (ignored if dry_run).
    #[serde(default)]
    pub persist_trace: Option<bool>,
    /// If set, run the same pipeline with these hypothetical constraints; response includes what_if_result.
    #[serde(rename = "what_if_constraints", default)]
    pub what_if_constraints: Option<ResourceConstraints>,
}

#[derive(Serialize, ToSchema)]
pub struct SelectMemoryResponse {
    #[serde(rename = "memory_config")]
    pub memory_config: MemoryConfig,
    #[serde(rename = "performance_prediction")]
    pub performance_prediction: PerformancePrediction,
    #[serde(rename = "resource_requirements")]
    pub resource_requirements: ResourceRequirements,
    /// Present when request had explain or dry_run true.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace: Option<crate::services::scheduler::DecisionTrace>,
    /// Present when request had what_if_constraints; result under hypothetical constraints.
    #[serde(rename = "what_if_result", skip_serializing_if = "Option::is_none")]
    pub what_if_result: Option<crate::services::scheduler::MemorySelectionResult>,
}

pub async fn select_memory_config(
    Json(request): Json<SelectMemoryRequest>,
) -> JsonResult<SelectMemoryResponse> {
    let dry_run = request.dry_run == Some(true);
    let explain = request.explain == Some(true);

    if dry_run || explain {
        let trace = SCHEDULER
            .adaptive_memory_selection_trace(
                &request.task_context,
                &request.resource_constraints,
                &request.preferences,
            )
            .await?;
        let r = &trace.final_result;
        if !dry_run {
            let config_id = MemoryConfigRepository::create(
                &request.task_context.user_id,
                &request.task_context.agent_id,
                &format!("Config for task {}", trace.task_id),
                "optimized",
                &r.memory_config,
            )
            .await?;
            let _ = WeightHistoryRepository::create(
                &trace.task_id,
                &trace.initial_memory_config.memory_weights,
                &trace.weight_adjustment.adjusted_weights,
                &trace.weight_adjustment.adjustment_reasons,
                ((trace.cost_benefit_ratio - 1.0) * 0.1) as f32,
                None,
            )
            .await;
            tracing::info!(config_id = %config_id, task_id = %trace.task_id, "Persisted from trace (explain=true)");
        }
        if request.persist_trace == Some(true) && !dry_run {
            let trace_json = serde_json::to_string(&trace).map_err(|e| {
                crate::AppError::Internal(format!("Failed to serialize trace: {}", e))
            })?;
            let _ = DecisionTraceRepository::create(&trace.task_id, &trace_json).await;
        }
        let what_if_result = if let Some(ref w) = request.what_if_constraints {
            SCHEDULER
                .adaptive_memory_selection_trace(&request.task_context, w, &request.preferences)
                .await
                .ok()
                .map(|t| t.final_result)
        } else {
            None
        };
        return json_ok(SelectMemoryResponse {
            memory_config: r.memory_config.clone(),
            performance_prediction: r.performance_prediction.clone(),
            resource_requirements: r.resource_requirements.clone(),
            trace: Some(trace),
            what_if_result,
        });
    }

    let result = SCHEDULER
        .adaptive_memory_selection(
            &request.task_context,
            &request.resource_constraints,
            &request.preferences,
        )
        .await?;

    let what_if_result = if let Some(ref w) = request.what_if_constraints {
        SCHEDULER
            .adaptive_memory_selection_trace(&request.task_context, w, &request.preferences)
            .await
            .ok()
            .map(|t| t.final_result)
    } else {
        None
    };

    json_ok(SelectMemoryResponse {
        memory_config: result.memory_config,
        performance_prediction: result.performance_prediction,
        resource_requirements: result.resource_requirements,
        trace: None,
        what_if_result,
    })
}

pub async fn select_memory_config_trace(
    Json(request): Json<SelectMemoryRequest>,
) -> JsonResult<crate::services::scheduler::DecisionTrace> {
    let trace = SCHEDULER
        .adaptive_memory_selection_trace(
            &request.task_context,
            &request.resource_constraints,
            &request.preferences,
        )
        .await?;
    if request.persist_trace == Some(true) {
        let trace_json = serde_json::to_string(&trace)
            .map_err(|e| crate::AppError::Internal(format!("Failed to serialize trace: {}", e)))?;
        let _ = DecisionTraceRepository::create(&trace.task_id, &trace_json).await;
    }
    json_ok(trace)
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ListTracesQuery {
    pub task_id: Option<String>,
    pub limit: Option<i32>,
}

#[derive(Serialize, ToSchema)]
pub struct DecisionTraceItem {
    #[serde(rename = "trace_id")]
    pub trace_id: String,
    #[serde(rename = "task_id")]
    pub task_id: String,
    #[serde(rename = "created_at")]
    pub created_at: String,
    #[serde(rename = "trace")]
    pub trace: crate::services::scheduler::DecisionTrace,
}

#[derive(Serialize, ToSchema)]
pub struct ListTracesResponse {
    #[serde(rename = "traces")]
    pub traces: Vec<DecisionTraceItem>,
}

pub async fn get_decision_traces(
    Query(q): Query<ListTracesQuery>,
) -> JsonResult<ListTracesResponse> {
    let rows = if let Some(ref task_id) = q.task_id {
        DecisionTraceRepository::get_by_task_id(task_id, q.limit).await?
    } else {
        DecisionTraceRepository::get_recent(q.limit, None, None).await?
    };
    let mut traces = Vec::with_capacity(rows.len());
    for row in rows {
        let trace: crate::services::scheduler::DecisionTrace =
            serde_json::from_str(&row.trace_json).map_err(|e| {
                crate::AppError::Internal(format!("Failed to parse stored trace: {}", e))
            })?;
        traces.push(DecisionTraceItem {
            trace_id: row.trace_id,
            task_id: row.task_id,
            created_at: row.created_at,
            trace,
        });
    }
    json_ok(ListTracesResponse { traces })
}

#[derive(Serialize, ToSchema)]
pub struct MemoryStatusResponse {
    #[serde(rename = "current_config")]
    pub current_config: MemoryConfig,
    #[serde(rename = "performance_metrics")]
    pub performance_metrics: PerformanceMetrics,
    #[serde(rename = "resource_status")]
    pub resource_status: ResourceStatus,
}

pub async fn get_memory_status() -> JsonResult<MemoryStatusResponse> {
    // 尝试获取最新配置（使用默认用户和代理ID）
    let config_row = MemoryConfigRepository::get_latest("default_user", "default_agent")
        .await
        .unwrap_or(None);

    let (current_config, performance_metrics) = if let Some(row) = config_row {
        let config = MemoryConfigRepository::row_to_memory_config(&row);
        // 获取该配置的最新性能指标
        let metrics_rows = PerformanceMetricsRepository::get_by_config_and_time_range(
            &row.config_id,
            None,
            None,
            Some(1),
        )
        .await
        .unwrap_or_default();

        let metrics = if let Some(metric_row) = metrics_rows.first() {
            PerformanceMetricsRepository::row_to_performance_metrics(metric_row)
        } else {
            PerformanceMetrics {
                efficiency_score: 0.85,
                coherence_score: 0.92,
                response_time_ms: 850,
                memory_usage_mb: 256,
                cpu_usage_percent: 35,
            }
        };
        (config, metrics)
    } else {
        // 默认配置
        (
            MemoryConfig {
                primary_memory: MemoryType::Stm,
                secondary_memory: vec![MemoryType::Ltm],
                memory_weights: MemoryWeights {
                    stm: 1.0,
                    ltm: 0.6,
                    kg: 0.0,
                    mm: 0.0,
                },
                reasoning_depth: "medium".to_string(),
                enable_multimodal: false,
            },
            PerformanceMetrics {
                efficiency_score: 0.85,
                coherence_score: 0.92,
                response_time_ms: 850,
                memory_usage_mb: 256,
                cpu_usage_percent: 35,
            },
        )
    };

    let resource_status = MONITOR.get_current_status().await;

    json_ok(MemoryStatusResponse {
        current_config,
        performance_metrics,
        resource_status: resource_status.current_status,
    })
}

// ========== 任务特征分析器 API ==========

#[derive(Deserialize, ToSchema)]
pub struct AnalyzeTaskRequest {
    #[serde(rename = "task_context")]
    pub task_context: TaskContextInput,
}

#[derive(Serialize, ToSchema)]
pub struct AnalyzeTaskResponse {
    pub characteristics: TaskCharacteristics,
    #[serde(rename = "memory_strategy")]
    pub memory_strategy: MemoryStrategy,
    #[serde(rename = "confidence_score")]
    pub confidence_score: f64,
}

pub async fn analyze_task_characteristics(
    Json(request): Json<AnalyzeTaskRequest>,
) -> JsonResult<AnalyzeTaskResponse> {
    let (characteristics, memory_strategy, confidence_score) =
        ANALYZER.analyze_task_characteristics(&request.task_context);

    json_ok(AnalyzeTaskResponse {
        characteristics,
        memory_strategy,
        confidence_score,
    })
}

#[derive(Deserialize, ToSchema)]
pub struct BatchTask {
    #[serde(rename = "task_id")]
    pub task_id: String,
    #[serde(rename = "task_context")]
    pub task_context: TaskContextInput,
}

#[derive(Deserialize, ToSchema)]
pub struct BatchAnalyzeRequest {
    pub tasks: Vec<BatchTask>,
}

#[derive(Serialize, ToSchema)]
pub struct TaskResult {
    #[serde(rename = "task_id")]
    pub task_id: String,
    pub characteristics: TaskCharacteristics,
    #[serde(rename = "memory_strategy")]
    pub memory_strategy: MemoryStrategy,
}

#[derive(Serialize, ToSchema)]
pub struct BatchMetrics {
    #[serde(rename = "total_tasks")]
    pub total_tasks: usize,
    #[serde(rename = "processed_tasks")]
    pub processed_tasks: usize,
    #[serde(rename = "average_complexity")]
    pub average_complexity: f64,
    #[serde(rename = "processing_time_ms")]
    pub processing_time_ms: u64,
}

#[derive(Serialize, ToSchema)]
pub struct BatchAnalyzeResponse {
    pub results: Vec<TaskResult>,
    #[serde(rename = "batch_metrics")]
    pub batch_metrics: BatchMetrics,
}

pub async fn batch_analyze_characteristics(
    Json(request): Json<BatchAnalyzeRequest>,
) -> JsonResult<BatchAnalyzeResponse> {
    let mut results = Vec::new();
    let mut total_complexity = 0.0;

    for task in request.tasks {
        let (characteristics, memory_strategy, _) =
            ANALYZER.analyze_task_characteristics(&task.task_context);

        total_complexity += characteristics.complexity;

        results.push(TaskResult {
            task_id: task.task_id,
            characteristics,
            memory_strategy,
        });
    }

    let total_tasks = results.len();
    json_ok(BatchAnalyzeResponse {
        results,
        batch_metrics: BatchMetrics {
            total_tasks,
            processed_tasks: total_tasks,
            average_complexity: if total_tasks > 0 {
                total_complexity / total_tasks as f64
            } else {
                0.0
            },
            processing_time_ms: 150,
        },
    })
}

// ========== 性能预测模型 API ==========

#[derive(Deserialize, ToSchema)]
pub struct PredictPerformanceRequest {
    #[serde(rename = "task_profile")]
    pub task_profile: TaskCharacteristics,
    #[serde(rename = "memory_config")]
    pub memory_config: MemoryConfig,
}

#[derive(Serialize, ToSchema)]
pub struct PredictPerformanceResponse {
    #[serde(rename = "predicted_performance")]
    pub predicted_performance: PerformancePrediction,
    #[serde(rename = "synergy_factor")]
    pub synergy_factor: f64,
    #[serde(rename = "decay_factor")]
    pub decay_factor: f64,
    #[serde(rename = "performance_breakdown")]
    pub performance_breakdown: PerformanceBreakdown,
}

pub async fn predict_performance(
    Json(request): Json<PredictPerformanceRequest>,
) -> JsonResult<PredictPerformanceResponse> {
    let (predicted_performance, synergy_factor, decay_factor, performance_breakdown) =
        PREDICTOR.predict_memory_performance(&request.memory_config);

    json_ok(PredictPerformanceResponse {
        predicted_performance,
        synergy_factor,
        decay_factor,
        performance_breakdown,
    })
}

#[derive(Serialize, ToSchema)]
pub struct BaselinesResponse {
    #[serde(rename = "performance_baselines")]
    pub performance_baselines: PerformanceBaselines,
    #[serde(rename = "marginal_decay_factors")]
    pub marginal_decay_factors: MarginalDecayFactors,
}

pub async fn get_baselines() -> JsonResult<BaselinesResponse> {
    json_ok(BaselinesResponse {
        performance_baselines: PREDICTOR.get_baselines().clone(),
        marginal_decay_factors: PREDICTOR.get_marginal_decay_factors().clone(),
    })
}

// ========== 资源监控与优化器 API ==========

pub async fn get_resources() -> JsonResult<CurrentResourceStatus> {
    json_ok(MONITOR.get_current_status().await)
}

#[derive(Deserialize, ToSchema)]
pub struct CostBenefitRequest {
    #[serde(rename = "performance_prediction")]
    pub performance_prediction: PerformancePrediction,
    #[serde(rename = "resource_status")]
    pub resource_status: ResourceStatus,
}

#[derive(Serialize, ToSchema)]
pub struct CostBenefitResponse {
    #[serde(rename = "cost_benefit_ratio")]
    pub cost_benefit_ratio: f64,
    #[serde(rename = "performance_score")]
    pub performance_score: f64,
    #[serde(rename = "resource_cost")]
    pub resource_cost: f64,
    pub recommendation: String,
    #[serde(rename = "optimization_suggestions")]
    pub optimization_suggestions: Vec<String>,
}

pub async fn calculate_cost_benefit(
    Json(request): Json<CostBenefitRequest>,
) -> JsonResult<CostBenefitResponse> {
    let cost_benefit_ratio = MONITOR
        .calculate_cost_benefit_ratio(&request.performance_prediction, &request.resource_status);

    let performance_score = request.performance_prediction.efficiency_gain * 0.6
        + request.performance_prediction.coherence_gain * 0.4;

    let resource_cost = (request.resource_status.memory_usage_percent as f64 / 100.0) * 0.4
        + (request.resource_status.cpu_usage_percent as f64 / 100.0) * 0.4
        + (request.resource_status.response_time_ms as f64 / 2000.0) * 0.2;

    let recommendation = if cost_benefit_ratio > 1.5 {
        "optimal"
    } else if cost_benefit_ratio > 1.0 {
        "suboptimal"
    } else {
        "poor"
    };

    let mut suggestions = Vec::new();
    if cost_benefit_ratio < 1.0 {
        suggestions.push("Consider reducing LTM weight to improve cost-benefit ratio".to_string());
        suggestions.push("KG memory may provide better value for this task type".to_string());
    }

    json_ok(CostBenefitResponse {
        cost_benefit_ratio,
        performance_score,
        resource_cost,
        recommendation: recommendation.to_string(),
        optimization_suggestions: suggestions,
    })
}

#[derive(Deserialize, ToSchema)]
pub struct OptimizeRequest {
    #[serde(rename = "current_config")]
    pub current_config: MemoryConfig,
    #[serde(rename = "performance_goals")]
    pub performance_goals: PerformanceGoals,
}

pub async fn optimize(Json(request): Json<OptimizeRequest>) -> JsonResult<OptimizationResult> {
    let result = MONITOR.optimize_config(&request.current_config, &request.performance_goals);
    json_ok(result)
}

// ========== 动态权重调整器 API ==========

#[derive(Deserialize, ToSchema)]
pub struct AdjustWeightsRequest {
    #[serde(rename = "task_profile")]
    pub task_profile: TaskCharacteristics,
    #[serde(rename = "cost_benefit_ratio")]
    pub cost_benefit_ratio: f64,
    #[serde(rename = "current_weights")]
    pub current_weights: MemoryWeights,
}

#[derive(Serialize, ToSchema)]
pub struct AdjustWeightsResponse {
    #[serde(rename = "adjusted_weights")]
    pub adjusted_weights: MemoryWeights,
    #[serde(rename = "adjustment_reasons")]
    pub adjustment_reasons: crate::models::AdjustmentReasons,
    #[serde(rename = "confidence_score")]
    pub confidence_score: f64,
}

pub async fn adjust_weights(
    Json(request): Json<AdjustWeightsRequest>,
) -> JsonResult<AdjustWeightsResponse> {
    let (adjusted_weights, adjustment_reasons) = WEIGHT_ADJUSTER
        .adjust_memory_weights(
            &request.task_profile,
            request.cost_benefit_ratio,
            Some(&request.current_weights),
            None, // task_id can be added to request if needed
        )
        .await?;

    json_ok(AdjustWeightsResponse {
        adjusted_weights,
        adjustment_reasons,
        confidence_score: 0.88,
    })
}

#[derive(Serialize, ToSchema)]
pub struct HistoryItem {
    pub timestamp: String,
    #[serde(rename = "task_id")]
    pub task_id: String,
    #[serde(rename = "old_weights")]
    pub old_weights: MemoryWeights,
    #[serde(rename = "new_weights")]
    pub new_weights: MemoryWeights,
    pub reason: String,
    #[serde(rename = "performance_impact")]
    pub performance_impact: f64,
}

#[derive(Serialize, ToSchema)]
pub struct HistorySummary {
    #[serde(rename = "total_adjustments")]
    pub total_adjustments: usize,
    #[serde(rename = "average_performance_impact")]
    pub average_performance_impact: f64,
    #[serde(rename = "most_common_adjustment")]
    pub most_common_adjustment: String,
}

#[derive(Serialize, ToSchema)]
pub struct WeightHistoryResponse {
    #[serde(rename = "adjustment_history")]
    pub adjustment_history: Vec<HistoryItem>,
    pub summary: HistorySummary,
}

pub async fn get_weight_history() -> JsonResult<WeightHistoryResponse> {
    // 从数据库获取权重调整历史
    let history_rows = WeightHistoryRepository::get_by_time_range(None, None, Some(100))
        .await
        .unwrap_or_default();

    let adjustment_history: Vec<HistoryItem> = history_rows
        .iter()
        .filter_map(|row| {
            WeightHistoryRepository::row_to_history_item(row)
                .ok()
                .map(|item| HistoryItem {
                    timestamp: item.timestamp,
                    task_id: item.task_id,
                    old_weights: item.old_weights,
                    new_weights: item.new_weights,
                    reason: item.reason,
                    performance_impact: item.performance_impact as f64,
                })
        })
        .collect();

    let summary_row = WeightHistoryRepository::get_summary(None, None)
        .await
        .unwrap_or(crate::db::weights::WeightHistorySummary {
            total_adjustments: 0,
            average_performance_impact: 0.0,
            most_common_adjustment: "ltm_weight_increase".to_string(),
        });

    json_ok(WeightHistoryResponse {
        adjustment_history,
        summary: HistorySummary {
            total_adjustments: summary_row.total_adjustments,
            average_performance_impact: summary_row.average_performance_impact as f64,
            most_common_adjustment: summary_row.most_common_adjustment,
        },
    })
}

// ========== 系统管理 API ==========

#[derive(Serialize, ToSchema)]
pub struct ComponentStatus {
    pub scheduler: String,
    pub analyzer: String,
    pub predictor: String,
    pub monitor: String,
    #[serde(rename = "weight_adjuster")]
    pub weight_adjuster: String,
}

#[derive(Serialize, ToSchema)]
pub struct SystemPerformance {
    #[serde(rename = "avg_response_time_ms")]
    pub avg_response_time_ms: u64,
    #[serde(rename = "success_rate")]
    pub success_rate: f64,
    #[serde(rename = "error_rate")]
    pub error_rate: f64,
}

#[derive(Serialize, ToSchema)]
pub struct HealthResponse {
    pub status: String,
    pub timestamp: String,
    pub components: ComponentStatus,
    pub performance: SystemPerformance,
}

pub async fn health_check() -> JsonResult<HealthResponse> {
    use time::OffsetDateTime;

    json_ok(HealthResponse {
        status: "healthy".to_string(),
        timestamp: OffsetDateTime::now_utc().to_string(),
        components: ComponentStatus {
            scheduler: "healthy".to_string(),
            analyzer: "healthy".to_string(),
            predictor: "healthy".to_string(),
            monitor: "healthy".to_string(),
            weight_adjuster: "healthy".to_string(),
        },
        performance: SystemPerformance {
            avg_response_time_ms: 850,
            success_rate: 0.98,
            error_rate: 0.02,
        },
    })
}

#[derive(Serialize, ToSchema)]
pub struct ResourceLimitsConfig {
    #[serde(rename = "memory_usage")]
    pub memory_usage: f64,
    #[serde(rename = "cpu_usage")]
    pub cpu_usage: f64,
    #[serde(rename = "response_time")]
    pub response_time: f64,
    #[serde(rename = "storage_quota")]
    pub storage_quota: f64,
}

#[derive(Serialize, ToSchema)]
pub struct ConfigResponse {
    #[serde(rename = "resource_limits")]
    pub resource_limits: ResourceLimitsConfig,
    #[serde(rename = "performance_baselines")]
    pub performance_baselines: PerformanceBaselines,
    #[serde(rename = "marginal_decay_factors")]
    pub marginal_decay_factors: MarginalDecayFactors,
}

pub async fn get_config() -> JsonResult<ConfigResponse> {
    json_ok(ConfigResponse {
        resource_limits: ResourceLimitsConfig {
            memory_usage: 0.8,
            cpu_usage: 0.8,
            response_time: 2.0,
            storage_quota: 0.9,
        },
        performance_baselines: PREDICTOR.get_baselines().clone(),
        marginal_decay_factors: PREDICTOR.get_marginal_decay_factors().clone(),
    })
}

use crate::services::monitor::*;
use crate::services::weight_adjuster::*;

// ========== 记忆配置管理 API ==========

#[derive(Deserialize, ToSchema)]
pub struct ListMemoryConfigsRequest {
    pub page: Option<u32>,
    #[serde(rename = "pageSize")]
    pub page_size: Option<u32>,
    #[serde(rename = "userId")]
    pub user_id: Option<String>,
    #[serde(rename = "agentId")]
    pub agent_id: Option<String>,
    pub status: Option<String>,
    #[serde(rename = "configType")]
    pub config_type: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct ListMemoryConfigsResponse {
    pub data: Vec<crate::db::memory::MemoryConfigRow>,
    pub total: u64,
    pub page: u32,
    #[serde(rename = "pageSize")]
    pub page_size: u32,
}

#[derive(Deserialize, ToSchema)]
pub struct CreateMemoryConfigRequest {
    #[serde(rename = "userId")]
    pub user_id: String,
    #[serde(rename = "agentId")]
    pub agent_id: String,
    #[serde(rename = "configName")]
    pub config_name: String,
    #[serde(rename = "configType")]
    pub config_type: String,
    #[serde(rename = "stmEnabled")]
    pub stm_enabled: i32,
    #[serde(rename = "stmMaxLength")]
    pub stm_max_length: i32,
    #[serde(rename = "stmRetentionHours")]
    pub stm_retention_hours: i32,
    #[serde(rename = "ltmEnabled")]
    pub ltm_enabled: i32,
    #[serde(rename = "ltmMaxEntries")]
    pub ltm_max_entries: i32,
    #[serde(rename = "ltmQualityThreshold")]
    pub ltm_quality_threshold: f64,
    #[serde(rename = "kgEnabled")]
    pub kg_enabled: i32,
    #[serde(rename = "kgMaxEntities")]
    pub kg_max_entities: i32,
    #[serde(rename = "kgConfidenceThreshold")]
    pub kg_confidence_threshold: f64,
    #[serde(rename = "mmEnabled")]
    pub mm_enabled: i32,
    #[serde(rename = "mmMaxEntries")]
    pub mm_max_entries: i32,
    #[serde(rename = "mmModalityTypes")]
    pub mm_modality_types: Option<String>,
    #[serde(rename = "maxResponseTimeMs")]
    pub max_response_time_ms: i32,
    #[serde(rename = "maxMemoryUsageMb")]
    pub max_memory_usage_mb: i32,
    #[serde(rename = "maxCpuUsagePercent")]
    pub max_cpu_usage_percent: i32,
    pub status: String,
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateMemoryConfigRequest {
    #[serde(rename = "userId")]
    pub user_id: Option<String>,
    #[serde(rename = "agentId")]
    pub agent_id: Option<String>,
    #[serde(rename = "configName")]
    pub config_name: Option<String>,
    #[serde(rename = "configType")]
    pub config_type: Option<String>,
    #[serde(rename = "stmEnabled")]
    pub stm_enabled: Option<i32>,
    #[serde(rename = "stmMaxLength")]
    pub stm_max_length: Option<i32>,
    #[serde(rename = "stmRetentionHours")]
    pub stm_retention_hours: Option<i32>,
    #[serde(rename = "ltmEnabled")]
    pub ltm_enabled: Option<i32>,
    #[serde(rename = "ltmMaxEntries")]
    pub ltm_max_entries: Option<i32>,
    #[serde(rename = "ltmQualityThreshold")]
    pub ltm_quality_threshold: Option<f64>,
    #[serde(rename = "kgEnabled")]
    pub kg_enabled: Option<i32>,
    #[serde(rename = "kgMaxEntities")]
    pub kg_max_entities: Option<i32>,
    #[serde(rename = "kgConfidenceThreshold")]
    pub kg_confidence_threshold: Option<f64>,
    #[serde(rename = "mmEnabled")]
    pub mm_enabled: Option<i32>,
    #[serde(rename = "mmMaxEntries")]
    pub mm_max_entries: Option<i32>,
    #[serde(rename = "mmModalityTypes")]
    pub mm_modality_types: Option<String>,
    #[serde(rename = "maxResponseTimeMs")]
    pub max_response_time_ms: Option<i32>,
    #[serde(rename = "maxMemoryUsageMb")]
    pub max_memory_usage_mb: Option<i32>,
    #[serde(rename = "maxCpuUsagePercent")]
    pub max_cpu_usage_percent: Option<i32>,
    pub status: Option<String>,
}

pub async fn list_memory_configs(
    Query(query): Query<ListMemoryConfigsRequest>,
) -> JsonResult<ListMemoryConfigsResponse> {
    let page = query.page;
    let page_size = query.page_size;
    let user_id = query.user_id;
    let agent_id = query.agent_id;
    let status = query.status;
    let config_type = query.config_type;

    let (rows, total) = MemoryConfigRepository::list(
        page,
        page_size,
        user_id.as_deref(),
        agent_id.as_deref(),
        status.as_deref(),
        config_type.as_deref(),
    )
    .await?;

    json_ok(ListMemoryConfigsResponse {
        data: rows,
        total,
        page: page.unwrap_or(1),
        page_size: page_size.unwrap_or(20),
    })
}

pub async fn get_memory_config(
    Path(config_id): Path<String>,
) -> JsonResult<crate::db::memory::MemoryConfigRow> {
    let config = MemoryConfigRepository::get_by_id(&config_id)
        .await?
        .ok_or_else(|| crate::AppError::NotFound(format!("Config {} not found", config_id)))?;

    json_ok(config)
}

pub async fn create_memory_config(
    Json(req): Json<CreateMemoryConfigRequest>,
) -> JsonResult<serde_json::Value> {
    let config_id = ulid::Ulid::new().to_string();

    // 创建 MemoryConfigRow
    let row = crate::db::memory::MemoryConfigRow {
        config_id: config_id.clone(),
        user_id: req.user_id,
        agent_id: req.agent_id,
        config_name: req.config_name,
        config_type: req.config_type,
        stm_enabled: req.stm_enabled as i16,
        stm_max_length: req.stm_max_length,
        stm_retention_hours: req.stm_retention_hours,
        ltm_enabled: req.ltm_enabled as i16,
        ltm_max_entries: req.ltm_max_entries,
        ltm_quality_threshold: req.ltm_quality_threshold as f32,
        kg_enabled: req.kg_enabled as i16,
        kg_max_entities: req.kg_max_entities,
        kg_confidence_threshold: req.kg_confidence_threshold as f32,
        mm_enabled: req.mm_enabled as i16,
        mm_max_entries: req.mm_max_entries,
        mm_modality_types: req.mm_modality_types,
        max_response_time_ms: req.max_response_time_ms,
        max_memory_usage_mb: req.max_memory_usage_mb,
        max_cpu_usage_percent: req.max_cpu_usage_percent,
        created_at: time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_else(|_| String::from("")),
        updated_at: time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_else(|_| String::from("")),
        status: req.status,
    };

    // 使用 create 方法插入
    let pool = crate::db::pool();
    sqlx::query(
        r#"
        INSERT INTO memory_configurations (
            config_id, user_id, agent_id, config_name, config_type,
            stm_enabled, stm_max_length, stm_retention_hours,
            ltm_enabled, ltm_max_entries, ltm_quality_threshold,
            kg_enabled, kg_max_entities, kg_confidence_threshold,
            mm_enabled, mm_max_entries, mm_modality_types,
            max_response_time_ms, max_memory_usage_mb, max_cpu_usage_percent,
            status
        ) VALUES (
            $1, $2, $3, $4, $5,
            $6, $7, $8,
            $9, $10, $11,
            $12, $13, $14,
            $15, $16, $17,
            $18, $19, $20,
            $21
        )
        "#,
    )
    .bind(&row.config_id)
    .bind(&row.user_id)
    .bind(&row.agent_id)
    .bind(&row.config_name)
    .bind(&row.config_type)
    .bind(row.stm_enabled)
    .bind(row.stm_max_length)
    .bind(row.stm_retention_hours)
    .bind(row.ltm_enabled)
    .bind(row.ltm_max_entries)
    .bind(row.ltm_quality_threshold)
    .bind(row.kg_enabled)
    .bind(row.kg_max_entities)
    .bind(row.kg_confidence_threshold)
    .bind(row.mm_enabled)
    .bind(row.mm_max_entries)
    .bind(&row.mm_modality_types)
    .bind(row.max_response_time_ms)
    .bind(row.max_memory_usage_mb)
    .bind(row.max_cpu_usage_percent)
    .bind(&row.status)
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create memory configuration: {}", e);
        crate::AppError::Internal(format!("Database error: {}", e))
    })?;

    json_ok(serde_json::json!({ "config_id": config_id }))
}

pub async fn update_memory_config(
    Path(config_id): Path<String>,
    Json(update_req): Json<UpdateMemoryConfigRequest>,
) -> JsonResult<serde_json::Value> {
    // 获取现有配置
    let mut existing = MemoryConfigRepository::get_by_id(&config_id)
        .await?
        .ok_or_else(|| crate::AppError::NotFound(format!("Config {} not found", config_id)))?;

    // 更新字段
    if let Some(uid) = update_req.user_id {
        existing.user_id = uid;
    }
    if let Some(aid) = update_req.agent_id {
        existing.agent_id = aid;
    }
    if let Some(name) = update_req.config_name {
        existing.config_name = name;
    }
    if let Some(ct) = update_req.config_type {
        existing.config_type = ct;
    }
    if let Some(val) = update_req.stm_enabled {
        existing.stm_enabled = val as i16;
    }
    if let Some(val) = update_req.stm_max_length {
        existing.stm_max_length = val;
    }
    if let Some(val) = update_req.stm_retention_hours {
        existing.stm_retention_hours = val;
    }
    if let Some(val) = update_req.ltm_enabled {
        existing.ltm_enabled = val as i16;
    }
    if let Some(val) = update_req.ltm_max_entries {
        existing.ltm_max_entries = val;
    }
    if let Some(val) = update_req.ltm_quality_threshold {
        existing.ltm_quality_threshold = val as f32;
    }
    if let Some(val) = update_req.kg_enabled {
        existing.kg_enabled = val as i16;
    }
    if let Some(val) = update_req.kg_max_entities {
        existing.kg_max_entities = val;
    }
    if let Some(val) = update_req.kg_confidence_threshold {
        existing.kg_confidence_threshold = val as f32;
    }
    if let Some(val) = update_req.mm_enabled {
        existing.mm_enabled = val as i16;
    }
    if let Some(val) = update_req.mm_max_entries {
        existing.mm_max_entries = val;
    }
    if update_req.mm_modality_types.is_some() {
        existing.mm_modality_types = update_req.mm_modality_types;
    }
    if let Some(val) = update_req.max_response_time_ms {
        existing.max_response_time_ms = val;
    }
    if let Some(val) = update_req.max_memory_usage_mb {
        existing.max_memory_usage_mb = val;
    }
    if let Some(val) = update_req.max_cpu_usage_percent {
        existing.max_cpu_usage_percent = val;
    }
    if let Some(st) = update_req.status {
        existing.status = st;
    }

    MemoryConfigRepository::update(&config_id, &existing).await?;

    json_ok(serde_json::json!({ "success": true }))
}

pub async fn delete_memory_config(Path(config_id): Path<String>) -> JsonResult<serde_json::Value> {
    MemoryConfigRepository::delete(&config_id).await?;

    json_ok(serde_json::json!({ "success": true }))
}
