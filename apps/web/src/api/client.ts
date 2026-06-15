// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import type {
  WorkflowRecord,
  WorkflowVersionRecord,
  WorkflowGraph,
  ExecutionRecord,
  ExecutionSummary,
  WebhookInfo,
  WebhookRecord,
  CredentialSummary,
  ScheduleSummary,
  AuditEvent,
  WorkflowExport,
  EnvVarRecord,
  EnvSetSummary,
  WorkspaceRecord,
  ProjectRecord,
  WorkflowComment,
  EventSubscription,
  EventType,
} from '../types'
import { getStoredAuth } from '../auth'
import { getAttribution } from './attribution'

// Re-export shared domain types so consumers can import them from the API
// client module (the canonical definitions live in ../types).
export type {
  WorkflowRecord,
  WorkflowVersionRecord,
  WorkflowGraph,
  ExecutionRecord,
  ExecutionSummary,
  WebhookInfo,
  WebhookRecord,
  CredentialSummary,
  ScheduleSummary,
  AuditEvent,
  WorkflowExport,
  EnvVarRecord,
  EnvSetSummary,
  WorkspaceRecord,
  ProjectRecord,
  WorkflowComment,
  EventSubscription,
  EventType,
} from '../types'

async function request<T>(
  path: string,
  options?: RequestInit & { params?: Record<string, string | undefined> },
): Promise<T> {
  let url = path
  if (options?.params) {
    const search = new URLSearchParams()
    for (const [k, v] of Object.entries(options.params)) {
      if (v !== undefined) search.set(k, v)
    }
    const qs = search.toString()
    if (qs) url += '?' + qs
  }
  const authHeaders: Record<string, string> = {}
  const stored = getStoredAuth()
  if (stored) authHeaders['Authorization'] = `Bearer ${stored.token}`
  const res = await fetch(url, {
    headers: { 'Content-Type': 'application/json', ...authHeaders, ...options?.headers },
    ...options,
  })
  if (!res.ok) {
    const body = await res.text().catch(() => '')
    throw new Error(`${res.status} ${res.statusText}: ${body}`)
  }
  return res.json() as Promise<T>
}

async function requestWithMeta<T>(
  path: string,
  options?: RequestInit & { params?: Record<string, string | undefined> },
): Promise<{ data: T; total: number }> {
  let url = path
  if (options?.params) {
    const search = new URLSearchParams()
    for (const [k, v] of Object.entries(options.params)) {
      if (v !== undefined) search.set(k, v)
    }
    const qs = search.toString()
    if (qs) url += '?' + qs
  }
  const authHeaders: Record<string, string> = {}
  const stored = getStoredAuth()
  if (stored) authHeaders['Authorization'] = `Bearer ${stored.token}`
  const res = await fetch(url, {
    headers: { 'Content-Type': 'application/json', ...authHeaders, ...options?.headers },
    ...options,
  })
  if (!res.ok) {
    const body = await res.text().catch(() => '')
    throw new Error(`${res.status} ${res.statusText}: ${body}`)
  }
  const total = parseInt(res.headers.get('X-Total-Count') ?? '0', 10)
  const data = (await res.json()) as T
  return { data, total }
}

// ── Workflows ─────────────────────────────────────────────────────────────────

export function listWorkflows(
  tenantId: string,
  projectId: string,
  status?: string,
  tag?: string,
  folder?: string,
): Promise<WorkflowRecord[]> {
  return request('/v1/workflows', {
    params: { tenant_id: tenantId, project_id: projectId, status, tag, folder },
  })
}

export function getWorkflow(tenantId: string, workflowId: string): Promise<WorkflowRecord> {
  return request(`/v1/workflows/${workflowId}`, { params: { tenant_id: tenantId } })
}

export function createWorkflow(
  tenantId: string,
  workspaceId: string,
  projectId: string,
  name: string,
  description?: string,
): Promise<WorkflowRecord> {
  return request('/v1/workflows', {
    method: 'POST',
    body: JSON.stringify({ tenant_id: tenantId, workspace_id: workspaceId, project_id: projectId, name, description }),
  })
}

export function moveWorkflowToFolder(
  tenantId: string,
  workflowId: string,
  folder: string | null,
): Promise<WorkflowRecord> {
  return request(`/v1/workflows/${workflowId}/move`, {
    method: 'POST',
    body: JSON.stringify({ tenant_id: tenantId, folder }),
  })
}

export function duplicateWorkflow(
  tenantId: string,
  workflowId: string,
): Promise<WorkflowRecord> {
  return request(`/v1/workflows/${workflowId}/duplicate`, {
    method: 'POST',
    body: JSON.stringify({ tenant_id: tenantId }),
  })
}

export function renameWorkflow(
  tenantId: string,
  workflowId: string,
  name: string,
): Promise<WorkflowRecord> {
  return request(`/v1/workflows/${workflowId}`, {
    method: 'PATCH',
    body: JSON.stringify({ tenant_id: tenantId, name }),
  })
}

export function updateWorkflowTags(
  tenantId: string,
  workflowId: string,
  name: string,
  tags: string[],
): Promise<WorkflowRecord> {
  return request(`/v1/workflows/${workflowId}`, {
    method: 'PATCH',
    body: JSON.stringify({ tenant_id: tenantId, name, tags }),
  })
}

export function updateWorkflowDescription(
  tenantId: string,
  workflowId: string,
  name: string,
  description?: string,
): Promise<WorkflowRecord> {
  return request(`/v1/workflows/${workflowId}`, {
    method: 'PATCH',
    body: JSON.stringify({ tenant_id: tenantId, name, description: description ?? null }),
  })
}

export function updateWorkflowRateLimit(
  tenantId: string,
  workflowId: string,
  name: string,
  maxRunsPerHour: number | null,
): Promise<WorkflowRecord> {
  return request(`/v1/workflows/${workflowId}`, {
    method: 'PATCH',
    body: JSON.stringify({ tenant_id: tenantId, name, max_runs_per_hour: maxRunsPerHour }),
  })
}

export function updateWorkflowSla(
  tenantId: string,
  workflowId: string,
  name: string,
  slaSeconds: number | null,
): Promise<WorkflowRecord> {
  return request(`/v1/workflows/${workflowId}`, {
    method: 'PATCH',
    body: JSON.stringify({ tenant_id: tenantId, name, sla_seconds: slaSeconds }),
  })
}

