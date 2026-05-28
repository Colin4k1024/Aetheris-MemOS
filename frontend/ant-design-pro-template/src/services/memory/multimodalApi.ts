import { request } from '@umijs/max';

// ========== 多模态记忆 ==========

/** 存储多模态记忆 POST /api/mm/store */
export async function storeMm(
  body: API.StoreMmRequest,
  options?: { [key: string]: any },
) {
  return request<API.StoreMmResponse>('/api/mm/store', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    data: body,
    ...(options || {}),
  });
}

/** 获取多模态记忆列表 GET /api/mm/list */
export async function listMm(
  params?: { modality_type?: string; limit?: number; offset?: number },
  options?: { [key: string]: any },
) {
  return request<API.MMEntryListResponse>('/api/mm/list', {
    method: 'GET',
    params,
    ...(options || {}),
  });
}

/** 获取多模态记忆 GET /api/mm/entry/{entry_id} */
export async function getMm(
  entryId: string,
  options?: { [key: string]: any },
) {
  return request<API.GetMmResponse>(`/api/mm/entry/${entryId}`, {
    method: 'GET',
    ...(options || {}),
  });
}

/** 获取会话多模态记忆 GET /api/mm/session/{session_id} */
export async function getSessionMm(
  sessionId: string,
  params?: { limit?: number },
  options?: { [key: string]: any },
) {
  return request<API.GetSessionMmResponse>(`/api/mm/session/${sessionId}`, {
    method: 'GET',
    params,
    ...(options || {}),
  });
}

/** 按模态获取多模态记忆 GET /api/mm/modality/{modality_type} */
export async function getModalityMm(
  modalityType: string,
  params?: { limit?: number },
  options?: { [key: string]: any },
) {
  return request<API.GetModalityMmResponse>(`/api/mm/modality/${modalityType}`, {
    method: 'GET',
    params,
    ...(options || {}),
  });
}
