// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type MutableRefObject } from 'react'
import {
  ReactFlow,
  Background,
  Controls,
  MiniMap,
  addEdge,
  useNodesState,
  useEdgesState,
  useReactFlow,
  Handle,
  Position,
  BackgroundVariant,
  type Node,
  type Edge,
  type Connection,
  type NodeProps,
} from '@xyflow/react'
import '@xyflow/react/dist/style.css'
import type { ApiNode, ApiEdge, NodeType, NodeExecutionRecord } from '../types'
import { NodeIcon } from './nodeIcons'
import { nodePreview } from './nodePreview'
import { labelLocale } from './panels/i18nLabels'

const NodeStatusContext = createContext<Record<string, NodeExecutionRecord>>({})
const NodeWarningContext = createContext<Set<string>>(new Set())

// ── Data shapes ──────────────────────────────────────────────────────────────

export interface FlowNodeData extends Record<string, unknown> {
  label: string
  nodeType: NodeType
  config: Record<string, unknown>
}

export type FlowNode = Node<FlowNodeData>
export type FlowEdge = Edge<{ conditionLabel?: string }>

// ── Layout ────────────────────────────────────────────────────────────────────

function computePositions(
  nodes: ApiNode[],
  edges: ApiEdge[],
): Record<string, { x: number; y: number }> {
  const outgoing: Record<string, string[]> = {}
  const indegree: Record<string, number> = {}
  for (const n of nodes) { outgoing[n.id] = []; indegree[n.id] = 0 }
  for (const e of edges) {
    outgoing[e.source]?.push(e.target)
    if (indegree[e.target] !== undefined) indegree[e.target]++
  }

  const levels: Record<string, number> = {}
  const queue = nodes.filter((n) => indegree[n.id] === 0).map((n) => n.id)
  for (const id of queue) levels[id] = 0

  let i = 0
  while (i < queue.length) {
    const id = queue[i++]
    for (const t of outgoing[id] ?? []) {
      levels[t] = Math.max(levels[t] ?? 0, (levels[id] ?? 0) + 1)
      queue.push(t)
    }
  }

  const byLevel: Record<number, string[]> = {}
  for (const [id, lv] of Object.entries(levels)) {
    ;(byLevel[lv] ??= []).push(id)
  }

  const positions: Record<string, { x: number; y: number }> = {}
  const X = 280, Y = 120
  for (const [lvStr, ids] of Object.entries(byLevel)) {
    const lv = Number(lvStr)
    ids.sort().forEach((id, idx) => {
      positions[id] = { x: lv * X + 80, y: (idx - (ids.length - 1) / 2) * Y + 220 }
    })
  }
  return positions
}

// ── Format conversion ─────────────────────────────────────────────────────────

export function toFlowNodes(
  apiNodes: ApiNode[],
  positions: Record<string, { x: number; y: number }>,
): FlowNode[] {
  return apiNodes.map((n, idx) => ({
    id: n.id,
    type: n.type,
    position: positions[n.id] ?? { x: (idx % 4) * 280 + 80, y: Math.floor(idx / 4) * 120 + 80 },
    data: {
      label: n.id,
      nodeType: n.type,
      config: (n.config as Record<string, unknown>) ?? {},
    },
  }))
}

export function toFlowEdges(apiEdges: ApiEdge[]): FlowEdge[] {
  return apiEdges.map((e) => ({
    id: `e-${e.source}-${e.target}`,
    source: e.source,
    target: e.target,
    label: e.condition_label ?? undefined,
    data: { conditionLabel: e.condition_label },
    style: { stroke: '#30363d', strokeWidth: 2 },
    labelStyle: { fill: '#8b949e', fontSize: 11, fontWeight: 600 },
    labelBgStyle: { fill: '#21262d', fillOpacity: 0.9 },
    labelBgPadding: [4, 6] as [number, number],
    labelBgBorderRadius: 4,
  }))
}

export function fromFlowGraph(
  nodes: FlowNode[],
  edges: FlowEdge[],
): { nodes: ApiNode[]; edges: ApiEdge[] } {
  return {
    nodes: nodes.map((n) => ({
      id: n.id,
      type: n.data.nodeType,
      config: Object.keys(n.data.config).length > 0 ? n.data.config : undefined,
    })),
    edges: edges.map((e) => ({
      source: e.source,
      target: e.target,
      ...(e.data?.conditionLabel ? { condition_label: e.data.conditionLabel as 'true' | 'false' | 'error' } : {}),
    })),
  }
}

export function graphFromApi(
  apiNodes: ApiNode[],
  apiEdges: ApiEdge[],
): { nodes: FlowNode[]; edges: FlowEdge[] } {
  const positions = computePositions(apiNodes, apiEdges)
  return { nodes: toFlowNodes(apiNodes, positions), edges: toFlowEdges(apiEdges) }
}

// ── Custom node component ─────────────────────────────────────────────────────