export function updateWorkflowMaxConcurrentRuns(
  tenantId: string,
  workflowId: string,
  name: string,
  maxConcurrentRuns: number | null,
): Promise<WorkflowRecord> {
  return request(`/v1/workflows/${workflowId}`, {
    method: 'PATCH',
    body: JSON.stringify({ tenant_id: tenantId, name, max_concurrent_runs: maxConcurrentRuns }),
  })
}

export function updateWorkflowBudget(
  tenantId: string,
  workflowId: string,
  name: string,
  budgetUsd: number | null,
): Promise<WorkflowRecord> {
  return request(`/v1/workflows/${workflowId}`, {
    method: 'PATCH',
    body: JSON.stringify({ tenant_id: tenantId, name, budget_usd: budgetUsd }),
  })
}

export function updateWorkflowReadme(
  tenantId: string,
  workflowId: string,
  name: string,
  readme: string,
): Promise<WorkflowRecord> {
  return request(`/v1/workflows/${workflowId}`, {
    method: 'PATCH',
    body: JSON.stringify({ tenant_id: tenantId, name, readme: readme || null }),
  })
}

export function pinWorkflow(tenantId: string, workflowId: string): Promise<WorkflowRecord> {
  return request(`/v1/workflows/${workflowId}/pin`, {
    method: 'POST',
    body: JSON.stringify({ tenant_id: tenantId }),
  })
}

export function unpinWorkflow(tenantId: string, workflowId: string): Promise<WorkflowRecord> {
  return request(`/v1/workflows/${workflowId}/unpin`, {
    method: 'POST',
    body: JSON.stringify({ tenant_id: tenantId }),
  })
}

export function archiveWorkflow(tenantId: string, workflowId: string): Promise<WorkflowRecord> {
  return request(`/v1/workflows/${workflowId}/archive`, {
    method: 'POST',
    body: JSON.stringify({ tenant_id: tenantId }),
  })
}

export function restoreWorkflow(tenantId: string, workflowId: string): Promise<WorkflowRecord> {
  return request(`/v1/workflows/${workflowId}/restore`, {
    method: 'POST',
    body: JSON.stringify({ tenant_id: tenantId }),
  })
}

export function lockWorkflow(tenantId: string, workflowId: string): Promise<WorkflowRecord> {
  return request(`/v1/workflows/${workflowId}/lock`, {
    method: 'POST',
    body: JSON.stringify({ tenant_id: tenantId }),
  })
}

export function unlockWorkflow(tenantId: string, workflowId: string): Promise<WorkflowRecord> {
  return request(`/v1/workflows/${workflowId}/unlock`, {
    method: 'POST',
    body: JSON.stringify({ tenant_id: tenantId }),
  })
}

export function setWorkflowVisibility(
  tenantId: string,
  workflowId: string,
  visibility: 'tenant' | 'private',
): Promise<WorkflowRecord> {
  return request(`/v1/workflows/${workflowId}/visibility`, {
    method: 'PATCH',
    body: JSON.stringify({ tenant_id: tenantId, visibility }),
  })
}

// ── Workflow Versions ─────────────────────────────────────────────────────────

export function listVersions(
  tenantId: string,
  workflowId: string,
): Promise<WorkflowVersionRecord[]> {
  return request(`/v1/workflows/${workflowId}/versions`, { params: { tenant_id: tenantId } })
}

export function getVersion(
  tenantId: string,
  versionId: string,
): Promise<WorkflowVersionRecord> {
  return request(`/v1/workflow-versions/${versionId}`, { params: { tenant_id: tenantId } })
}

export function createVersion(
  tenantId: string,
  workflowId: string,
  graph: WorkflowGraph,
  message?: string,
): Promise<WorkflowVersionRecord> {
  return request(`/v1/workflows/${workflowId}/versions`, {
    method: 'POST',
    body: JSON.stringify({ tenant_id: tenantId, graph, message: message || undefined }),
  })
}

export function rollbackVersion(
  tenantId: string,
  workflowId: string,
  versionId: string,
): Promise<WorkflowVersionRecord> {
  return request(`/v1/workflows/${workflowId}/rollback/${versionId}?tenant_id=${encodeURIComponent(tenantId)}`, {
    method: 'POST',
  })
}

export function publishVersion(
  tenantId: string,
  versionId: string,
): Promise<WorkflowVersionRecord> {
  return request(`/v1/workflow-versions/${versionId}/publish`, {
    method: 'POST',
    body: JSON.stringify({ tenant_id: tenantId }),
  })
}

// ── Executions ────────────────────────────────────────────────────────────────

export function startExecutionFromWorkflow(
  tenantId: string,
  workflowId: string,
  inputJson: string,
  envSet?: string,
  label?: string,
  callbackUrl?: string,
  dryRun?: boolean,
): Promise<ExecutionRecord> {
  return request(`/v1/workflows/${workflowId}/executions`, {
    method: 'POST',
    body: JSON.stringify({ tenant_id: tenantId, input_json: inputJson, env_set: envSet, label, callback_url: callbackUrl, ...(dryRun ? { dry_run: true } : {}) }),
  })
}

export function startExecutionFromVersion(
  tenantId: string,
  versionId: string,
  inputJson: string,
  envSet?: string,
  label?: string,
): Promise<ExecutionRecord> {
  return request(`/v1/workflow-versions/${versionId}/executions`, {
    method: 'POST',
    body: JSON.stringify({ tenant_id: tenantId, input_json: inputJson, env_set: envSet, label }),
  })
}

export function startExecutionBatch(
  requests: { tenant_id: string; workflow_id: string; workflow_version_id: string; graph: WorkflowGraph; input_json: string; label?: string }[],
): Promise<ExecutionRecord[]> {
  return request('/v1/executions/batch', {
    method: 'POST',
    body: JSON.stringify({ requests }),
  })
}

export function getExecution(tenantId: string, executionId: string): Promise<ExecutionRecord> {
  return request(`/v1/executions/${executionId}`, { params: { tenant_id: tenantId } })
}

export function listExecutions(
  tenantId: string,
  workflowId?: string,
  label?: string,
): Promise<ExecutionSummary[]> {
  return request('/v1/executions', { params: { tenant_id: tenantId, workflow_id: workflowId, label } })
}

