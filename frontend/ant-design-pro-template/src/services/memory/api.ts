// @ts-ignore
/* eslint-disable */
import { request } from '@umijs/max';

/** 自适应记忆选择 POST /api/v1/memory/adaptive */
export async function selectMemoryConfig(
  body: API.SelectMemoryRequest,
  options?: { [key: string]: any },
) {
  return request<API.SelectMemoryResponse>('/api/v1/memory/adaptive', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    data: body,
    ...(options || {}),
  });
}

/** 获取记忆状态 GET /api/v1/memory/adaptive */
export async function getMemoryStatus(options?: { [key: string]: any }) {
  return request<API.MemoryStatusResponse>('/api/v1/memory/adaptive', {
    method: 'GET',
    ...(options || {}),
  });
}

/** 决策链路追踪（完整 pipeline，不落库）POST /api/v1/memory/adaptive/trace */
export async function getDecisionTrace(
  body: API.SelectMemoryRequest,
  options?: { [key: string]: any },
) {
  return request<API.DecisionTrace>('/api/v1/memory/adaptive/trace', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    data: body,
    ...(options || {}),
  });
}

/** 分析任务特征 POST /api/v1/memory/analyzer/task-characteristics */
export async function analyzeTaskCharacteristics(
  body: API.AnalyzeTaskRequest,
  options?: { [key: string]: any },
) {
  return request<API.AnalyzeTaskResponse>('/api/v1/memory/analyzer/task-characteristics', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    data: body,
    ...(options || {}),
  });
}

/** 批量分析任务特征 POST /api/v1/memory/analyzer/batch-characteristics */
export async function batchAnalyzeCharacteristics(
  body: API.BatchAnalyzeRequest,
  options?: { [key: string]: any },
) {
  return request<API.BatchAnalyzeResponse>('/api/v1/memory/analyzer/batch-characteristics', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    data: body,
    ...(options || {}),
  });
}

/** 预测性能 POST /api/v1/memory/predictor/performance */
export async function predictPerformance(
  body: API.PredictPerformanceRequest,
  options?: { [key: string]: any },
) {
  return request<API.PredictPerformanceResponse>('/api/v1/memory/predictor/performance', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    data: body,
    ...(options || {}),
  });
}

/** 获取性能基准 GET /api/v1/memory/predictor/baselines */
export async function getBaselines(options?: { [key: string]: any }) {
  return request<API.BaselinesResponse>('/api/v1/memory/predictor/baselines', {
    method: 'GET',
    ...(options || {}),
  });
}

/** 获取资源状态 GET /api/v1/memory/monitor/resources */
export async function getResources(options?: { [key: string]: any }) {
  return request<API.CurrentResourceStatus>('/api/v1/memory/monitor/resources', {
    method: 'GET',
    ...(options || {}),
  });
}

/** 计算成本效益比 POST /api/v1/memory/monitor/cost-benefit */
export async function calculateCostBenefit(
  body: API.CostBenefitRequest,
  options?: { [key: string]: any },
) {
  return request<API.CostBenefitResponse>('/api/v1/memory/monitor/cost-benefit', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    data: body,
    ...(options || {}),
  });
}

/** 资源优化 POST /api/v1/memory/monitor/optimize */
export async function optimize(
  body: API.OptimizeRequest,
  options?: { [key: string]: any },
) {
  return request<API.OptimizationResult>('/api/v1/memory/monitor/optimize', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    data: body,
    ...(options || {}),
  });
}

/** 调整权重 POST /api/v1/memory/weights/adjust */
export async function adjustWeights(
  body: API.AdjustWeightsRequest,
  options?: { [key: string]: any },
) {
  return request<API.AdjustWeightsResponse>('/api/v1/memory/weights/adjust', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    data: body,
    ...(options || {}),
  });
}

/** 获取权重历史 GET /api/v1/memory/weights/history */
export async function getWeightHistory(options?: { [key: string]: any }) {
  return request<API.WeightHistoryResponse>('/api/v1/memory/weights/history', {
    method: 'GET',
    ...(options || {}),
  });
}

/** 健康检查 GET /api/v1/memory/health */
export async function healthCheck(options?: { [key: string]: any }) {
  return request<API.HealthResponse>('/api/v1/memory/health', {
    method: 'GET',
    ...(options || {}),
  });
}

/** 获取配置 GET /api/v1/memory/config */
export async function getConfig(options?: { [key: string]: any }) {
  return request<API.ConfigResponse>('/api/v1/memory/config', {
    method: 'GET',
    ...(options || {}),
  });
}

/** 获取记忆配置列表 GET /api/v1/memory/configs */
export async function listMemoryConfigs(
  params: API.ListMemoryConfigsParams,
  options?: { [key: string]: any },
) {
  return request<API.ListMemoryConfigsResponse>('/api/v1/memory/configs', {
    method: 'GET',
    params,
    ...(options || {}),
  });
}

/** 获取记忆配置详情 GET /api/v1/memory/configs/{config_id} */
export async function getMemoryConfig(
  configId: string,
  options?: { [key: string]: any },
) {
  return request<API.MemoryConfigRow>(`/api/v1/memory/configs/${configId}`, {
    method: 'GET',
    ...(options || {}),
  });
}

/** 创建记忆配置 POST /api/v1/memory/configs */
export async function createMemoryConfig(
  body: API.CreateMemoryConfigRequest,
  options?: { [key: string]: any },
) {
  return request<{ config_id: string }>('/api/v1/memory/configs', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    data: body,
    ...(options || {}),
  });
}

/** 更新记忆配置 PUT /api/v1/memory/configs/{config_id} */
export async function updateMemoryConfig(
  configId: string,
  body: API.UpdateMemoryConfigRequest,
  options?: { [key: string]: any },
) {
  return request<{ success: boolean }>(`/api/v1/memory/configs/${configId}`, {
    method: 'PUT',
    headers: {
      'Content-Type': 'application/json',
    },
    data: body,
    ...(options || {}),
  });
}

/** 删除记忆配置 DELETE /api/v1/memory/configs/{config_id} */
export async function deleteMemoryConfig(
  configId: string,
  options?: { [key: string]: any },
) {
  return request<{ success: boolean }>(`/api/v1/memory/configs/${configId}`, {
    method: 'DELETE',
    ...(options || {}),
  });
}