// Per-node accent colour (drives minimap dots and the node header background).
export const NODE_COLORS: Record<string, string> = {
              trigger: '#238636',
              http: '#1f6feb',
              agent: '#8957e5',
              condition: '#d29922',
              approval: '#0891b2',
              map: '#e05d44',
              filter: '#0891b2',
              aggregate: '#7c3aed',
              sort: '#d97706',
              transform: '#0d9488',
              delay: '#b45309',
              sub_workflow: '#be185d',
              assert: '#dc2626',
              catch: '#ea580c',
              fan_out: '#0891b2',
              fan_in: '#0891b2',
              code: '#7c3aed',
              slack: '#4a154b',
              email: '#0369a1',
              openai: '#10a37f',
              rag: '#7c3aed',
              rag_ingest: '#7c3aed',
              custom: '#475569',
              gemini: '#4285f4',
              database: '#336791',
              extract: '#0f766e',
              merge: '#7c2d12',
              loop: '#6d28d9',
              graphql: '#e10098',
              validate: '#15803d',
              note: '#b45309',
              claude: '#c96442',
              split: '#0369a1',
              join: '#0891b2',
              switch: '#7c3aed',
              random: '#0f766e',
              dedupe: '#1d4ed8',
              regex: '#92400e',
              csv: '#166534',
              rename: '#065f46',
              format: '#1e3a5f',
              github: '#24292e',
              webhook: '#6d28d9',
              jira: '#0052cc',
              notion: '#000000',
              linear: '#5e6ad2',
              airtable: '#18bfff',
              for_each: '#7c3aed',
              discord: '#5865f2',
              teams: '#6264a7',
              sheets: '#0f9d58',
              xml: '#7b341e',
              yaml: '#2c5282',
              twilio: '#f22f46',
              stripe: '#635bff',
              crypto: '#134e4a',
              hubspot: '#ff5c35',
              date: '#0369a1',
              zendesk: '#03363d',
              redis: '#dc2626',
              elasticsearch: '#f59e0b',
              pagerduty: '#06b6d4',
              handlebars: '#7c3aed',
              math: '#0e7490',
              array_utils: '#065f46',
              shopify: '#96bf48',
              datadog: '#632ca6',
              salesforce: '#00a1e0',
              freshdesk: '#25c16f',
              mailgun: '#f06b26',
              asana: '#f06a6a',
              servicenow: '#81b5a1',
              confluence: '#0052cc',
              bitbucket: '#0052cc',
              azure_devops: '#0078d4',
              twitch: '#9146ff',
              figma: '#f24e1e',
              dropbox: '#0061ff',
              cloudflare: '#f38020',
              box: '#0061fe',
              okta: '#007dc1',
              zoom: '#2d8cff',
              spotify: '#1db954',
              typeform: '#262627',
              webflow: '#4353ff',
              intercom: '#1f8ded',
              pipedrive: '#d4452c',
              trello: '#0052cc',
              monday: '#ff3d57',
              clickup: '#7b68ee',
              amplitude: '#1da462',
              mixpanel: '#7856ff',
              segment: '#52bd95',
              sendgrid: '#1a82e2',
              braintree: '#009cde',
              paypal: '#003087',
              razorpay: '#3395ff',
              firebase: '#ff6d00',
              supabase: '#3ecf8e',
              mailchimp: '#ffe01b',
              activecampaign: '#356ae6',
              klaviyo: '#1a1a1a',
              resend: '#000000',
              contentful: '#2478cc',
              algolia: '#003dff',
              postmark: '#ffdd00',
              vonage: '#9b59b6',
              telegram: '#2ca5e0',
              replicate: '#000000',
              mistral: '#ff7000',
              whatsapp: '#25d366',
              googledocs: '#4285f4',
              perplexity: '#20b2aa',
              cohere: '#d4b896',
              googledrive: '#1fa463',
              woocommerce: '#7f54b3',
              pinecone: '#1a1a2e',
              togetherai: '#3d5af1',
              awss3: '#ff9900',
              huggingface: '#ff9d00',
              groq: '#f55036',
              openrouter: '#6467f2',
              qdrant: '#dc244c',
              cloudinary: '#3448c5',
              gcal: '#1a73e8',
              docusign: '#ff5400',
              xero: '#1ab4d7',
              calendly: '#006bff',
              apify: '#00c4b4',
              ganalytics: '#e37400',
              neon: '#00e599',
              copper: '#e8762b',
              azure_openai: '#0078d4',
              grok: '#111827',
              ollama: '#0ea5e9',
              weaviate: '#00c9a7',
              chroma: '#ff6b6b',
              mongodb: '#13aa52',
              clickhouse: '#ffcc00',
              gcs: '#4285f4',
              azure_blob: '#0078d4',
              hash: '#134e4a',
              jwt: '#134e4a',
              vertex: '#4285f4',
              sqs: '#ff4f8b',
              sns: '#ff9900',
              bedrock: '#ff9900',
              milvus: '#00a1ea',
              kafka: '#231f20',
              rabbitmq: '#ff6600',
              zip: '#0d9488',
              image: '#0d9488',
              pdf_extract: '#0d9488',
              ocr: '#0d9488',
              feishu: '#00d6b9',
              dingtalk: '#1296db',
              wecom: '#2f90eb',
              embedding: '#10a37f',
              reranker: '#d4b896',
              text_splitter: '#0d9488',
              structured_output: '#10a37f',
              classifier: '#10a37f',
              image_gen: '#10a37f',
              video_gen: '#10a37f',
              speech_to_text: '#10a37f',
              tts: '#10a37f',
              html_extract: '#0d9488',
              rss: '#0d9488',
              mysql: '#00758f',
              snowflake: '#29b5e8',
              bigquery: '#4285f4',
              sqlserver: '#a91d22',
              ftp: '#8b5cf6',
              sftp: '#7c3aed',
              ssh: '#334155',
              imap: '#be123c',
              wait: '#0891b2',
              deepseek: '#4d6bfe',
              qwen: '#6200ea',
              zhipu: '#00897b',
              moonshot: '#1a237e',
              doubao: '#0078ff',
              minimax: '#ff6f00',
              ernie: '#2979ff',
              hunyuan: '#00bcd4',
            }

