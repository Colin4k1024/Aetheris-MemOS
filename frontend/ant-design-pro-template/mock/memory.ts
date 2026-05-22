/**
 * Mock data for Memory System API endpoints.
 * Used during development when the real backend is not available.
 * Mock data is returned directly (no { data: ... } wrapper) to match
 * what the backend returns after axios unwraps the response.
 */
import type { Request, Response } from 'express';

const waitTime = (time: number = 300) =>
  new Promise((resolve) => setTimeout(resolve, time));

// ── Mock Data ────────────────────────────────────────────────────────────────

const mockMemoryStatus = {
  performance_metrics: {
    efficiency_score: 0.78 + Math.random() * 0.1,
    coherence_score: 1.32 + Math.random() * 0.2,
    response_time_ms: 45 + Math.floor(Math.random() * 20),
    cpu_usage_percent: 32 + Math.floor(Math.random() * 15),
  },
  resource_status: {
    memory_usage_mb: 512 + Math.floor(Math.random() * 128),
    memory_usage_percent: 42 + Math.floor(Math.random() * 10),
    cpu_usage_percent: 28 + Math.floor(Math.random() * 12),
    storage_usage_percent: 35 + Math.floor(Math.random() * 8),
    response_time_ms: 48 + Math.floor(Math.random() * 15),
    alerts: [] as string[],
  },
  current_config: {
    primary_memory: 'stm',
    secondary_memory: ['ltm', 'kg'],
    reasoning_depth: 'medium',
    enable_multimodal: false,
    memory_weights: {
      stm: 0.85,
      ltm: 0.60,
      kg: 0.40,
      mm: 0.00,
    },
  },
};

const mockHealth = {
  status: 'healthy',
  version: '0.1.0',
  uptime_seconds: 86400 + Math.floor(Math.random() * 3600),
  components: {
    database: 'healthy',
    cache: 'healthy',
    vector_db: 'healthy',
    llm: 'healthy',
    embedding: 'healthy',
  },
};

const mockBaselines = {
  performance_baselines: {
    stm: { efficiency_gain: 0.32, coherence_gain: 0.45 },
    ltm: { efficiency_gain: 0.28, coherence_gain: 0.62 },
    kg: { efficiency_gain: 0.21, coherence_gain: 0.78 },
    mm: { efficiency_gain: 0.18, coherence_gain: 0.55 },
  },
  marginal_decay_factors: {
    stm_to_ltm: 0.72,
    ltm_to_kg: 0.85,
    kg_to_mm: 0.91,
  },
};

const mockResources = {
  status: 'healthy',
  current_status: {
    memory_usage_mb: 512,
    memory_usage_percent: 42,
    cpu_usage_percent: 28,
    storage_usage_percent: 35,
    response_time_ms: 48,
  },
  resource_limits: {
    memory_limit_mb: 2048,
    cpu_limit_percent: 80,
    response_time_limit_ms: 2000,
    storage_limit_gb: 100,
  },
  alerts: [] as string[],
};

const mockWeightHistory = {
  summary: {
    total_adjustments: 24,
    average_performance_impact: 0.063,
    most_common_adjustment: 'ltm_increase',
  },
  adjustment_history: [
    {
      timestamp: new Date(Date.now() - 3600000 * 4).toISOString(),
      task_id: 'task_001',
      old_weights: { stm: 0.80, ltm: 0.50, kg: 0.30, mm: 0.00 },
      new_weights: { stm: 0.85, ltm: 0.60, kg: 0.40, mm: 0.00 },
      performance_impact: 0.082,
      reason: 'Increased LTM weight for better context retention',
    },
    {
      timestamp: new Date(Date.now() - 3600000 * 8).toISOString(),
      task_id: 'task_002',
      old_weights: { stm: 0.75, ltm: 0.45, kg: 0.25, mm: 0.10 },
      new_weights: { stm: 0.80, ltm: 0.50, kg: 0.30, mm: 0.00 },
      performance_impact: 0.055,
      reason: 'Balanced weights for mixed task types',
    },
    {
      timestamp: new Date(Date.now() - 3600000 * 12).toISOString(),
      task_id: 'task_003',
      old_weights: { stm: 0.70, ltm: 0.40, kg: 0.20, mm: 0.05 },
      new_weights: { stm: 0.75, ltm: 0.45, kg: 0.25, mm: 0.10 },
      performance_impact: 0.048,
      reason: 'Slight increase across all memory layers',
    },
  ],
};

const mockSelectMemory = {
  memory_config: {
    primary_memory: 'stm',
    secondary_memory: ['ltm', 'kg'],
    reasoning_depth: 'medium',
    enable_multimodal: false,
    memory_weights: { stm: 0.82, ltm: 0.58, kg: 0.38, mm: 0.00 },
  },
  performance_prediction: {
    efficiency_gain: 0.31,
    coherence_gain: 0.58,
    resource_cost: 0.52,
    cost_benefit_ratio: 1.71,
    confidence_score: 0.82,
  },
};

