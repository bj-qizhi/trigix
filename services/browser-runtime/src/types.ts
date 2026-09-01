export const actionTypes = [
  'navigate', 'click', 'input', 'wait', 'extract', 'screenshot', 'cookies',
  'upload', 'download', 'pdf', 'network', 'har', 'trace', 'page', 'evaluate',
] as const

export type BrowserActionType = typeof actionTypes[number]
export type BrowserTaskStatus = 'queued' | 'running' | 'completed' | 'failed' | 'timeout' | 'cancelled'
export type BrowserSessionStatus = 'active' | 'closing' | 'closed' | 'expired' | 'failed'
export type BrowserErrorCode =
  | 'BROWSER_INVALID_REQUEST'
  | 'BROWSER_UNAUTHORIZED'
  | 'BROWSER_URL_BLOCKED'
  | 'BROWSER_SESSION_NOT_FOUND'
  | 'BROWSER_SESSION_EXPIRED'
  | 'BROWSER_SELECTOR_NOT_FOUND'
  | 'BROWSER_NAVIGATION_FAILED'
  | 'BROWSER_ACTION_FAILED'
  | 'BROWSER_BROWSER_CRASHED'
  | 'BROWSER_TASK_TIMEOUT'
  | 'BROWSER_TASK_CANCELLED'
  | 'BROWSER_RESOURCE_LIMIT'
  | 'BROWSER_ARTIFACT_FAILED'
  | 'BROWSER_INTERNAL_ERROR'

export interface BrowserAction {
  id?: string
  type: BrowserActionType
  params: Record<string, unknown>
  timeout_ms?: number
}

export interface CreateBrowserTaskRequest {
  tenant_id: string
  workflow_id?: string
  execution_id?: string
  node_id?: string
  session_id?: string
  timeout_ms?: number
  actions: BrowserAction[]
}

export interface BrowserActionError { code: BrowserErrorCode; message: string }

export interface BrowserActionResult {
  action_id?: string
  type: BrowserActionType
  success: boolean
  started_at: string
  completed_at: string
  duration_ms: number
  data?: unknown
  artifact_ids?: string[]
  error?: BrowserActionError
}

export interface BrowserTaskResult {
  actions: BrowserActionResult[]
  final_url?: string
  title?: string
  duration_ms: number
}

export interface BrowserTask {
  id: string
  tenant_id: string
  workflow_id?: string
  execution_id?: string
  node_id?: string
  session_id?: string
  status: BrowserTaskStatus
  actions: BrowserAction[]
  timeout_ms: number
  created_at: string
  started_at?: string
  completed_at?: string
  result?: BrowserTaskResult
  error?: BrowserActionError & { action_index?: number; action_type?: BrowserActionType }
}

export interface BrowserSessionView {
  id: string
  tenant_id: string
  execution_id?: string
  status: BrowserSessionStatus
  created_at: string
  last_activity_at: string
  expires_at: string
}

export type BrowserArtifactType = 'screenshot' | 'download' | 'pdf' | 'trace' | 'har'

export interface BrowserArtifact {
  id: string
  tenant_id: string
  execution_id?: string
  task_id?: string
  type: BrowserArtifactType
  content_type: string
  size: number
  storage_key: string
  created_at: string
}

export const terminalStatuses = new Set<BrowserTaskStatus>(['completed', 'failed', 'timeout', 'cancelled'])