const NODE_LABELS: Record<NodeType, string> = {
  trigger: 'Trigger',
  http: 'HTTP',
  agent: 'Agent',
  condition: 'Condition',
  approval: 'Approval',
  map: 'Map',
  filter: 'Filter',
  aggregate: 'Aggregate',
  sort: 'Sort',
  transform: 'Transform',
  delay: 'Delay',
  sub_workflow: 'Sub-Workflow',
  assert: 'Assert',
  catch: 'Catch',
  fan_out: 'Fan-Out',
  fan_in: 'Fan-In',
  code: 'Code',
  slack: 'Slack',
  email: 'Email',
  openai: 'OpenAI',
  rag: 'RAG',
  rag_ingest: 'RAG Ingest',
  custom: 'Custom',
  gemini: 'Gemini',
  database: 'Database',
  extract: 'Extract',
  merge: 'Merge',
  loop: 'Loop',
  graphql: 'GraphQL',
  validate: 'Validate',
  note: 'Note',
  claude: 'Claude',
  split: 'Split',
  join: 'Join',
  switch: 'Switch',
  random: 'Random',
  dedupe: 'Dedupe',
  regex: 'Regex',
  csv: 'CSV Parse',
  rename: 'Rename',
  format: 'Format',
  github: 'GitHub',
  webhook: 'Webhook Send',
  jira: 'Jira',
  notion: 'Notion',
  linear: 'Linear',
  airtable: 'Airtable',
  for_each: 'For Each',
  discord: 'Discord',
  teams: 'MS Teams',
  sheets: 'Google Sheets',
  xml: 'XML Parse',
  yaml: 'YAML',
  twilio: 'Twilio SMS',
  stripe: 'Stripe',
  crypto: 'Crypto',
  hubspot: 'HubSpot',
  date: 'Date/Time',
  zendesk: 'Zendesk',
  redis: 'Redis Cache',
  elasticsearch: 'Elasticsearch',
  pagerduty: 'PagerDuty',
  handlebars: 'HB Template',
  math: 'Math',
  array_utils: 'Array Utils',
  shopify: 'Shopify',
  datadog: 'Datadog',
  salesforce: 'Salesforce',
  freshdesk: 'Freshdesk',
  mailgun: 'Mailgun',
  asana: 'Asana',
  servicenow: 'ServiceNow',
  confluence: 'Confluence',
  bitbucket: 'Bitbucket',
  azure_devops: 'Azure DevOps',
  twitch: 'Twitch',
  figma: 'Figma',
  dropbox: 'Dropbox',
  cloudflare: 'Cloudflare',
  box: 'Box',
  okta: 'Okta',
  zoom: 'Zoom',
  spotify: 'Spotify',
  typeform: 'Typeform',
  webflow: 'Webflow',
  intercom: 'Intercom',
  pipedrive: 'Pipedrive',
  trello: 'Trello',
  monday: 'Monday',
  clickup: 'ClickUp',
  amplitude: 'Amplitude',
  mixpanel: 'Mixpanel',
  segment: 'Segment',
  sendgrid: 'SendGrid',
  braintree: 'Braintree',
  paypal: 'PayPal',
  razorpay: 'Razorpay',
  firebase: 'Firebase',
  supabase: 'Supabase',
  mailchimp: 'Mailchimp',
  activecampaign: 'ActiveCampaign',
  klaviyo: 'Klaviyo',
  resend: 'Resend',
  contentful: 'Contentful',
  algolia: 'Algolia',
  postmark: 'Postmark',
  vonage: 'Vonage',
  telegram: 'Telegram',
  replicate: 'Replicate',
  mistral: 'Mistral',
  whatsapp: 'WhatsApp',
  googledocs: 'Google Docs',
  perplexity: 'Perplexity',
  cohere: 'Cohere',
  googledrive: 'Google Drive',
  woocommerce: 'WooCommerce',
  pinecone: 'Pinecone',
  togetherai: 'Together AI',
  awss3: 'AWS S3',
  huggingface: 'Hugging Face',
  groq: 'Groq',
  openrouter: 'OpenRouter',
  qdrant: 'Qdrant',
  cloudinary: 'Cloudinary',
  gcal: 'Google Calendar',
  docusign: 'DocuSign',
  xero: 'Xero',
  calendly: 'Calendly',
  apify: 'Apify',
  ganalytics: 'Google Analytics',
  neon: 'Neon',
  copper: 'Copper CRM',
  azure_openai: 'Azure OpenAI',
  grok: 'xAI Grok',
  ollama: 'Ollama',
  weaviate: 'Weaviate',
  chroma: 'Chroma',
  mongodb: 'MongoDB',
  clickhouse: 'ClickHouse',
  gcs: 'Cloud Storage',
  azure_blob: 'Azure Blob',
  hash: 'Hash / HMAC',
  jwt: 'JWT',
  vertex: 'Vertex AI',
  sqs: 'AWS SQS',
  sns: 'AWS SNS',
  bedrock: 'AWS Bedrock',
  milvus: 'Milvus',
  kafka: 'Kafka',
  rabbitmq: 'RabbitMQ',
  zip: 'Zip',
  image: 'Image',
  pdf_extract: 'PDF Extract',
  ocr: 'OCR',
  feishu: '飞书 / Lark',
  dingtalk: '钉钉',
  wecom: '企业微信',
  embedding: 'Embedding',
  reranker: 'Reranker',
  text_splitter: 'Text Splitter',
  structured_output: 'Structured Output',
  classifier: 'Classifier',
  image_gen: 'Image Gen',
  video_gen: 'Video Gen',
  speech_to_text: 'Speech → Text',
  tts: 'Text → Speech',
  html_extract: 'HTML Extract',
  rss: 'RSS Feed',
  mysql: 'MySQL',
  snowflake: 'Snowflake',
  bigquery: 'BigQuery',
  sqlserver: 'SQL Server',
  ftp: 'FTP',
  sftp: 'SFTP',
  ssh: 'SSH',
  imap: 'IMAP',
  wait: 'Wait',
  deepseek: 'DeepSeek',
  qwen: '通义千问',
  zhipu: '智谱 GLM',
  moonshot: 'Moonshot (Kimi)',
  doubao: '豆包',
  minimax: 'MiniMax',
  ernie: '文心一言',
  hunyuan: '混元',
}

