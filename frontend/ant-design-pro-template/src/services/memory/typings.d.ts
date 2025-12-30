declare namespace API {
  // ========== 自适应记忆调度器 ==========
  type SelectMemoryRequest = {
    task_context: TaskContext;
    resource_constraints: ResourceConstraints;
    preferences: TaskPreferences;
  };

  type SelectMemoryResponse = {
    memory_config: MemoryConfig;
    performance_prediction: PerformancePrediction;
    resource_requirements: ResourceRequirements;
  };

  type MemoryStatusResponse = {
    current_config: MemoryConfig;
    performance_metrics: PerformanceMetrics;
    resource_status: ResourceStatus;
  };

  // ========== 任务特征分析器 ==========
  type AnalyzeTaskRequest = {
    task_context: TaskContextInput;
  };

  type AnalyzeTaskResponse = {
    characteristics: TaskCharacteristics;
    memory_strategy: MemoryStrategy;
    confidence_score: number;
  };

  type BatchAnalyzeRequest = {
    tasks: BatchTask[];
  };

  type BatchTask = {
    task_id: string;
    task_context: TaskContextInput;
  };

  type BatchAnalyzeResponse = {
    results: TaskResult[];
    batch_metrics: BatchMetrics;
  };

  type TaskResult = {
    task_id: string;
    characteristics: TaskCharacteristics;
    memory_strategy: MemoryStrategy;
  };

  type BatchMetrics = {
    total_tasks: number;
    processed_tasks: number;
    average_complexity: number;
    processing_time_ms: number;
  };

  // ========== 性能预测模型 ==========
  type PredictPerformanceRequest = {
    task_profile: TaskCharacteristics;
    memory_config: MemoryConfig;
  };

  type PredictPerformanceResponse = {
    predicted_performance: PerformancePrediction;
    synergy_factor: number;
    decay_factor: number;
    performance_breakdown: PerformanceBreakdown;
  };

  type BaselinesResponse = {
    performance_baselines: PerformanceBaselines;
    marginal_decay_factors: MarginalDecayFactors;
  };

  // ========== 资源监控 ==========
  type CostBenefitRequest = {
    performance_prediction: PerformancePrediction;
    resource_status: ResourceStatus;
  };

  type CostBenefitResponse = {
    cost_benefit_ratio: number;
    performance_score: number;
    resource_cost: number;
    recommendation: string;
    optimization_suggestions: string[];
  };

  type OptimizeRequest = {
    current_config: MemoryConfig;
    performance_goals: PerformanceGoals;
  };

  type PerformanceGoals = {
    target_efficiency: number;
    target_coherence: number;
    max_resource_cost: number;
  };

  // ========== 权重调整 ==========
  type AdjustWeightsRequest = {
    task_profile: TaskCharacteristics;
    cost_benefit_ratio: number;
    current_weights: MemoryWeights;
  };

  type AdjustWeightsResponse = {
    adjusted_weights: MemoryWeights;
    adjustment_reasons: AdjustmentReasons;
    confidence_score: number;
  };

  type WeightHistoryResponse = {
    adjustment_history: HistoryItem[];
    summary: HistorySummary;
  };

  type HistoryItem = {
    timestamp: string;
    task_id: string;
    old_weights: MemoryWeights;
    new_weights: MemoryWeights;
    reason: string;
    performance_impact: number;
  };

  type HistorySummary = {
    total_adjustments: number;
    average_performance_impact: number;
    most_common_adjustment: string;
  };

  // ========== 系统管理 ==========
  type HealthResponse = {
    status: string;
    timestamp: string;
    components: ComponentStatus;
    performance: SystemPerformance;
  };

  type ComponentStatus = {
    scheduler: string;
    analyzer: string;
    predictor: string;
    monitor: string;
    weight_adjuster: string;
  };

  type SystemPerformance = {
    avg_response_time_ms: number;
    success_rate: number;
    error_rate: number;
  };

  type ConfigResponse = {
    resource_limits: ResourceLimitsConfig;
    performance_baselines: PerformanceBaselines;
    marginal_decay_factors: MarginalDecayFactors;
  };

  type ResourceLimitsConfig = {
    memory_usage: number;
    cpu_usage: number;
    response_time: number;
    storage_quota: number;
  };

  // ========== 数据模型 ==========
  type TaskContext = {
    task_id: string;
    task_type: 'conversation' | 'task' | 'query';
    complexity: number;
    modality_requirements: Modality[];
    temporal_scope: 'short' | 'medium' | 'long';
    reasoning_depth: 'shallow' | 'medium' | 'deep';
    context_dependency: number;
    user_id: string;
    agent_id: string;
  };

  type Modality = 'text' | 'image' | 'audio' | 'video';

  type ResourceConstraints = {
    max_memory_usage_mb: number;
    max_cpu_usage_percent: number;
    max_response_time_ms: number;
    storage_quota_percent: number;
  };

  type TaskPreferences = {
    prioritize_efficiency: boolean;
    prioritize_coherence: boolean;
    enable_multimodal: boolean;
    enable_reasoning: boolean;
  };

  type TaskContextInput = {
    content: string;
    modality: string[];
    context_history: ContextHistoryItem[];
    task_metadata?: TaskMetadata;
  };

  type ContextHistoryItem = {
    role: string;
    content: string;
    timestamp: string;
  };

  type TaskMetadata = {
    domain?: string;
    complexity_hint?: string;
    expected_duration?: string;
  };

  type TaskCharacteristics = {
    complexity: number;
    modality_count: number;
    temporal_scope: string;
    reasoning_depth: number;
    context_dependency: number;
  };

  type MemoryConfig = {
    primary_memory: 'stm' | 'ltm' | 'kg' | 'mm';
    secondary_memory: ('stm' | 'ltm' | 'kg' | 'mm')[];
    memory_weights: MemoryWeights;
    reasoning_depth: string;
    enable_multimodal: boolean;
  };

  type MemoryWeights = {
    stm: number;
    ltm: number;
    kg: number;
    mm: number;
  };

  type MemoryStrategy = {
    primary_memory: string;
    secondary_memory: string[];
    enable_multimodal: boolean;
    reasoning_depth: string;
  };

  type PerformancePrediction = {
    efficiency_gain: number;
    coherence_gain: number;
    resource_cost: number;
    cost_benefit_ratio?: number;
    confidence_score?: number;
  };

  type PerformanceBaselines = {
    stm: PerformanceBaseline;
    ltm: PerformanceBaseline;
    kg: PerformanceBaseline;
    mm: PerformanceBaseline;
  };

  type PerformanceBaseline = {
    efficiency_gain: number;
    coherence_gain: number;
    resource_cost: number;
  };

  type MarginalDecayFactors = {
    stm_to_ltm: number;
    ltm_to_kg: number;
    kg_to_mm: number;
  };

  type PerformanceBreakdown = {
    stm_contribution: number;
    ltm_contribution: number;
    kg_contribution: number;
    mm_contribution: number;
  };

  type PerformanceMetrics = {
    efficiency_score: number;
    coherence_score: number;
    response_time_ms: number;
    memory_usage_mb: number;
    cpu_usage_percent: number;
  };

  type ResourceStatus = {
    memory_usage_mb: number;
    memory_usage_percent: number;
    cpu_usage_percent: number;
    response_time_ms: number;
    storage_usage_percent: number;
  };

  type CurrentResourceStatus = {
    current_status: ResourceStatus;
    resource_limits: ResourceLimits;
    status: string;
    alerts: string[];
  };

  type ResourceLimits = {
    memory_limit_mb: number;
    cpu_limit_percent: number;
    response_time_limit_ms: number;
    storage_limit_percent: number;
  };

  type ResourceRequirements = {
    estimated_memory_mb: number;
    estimated_cpu_percent: number;
    estimated_response_time_ms: number;
  };

  type AdjustmentReasons = {
    stm: string;
    ltm: string;
    kg: string;
    mm: string;
  };

  type OptimizationResult = {
    optimization_suggestions: OptimizationSuggestion[];
    optimized_config: MemoryConfig;
    predicted_improvement: PredictedImprovement;
  };

  type OptimizationSuggestion = {
    type: string;
    description: string;
    expected_improvement: number;
    risk_level: string;
  };

  type PredictedImprovement = {
    efficiency_gain: number;
    coherence_gain: number;
    resource_cost_reduction: number;
  };

  // ========== 记忆配置管理 ==========
  type MemoryConfigRow = {
    config_id: string;
    user_id: string;
    agent_id: string;
    config_name: string;
    config_type: string;
    stm_enabled: number;
    stm_max_length: number;
    stm_retention_hours: number;
    ltm_enabled: number;
    ltm_max_entries: number;
    ltm_quality_threshold: number;
    kg_enabled: number;
    kg_max_entities: number;
    kg_confidence_threshold: number;
    mm_enabled: number;
    mm_max_entries: number;
    mm_modality_types?: string;
    max_response_time_ms: number;
    max_memory_usage_mb: number;
    max_cpu_usage_percent: number;
    created_at: string;
    updated_at: string;
    status: string;
  };

  type ListMemoryConfigsParams = {
    page?: number;
    pageSize?: number;
    userId?: string;
    agentId?: string;
    status?: string;
    configType?: string;
  };

  type ListMemoryConfigsResponse = {
    data: MemoryConfigRow[];
    total: number;
    page: number;
    pageSize: number;
  };

  type CreateMemoryConfigRequest = {
    userId: string;
    agentId: string;
    configName: string;
    configType: string;
    stmEnabled: number;
    stmMaxLength: number;
    stmRetentionHours: number;
    ltmEnabled: number;
    ltmMaxEntries: number;
    ltmQualityThreshold: number;
    kgEnabled: number;
    kgMaxEntities: number;
    kgConfidenceThreshold: number;
    mmEnabled: number;
    mmMaxEntries: number;
    mmModalityTypes?: string;
    maxResponseTimeMs: number;
    maxMemoryUsageMb: number;
    maxCpuUsagePercent: number;
    status: string;
  };

  type UpdateMemoryConfigRequest = {
    userId?: string;
    agentId?: string;
    configName?: string;
    configType?: string;
    stmEnabled?: number;
    stmMaxLength?: number;
    stmRetentionHours?: number;
    ltmEnabled?: number;
    ltmMaxEntries?: number;
    ltmQualityThreshold?: number;
    kgEnabled?: number;
    kgMaxEntities?: number;
    kgConfidenceThreshold?: number;
    mmEnabled?: number;
    mmMaxEntries?: number;
    mmModalityTypes?: string;
    maxResponseTimeMs?: number;
    maxMemoryUsageMb?: number;
    maxCpuUsagePercent?: number;
    status?: string;
  };
}

