import { request } from '@umijs/max';

/**
 * Memory Governance API (#130) — belief lifecycle administration surface.
 *
 * Backend contract (backend/src/routers/memory_governance.rs):
 *   Base path: /api/v1/governance
 *   Reads are member-scoped (non-admin callers are pinned to their own
 *   subject server-side); mutations require the Admin/Owner role.
 */

export interface GovernanceBelief {
  id: string;
  tenant_id: string;
  principal_id: string;
  subject: string;
  predicate: string;
  object: string;
  status: string;
  source: string;
  trust: number;
  risk: string;
  valid_from: string;
  valid_to: string | null;
  recorded_at: string;
  supersedes_id: string | null;
  superseded_by_id: string | null;
  needs_confirm: boolean;
  metadata_json: string;
  single_valued: boolean;
  last_confirmed_at: string;
}

export interface GovernanceEvidence {
  id: string;
  belief_id: string | null;
  candidate_id: string | null;
  event_id: string | null;
  kind: string;
  content_hash: string;
}

export interface GovernanceAuditRow {
  event_id: string;
  event_type: string;
  actor_id: string | null;
  resource_id: string | null;
  correlation_id: string | null;
  metadata_json: string;
  created_at: string | null;
}

export interface GovernanceCandidate {
  id: string;
  subject: string;
  predicate: string;
  object: string;
  source: string;
  status: string;
  decision: string | null;
  rejection_reason: string | null;
  created_at: string;
}

export interface GovernanceStats {
  active_beliefs: number;
  pending_confirm: number;
  quarantined: number;
}

export interface MutationResult {
  ok: boolean;
  belief_id: string;
  detail: string | null;
}

export interface RollbackResult {
  ok: boolean;
  closed_belief_id: string;
  restored_belief_id: string;
}

/** List beliefs (non-admin callers only ever see their own subject). */
export async function listBeliefs(
  params?: { subject?: string; predicate?: string; include_history?: boolean; limit?: number },
  options?: { [key: string]: any },
) {
  return request<{ beliefs: GovernanceBelief[] }>('/api/v1/governance/beliefs', {
    method: 'GET',
    params,
    ...(options || {}),
  });
}

/** Full traceability: belief + provenance evidence + audit chain. */
export async function getBeliefTrace(id: string, options?: { [key: string]: any }) {
  return request<{
    belief: GovernanceBelief;
    evidence: GovernanceEvidence[];
    audit: GovernanceAuditRow[];
  }>(`/api/v1/governance/beliefs/${id}/trace`, { method: 'GET', ...(options || {}) });
}

/** The confirmation / quarantine queues (admin-only). */
export async function listCandidates(
  params?: { status?: 'pending' | 'quarantined'; limit?: number },
  options?: { [key: string]: any },
) {
  return request<{ candidates: GovernanceCandidate[] }>('/api/v1/governance/candidates', {
    method: 'GET',
    params,
    ...(options || {}),
  });
}

export async function confirmBelief(id: string, options?: { [key: string]: any }) {
  return request<MutationResult>(`/api/v1/governance/beliefs/${id}/confirm`, {
    method: 'POST',
    ...(options || {}),
  });
}

export async function denyBelief(id: string, options?: { [key: string]: any }) {
  return request<MutationResult>(`/api/v1/governance/beliefs/${id}/deny`, {
    method: 'POST',
    ...(options || {}),
  });
}

export async function archiveBelief(id: string, options?: { [key: string]: any }) {
  return request<MutationResult>(`/api/v1/governance/beliefs/${id}/archive`, {
    method: 'POST',
    ...(options || {}),
  });
}

/** Roll the current edge back to its predecessor (#124 rollback capability). */
export async function rollbackBelief(id: string, options?: { [key: string]: any }) {
  return request<RollbackResult>(`/api/v1/governance/beliefs/${id}/rollback`, {
    method: 'POST',
    ...(options || {}),
  });
}

/** GDPR forget: archive every open belief of a subject. */
export async function forgetSubject(subject: string, options?: { [key: string]: any }) {
  return request<MutationResult>(`/api/v1/governance/subjects/${encodeURIComponent(subject)}/forget`, {
    method: 'POST',
    ...(options || {}),
  });
}

export async function getGovernanceStats(options?: { [key: string]: any }) {
  return request<GovernanceStats>('/api/v1/governance/stats', {
    method: 'GET',
    ...(options || {}),
  });
}

export async function mergePrincipals(
  body: { anonymous_principal_id: string; person_principal_id: string },
  options?: { [key: string]: any },
) {
  return request<MutationResult>('/api/v1/governance/principals/merge', {
    method: 'POST',
    data: body,
    ...(options || {}),
  });
}

export async function unmergePrincipal(
  body: { anonymous_principal_id: string },
  options?: { [key: string]: any },
) {
  return request<MutationResult>('/api/v1/governance/principals/unmerge', {
    method: 'POST',
    data: body,
    ...(options || {}),
  });
}