export function listExecutionsPage(
  tenantId: string,
  opts: { limit?: number; offset?: number; workflowId?: string; label?: string; status?: string; outputContains?: string } = {},
): Promise<{ data: ExecutionSummary[]; total: number }> {
  return requestWithMeta<ExecutionSummary[]>('/v1/executions', {
    params: {
      tenant_id: tenantId,
      workflow_id: opts.workflowId,
      label: opts.label,
      status: opts.status,
      output_contains: opts.outputContains,
      limit: opts.limit?.toString(),
      offset: opts.offset?.toString(),
    },
  })
}

// ── Approvals ─────────────────────────────────────────────────────────────────

export function approveExecution(executionId: string, comment?: string): Promise<{ ok: boolean }> {
  return request(`/v1/executions/${executionId}/approve`, {
    method: 'POST',
    body: JSON.stringify({ comment }),
  })
}

export function rejectExecution(executionId: string, comment?: string): Promise<{ ok: boolean }> {
  return request(`/v1/executions/${executionId}/reject`, {
    method: 'POST',
    body: JSON.stringify({ comment }),
  })
}

export function cancelExecution(tenantId: string, executionId: string): Promise<{ ok: boolean }> {
  return request(`/v1/executions/${executionId}/cancel`, {
    method: 'POST',
    body: JSON.stringify({ tenant_id: tenantId }),
  })
}

export function cancelAllRunningExecutions(tenantId: string): Promise<{ cancelled: number }> {
  return request('/v1/executions/cancel-running', {
    method: 'POST',
    body: JSON.stringify({ tenant_id: tenantId }),
  })
}

export function retryExecution(tenantId: string, executionId: string, opts?: { input_json?: string; label?: string }): Promise<ExecutionRecord> {
  return request(`/v1/executions/${executionId}/retry`, {
    method: 'POST',
    body: JSON.stringify({ tenant_id: tenantId, ...opts }),
  })
}

export function deleteExecution(tenantId: string, executionId: string): Promise<void> {
  return request(`/v1/executions/${executionId}`, {
    method: 'DELETE',
    params: { tenant_id: tenantId },
  })
}

export function patchExecutionLabel(tenantId: string, executionId: string, label: string | null): Promise<void> {
  return request(`/v1/executions/${executionId}`, {
    method: 'PATCH',
    body: JSON.stringify({ tenant_id: tenantId, label }),
  })
}

export function setExecutionNote(tenantId: string, executionId: string, note: string | null): Promise<{ ok: boolean; note: string | null }> {
  return request(`/v1/executions/${executionId}/note`, {
    method: 'POST',
    body: JSON.stringify({ tenant_id: tenantId, note }),
  })
}

export function starExecution(tenantId: string, executionId: string): Promise<{ ok: boolean; starred: boolean }> {
  return request(`/v1/executions/${executionId}/star`, {
    method: 'POST',
    params: { tenant_id: tenantId },
  })
}

export function unstarExecution(tenantId: string, executionId: string): Promise<{ ok: boolean; starred: boolean }> {
  return request(`/v1/executions/${executionId}/unstar`, {
    method: 'POST',
    params: { tenant_id: tenantId },
  })
}

// ── Webhooks ──────────────────────────────────────────────────────────────────

export function getWebhook(tenantId: string, versionId: string): Promise<WebhookInfo> {
  return request(`/v1/workflow-versions/${versionId}/webhook`, {
    method: 'POST',
    body: JSON.stringify({ tenant_id: tenantId }),
  })
}

// ── Credentials ───────────────────────────────────────────────────────────────

export function listCredentials(tenantId: string): Promise<CredentialSummary[]> {
  return request('/v1/credentials', { params: { tenant_id: tenantId } })
}

export function createCredential(
  tenantId: string,
  name: string,
  value: string,
): Promise<CredentialSummary> {
  return request('/v1/credentials', {
    method: 'POST',
    body: JSON.stringify({ tenant_id: tenantId, name, value }),
  })
}

export function deleteCredential(tenantId: string, id: string): Promise<void> {
  return request(`/v1/credentials/${id}`, {
    method: 'DELETE',
    params: { tenant_id: tenantId },
  })
}

export function updateCredential(
  tenantId: string,
  id: string,
  patch: { value?: string; description?: string | null; expires_at?: number | null },
): Promise<CredentialSummary> {
  return request(`/v1/credentials/${id}`, {
    method: 'PATCH',
    body: JSON.stringify({ tenant_id: tenantId, ...patch }),
  })
}

export function listExpiringCredentials(
  tenantId: string,
  withinDays = 30,
): Promise<CredentialSummary[]> {
  return request('/v1/credentials/expiring', {
    params: { tenant_id: tenantId, within_days: String(withinDays) },
  })
}

export interface CredentialUsageEntry {
  workflow_id: string
  workflow_name: string
  version_id: string
  version: number
}

export function getCredentialUsage(
  tenantId: string,
): Promise<{ usages: Record<string, CredentialUsageEntry[]> }> {
  return request('/v1/credentials/usage', { params: { tenant_id: tenantId } })
}

// ── Schedules ─────────────────────────────────────────────────────────────────

export function listSchedules(tenantId: string): Promise<ScheduleSummary[]> {
  return request('/v1/schedules', { params: { tenant_id: tenantId } })
}

export function pauseSchedule(versionId: string): Promise<{ ok: boolean }> {
  return request(`/v1/schedules/${versionId}/pause`, { method: 'POST' })
}

export function resumeSchedule(versionId: string): Promise<{ ok: boolean }> {
  return request(`/v1/schedules/${versionId}/resume`, { method: 'POST' })
}

// ── Export / Import ───────────────────────────────────────────────────────────

export function exportWorkflow(tenantId: string, workflowId: string): Promise<WorkflowExport> {
  return request(`/v1/workflows/${workflowId}/export`, {
    params: { tenant_id: tenantId },
  })
}

export function importWorkflow(
  tenantId: string,
  workspaceId: string,
  projectId: string,
  name: string,
  graph: WorkflowGraph,
  opts?: { description?: string; readme?: string; tags?: string[] },
): Promise<WorkflowRecord> {
  return request('/v1/workflows/import', {
    method: 'POST',
    body: JSON.stringify({
      tenant_id: tenantId, workspace_id: workspaceId, project_id: projectId,
      name, graph,
      description: opts?.description,
      readme: opts?.readme,
      tags: opts?.tags,
    }),
  })
}

