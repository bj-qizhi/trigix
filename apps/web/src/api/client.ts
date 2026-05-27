import type {
  WorkflowRecord,
  WorkflowVersionRecord,
  WorkflowGraph,
  ExecutionRecord,
  ExecutionSummary,
  WebhookInfo,
  CredentialSummary,
  ScheduleSummary,
  AuditEvent,
  WorkflowExport,
} from '../types'
import { getStoredAuth } from '../auth'

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

// ── Workflows ─────────────────────────────────────────────────────────────────

export function listWorkflows(
  tenantId: string,
  projectId: string,
  status?: string,
): Promise<WorkflowRecord[]> {
  return request('/v1/workflows', {
    params: { tenant_id: tenantId, project_id: projectId, status },
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
): Promise<WorkflowRecord> {
  return request('/v1/workflows', {
    method: 'POST',
    body: JSON.stringify({ tenant_id: tenantId, workspace_id: workspaceId, project_id: projectId, name }),
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
): Promise<WorkflowVersionRecord> {
  return request(`/v1/workflows/${workflowId}/versions`, {
    method: 'POST',
    body: JSON.stringify({ tenant_id: tenantId, graph }),
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
): Promise<ExecutionRecord> {
  return request(`/v1/workflows/${workflowId}/executions`, {
    method: 'POST',
    body: JSON.stringify({ tenant_id: tenantId, input_json: inputJson }),
  })
}

export function startExecutionFromVersion(
  tenantId: string,
  versionId: string,
  inputJson: string,
): Promise<ExecutionRecord> {
  return request(`/v1/workflow-versions/${versionId}/executions`, {
    method: 'POST',
    body: JSON.stringify({ tenant_id: tenantId, input_json: inputJson }),
  })
}

export function getExecution(tenantId: string, executionId: string): Promise<ExecutionRecord> {
  return request(`/v1/executions/${executionId}`, { params: { tenant_id: tenantId } })
}

export function listExecutions(
  tenantId: string,
  workflowId?: string,
): Promise<ExecutionSummary[]> {
  return request('/v1/executions', { params: { tenant_id: tenantId, workflow_id: workflowId } })
}

// ── Approvals ─────────────────────────────────────────────────────────────────

export function approveExecution(executionId: string): Promise<{ ok: boolean }> {
  return request(`/v1/executions/${executionId}/approve`, {
    method: 'POST',
    body: JSON.stringify({}),
  })
}

export function rejectExecution(executionId: string): Promise<{ ok: boolean }> {
  return request(`/v1/executions/${executionId}/reject`, {
    method: 'POST',
    body: JSON.stringify({}),
  })
}

export function cancelExecution(tenantId: string, executionId: string): Promise<{ ok: boolean }> {
  return request(`/v1/executions/${executionId}/cancel`, {
    method: 'POST',
    body: JSON.stringify({ tenant_id: tenantId }),
  })
}

export function retryExecution(tenantId: string, executionId: string): Promise<ExecutionRecord> {
  return request(`/v1/executions/${executionId}/retry`, {
    method: 'POST',
    body: JSON.stringify({ tenant_id: tenantId }),
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

// ── Schedules ─────────────────────────────────────────────────────────────────

export function listSchedules(tenantId: string): Promise<ScheduleSummary[]> {
  return request('/v1/schedules', { params: { tenant_id: tenantId } })
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
): Promise<WorkflowRecord> {
  return request('/v1/workflows/import', {
    method: 'POST',
    body: JSON.stringify({ tenant_id: tenantId, workspace_id: workspaceId, project_id: projectId, name, graph }),
  })
}

// ── Audit Log ─────────────────────────────────────────────────────────────────

export function listAuditLog(tenantId: string, limit?: number): Promise<AuditEvent[]> {
  return request('/v1/audit-log', {
    params: { tenant_id: tenantId, limit: limit?.toString() },
  })
}