// Chinese titles for the *generic* node types. Brand/product nodes (Slack,
// GitHub, OpenAI, MySQL, …) are intentionally absent — they keep their English
// names, which match their docs. Falls back to NODE_LABELS when not present.
const NODE_LABELS_ZH: Partial<Record<NodeType, string>> = {
  trigger: '触发器',
  agent: 'AI 智能体',
  condition: '条件分支',
  approval: '人工审批',
  map: '映射',
  filter: '过滤',
  aggregate: '聚合',
  sort: '排序',
  transform: '转换',
  delay: '延时',
  sub_workflow: '子工作流',
  assert: '断言',
  catch: '错误捕获',
  fan_out: '并行分发',
  fan_in: '并行汇总',
  code: '代码',
  email: '邮件',
  rag: 'RAG 检索',
  rag_ingest: 'RAG 入库',
  custom: '自定义',
  database: '数据库',
  extract: '字段提取',
  merge: '合并',
  loop: '循环',
  validate: '校验',
  note: '注释',
  split: '拆分',
  join: '拼接',
  switch: '多路分支',
  random: '随机',
  dedupe: '去重',
  regex: '正则',
  csv: 'CSV 解析',
  rename: '重命名',
  format: '格式化',
  webhook: 'Webhook 发送',
  for_each: '遍历执行',
  xml: 'XML 解析',
  crypto: '加密',
  date: '日期/时间',
  math: '数学运算',
  array_utils: '数组工具',
  handlebars: 'HB 模板',
  redis: 'Redis 缓存',
  hash: '哈希 / HMAC',
  zip: '压缩包',
  image: '图片',
  pdf_extract: 'PDF 提取',
  ocr: 'OCR',
  embedding: '嵌入向量',
  reranker: '重排序',
  text_splitter: '文本分块',
  structured_output: '结构化输出',
  classifier: '分类器',
  image_gen: '图像生成',
  video_gen: '视频生成',
  speech_to_text: '语音转文字',
  tts: '文字转语音',
  html_extract: 'HTML 提取',
  rss: 'RSS 订阅',
  wait: '等待',
}

/** Display title for a node type, localized to the active editor locale. */
export function nodeTitle(nt: NodeType): string {
  if (labelLocale() === 'zh' && NODE_LABELS_ZH[nt]) return NODE_LABELS_ZH[nt]!
  return NODE_LABELS[nt] ?? nt
}