export interface GenerateWorkflowResult {
  graph: WorkflowGraph
  name: string
  description: string
  workflow?: WorkflowRecord
}

export function generateWorkflow(
  prompt: string,
  opts: {
    tenantId?: string
    workspaceId?: string
    projectId?: string
    apiKey?: string
    model?: string
    create?: boolean
  } = {},
): Promise<GenerateWorkflowResult> {
  return request('/v1/workflows/generate', {
    method: 'POST',
    body: JSON.stringify({
      prompt,
      tenant_id: opts.tenantId ?? '',
      workspace_id: opts.workspaceId,
      project_id: opts.projectId,
      api_key: opts.apiKey,
      model: opts.model,
      create: opts.create ?? false,
    }),
  })
}

// ── Copilot ───────────────────────────────────────────────────────────────────

export function copilotQuery(
  message: string,
  opts: { tenantId?: string; graphJson?: string; apiKey?: string; model?: string } = {},
): Promise<{ reply: string }> {
  return request('/v1/copilot', {
    method: 'POST',
    body: JSON.stringify({
      message,
      tenant_id: opts.tenantId ?? '',
      graph_json: opts.graphJson,
      api_key: opts.apiKey,
      model: opts.model,
    }),
  })
}

// ── Audit Log ─────────────────────────────────────────────────────────────────

export function listAuditLog(tenantId: string, limit?: number, resourceId?: string): Promise<AuditEvent[]> {
  return request('/v1/audit-log', {
    params: { tenant_id: tenantId, limit: limit?.toString(), resource_id: resourceId },
  })
}

export interface TokenUsageSummary {
  prompt_tokens: number
  completion_tokens: number
  total_tokens: number
  by_model: Record<string, { prompt_tokens: number; completion_tokens: number; total_tokens: number }>
}

export function getTokenUsage(tenantId: string, days?: number): Promise<TokenUsageSummary> {
  return request('/v1/token-usage', {
    params: { tenant_id: tenantId, days: days?.toString() },
  })
}

export interface AcquisitionChannel {
  channel: string
  signups: number
  paid: number
  revenue_cents: number
}

/** Operator-only (admin): acquisition ROI per channel (signups, paid, revenue). */
export function getAcquisitionChannels(): Promise<AcquisitionChannel[]> {
  return request('/v1/analytics/attribution')
}

export interface NodeTypeStat {
  node_type: string
  total: number
  succeeded: number
  failed: number
  skipped: number
  avg_duration_ms?: number
}

export function getNodeTypeAnalytics(tenantId: string): Promise<NodeTypeStat[]> {
  return request('/v1/analytics/node-types', { params: { tenant_id: tenantId } })
}

export interface WorkflowDepEdge { from_workflow_id: string; to_workflow_id: string; node_type: string }
export interface WorkflowDepsResponse { edges: WorkflowDepEdge[] }

export function getWorkflowDeps(tenantId: string): Promise<WorkflowDepsResponse> {
  return request('/v1/analytics/workflow-deps', { params: { tenant_id: tenantId } })
}

export interface WorkflowStatRow {
  workflow_id: string
  total: number
  succeeded: number
  failed: number
  cancelled: number
  running: number
  avg_duration_secs: number | null
  last_run_at: number | null
}
export interface WorkflowStatsAnalyticsResponse { rows: WorkflowStatRow[]; since: number }

export function getWorkflowStatsAnalytics(tenantId: string, days = 30): Promise<WorkflowStatsAnalyticsResponse> {
  return request('/v1/analytics/workflow-stats', { params: { tenant_id: tenantId, days: String(days) } })
}

export interface SlaBreachEntry {
  execution_id: string
  workflow_id: string
  workflow_name: string
  sla_seconds: number
  elapsed_seconds: number
  overage_seconds: number
  started_at: number
  finished_at: number
}

export interface SlaBreachesResponse {
  breaches: SlaBreachEntry[]
  total_workflows_with_sla: number
  compliance_rate: number
  total_completed: number
}

export function getSlaBreaches(tenantId: string, days = 30): Promise<SlaBreachesResponse> {
  return request('/v1/analytics/sla-breaches', { params: { tenant_id: tenantId, days: String(days) } })
}

export interface TopErrorEntry {
  error_message: string
  count: number
  node_type: string
  workflow_id: string
  workflow_name: string
  last_seen: number
}

export interface ErrorAnalysisResponse {
  top_errors: TopErrorEntry[]
  total_failed_nodes: number
  distinct_error_types: number
}

export function getErrorAnalysis(tenantId: string, days = 30): Promise<ErrorAnalysisResponse> {
  return request('/v1/analytics/errors', { params: { tenant_id: tenantId, days: String(days) } })
}

// ── Environment Variables ─────────────────────────────────────────────────────

export function listEnvVars(tenantId: string, set?: string): Promise<EnvVarRecord[]> {
  return request('/v1/env-vars', { params: { tenant_id: tenantId, set } })
}

export function upsertEnvVar(tenantId: string, key: string, value: string, set?: string): Promise<EnvVarRecord> {
  return request(`/v1/env-vars/${encodeURIComponent(key)}`, {
    method: 'PUT',
    body: JSON.stringify({ value }),
    params: { tenant_id: tenantId, set },
  })
}

export function deleteEnvVar(tenantId: string, key: string, set?: string): Promise<void> {
  return request(`/v1/env-vars/${encodeURIComponent(key)}`, {
    method: 'DELETE',
    params: { tenant_id: tenantId, set },
  })
}

export function listEnvSets(tenantId: string): Promise<EnvSetSummary[]> {
  return request('/v1/env-sets', { params: { tenant_id: tenantId } })
}

export function deleteEnvSet(tenantId: string, name: string): Promise<void> {
  return request('/v1/env-sets', {
    method: 'DELETE',
    params: { tenant_id: tenantId, name },
  })
}

// ── Workspaces / Projects ─────────────────────────────────────────────────────

export function listWorkspaces(tenantId: string): Promise<WorkspaceRecord[]> {
  return request('/v1/workspaces', { params: { tenant_id: tenantId } })
}