const mockDecisionTrace = {
  task_id: 'task_demo_001',
  timestamp: new Date().toISOString(),
  analyzer: {
    task_characteristics: {
      complexity: 0.62,
      modality_count: 2,
      temporal_scope: 'medium',
      reasoning_depth: 0.58,
    },
    memory_strategy: {
      primary_memory: 'stm',
      secondary_memory: ['ltm'],
      reasoning_depth: 'medium',
      confidence_score: 0.85,
    },
  },
  resource_status: mockResources,
  initial_memory_config: mockSelectMemory.memory_config,
  predictor: {
    performance_prediction: {
      efficiency_gain: 0.31,
      coherence_gain: 0.58,
      resource_cost: 0.52,
    },
    synergy_factor: 1.42,
    decay_factor: 0.78,
  },
  cost_benefit_ratio: 1.71,
  weight_adjustment: {
    adjusted_weights: { stm: 0.85, ltm: 0.60, kg: 0.40, mm: 0.00 },
    adjustment_reasons: {
      ltm: 'Context-dependent task benefits from increased LTM weight',
      kg: 'Structured knowledge improves coherence',
      mm: 'Multimodal disabled for text-only task',
    },
  },
  final_result: {
    resource_requirements: {
      estimated_memory_mb: 420,
      estimated_cpu_percent: 35,
      estimated_response_time_ms: 52,
    },
    performance_prediction: {
      efficiency_gain: 0.33,
      coherence_gain: 0.62,
      resource_cost: 0.55,
    },
  },
  memory_contributions: [
    { memory_type: 'stm', weight: 0.85, reason: 'Primary working memory for current context' },
    { memory_type: 'ltm', weight: 0.60, reason: 'Historical context retrieval' },
    { memory_type: 'kg', weight: 0.40, reason: 'Entity relationships and facts' },
    { memory_type: 'mm', weight: 0.00, reason: 'No multimodal content detected' },
  ],
};

const mockAnalyzeTask = {
  characteristics: {
    complexity: 0.58,
    reasoning_depth: 0.52,
    context_dependency: 0.65,
    modality_count: 2,
    temporal_scope: 'medium',
  },
  confidence_score: 0.82,
  memory_strategy: {
    primary_memory: 'stm',
    secondary_memory: ['ltm'],
    reasoning_depth: 'medium',
    enable_multimodal: false,
  },
};

const mockSessions = {
  sessions: [
    {
      session_id: 'sess_001',
      user_id: 'user_1',
      agent_id: 'agent_1',
      status: 'active',
      message_count: 12,
      created_at: new Date(Date.now() - 3600000).toISOString(),
      last_accessed_at: new Date(Date.now() - 300000).toISOString(),
    },
    {
      session_id: 'sess_002',
      user_id: 'user_1',
      agent_id: 'agent_1',
      status: 'active',
      message_count: 8,
      created_at: new Date(Date.now() - 7200000).toISOString(),
      last_accessed_at: new Date(Date.now() - 600000).toISOString(),
    },
  ],
  total: 2,
};

const mockLtmEntries = {
  entries: [
    {
      entry_id: 'ltm_001',
      title: 'Project Architecture Overview',
      content_type: 'document',
      source_type: 'task',
      quality_score: 0.85,
      category: 'architecture',
      created_at: new Date(Date.now() - 86400000).toISOString(),
    },
    {
      entry_id: 'ltm_002',
      title: 'API Design Guidelines',
      content_type: 'document',
      source_type: 'task',
      quality_score: 0.78,
      category: 'api',
      created_at: new Date(Date.now() - 172800000).toISOString(),
    },
  ],
  total: 2,
};

const mockEntities = {
  entities: [
    {
      entity_id: 'ent_001',
      entity_name: 'Memory System',
      entity_type: 'system',
      description: 'Adaptive memory management system for AI agents',
    },
    {
      entity_id: 'ent_002',
      entity_name: 'STM Layer',
      entity_type: 'component',
      description: 'Short-term memory layer for immediate context',
    },
  ],
  total: 2,
};

const mockMmEntries = {
  entries: [
    {
      entry_id: 'mm_001',
      title: 'System Diagram',
      modality_type: 'image',
      source_id: 'img_001',
      session_id: 'sess_001',
      description: 'Architecture diagram from project meeting',
    },
  ],
  total: 1,
};

const mockConfigList = {
  data: [
    {
      config_id: 'cfg_001',
      config_name: 'Default Config',
      config_type: 'default',
      status: 'active',
      user_id: 'user_1',
      agent_id: 'agent_1',
      stm_enabled: 1,
      stm_max_length: 4096,
      stm_retention_hours: 24,
      ltm_enabled: 1,
      ltm_max_entries: 10000,
      ltm_quality_threshold: 0.5,
      kg_enabled: 1,
      kg_max_entities: 1000,
      kg_confidence_threshold: 0.7,
      mm_enabled: 0,
      mm_max_entries: 0,
      mm_modality_types: null,
      max_response_time_ms: 2000,
      max_memory_usage_mb: 1024,
      max_cpu_usage_percent: 80,
      created_at: new Date(Date.now() - 604800000).toISOString(),
    },
  ],
  total: 1,
};