function FlowNodeComponent({ data, selected, id }: NodeProps) {
  const statuses = useContext(NodeStatusContext)
  const warnings = useContext(NodeWarningContext)
  const execResult = statuses[id]
  const hasWarning = warnings.has(id)
  const d = data as FlowNodeData
  const nt = d.nodeType
  const label = (d.config?.node_label as string | undefined) || (nt ? nodeTitle(nt) : 'Node')

  const preview = nodePreview(nt, d.config ?? {})

  const execClass = execResult ? `exec-${execResult.status}` : ''

  const DOT_COLORS: Record<string, string> = {
    succeeded: 'var(--success-text)',
    failed: 'var(--danger-text)',
    running: 'var(--link)',
    waiting_approval: 'var(--approval-text)',
    skipped: 'var(--muted)',
  }

  return (
    <div className={`flow-node flow-node-${nt ?? 'unknown'} ${selected ? 'selected' : ''} ${execClass}`}>
      {execResult && (
        <span
          className="flow-node-status-dot"
          style={{ background: DOT_COLORS[execResult.status] ?? 'var(--muted)' }}
        />
      )}
      {nt !== 'trigger' && (
        <Handle type="target" position={Position.Left} style={{ background: '#30363d' }} />
      )}
      <div className="flow-node-header" style={{ background: nt ? (NODE_COLORS[nt] ?? 'var(--accent)') : 'var(--accent)' }}>
        <span className="flow-node-ic">{nt ? <NodeIcon type={nt} size={15} /> : null}</span>
        <span>{label}</span>
        <span style={{ opacity: 0.6, fontWeight: 400, fontSize: 11, marginLeft: 'auto' }}>{id}</span>
        {hasWarning && (
          <span title="Missing required config" style={{ color: '#fbbf24', fontSize: 12, marginLeft: 4 }}>⚠</span>
        )}
      </div>
      <div className="flow-node-body">
        {preview || <span style={{ opacity: 0.4 }}>No config</span>}
      </div>
      <Handle type="source" position={Position.Right} style={{ background: '#30363d' }} />
    </div>
  )
}

const nodeTypes = {
  trigger: FlowNodeComponent,
  http: FlowNodeComponent,
  agent: FlowNodeComponent,
  condition: FlowNodeComponent,
  approval: FlowNodeComponent,
  map: FlowNodeComponent,
  filter: FlowNodeComponent,
  aggregate: FlowNodeComponent,
  sort: FlowNodeComponent,
  transform: FlowNodeComponent,
  delay: FlowNodeComponent,
  sub_workflow: FlowNodeComponent,
  assert: FlowNodeComponent,
  catch: FlowNodeComponent,
  fan_out: FlowNodeComponent,
  fan_in: FlowNodeComponent,
  code: FlowNodeComponent,
  slack: FlowNodeComponent,
  email: FlowNodeComponent,
  openai: FlowNodeComponent,
  rag: FlowNodeComponent,
  rag_ingest: FlowNodeComponent,
  custom: FlowNodeComponent,
  gemini: FlowNodeComponent,
  database: FlowNodeComponent,
  extract: FlowNodeComponent,
  merge: FlowNodeComponent,
  loop: FlowNodeComponent,
  graphql: FlowNodeComponent,
  validate: FlowNodeComponent,
  note: FlowNodeComponent,
  claude: FlowNodeComponent,
  split: FlowNodeComponent,
  join: FlowNodeComponent,
  switch: FlowNodeComponent,
  random: FlowNodeComponent,
  dedupe: FlowNodeComponent,
  regex: FlowNodeComponent,
  csv: FlowNodeComponent,
  rename: FlowNodeComponent,
  format: FlowNodeComponent,
  github: FlowNodeComponent,
  webhook: FlowNodeComponent,
  jira: FlowNodeComponent,
  notion: FlowNodeComponent,
  linear: FlowNodeComponent,
  airtable: FlowNodeComponent,
  for_each: FlowNodeComponent,
  discord: FlowNodeComponent,
  teams: FlowNodeComponent,
  sheets: FlowNodeComponent,
  xml: FlowNodeComponent,
  yaml: FlowNodeComponent,
  twilio: FlowNodeComponent,
  stripe: FlowNodeComponent,
  crypto: FlowNodeComponent,
  hubspot: FlowNodeComponent,
  date: FlowNodeComponent,
  zendesk: FlowNodeComponent,
  redis: FlowNodeComponent,
  elasticsearch: FlowNodeComponent,
  pagerduty: FlowNodeComponent,
  handlebars: FlowNodeComponent,
  math: FlowNodeComponent,
  array_utils: FlowNodeComponent,
  shopify: FlowNodeComponent,
  datadog: FlowNodeComponent,
  salesforce: FlowNodeComponent,
  freshdesk: FlowNodeComponent,
  mailgun: FlowNodeComponent,
  asana: FlowNodeComponent,
  servicenow: FlowNodeComponent,
  confluence: FlowNodeComponent,
  bitbucket: FlowNodeComponent,
  azure_devops: FlowNodeComponent,
  twitch: FlowNodeComponent,
  figma: FlowNodeComponent,
  dropbox: FlowNodeComponent,
  cloudflare: FlowNodeComponent,
  box: FlowNodeComponent,
  okta: FlowNodeComponent,
  zoom: FlowNodeComponent,
  spotify: FlowNodeComponent,
  typeform: FlowNodeComponent,
  webflow: FlowNodeComponent,
  intercom: FlowNodeComponent,
  pipedrive: FlowNodeComponent,
  trello: FlowNodeComponent,
  monday: FlowNodeComponent,
  clickup: FlowNodeComponent,
  amplitude: FlowNodeComponent,
  mixpanel: FlowNodeComponent,
  segment: FlowNodeComponent,
  sendgrid: FlowNodeComponent,
  braintree: FlowNodeComponent,
  paypal: FlowNodeComponent,
  razorpay: FlowNodeComponent,
  firebase: FlowNodeComponent,
  supabase: FlowNodeComponent,
  mailchimp: FlowNodeComponent,
  activecampaign: FlowNodeComponent,
  klaviyo: FlowNodeComponent,
  resend: FlowNodeComponent,
  contentful: FlowNodeComponent,
  algolia: FlowNodeComponent,
  postmark: FlowNodeComponent,
  vonage: FlowNodeComponent,
  telegram: FlowNodeComponent,
  replicate: FlowNodeComponent,
  mistral: FlowNodeComponent,
  whatsapp: FlowNodeComponent,
  googledocs: FlowNodeComponent,
  perplexity: FlowNodeComponent,
  cohere: FlowNodeComponent,
  googledrive: FlowNodeComponent,
  woocommerce: FlowNodeComponent,
  pinecone: FlowNodeComponent,
  togetherai: FlowNodeComponent,
  awss3: FlowNodeComponent,
  huggingface: FlowNodeComponent,
  groq: FlowNodeComponent,
  openrouter: FlowNodeComponent,
  qdrant: FlowNodeComponent,
  cloudinary: FlowNodeComponent,
  gcal: FlowNodeComponent,
  docusign: FlowNodeComponent,
  xero: FlowNodeComponent,
  calendly: FlowNodeComponent,
  apify: FlowNodeComponent,
  ganalytics: FlowNodeComponent,
  neon: FlowNodeComponent,
  copper: FlowNodeComponent,
  azure_openai: FlowNodeComponent,
  grok: FlowNodeComponent,
  ollama: FlowNodeComponent,
  weaviate: FlowNodeComponent,
  chroma: FlowNodeComponent,
  mongodb: FlowNodeComponent,
  clickhouse: FlowNodeComponent,
  gcs: FlowNodeComponent,
  azure_blob: FlowNodeComponent,
  hash: FlowNodeComponent,
  jwt: FlowNodeComponent,
  vertex: FlowNodeComponent,
  sqs: FlowNodeComponent,
  sns: FlowNodeComponent,
  bedrock: FlowNodeComponent,
  milvus: FlowNodeComponent,
  kafka: FlowNodeComponent,
  rabbitmq: FlowNodeComponent,
  zip: FlowNodeComponent,
  image: FlowNodeComponent,
  pdf_extract: FlowNodeComponent,
  ocr: FlowNodeComponent,
  feishu: FlowNodeComponent,
  dingtalk: FlowNodeComponent,
  wecom: FlowNodeComponent,
  embedding: FlowNodeComponent,
  reranker: FlowNodeComponent,
  text_splitter: FlowNodeComponent,
  structured_output: FlowNodeComponent,
  classifier: FlowNodeComponent,
  image_gen: FlowNodeComponent,
  video_gen: FlowNodeComponent,
  speech_to_text: FlowNodeComponent,
  tts: FlowNodeComponent,
  html_extract: FlowNodeComponent,
  rss: FlowNodeComponent,
  mysql: FlowNodeComponent,
  snowflake: FlowNodeComponent,
  bigquery: FlowNodeComponent,
  sqlserver: FlowNodeComponent,
  ftp: FlowNodeComponent,
  sftp: FlowNodeComponent,
  ssh: FlowNodeComponent,
  imap: FlowNodeComponent,
  wait: FlowNodeComponent,
  deepseek: FlowNodeComponent,
  qwen: FlowNodeComponent,
  zhipu: FlowNodeComponent,
  moonshot: FlowNodeComponent,
  doubao: FlowNodeComponent,
  minimax: FlowNodeComponent,
  ernie: FlowNodeComponent,
  hunyuan: FlowNodeComponent,
}