export function createWorkspace(tenantId: string, name: string, description?: string): Promise<WorkspaceRecord> {
  return request('/v1/workspaces', {
    method: 'POST',
    body: JSON.stringify({ tenant_id: tenantId, name, description }),
  })
}

export function deleteWorkspace(tenantId: string, workspaceId: string): Promise<void> {
  return request(`/v1/workspaces/${workspaceId}`, {
    method: 'DELETE',
    params: { tenant_id: tenantId },
  })
}

export function listProjects(tenantId: string, workspaceId: string): Promise<ProjectRecord[]> {
  return request(`/v1/workspaces/${workspaceId}/projects`, { params: { tenant_id: tenantId } })
}

export function createProject(tenantId: string, workspaceId: string, name: string, description?: string): Promise<ProjectRecord> {
  return request(`/v1/workspaces/${workspaceId}/projects`, {
    method: 'POST',
    body: JSON.stringify({ tenant_id: tenantId, name, description }),
  })
}

export function deleteProject(tenantId: string, projectId: string): Promise<void> {
  return request(`/v1/projects/${projectId}`, {
    method: 'DELETE',
    params: { tenant_id: tenantId },
  })
}

// ── Webhooks ──────────────────────────────────────────────────────────────────

export function listWebhooks(tenantId: string): Promise<WebhookRecord[]> {
  return request('/v1/webhooks', { params: { tenant_id: tenantId } })
}

export function deleteWebhook(tenantId: string, token: string): Promise<void> {
  return request(`/v1/webhooks/${token}`, {
    method: 'DELETE',
    params: { tenant_id: tenantId },
  })
}

// ── Variables ─────────────────────────────────────────────────────────────────

export interface Variable { key: string; value: unknown }

export function listVariables(tenantId: string, workflowId: string): Promise<Variable[]> {
  return request(`/v1/workflows/${workflowId}/variables`, { params: { tenant_id: tenantId } })
}

export function setVariable(tenantId: string, workflowId: string, key: string, value: unknown): Promise<Variable> {
  return request(`/v1/workflows/${workflowId}/variables/${encodeURIComponent(key)}`, {
    method: 'PUT',
    body: JSON.stringify({ value }),
    params: { tenant_id: tenantId },
  })
}

export function deleteVariable(tenantId: string, workflowId: string, key: string): Promise<void> {
  return request(`/v1/workflows/${workflowId}/variables/${encodeURIComponent(key)}`, {
    method: 'DELETE',
    params: { tenant_id: tenantId },
  })
}

export function incrementVariable(tenantId: string, workflowId: string, key: string, by = 1): Promise<Variable> {
  return request(`/v1/workflows/${workflowId}/variables/${encodeURIComponent(key)}/increment`, {
    method: 'POST',
    body: JSON.stringify({ by }),
    params: { tenant_id: tenantId },
  })
}

// ── API Keys ──────────────────────────────────────────────────────────────────

export interface ApiKeyRecord { id: string; tenant_id: string; name: string; prefix: string; created_at: number }

export function listApiKeys(tenantId: string): Promise<ApiKeyRecord[]> {
  return request('/v1/api-keys', { params: { tenant_id: tenantId } })
}

export function createApiKey(tenantId: string, name: string): Promise<ApiKeyRecord & { key: string }> {
  return request('/v1/api-keys', { method: 'POST', body: JSON.stringify({ tenant_id: tenantId, name }) })
}

export function deleteApiKey(tenantId: string, id: string): Promise<void> {
  return request(`/v1/api-keys/${id}`, { method: 'DELETE', params: { tenant_id: tenantId } })
}

// ── Cron preview ──────────────────────────────────────────────────────────────

export function previewCron(expression: string, count = 5): Promise<{ next_times: string[]; error?: string }> {
  return request('/v1/cron/preview', { method: 'POST', body: JSON.stringify({ expression, count }) })
}

export interface WorkflowStats {
  total: number
  succeeded: number
  failed: number
  running: number
  avg_duration_secs: number | null
}

export function getWorkflowStats(tenantId: string, workflowId: string): Promise<WorkflowStats> {
  return request(`/v1/workflows/${workflowId}/stats?tenant_id=${encodeURIComponent(tenantId)}`)
}

export function getLatestExecution(tenantId: string, workflowId: string): Promise<ExecutionSummary | null> {
  return request(`/v1/workflows/${workflowId}/latest-execution?tenant_id=${encodeURIComponent(tenantId)}`)
}

export interface WorkflowHealthIssue {
  severity: 'error' | 'warning'
  message: string
}

export interface WorkflowHealthReport {
  workflow_id: string
  status: 'healthy' | 'warning' | 'error'
  issues: WorkflowHealthIssue[]
  published_version_id: string | null
  last_run_status: string | null
  last_run_at: number | null
}

export function getWorkflowHealth(tenantId: string, workflowId: string): Promise<WorkflowHealthReport> {
  return request(`/v1/workflows/${workflowId}/health?tenant_id=${encodeURIComponent(tenantId)}`)
}

export function getWorkflowJsonSchema(tenantId: string, workflowId: string): Promise<Record<string, unknown>> {
  return request(`/v1/workflows/${workflowId}/json-schema?tenant_id=${encodeURIComponent(tenantId)}`)
}

export interface WorkflowEstimate {
  sample_count: number
  p50_secs: number | null
  p95_secs: number | null
  min_secs: number | null
  max_secs: number | null
}

export function getWorkflowEstimate(tenantId: string, workflowId: string): Promise<WorkflowEstimate> {
  return request(`/v1/workflows/${workflowId}/estimate?tenant_id=${encodeURIComponent(tenantId)}`)
}

export interface NodeStat {
  node_id: string
  node_type: string
  total: number
  succeeded: number
  failed: number
  skipped: number
  avg_duration_ms: number | null
}

export function getWorkflowNodeStats(tenantId: string, workflowId: string): Promise<NodeStat[]> {
  return request(`/v1/workflows/${workflowId}/node-stats?tenant_id=${encodeURIComponent(tenantId)}`)
}

export interface ExecutionStats {
  total: number
  running: number
  waiting_approval: number
  succeeded: number
  failed: number
  cancelled: number
  by_trigger: Record<string, number>
  avg_duration_secs: number | null
}

