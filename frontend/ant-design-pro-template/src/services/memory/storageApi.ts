import { request } from '@umijs/max';

// ========== 决策追踪历史 ==========

/** 获取决策追踪列表 GET /api/v1/memory/traces */
export async function getDecisionTraces(
  params?: { page?: number; pageSize?: number; taskId?: string },
  options?: { [key: string]: any },
) {
  return request<{ data: API.DecisionTraceItem[]; total: number }>('/api/v1/memory/traces', {
    method: 'GET',
    params,
    ...(options || {}),
  });
}

// ========== 记忆存储 (STM/LTM) ==========

/** 存储短期记忆 POST /api/v1/memory/storage/stm */
export async function storeStm(
  body: API.StoreStmRequest,
  options?: { [key: string]: any },
) {
  return request<API.StoreStmResponse>('/api/v1/memory/storage/stm', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    data: body,
    ...(options || {}),
  });
}

/** 获取会话消息 GET /api/v1/memory/storage/stm/{session_id} */
export async function getSessionMessages(
  sessionId: string,
  params?: { limit?: number },
  options?: { [key: string]: any },
) {
  return request<API.SessionMessagesResponse>(`/api/v1/memory/storage/stm/${sessionId}`, {
    method: 'GET',
    params,
    ...(options || {}),
  });
}

/** 存储长期记忆 POST /api/v1/memory/storage/ltm */
export async function storeLtm(
  body: API.StoreLtmRequest,
  options?: { [key: string]: any },
) {
  return request<API.StoreLtmResponse>('/api/v1/memory/storage/ltm', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    data: body,
    ...(options || {}),
  });
}

/** 手动转移 STM 到 LTM POST /api/v1/memory/storage/transfer */
export async function transferStmToLtm(
  body: API.TransferRequest,
  options?: { [key: string]: any },
) {
  return request<API.TransferResponse>('/api/v1/memory/storage/transfer', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    data: body,
    ...(options || {}),
  });
}

/** 批量存储长期记忆 POST /api/v1/memory/storage/batch-ltm */
export async function batchStoreLtm(
  body: API.BatchStoreLtmRequest,
  options?: { [key: string]: any },
) {
  return request<API.BatchStoreLtmResponse>('/api/v1/memory/storage/batch-ltm', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    data: body,
    ...(options || {}),
  });
}

// ========== 记忆搜索 ==========

/** 搜索短期记忆 POST /api/v1/memory/search/stm */
export async function searchStm(
  body: API.SearchStmRequest,
  options?: { [key: string]: any },
) {
  return request<API.SearchStmResponse>('/api/v1/memory/search/stm', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    data: body,
    ...(options || {}),
  });
}

/** 搜索长期记忆 POST /api/v1/memory/search/ltm */
export async function searchLtm(
  body: API.SearchLtmRequest,
  options?: { [key: string]: any },
) {
  return request<API.SearchLtmResponse>('/api/v1/memory/search/ltm', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    data: body,
    ...(options || {}),
  });
}

/** 获取 LTM 条目详情 GET /api/v1/memory/search/ltm/{entry_id} */
export async function getLtmEntry(
  entryId: string,
  options?: { [key: string]: any },
) {
  return request<API.GetLtmEntryResponse>(`/api/v1/memory/search/ltm/${entryId}`, {
    method: 'GET',
    ...(options || {}),
  });
}

/** 混合搜索 POST /api/v1/memory/search/hybrid */
export async function hybridSearch(
  body: API.HybridSearchRequest,
  options?: { [key: string]: any },
) {
  return request<API.HybridSearchResponse>('/api/v1/memory/search/hybrid', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    data: body,
    ...(options || {}),
  });
}

/** 基于实体搜索 POST /api/v1/memory/search/entity */
export async function searchByEntity(
  body: API.SearchByEntityRequest,
  options?: { [key: string]: any },
) {
  return request<API.SearchByEntityResponse>('/api/v1/memory/search/entity', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    data: body,
    ...(options || {}),
  });
}

// ========== 记忆列表 ==========

/** 获取会话列表 GET /api/v1/memory/storage/sessions */
export async function listSessions(
  params?: { user_id?: string; status?: string; limit?: number; offset?: number },
  options?: { [key: string]: any },
) {
  return request<API.SessionListResponse>('/api/v1/memory/storage/sessions', {
    method: 'GET',
    params,
    ...(options || {}),
  });
}

/** 获取LTM条目列表 GET /api/v1/memory/search/ltm */
export async function listLtmEntries(
  params?: { category?: string; status?: string; limit?: number; offset?: number },
  options?: { [key: string]: any },
) {
  return request<API.LtmEntryListResponse>('/api/v1/memory/search/ltm', {
    method: 'GET',
    params,
    ...(options || {}),
  });
}