// ── Canvas component ──────────────────────────────────────────────────────────

interface Props {
  initialNodes: FlowNode[]
  initialEdges: FlowEdge[]
  selectedNodeId: string | null
  onSelectionChange: (nodeId: string | null) => void
  onNodesUpdated: (nodes: FlowNode[]) => void
  onEdgesUpdated: (edges: FlowEdge[]) => void
  onDropNode?: (type: NodeType, position: { x: number; y: number }) => void
  nodeStatuses?: Record<string, NodeExecutionRecord>
  warningNodeIds?: Set<string>
  snapToGrid?: boolean
  showMinimap?: boolean
  fitViewRef?: MutableRefObject<(() => void) | null>
  fitToNodeRef?: MutableRefObject<((nodeId: string) => void) | null>
  highlightedNodeIds?: Set<string>
  bgVariant?: 'dots' | 'grid' | 'lines' | 'none'
  nodeHeatmapMap?: Map<string, string>
  defaultViewport?: { x: number; y: number; zoom: number }
  onViewportChange?: (vp: { x: number; y: number; zoom: number }) => void
}

function FitViewBridge({
  fitViewRef,
  fitToNodeRef,
}: {
  fitViewRef: MutableRefObject<(() => void) | null>
  fitToNodeRef?: MutableRefObject<((nodeId: string) => void) | null>
}) {
  const { fitView } = useReactFlow()
  useEffect(() => {
    fitViewRef.current = () => fitView({ padding: 0.15 })
    if (fitToNodeRef) fitToNodeRef.current = (id: string) => fitView({ nodes: [{ id }], padding: 0.5, maxZoom: 1.5 })
  }, [fitView, fitViewRef, fitToNodeRef])
  return null
}

