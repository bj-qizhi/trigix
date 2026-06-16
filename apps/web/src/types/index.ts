// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

export type NodeType = 'trigger' | 'http' | 'agent' | 'condition' | 'approval' | 'map' | 'filter' | 'aggregate' | 'sort' | 'transform' | 'delay' | 'sub_workflow' | 'assert' | 'catch' | 'fan_out' | 'fan_in' | 'code' | 'slack' | 'email' | 'openai' | 'gemini' | 'database' | 'extract' | 'merge' | 'loop' | 'graphql' | 'validate' | 'note' | 'claude' | 'split' | 'join' | 'switch' | 'random' | 'dedupe' | 'regex' | 'csv' | 'rename' | 'format' | 'github' | 'webhook' | 'jira' | 'notion' | 'linear' | 'airtable' | 'for_each' | 'discord' | 'teams' | 'sheets' | 'xml' | 'yaml' | 'twilio' | 'stripe' | 'crypto' | 'hubspot' | 'date' | 'zendesk' | 'redis' | 'elasticsearch' | 'pagerduty' | 'handlebars' | 'math' | 'array_utils' | 'shopify' | 'datadog' | 'salesforce' | 'freshdesk' | 'mailgun' | 'asana' | 'servicenow' | 'confluence' | 'bitbucket' | 'azure_devops' | 'twitch' | 'figma' | 'dropbox' | 'cloudflare' | 'box' | 'okta' | 'zoom' | 'spotify' | 'typeform' | 'webflow' | 'intercom' | 'pipedrive' | 'trello' | 'monday' | 'clickup' | 'amplitude' | 'mixpanel' | 'segment' | 'sendgrid' | 'braintree' | 'paypal' | 'razorpay' | 'firebase' | 'supabase' | 'mailchimp' | 'activecampaign' | 'klaviyo' | 'resend' | 'contentful' | 'algolia' | 'postmark' | 'vonage' | 'telegram'
 | 'replicate' | 'mistral' | 'whatsapp' | 'googledocs'
 | 'perplexity' | 'cohere' | 'googledrive' | 'woocommerce'
 | 'pinecone' | 'togetherai' | 'awss3' | 'huggingface'
 | 'groq' | 'openrouter' | 'qdrant' | 'cloudinary'
 | 'gcal' | 'docusign' | 'xero' | 'calendly'
 | 'apify' | 'ganalytics' | 'neon' | 'copper'
 | 'azure_openai' | 'grok' | 'ollama' | 'weaviate' | 'chroma' | 'mongodb' | 'clickhouse' | 'gcs' | 'azure_blob' | 'hash' | 'jwt' | 'vertex' | 'sqs' | 'sns' | 'bedrock' | 'milvus' | 'kafka' | 'rabbitmq' | 'zip' | 'image' | 'pdf_extract' | 'ocr' | 'feishu' | 'dingtalk' | 'wecom'
 | 'embedding' | 'reranker' | 'text_splitter' | 'structured_output' | 'classifier' | 'image_gen' | 'speech_to_text' | 'tts'
 | 'deepseek' | 'qwen' | 'zhipu' | 'moonshot'
 | 'doubao' | 'minimax' | 'ernie' | 'hunyuan'
 | 'rag' | 'rag_ingest' | 'custom'

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

export interface InputField {
  key: string
  field_type: 'string' | 'number' | 'boolean' | 'json'
  required: boolean
  description: string
  default_value?: string
}

export interface WorkflowGraph {
  workflow_version_id: string
  nodes: ApiNode[]
  edges: ApiEdge[]
  input_schema?: InputField[]
}

export interface WorkflowRecord {
  tags?: string[]
  description?: string
  pinned?: boolean
  readme?: string
  folder?: string
  locked?: boolean
  created_by?: string
  visibility?: 'tenant' | 'private'
  id: string
  tenant_id: string
  workspace_id: string
  project_id: string
  name: string
  status: 'draft' | 'published' | 'archived'
  latest_version_id: string | null
  updated_at?: number
  created_at?: number
  sla_seconds?: number
  max_runs_per_hour?: number
  max_concurrent_runs?: number
  budget_usd?: number
}

export interface WorkflowVersionRecord {
  id: string
  tenant_id: string
  workflow_id: string
  version: number
  status: 'draft' | 'published'
  graph: WorkflowGraph
  message?: string
}

export type ExecutionStatus = 'running' | 'waiting_approval' | 'succeeded' | 'failed' | 'cancelled'
export type NodeStatus = 'running' | 'waiting_approval' | 'succeeded' | 'failed' | 'skipped'

export interface NodeExecutionRecord {
  node_id: string
  node_type: string
  status: NodeStatus
  output_json: string | null
  error: string | null
  duration_ms?: number
  started_at_ms?: number
  retry_count?: number
}

export interface ExecutionRecord {
  id: string
  tenant_id: string
  workflow_id: string
  workflow_version_id: string
  status: ExecutionStatus
  node_results: NodeExecutionRecord[]
  input_json?: string | null
  output_json?: string | null
  started_at: number
  finished_at?: number
  label?: string
  trigger_type?: 'manual' | 'webhook' | 'schedule' | 'retry'
  dry_run?: boolean
  note?: string | null
  starred?: boolean
  node_count?: number
  completed_node_count?: number
  retried_from?: string
  graph?: WorkflowGraph
}

export interface ExecutionSummary {
  id: string
  tenant_id: string
  workflow_id: string
  workflow_version_id: string
  status: ExecutionStatus
  started_at: number
  finished_at?: number
  label?: string
  trigger_type?: 'manual' | 'webhook' | 'schedule' | 'retry'
  dry_run?: boolean
  starred?: boolean
  node_count?: number
  completed_node_count?: number
  retried_from?: string
}

export interface WebhookInfo {
  token: string
  url: string
  secret?: string
}

export interface WebhookRecord {
  token: string
  tenant_id: string
  workflow_id: string
  workflow_version_id: string
  secret?: string
  condition_expr?: string
  max_calls_per_minute?: number
  paused?: boolean
  payload_transform_script?: string
}

export interface CredentialSummary {
  id: string
  name: string
  description?: string
  expires_at?: number
  created_at: number
  updated_at: number
}

export interface ScheduleSummary {
  workflow_id: string
  workflow_version_id: string
  interval_secs: number
  cron_expression?: string
  secs_until_next_run: number
  paused: boolean
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

export interface EnvVarRecord {
  key: string
  value: string
}

export interface EnvSetSummary {
  name: string
  var_count: number
}

export interface WorkspaceRecord {
  id: string
  tenant_id: string
  name: string
  description?: string | null
}

export interface ProjectRecord {
  id: string
  tenant_id: string
  workspace_id: string
  name: string
  description?: string | null
}

export interface WorkflowComment {
  id: string
  tenant_id: string
  workflow_id: string
  author: string
  body: string
  created_at: number
  edited_at?: number | null
}

export type EventType = 'execution.started' | 'execution.completed' | 'execution.failed' | 'execution.cancelled'

export interface EventSubscription {
  id: string
  tenant_id: string
  url: string
  events: EventType[]
  created_at: number
  description?: string
}
