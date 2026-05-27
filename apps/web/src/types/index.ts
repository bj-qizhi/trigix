export type NodeType = 'trigger' | 'http' | 'agent' | 'condition' | 'approval' | 'map' | 'filter' | 'aggregate' | 'sort' | 'transform' | 'delay' | 'sub_workflow' | 'assert' | 'catch' | 'fan_out' | 'fan_in' | 'code'

export interface ApiNode {
  id: string
  type: NodeType
  config?: Record<string, unknown> | null
}

export interface ApiEdge {
  source: string
  target: string
  condition_label?: 'true' | 'false' | 'error'
}

export interface WorkflowGraph {
  workflow_version_id: string
  nodes: ApiNode[]
  edges: ApiEdge[]
}

export interface WorkflowRecord {
  id: string
  tenant_id: string
  workspace_id: string
  project_id: string
  name: string
  status: 'draft' | 'published' | 'archived'
  latest_version_id: string | null
}

export interface WorkflowVersionRecord {
  id: string
  tenant_id: string
  workflow_id: string
  version: number
  status: 'draft' | 'published'
  graph: WorkflowGraph
}

export type ExecutionStatus = 'running' | 'waiting_approval' | 'succeeded' | 'failed' | 'cancelled'
export type NodeStatus = 'running' | 'waiting_approval' | 'succeeded' | 'failed' | 'skipped'

export interface NodeExecutionRecord {
  node_id: string
  node_type: string
  status: NodeStatus
  output_json: string | null
  error: string | null
}

export interface ExecutionRecord {
  id: string
  tenant_id: string
  workflow_id: string
  workflow_version_id: string
  status: ExecutionStatus
  node_results: NodeExecutionRecord[]
  started_at: number
  finished_at?: number
}

export interface ExecutionSummary {
  id: string
  tenant_id: string
  workflow_id: string
  workflow_version_id: string
  status: ExecutionStatus
  started_at: number
}

export interface WebhookInfo {
  token: string
  url: string
}

export interface CredentialSummary {
  id: string
  name: string
}

export interface ScheduleSummary {
  workflow_id: string
  workflow_version_id: string
  interval_secs: number
  secs_until_next_run: number
}

export interface WorkflowExport {
  name: string
  graph: WorkflowGraph
  exported_at: number
}

export interface AuditEvent {
  id: string
  tenant_id: string
  action: string
  resource_type: string
  resource_id: string
  detail: string | null
  timestamp: number
}