export function Canvas({
  initialNodes,
  initialEdges,
  selectedNodeId,
  onSelectionChange,
  onNodesUpdated,
  onEdgesUpdated,
  onDropNode,
  nodeStatuses = {},
  warningNodeIds,
  snapToGrid = false,
  showMinimap = true,
  fitViewRef,
  fitToNodeRef,
  highlightedNodeIds,
  bgVariant = 'dots',
  nodeHeatmapMap,
  defaultViewport,
  onViewportChange,
}: Props) {
  const [nodes, setNodes, onNodesChangeRaw] = useNodesState<FlowNode>(initialNodes)
  const [edges, setEdges, onEdgesChange] = useEdgesState(initialEdges)
  const [connError, setConnError] = useState<string | null>(null)
  const connErrorTimer = useRef<ReturnType<typeof setTimeout> | null>(null)

  const showConnError = useCallback((msg: string) => {
    setConnError(msg)
    if (connErrorTimer.current) clearTimeout(connErrorTimer.current)
    connErrorTimer.current = setTimeout(() => setConnError(null), 2500)
  }, [])

  // Palette → canvas drag-and-drop. The RF instance (captured via onInit) gives
  // screenToFlowPosition so the node lands under the cursor.
  const rfInstance = useRef<{ screenToFlowPosition: (p: { x: number; y: number }) => { x: number; y: number } } | null>(null)
  const onDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    e.dataTransfer.dropEffect = 'move'
  }, [])
  const onDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    const type = e.dataTransfer.getData('application/trigix-node') as NodeType
    if (!type || !onDropNode || !rfInstance.current) return
    const position = rfInstance.current.screenToFlowPosition({ x: e.clientX, y: e.clientY })
    onDropNode(type, position)
  }, [onDropNode])

  const onNodesChange = useCallback(
    (changes: import('@xyflow/react').NodeChange<FlowNode>[]) => {
      const removals = changes.filter((c) => c.type === 'remove')
      if (removals.length > 0) {
        const connected = removals.filter((c) =>
          edges.some((e) => e.source === c.id || e.target === c.id)
        )
        if (connected.length > 0) {
          const names = connected.map((c) => c.id).join(', ')
          if (!window.confirm(`Delete node${connected.length > 1 ? 's' : ''} "${names}"? This will also remove connected edges.`)) {
            return
          }
        }
      }
      onNodesChangeRaw(changes)
    },
    [onNodesChangeRaw, edges],
  )

  // Sync nodes from parent (palette adds, version loads).
  // Kept separate from edges so node drags don't reset edge state.
  useEffect(() => {
    setNodes(initialNodes)
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [initialNodes])

  // Sync edges only when the parent explicitly provides a new edge set (version load).
  useEffect(() => {
    setEdges(initialEdges)
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [initialEdges])

  useEffect(() => {
    onNodesUpdated(nodes)
  }, [nodes, onNodesUpdated])

  useEffect(() => {
    onEdgesUpdated(edges as FlowEdge[])
  }, [edges, onEdgesUpdated])

  // When drawing a new edge from a condition node, auto-assign true/false label.
  const onConnect = useCallback(
    (connection: Connection) => {
      // Validate: no self-loops
      if (connection.source === connection.target) {
        showConnError('Cannot connect a node to itself.')
        return
      }
      // Validate: no duplicate edges (same source→target)
      const duplicate = edges.some(
        (e) => e.source === connection.source && e.target === connection.target,
      )
      if (duplicate) {
        showConnError('A connection between these nodes already exists.')
        return
      }
      // Validate: trigger node cannot have incoming edges
      const targetNode = nodes.find((n) => n.id === connection.target)
      if (targetNode?.data.nodeType === 'trigger') {
        showConnError('Trigger nodes cannot have incoming connections.')
        return
      }
      // Validate: adding this edge would not create a cycle (DFS reachability from target back to source)
      if (connection.source && connection.target) {
        const adj = new Map<string, string[]>()
        for (const e of edges) { adj.set(e.source, [...(adj.get(e.source) ?? []), e.target]) }
        const visited = new Set<string>()
        const hasCycle = (node: string): boolean => {
          if (node === connection.source) return true
          if (visited.has(node)) return false
          visited.add(node)
          return (adj.get(node) ?? []).some(hasCycle)
        }
        if (hasCycle(connection.target)) {
          showConnError('This connection would create a cycle.')
          return
        }
      }
      const sourceNode = nodes.find((n) => n.id === connection.source)
      const isCondition = sourceNode?.data.nodeType === 'condition'
      const isCatchTarget = targetNode?.data.nodeType === 'catch'
      let conditionLabel: string | undefined
      if (isCatchTarget) {
        conditionLabel = 'error'
      } else if (isCondition) {
        const existingLabels = edges
          .filter((e) => e.source === connection.source)
          .map((e) => (e as FlowEdge).data?.conditionLabel)
        conditionLabel = existingLabels.includes('true') ? 'false' : 'true'
      }
      setEdges((eds) =>
        addEdge(
          {
            ...connection,
            label: conditionLabel,
            data: { conditionLabel },
            style: { stroke: '#30363d', strokeWidth: 2 },
            labelStyle: { fill: '#8b949e', fontSize: 11, fontWeight: 600 },
            labelBgStyle: { fill: '#21262d', fillOpacity: 0.9 },
            labelBgPadding: [4, 6] as [number, number],
            labelBgBorderRadius: 4,
          },
          eds,
        ),
      )
    },
    [setEdges, nodes, edges, showConnError],
  )

  // Click an edge from a condition node to toggle its label.
  const onEdgeClick = useCallback(
    (_: React.MouseEvent, edge: Edge) => {
      const sourceNode = nodes.find((n) => n.id === edge.source)
      if (sourceNode?.data.nodeType !== 'condition') return
      const current = (edge as FlowEdge).data?.conditionLabel
      const next = current === 'true' ? 'false' : current === 'false' ? undefined : 'true'
      setEdges((eds) =>
        eds.map((e) =>
          e.id === edge.id
            ? {
                ...e,
                label: next,
                data: { conditionLabel: next },
              }
            : e,
        ),
      )
    },
    [nodes, setEdges],
  )

  const handleNodeClick = useCallback(
    (_: React.MouseEvent, node: Node) => onSelectionChange(node.id),
    [onSelectionChange],
  )

  const handlePaneClick = useCallback(() => onSelectionChange(null), [onSelectionChange])

  const nodesWithSelection = nodes.map((n) => {
    const highlighted = highlightedNodeIds?.has(n.id)
    const heatColor = nodeHeatmapMap?.get(n.id)
    return {
      ...n,
      selected: n.id === selectedNodeId,
      style: {
        ...n.style,
        ...(highlighted ? { outline: '2px solid var(--accent, #2563eb)', outlineOffset: 2, borderRadius: 6 } : {}),
        ...(heatColor ? { boxShadow: `inset 0 0 0 2000px ${heatColor}` } : {}),
      },
    }
  })

  const displayEdges = useMemo(() => {
    const hasStatuses = Object.keys(nodeStatuses).length > 0
    if (!hasStatuses) return edges
    return edges.map((e) => {
      const result = nodeStatuses[e.source]
      if (result?.status === 'succeeded') return { ...e, style: { stroke: '#3fb950', strokeWidth: 2 } }
      if (result?.status === 'failed')    return { ...e, style: { stroke: '#f85149', strokeWidth: 2 } }
      if (result?.status === 'running')   return { ...e, style: { stroke: '#58a6ff', strokeWidth: 2 }, animated: true }
      return { ...e, style: { stroke: '#30363d', strokeWidth: 2 } }
    })
  }, [edges, nodeStatuses])

  return (
    <NodeStatusContext.Provider value={nodeStatuses}>
    <NodeWarningContext.Provider value={warningNodeIds ?? new Set()}>
      <ReactFlow
        nodes={nodesWithSelection}
        edges={displayEdges}
        onNodesChange={onNodesChange}
        onEdgesChange={onEdgesChange}
        onConnect={onConnect}
        onEdgeClick={onEdgeClick}
        onNodeClick={handleNodeClick}
        onPaneClick={handlePaneClick}
        onInit={(inst) => { rfInstance.current = inst }}
        onDrop={onDrop}
        onDragOver={onDragOver}
        nodeTypes={nodeTypes}
        fitView={!defaultViewport}
        defaultViewport={defaultViewport}
        onMoveEnd={(_event, vp) => onViewportChange?.(vp)}
        deleteKeyCode="Delete"
        proOptions={{ hideAttribution: true }}
        snapToGrid={snapToGrid}
        snapGrid={[16, 16]}
      >
        {bgVariant !== 'none' && (
          <Background
            variant={bgVariant === 'grid' ? BackgroundVariant.Cross : bgVariant === 'lines' ? BackgroundVariant.Lines : BackgroundVariant.Dots}
            gap={bgVariant === 'dots' ? 24 : 20}
            size={bgVariant === 'dots' ? 1 : 6}
            color="#21262d"
          />
        )}
        <Controls style={{ background: 'var(--panel)', border: '1px solid var(--border)' }} />
        {showMinimap && <MiniMap
          nodeColor={(n) => {
            const nt = (n.data as FlowNodeData)?.nodeType
            return nt ? (NODE_COLORS[nt] ?? '#30363d') : '#30363d'
          }}
          style={{ background: 'var(--panel)', border: '1px solid var(--border)' }}
        />}
        {fitViewRef && <FitViewBridge fitViewRef={fitViewRef} fitToNodeRef={fitToNodeRef} />}
        {connError && (
          <div style={{
            position: 'absolute', top: 12, left: '50%', transform: 'translateX(-50%)',
            background: '#450a0a', border: '1px solid #dc2626', color: '#fca5a5',
            padding: '6px 14px', borderRadius: 6, fontSize: 13, zIndex: 100,
            pointerEvents: 'none', whiteSpace: 'nowrap',
          }}>
            ⚠ {connError}
          </div>
        )}
      </ReactFlow>
    </NodeWarningContext.Provider>
    </NodeStatusContext.Provider>
  )
}