export function getExecutionStats(tenantId: string): Promise<ExecutionStats> {
  return request('/v1/executions/stats', { params: { tenant_id: tenantId } })
}

// ── System info ───────────────────────────────────────────────────────────────

export interface SystemInfo {
  version: string
  node_types: number
  auth_required: boolean
  rust_edition: string
  features: string[]
  /** Captcha provider + public site key, present only when captcha is enforced. */
  captcha_provider?: string | null
  captcha_site_key?: string | null
}

export function getSystemInfo(): Promise<SystemInfo> {
  return request('/v1/system/info')
}

export interface QueueDepthInfo {
  queue_depth: number | null
  stream: string
}

export function getQueueDepth(): Promise<QueueDepthInfo> {
  return request('/v1/system/queue-depth')
}

// ── Global search ─────────────────────────────────────────────────────────────

export interface SearchWorkflowHit {
  id: string
  name: string
  status: string
  description?: string
}

export interface SearchExecutionHit {
  id: string
  workflow_id: string
  status: string
  label?: string
}

export interface SearchResult {
  workflows: SearchWorkflowHit[]
  executions: SearchExecutionHit[]
}

export function search(tenantId: string, q: string): Promise<SearchResult> {
  return request('/v1/search', { params: { tenant_id: tenantId, q } })
}

export interface FormTokenRecord {
  token: string
  title: string
  description?: string
  workflow_id: string
  input_schema: unknown[]
  created_at: number
}

export function publishForm(
  tenantId: string,
  workflowId: string,
  title: string,
  description?: string,
): Promise<{ token: string; title: string; workflow_id: string }> {
  return request(`/v1/workflows/${workflowId}/publish-form`, {
    method: 'POST',
    body: JSON.stringify({ tenant_id: tenantId, title, description }),
  })
}

export function listForms(tenantId: string, workflowId: string): Promise<FormTokenRecord[]> {
  return request(`/v1/workflows/${workflowId}/forms`, { params: { tenant_id: tenantId } })
}

export function getForm(token: string): Promise<FormTokenRecord> {
  return request(`/v1/forms/${token}`)
}

export function deleteForm(token: string): Promise<void> {
  return request(`/v1/forms/${token}`, { method: 'DELETE' })
}

export function submitForm(token: string, inputJson: string): Promise<ExecutionRecord> {
  return request(`/v1/forms/${token}/submit`, {
    method: 'POST',
    body: JSON.stringify({ input_json: inputJson }),
  })
}

export interface TestCase {
  id: string
  tenant_id: string
  workflow_id: string
  name: string
  input_json: string
  expected_output?: string
  created_at: number
  updated_at: number
}

export interface TestCaseRunResult {
  test_case_id: string
  execution_id: string
  status: string
  passed: boolean
  output_json?: string
  expected_output?: string
}

export function listTestCases(tenantId: string, workflowId: string): Promise<TestCase[]> {
  return request(`/v1/workflows/${workflowId}/test-cases`, { params: { tenant_id: tenantId } })
}

export function createTestCase(
  tenantId: string,
  workflowId: string,
  name: string,
  inputJson: string,
  expectedOutput?: string,
): Promise<TestCase> {
  return request(`/v1/workflows/${workflowId}/test-cases`, {
    method: 'POST',
    body: JSON.stringify({ tenant_id: tenantId, name, input_json: inputJson, expected_output: expectedOutput }),
  })
}

export function updateTestCase(
  id: string,
  fields: { name?: string; input_json?: string; expected_output?: string },
): Promise<TestCase> {
  return request(`/v1/test-cases/${id}`, {
    method: 'PATCH',
    body: JSON.stringify(fields),
  })
}

export function deleteTestCase(id: string): Promise<void> {
  return request(`/v1/test-cases/${id}`, { method: 'DELETE' })
}

export function runTestCase(id: string): Promise<TestCaseRunResult> {
  return request(`/v1/test-cases/${id}/run`, { method: 'POST' })
}


export function listComments(tenantId: string, workflowId: string): Promise<WorkflowComment[]> {
  return request(`/v1/workflows/${workflowId}/comments`, { params: { tenant_id: tenantId } })
}

export function createComment(tenantId: string, workflowId: string, author: string, body: string): Promise<WorkflowComment> {
  return request(`/v1/workflows/${workflowId}/comments`, {
    method: 'POST',
    body: JSON.stringify({ tenant_id: tenantId, author, body }),
  })
}

export function editComment(tenantId: string, commentId: string, body: string): Promise<WorkflowComment> {
  return request(`/v1/comments/${commentId}`, {
    method: 'PATCH',
    body: JSON.stringify({ tenant_id: tenantId, body }),
  })
}

export function deleteComment(tenantId: string, commentId: string): Promise<void> {
  return request(`/v1/comments/${commentId}`, {
    method: 'DELETE',
    params: { tenant_id: tenantId },
  })
}

// ── Event Subscriptions ───────────────────────────────────────────────────────

export function listEventSubscriptions(tenantId: string): Promise<EventSubscription[]> {
  return request('/v1/event-subscriptions', { params: { tenant_id: tenantId } })
}

export function createEventSubscription(
  tenantId: string,
  url: string,
  events: EventType[],
  description?: string,
): Promise<EventSubscription> {
  return request('/v1/event-subscriptions', {
    method: 'POST',
    body: JSON.stringify({ tenant_id: tenantId, url, events, description }),
  })
}

export function deleteEventSubscription(tenantId: string, subId: string): Promise<void> {
  return request(`/v1/event-subscriptions/${subId}`, {
    method: 'DELETE',
    params: { tenant_id: tenantId },
  })
}

// ── User Auth ─────────────────────────────────────────────────────────────────

export interface User {
  id: string
  email: string
  name?: string
  tenant_id: string
  created_at: number
  email_verified?: boolean
}

export interface AuthResponse {
  token: string
  user: User
}

export function registerUser(email: string, password: string, name?: string, tenantId?: string, captchaToken?: string): Promise<AuthResponse> {
  return request('/v1/auth/register', {
    method: 'POST',
    body: JSON.stringify({ email, password, name, tenant_id: tenantId, captcha_token: captchaToken, attribution: getAttribution() }),
  })
}

