// @ts-ignore
/* eslint-disable */
import { request } from '@umijs/max';

// ========== 知识图谱实体 ==========

/** 创建实体 POST /api/kg/entities */
export async function createEntity(
  body: API.CreateEntityRequest,
  options?: { [key: string]: any },
) {
  return request<API.CreateEntityResponse>('/api/kg/entities', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    data: body,
    ...(options || {}),
  });
}

/** 按名称获取实体 GET /api/kg/entities/by-name/{name} */
export async function getEntityByName(
  name: string,
  options?: { [key: string]: any },
) {
  return request<API.GetEntityByNameResponse>(`/api/kg/entities/by-name/${name}`, {
    method: 'GET',
    ...(options || {}),
  });
}

/** 获取相关实体 GET /api/kg/entities/{entity_id}/related */
export async function getRelatedEntities(
  entityId: string,
  params?: { relationType?: string; limit?: number },
  options?: { [key: string]: any },
) {
  return request<API.GetRelatedEntitiesResponse>(`/api/kg/entities/${entityId}/related`, {
    method: 'GET',
    params,
    ...(options || {}),
  });
}

// ========== 知识图谱关系 ==========

/** 创建关系 POST /api/kg/relations */
export async function createRelation(
  body: API.CreateRelationRequest,
  options?: { [key: string]: any },
) {
  return request<API.CreateRelationResponse>('/api/kg/relations', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    data: body,
    ...(options || {}),
  });
}

// ========== 知识图谱搜索 ==========

/** 搜索知识 POST /api/kg/search */
export async function searchKnowledge(
  body: API.SearchKnowledgeRequest,
  options?: { [key: string]: any },
) {
  return request<API.SearchKnowledgeResponse>('/api/kg/search', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    data: body,
    ...(options || {}),
  });
}