// ── Mock Handlers ──────────────────────────────────────────────────────────

export default {
  // Memory Status
  'GET /api/v1/memory/adaptive': async (_req: Request, res: Response) => {
    await waitTime();
    return res.json(mockMemoryStatus);
  },

  // Health Check
  'GET /api/v1/memory/health': async (_req: Request, res: Response) => {
    await waitTime();
    return res.json(mockHealth);
  },

  // Baselines
  'GET /api/v1/memory/predictor/baselines': async (_req: Request, res: Response) => {
    await waitTime();
    return res.json(mockBaselines);
  },

  // Resources
  'GET /api/v1/memory/monitor/resources': async (_req: Request, res: Response) => {
    await waitTime();
    return res.json(mockResources);
  },

  // Cost Benefit
  'POST /api/v1/memory/monitor/cost-benefit': async (_req: Request, res: Response) => {
    await waitTime();
    return res.json({
      cost_benefit_ratio: 1.72,
      performance_score: 0.82,
      resource_cost: 0.48,
      recommendation: 'optimal',
      optimization_suggestions: [
        'Consider increasing LTM weight for improved context retention',
        'Current KG threshold is optimal for this task type',
      ],
    });
  },

  // Optimize
  'POST /api/v1/memory/monitor/optimize': async (_req: Request, res: Response) => {
    await waitTime();
    return res.json({
      optimized_config: {
        primary_memory: 'stm',
        secondary_memory: ['ltm', 'kg'],
        memory_weights: { stm: 0.88, ltm: 0.62, kg: 0.42, mm: 0.00 },
        reasoning_depth: 'medium',
        enable_multimodal: false,
      },
      predicted_improvement: {
        efficiency_gain: 0.35,
        coherence_gain: 0.68,
        resource_cost_reduction: 0.08,
      },
      optimization_suggestions: [
        {
          description: 'Increase STM retention for better immediate recall',
          risk_level: 'low',
          expected_improvement: 0.05,
        },
        {
          description: 'Optimize LTM quality threshold',
          risk_level: 'medium',
          expected_improvement: 0.03,
        },
      ],
    });
  },

  // Select Memory Config
  'POST /api/v1/memory/adaptive': async (_req: Request, res: Response) => {
    await waitTime();
    return res.json(mockSelectMemory);
  },

  // Decision Trace
  'POST /api/v1/memory/adaptive/trace': async (_req: Request, res: Response) => {
    await waitTime();
    return res.json(mockDecisionTrace);
  },

  // Analyze Task
  'POST /api/v1/memory/analyzer/task-characteristics': async (_req: Request, res: Response) => {
    await waitTime();
    return res.json(mockAnalyzeTask);
  },

  // Predict Performance
  'POST /api/v1/memory/predictor/performance': async (_req: Request, res: Response) => {
    await waitTime();
    return res.json({
      synergy_factor: 1.38,
      decay_factor: 0.75,
      performance_breakdown: {
        stm_contribution: 0.42,
        ltm_contribution: 0.28,
        kg_contribution: 0.18,
        mm_contribution: 0.12,
      },
    });
  },

  // Weight History
  'GET /api/v1/memory/weights/history': async (_req: Request, res: Response) => {
    await waitTime();
    return res.json(mockWeightHistory);
  },

  // Sessions (STM)
  'GET /api/v1/memory/storage/sessions': async (_req: Request, res: Response) => {
    await waitTime();
    return res.json(mockSessions);
  },

  // LTM Entries
  'GET /api/v1/memory/search/ltm': async (_req: Request, res: Response) => {
    await waitTime();
    return res.json(mockLtmEntries);
  },

  // KG Entities
  'GET /api/kg/entities': async (_req: Request, res: Response) => {
    await waitTime();
    return res.json(mockEntities);
  },

  // MM Entries
  'GET /api/mm/list': async (_req: Request, res: Response) => {
    await waitTime();
    return res.json(mockMmEntries);
  },

  // Memory Configs List
  'GET /api/v1/memory/configs': async (_req: Request, res: Response) => {
    await waitTime();
    return res.json(mockConfigList);
  },

  // Memory Config Create
  'POST /api/v1/memory/configs': async (_req: Request, res: Response) => {
    await waitTime();
    return res.json({ config_id: `cfg_${Date.now()}`, success: true });
  },

  // Memory Config Update
  'PUT /api/v1/memory/configs/:config_id': async (_req: Request, res: Response) => {
    await waitTime();
    return res.json({ success: true });
  },

  // Memory Config Delete
  'DELETE /api/v1/memory/configs/:config_id': async (_req: Request, res: Response) => {
    await waitTime();
    return res.json({ success: true });
  },
};