export function loginUser(email: string, password: string, captchaToken?: string): Promise<AuthResponse> {
  return request('/v1/auth/login', {
    method: 'POST',
    body: JSON.stringify({ email, password, captcha_token: captchaToken }),
  })
}

export function getCurrentUser(): Promise<User> {
  return request('/v1/auth/me')
}

// ── Enterprise SSO (OIDC) ──────────────────────────────────────────────────

export interface PublicSsoConnection {
  slug: string
  provider: string
}

export type SsoKind = 'oidc' | 'feishu' | 'dingtalk' | 'wechat_work'

export interface SsoConnection {
  id: string
  tenant_id: string
  slug: string
  provider: string
  kind: SsoKind
  issuer: string
  client_id: string
  agent_id?: string | null
  scopes: string
  enabled: boolean
  created_at: number
}

/** Enabled SSO connections to render login buttons (no secrets). */
export function listPublicSso(): Promise<PublicSsoConnection[]> {
  return request('/v1/sso/public')
}

export function listSsoConnections(): Promise<SsoConnection[]> {
  return request('/v1/sso-connections')
}

export function createSsoConnection(body: {
  slug: string
  provider: string
  kind?: SsoKind
  issuer?: string
  client_id: string
  client_secret: string
  agent_id?: string
  scopes?: string
}): Promise<SsoConnection> {
  return request('/v1/sso-connections', { method: 'POST', body: JSON.stringify(body) })
}

export function deleteSsoConnection(id: string): Promise<void> {
  return request(`/v1/sso-connections/${id}`, { method: 'DELETE' })
}

export function setSsoConnectionEnabled(id: string, enabled: boolean): Promise<void> {
  return request(`/v1/sso-connections/${id}`, { method: 'PATCH', body: JSON.stringify({ enabled }) })
}

// ── RAG knowledge base management ──────────────────────────────────────────

export interface KnowledgeBase {
  kb: string
  docs: number
  chunks: number
}

export interface RagDocument {
  doc_id: string
  chunks: number
  created_at: number
}

export function listKnowledgeBases(): Promise<{ knowledge_bases: KnowledgeBase[] }> {
  return request('/v1/rag/kbs')
}

export function listRagDocuments(kb: string): Promise<{ documents: RagDocument[] }> {
  return request('/v1/rag/documents', { params: { kb } })
}

export function ingestRagDocument(body: {
  kb: string
  doc_id: string
  text: string
  chunk_size?: number
  overlap?: number
}): Promise<{ doc_id: string; chunks: number; backend: string; dim: number }> {
  return request('/v1/rag/ingest', { method: 'POST', body: JSON.stringify(body) })
}

export function deleteRagDocument(kb: string, docId: string): Promise<{ deleted: number }> {
  return request(`/v1/rag/documents/${encodeURIComponent(kb)}/${encodeURIComponent(docId)}`, { method: 'DELETE' })
}

// ── Custom node registry (node SDK) ────────────────────────────────────────

export interface CustomNodeDef {
  id: string
  tenant_id: string
  slug: string
  label: string
  description: string
  endpoint: string
  config_schema: { properties?: Record<string, { type?: string; title?: string }> } & Record<string, unknown>
  created_at: number
}

export function listCustomNodes(): Promise<CustomNodeDef[]> {
  return request('/v1/custom-nodes')
}

export function upsertCustomNode(body: {
  slug: string
  label: string
  description?: string
  endpoint: string
  config_schema?: unknown
}): Promise<CustomNodeDef> {
  return request('/v1/custom-nodes', { method: 'POST', body: JSON.stringify(body) })
}

export function deleteCustomNode(id: string): Promise<void> {
  return request(`/v1/custom-nodes/${id}`, { method: 'DELETE' })
}

export function importCustomNodes(baseUrl: string): Promise<CustomNodeDef[]> {
  return request('/v1/custom-nodes/import', { method: 'POST', body: JSON.stringify({ base_url: baseUrl }) })
}

export interface UpdateProfileRequest {
  name?: string
  current_password?: string
  new_password?: string
}

export function updateProfile(body: UpdateProfileRequest): Promise<User> {
  return request('/v1/auth/me', {
    method: 'PATCH',
    body: JSON.stringify(body),
  })
}

// ── Admin: User Management ────────────────────────────────────────────────────

export function listAdminUsers(): Promise<User[]> {
  return request('/v1/admin/users')
}

export function deleteAdminUser(userId: string): Promise<void> {
  return request(`/v1/admin/users/${userId}`, { method: 'DELETE' })
}

export interface Invitation {
  id: string
  email: string
  token: string
  role: string
  tenant_id: string
  created_at: number
  expires_at: number
  used_at: number | null
}

export function listInvitations(): Promise<Invitation[]> {
  return request('/v1/admin/invitations')
}

export function createInvitation(email: string, role?: string, expiresHours?: number): Promise<Invitation> {
  return request('/v1/admin/invitations', {
    method: 'POST',
    body: JSON.stringify({ email, role, expires_hours: expiresHours }),
  })
}

export function deleteInvitation(inviteId: string): Promise<void> {
  return request(`/v1/admin/invitations/${inviteId}`, { method: 'DELETE' })
}

export function getInvitation(token: string): Promise<{ email: string; role: string; valid: boolean }> {
  return request(`/v1/invitations/${token}`)
}

export function acceptInvite(token: string, password: string, name?: string): Promise<AuthResponse> {
  return request('/v1/auth/accept-invite', {
    method: 'POST',
    body: JSON.stringify({ token, password, name }),
  })
}

// ── Organization Management ───────────────────────────────────────────────────

export interface OrgRecord {
  id: string
  name: string
  owner_id: string
  created_at: number
}

export interface OrgMember {
  org_id: string
  user_id: string
  role: string
  joined_at: number
}

export interface SwitchOrgResponse {
  token: string
  org_id: string
  tenant_id: string
  role: string
}

export function listOrgs(): Promise<OrgRecord[]> {
  return request('/v1/orgs')
}

export function createOrg(name: string): Promise<OrgRecord> {
  return request('/v1/orgs', { method: 'POST', body: JSON.stringify({ name }) })
}

export function getOrg(orgId: string): Promise<OrgRecord> {
  return request(`/v1/orgs/${orgId}`)
}

export function deleteOrg(orgId: string): Promise<void> {
  return request(`/v1/orgs/${orgId}`, { method: 'DELETE' })
}

export function listOrgMembers(orgId: string): Promise<OrgMember[]> {
  return request(`/v1/orgs/${orgId}/members`)
}

export function addOrgMember(orgId: string, userId: string, role: string): Promise<OrgMember> {
  return request(`/v1/orgs/${orgId}/members`, {
    method: 'POST',
    body: JSON.stringify({ user_id: userId, role }),
  })
}

export function removeOrgMember(orgId: string, userId: string): Promise<void> {
  return request(`/v1/orgs/${orgId}/members/${userId}`, { method: 'DELETE' })
}

export function switchOrg(orgId: string): Promise<SwitchOrgResponse> {
  return request(`/v1/orgs/${orgId}/switch`, { method: 'POST' })
}

// ── Password reset ────────────────────────────────────────────────────────────

export interface ForgotPasswordResponse {
  message: string
  token?: string
  expires_at: number
}

export function forgotPassword(email: string): Promise<ForgotPasswordResponse> {
  return request('/v1/auth/forgot-password', {
    method: 'POST',
    body: JSON.stringify({ email }),
  })
}

export function resetPassword(token: string, new_password: string): Promise<{ ok: boolean; message: string }> {
  return request('/v1/auth/reset-password', {
    method: 'POST',
    body: JSON.stringify({ token, new_password }),
  })
}

export function verifyEmail(token: string): Promise<{ ok: boolean; message: string }> {
  return request('/v1/auth/verify-email', {
    method: 'POST',
    body: JSON.stringify({ token }),
  })
}

export function resendVerification(email: string): Promise<{ ok: boolean; message: string }> {
  return request('/v1/auth/resend-verification', {
    method: 'POST',
    body: JSON.stringify({ email }),
  })
}

export interface NotificationPrefs {
  user_id: string
  email_on_failure: boolean
  email_on_success: boolean
}

export function getNotificationPrefs(): Promise<NotificationPrefs> {
  return request('/v1/auth/me/notifications')
}

export function updateNotificationPrefs(prefs: Omit<NotificationPrefs, 'user_id'>): Promise<NotificationPrefs> {
  return request('/v1/auth/me/notifications', {
    method: 'PUT',
    body: JSON.stringify(prefs),
  })
}

// ── Billing ────────────────────────────────────────────────────────────────────

export interface TenantQuota {
  tenant_id: string
  tier: string
  max_executions_per_month: number
  max_concurrent_executions: number
  max_workflows: number
}

export interface UsageSummary {
  tenant_id: string
  year_month: string
  executions_used: number
  tokens_used: number
}

export interface BillingStatus {
  quota: TenantQuota
  usage: UsageSummary
  executions_remaining: number
  usage_pct: number
  has_subscription: boolean
  stripe_enabled: boolean
  reset_in_secs: number
}

export function getBillingStatus(): Promise<BillingStatus> {
  return request('/v1/billing/status')
}

export function setTenantQuota(tenantId: string, tier: string): Promise<TenantQuota> {
  return request(`/v1/admin/billing/${tenantId}/quota`, {
    method: 'PUT',
    body: JSON.stringify({ tier }),
  })
}

export function getBillingHistory(months = 6): Promise<UsageSummary[]> {
  return request(`/v1/billing/history?months=${months}`)
}

export function createCheckoutSession(tier: string): Promise<{ url: string }> {
  return request('/v1/billing/checkout', {
    method: 'POST',
    body: JSON.stringify({ tier }),
  })
}

export function createPortalSession(): Promise<{ url: string }> {
  return request('/v1/billing/portal', { method: 'POST' })
}

export interface WebhookDelivery {
  id: string
  webhook_token: string
  tenant_id: string
  delivered_at: number
  status_code: number | null
  success: boolean
  error_message: string | null
  execution_id: string | null
}

export function setWebhookCondition(token: string, conditionExpr: string | null): Promise<WebhookRecord> {
  return request(`/v1/webhooks/${token}/condition`, {
    method: 'PATCH',
    body: JSON.stringify({ condition_expr: conditionExpr }),
  })
}

export function setWebhookRateLimit(token: string, maxCallsPerMinute: number | null): Promise<WebhookRecord> {
  return request(`/v1/webhooks/${token}/rate-limit`, {
    method: 'PATCH',
    body: JSON.stringify({ max_calls_per_minute: maxCallsPerMinute }),
  })
}

export function pauseWebhook(token: string): Promise<WebhookRecord> {
  return request(`/v1/webhooks/${token}/pause`, { method: 'POST' })
}

export function rotateWebhookSecret(token: string): Promise<{ secret: string }> {
  return request(`/v1/webhooks/${token}/rotate-secret`, { method: 'POST' })
}

export function resumeWebhook(token: string): Promise<WebhookRecord> {
  return request(`/v1/webhooks/${token}/resume`, { method: 'POST' })
}

export function setWebhookPayloadTransform(token: string, script: string | null): Promise<WebhookRecord> {
  return request(`/v1/webhooks/${token}/payload-transform`, {
    method: 'POST',
    body: JSON.stringify({ script }),
  })
}

export function listWebhookDeliveries(token: string, limit?: number): Promise<WebhookDelivery[]> {
  return request(`/v1/webhooks/${token}/deliveries`, {
    params: limit !== undefined ? { limit: String(limit) } : undefined,
  })
}

// ── In-App Notifications ──────────────────────────────────────────────────────

export interface AppNotification {
  id: string
  tenant_id: string
  user_id?: string
  title: string
  body: string
  level: string  // 'info' | 'warning' | 'error'
  read: boolean
  created_at: number
}

export interface NotificationsResponse {
  notifications: AppNotification[]
  unread_count: number
}

export function listNotifications(tenantId: string, limit = 50): Promise<NotificationsResponse> {
  return request('/v1/notifications', { params: { tenant_id: tenantId, limit: String(limit) } })
}

export function markNotificationRead(id: string): Promise<{ ok: boolean }> {
  return request(`/v1/notifications/${id}/read`, { method: 'POST' })
}

export function markAllNotificationsRead(): Promise<{ ok: boolean }> {
  return request('/v1/notifications/read-all', { method: 'POST' })
}

export function deleteNotification(id: string): Promise<void> {
  return request(`/v1/notifications/${id}`, { method: 'DELETE' })
}
