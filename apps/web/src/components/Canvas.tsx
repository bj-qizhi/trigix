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
  deepseek: 'DeepSeek',
  qwen: '通义千问',
  zhipu: '智谱 GLM',
  moonshot: 'Moonshot (Kimi)',
  doubao: '豆包',
  minimax: 'MiniMax',
  ernie: '文心一言',
  hunyuan: '混元',
}

const NODE_ICONS: Record<NodeType, string> = {
  trigger: '▶',
  http: '↗',
  agent: '✦',
  condition: '◇',
  approval: '✋',
  map: '⟳',
  filter: '⊃',
  aggregate: 'Σ',
  sort: '⇅',
  transform: '⇄',
  delay: '⏱',
  sub_workflow: '⤵',
  assert: '⊘',
  catch: '↻',
  fan_out: '⇉',
  fan_in: '⇇',
  code: '{ }',
  slack: '#',
  email: '@',
  openai: '⬡',
  rag: '⌕',
  rag_ingest: '⊕',
  custom: '⚙',
  gemini: '✦',
  database: '⊞',
  extract: '↳',
  merge: '⊕',
  loop: '↺',
  graphql: '◈',
  validate: '✔',
  note: '✎',
  claude: '◆',
  split: '⊸',
  join: '⊷',
  switch: '⇢',
  random: '⚂',
  dedupe: '⊟',
  regex: '.*',
  csv: '⊞',
  rename: '≫',
  format: 'Aa',
  github: '⬡',
  webhook: '↗',
  jira: 'J',
  notion: 'N',
  linear: 'L',
  airtable: 'A',
  for_each: '↻',
  discord: '◈',
  teams: 'T',
  sheets: '⊞',
  xml: '</>',
  yaml: '≡',
  twilio: '✉',
  stripe: '$',
  crypto: '⊛',
  hubspot: 'H',
  date: '⏲',
  zendesk: 'Z',
  redis: '⊕',
  elasticsearch: '🔍',
  pagerduty: '🔔',
  handlebars: '{}',
  math: '∑',
  array_utils: '[]',
  shopify: '🛍',
  datadog: '📊',
  salesforce: '☁',
  freshdesk: '🎫',
  mailgun: '✉',
  asana: '✅',
  servicenow: '⚙',
  confluence: '📄',
  bitbucket: '⑂',
  azure_devops: '🔷',
  twitch: '🎮',
  figma: '✏',
  dropbox: '📦',
  cloudflare: '☁',
  box: '📁',
  okta: '🔐',
  zoom: '📹',
  spotify: '🎵',
  typeform: '📋',
  webflow: '🌐',
  intercom: '💬',
  pipedrive: '🔗',
  trello: '📌',
  monday: '📅',
  clickup: '✅',
  amplitude: '📈',
  mixpanel: '📊',
  segment: '🔀',
  sendgrid: '📧',
  braintree: '💳',
  paypal: '🅿',
  razorpay: '💸',
  firebase: '🔥',
  supabase: '⚡',
  mailchimp: '🐒',
  activecampaign: '📣',
  klaviyo: '📩',
  resend: '✉',
  contentful: '📄',
  algolia: '🔍',
  postmark: '📮',
  vonage: '📱',
  telegram: '✈',
  replicate: '🔁',
  mistral: '🌬',
  whatsapp: '💬',
  googledocs: '📝',
  perplexity: '🔎',
  cohere: '🧠',
  googledrive: '📁',
  woocommerce: '🛒',
  pinecone: '🌲',
  togetherai: '🤝',
  awss3: '🪣',
  huggingface: '🤗',
  groq: '⚡',
  openrouter: '🔀',
  qdrant: '🎯',
  cloudinary: '☁',
  gcal: '📅',
  docusign: '✍',
  xero: '💹',
  calendly: '🗓',
  apify: '🕷',
  ganalytics: '📊',
  neon: '🌀',
  copper: '🔶',
  deepseek: '🐋',
  qwen: '🧩',
  zhipu: '🔮',
  moonshot: '🌙',
  doubao: '🫧',
  minimax: '🔥',
  ernie: '🦅',
  hunyuan: '☯',
}

function FlowNodeComponent({ data, selected, id }: NodeProps) {
  const statuses = useContext(NodeStatusContext)
  const warnings = useContext(NodeWarningContext)
  const execResult = statuses[id]
  const hasWarning = warnings.has(id)
  const d = data as FlowNodeData
  const nt = d.nodeType
  const label = (d.config?.node_label as string | undefined) || (nt ? (NODE_LABELS[nt] ?? nt) : 'Node')
  const icon = nt ? (NODE_ICONS[nt] ?? '●') : '●'

  const preview = (() => {
    if (!nt) return ''
    const c = d.config ?? {}
    if (nt === 'http') return (c.url as string) || 'No URL set'
    if (nt === 'agent') return (c.model as string) || 'claude-sonnet-4-6'
    if (nt === 'condition') return c.field ? `if ${String(c.field)}` : 'No field set'
    if (nt === 'approval') return 'Awaits human approval'
    if (nt === 'map') return c.items ? `map ${String(c.items)}` : 'No items set'
    if (nt === 'filter') {
      if (!c.items) return 'No items set'
      const op = (c.operator as string) || 'exists'
      return c.field ? `${String(c.field)} ${op}${c.value ? ` ${String(c.value)}` : ''}` : 'No field set'
    }
    if (nt === 'aggregate') {
      const op = (c.operation as string) || ''
      return op ? `${op}${c.field ? `(${String(c.field)})` : ''}` : 'No operation set'
    }
    if (nt === 'sort') {
      const ord = (c.order as string) || 'asc'
      return c.field ? `${String(c.field)} ${ord}` : 'No field set'
    }
    if (nt === 'transform') return c.template ? 'template configured' : 'No template set'
    if (nt === 'delay') {
      const s = c.seconds as number | undefined
      return s !== undefined ? `wait ${s}s` : 'No duration set'
    }
    if (nt === 'sub_workflow') return (c.workflow_id as string) || 'No workflow set'
    if (nt === 'assert') return c.condition ? `assert ${String(c.condition)}` : 'No condition set'
    if (nt === 'catch') return c.source ? `catch ${String(c.source)}` : 'Catches any error'
    if (nt === 'fan_out') return 'Splits into parallel branches'
    if (nt === 'fan_in') return 'Collects branch results'
    if (nt === 'code') return c.script ? String(c.script).split('\n')[0].slice(0, 40) : 'No script'
    if (nt === 'slack') return c.text ? String(c.text).slice(0, 40) : 'No message'
    if (nt === 'email') return c.to ? `to: ${String(c.to)}` : 'No recipient'
    if (nt === 'rag') return c.kb ? `kb: ${String(c.kb)}` : 'No knowledge base'
    if (nt === 'rag_ingest') return c.kb ? `kb: ${String(c.kb)}` : 'No knowledge base'
    if (nt === 'custom') return c.custom_node ? String(c.custom_node) : 'No custom node'
    if (nt === 'openai') return (c.model as string) || 'gpt-4o-mini'
    if (nt === 'gemini') return (c.model as string) || 'gemini-2.0-flash'
    if (nt === 'database') return c.query ? String(c.query).split('\n')[0].slice(0, 40) : 'No query'
    if (nt === 'extract') return c.path ? `path: ${String(c.path)}` : 'No path set'
    if (nt === 'merge') return 'Merges fields'
    if (nt === 'loop') return c.items ? `loop ${String(c.items)}` : 'No items set'
    if (nt === 'graphql') return c.url ? String(c.url).replace(/^https?:\/\//, '') : 'No URL set'
    if (nt === 'validate') return c.source ? `validate ${String(c.source)}` : 'No source set'
    if (nt === 'note') return (c.text as string) || 'Click to add note text'
    if (nt === 'claude') return (c.model as string) || 'claude-sonnet-4-6'
    if (nt === 'github') return c.endpoint ? `${String(c.method ?? 'GET')} ${String(c.endpoint)}` : 'No endpoint set'
    if (nt === 'webhook') return c.url ? String(c.url).replace(/^https?:\/\//, '') : 'No URL set'
    if (nt === 'jira') return c.endpoint ? `${String(c.method ?? 'GET')} ${String(c.endpoint)}` : 'No endpoint set'
    if (nt === 'notion') return c.endpoint ? `${String(c.method ?? 'GET')} ${String(c.endpoint)}` : 'No endpoint set'
    if (nt === 'linear') return c.query ? String(c.query).slice(0, 40) + (String(c.query).length > 40 ? '…' : '') : 'No query set'
    if (nt === 'airtable') return c.base_id && c.table ? `${String(c.base_id)}/${String(c.table)}` : 'No base/table set'
    if (nt === 'for_each') return c.workflow_id ? `→ ${String(c.workflow_id).slice(0, 20)}` : 'No workflow set'
    if (nt === 'discord') return c.content ? String(c.content).slice(0, 40) : 'No message content'
    if (nt === 'teams') return c.text ? String(c.text).slice(0, 40) : 'No message text'
    if (nt === 'sheets') return c.spreadsheet_id ? `${String(c.spreadsheet_id).slice(0, 20)} / ${String(c.range ?? 'Sheet1!A1')}` : 'No spreadsheet set'
    if (nt === 'xml') return c.source ? String(c.source).slice(0, 40) : 'No source set'
    if (nt === 'yaml') return `${String(c.mode ?? 'parse')} mode${c.source ? '' : ' — no source'}`
    if (nt === 'twilio') return c.to ? `→ ${String(c.to)}` : 'No recipient set'
    if (nt === 'stripe') return c.endpoint ? `${String(c.method ?? 'GET')} ${String(c.endpoint)}` : 'No endpoint set'
    if (nt === 'crypto') return `${String(c.operation ?? 'sha256')}${c.source ? '' : ' — no source'}`
    if (nt === 'hubspot') return c.endpoint ? `${String(c.method ?? 'GET')} ${String(c.endpoint)}` : 'No endpoint set'
    if (nt === 'date') return String(c.operation ?? 'now')
    if (nt === 'zendesk') return c.endpoint ? `${String(c.subdomain ?? '?')}: ${String(c.endpoint)}` : 'No endpoint set'
    if (nt === 'redis') return c.url ? `${String(c.operation ?? 'get')} ${String(c.key ?? '')}` : 'No URL set'
    if (nt === 'elasticsearch') return c.url ? `${String(c.method ?? 'GET')} ${String(c.endpoint ?? '/_search')}` : 'No URL set'
    if (nt === 'pagerduty') return c.summary ? String(c.summary).slice(0, 40) : 'No summary set'
    if (nt === 'handlebars') return c.template ? String(c.template).slice(0, 40) : 'No template set'
    if (nt === 'math') return `${String(c.operation ?? 'add')}(${c.a ?? '?'}, ${c.b ?? '?'})`
    if (nt === 'array_utils') return `${String(c.operation ?? 'chunk')}${c.source ? '' : ' — no source'}`
    if (nt === 'shopify') return c.shop ? `${String(c.method ?? 'GET')} ${String(c.shop)}${String(c.endpoint ?? '/products.json')}` : 'No shop set'
    if (nt === 'datadog') return c.endpoint ? `${String(c.method ?? 'GET')} ${String(c.endpoint)}` : 'No endpoint set'
    if (nt === 'salesforce') return c.instance_url ? `${String(c.method ?? 'GET')} ${String(c.endpoint ?? '/services/data/v59.0/sobjects')}` : 'No instance URL'
    if (nt === 'freshdesk') return c.domain ? `${String(c.domain)}${String(c.endpoint ?? '')}` : 'No domain set'
    if (nt === 'mailgun') return c.to ? `→ ${String(c.to)}` : 'No recipient set'
    if (nt === 'asana') return c.endpoint ? `${String(c.method ?? 'GET')} ${String(c.endpoint)}` : 'No endpoint set'
    if (nt === 'servicenow') return c.instance ? `${String(c.instance)}${String(c.endpoint ?? '/api/now/table/incident')}` : 'No instance set'
    if (nt === 'confluence') return c.endpoint ? `${String(c.method ?? 'GET')} ${String(c.endpoint)}` : 'No endpoint set'
    if (nt === 'bitbucket') return c.endpoint ? `${String(c.method ?? 'GET')} ${String(c.endpoint)}` : 'No endpoint set'
    if (nt === 'azure_devops') return c.organization ? `${String(c.organization)}${c.project ? '/'+String(c.project) : ''}${String(c.endpoint ?? '')}` : 'No organization set'
    if (nt === 'twitch') return c.endpoint ? `${String(c.method ?? 'GET')} ${String(c.endpoint)}` : 'No endpoint set'
    if (nt === 'figma') return c.endpoint ? String(c.endpoint).slice(0, 40) : 'No endpoint set'
    if (nt === 'dropbox') return `${String(c.operation ?? 'list_folder')}${c.path ? ' '+String(c.path) : ''}`
    if (nt === 'cloudflare') return c.endpoint ? `${String(c.method ?? 'GET')} ${String(c.endpoint)}` : 'No endpoint set'
    if (nt === 'box') return c.endpoint ? `${String(c.method ?? 'GET')} ${String(c.endpoint)}` : 'No endpoint set'
    if (nt === 'okta') return c.domain ? `${String(c.domain)}${String(c.endpoint ?? '')}` : 'No domain set'
    if (nt === 'zoom') return c.endpoint ? `${String(c.method ?? 'GET')} ${String(c.endpoint)}` : 'No endpoint set'
    if (nt === 'spotify') return c.endpoint ? `${String(c.method ?? 'GET')} ${String(c.endpoint)}` : 'No endpoint set'
    if (nt === 'typeform') return c.endpoint ? `${String(c.method ?? 'GET')} ${String(c.endpoint)}` : 'No endpoint set'
    if (nt === 'webflow') return c.endpoint ? `${String(c.method ?? 'GET')} ${String(c.endpoint)}` : 'No endpoint set'
    if (nt === 'intercom') return c.endpoint ? `${String(c.method ?? 'GET')} ${String(c.endpoint)}` : 'No endpoint set'
    if (nt === 'pipedrive') return c.endpoint ? `${String(c.method ?? 'GET')} ${String(c.endpoint)}` : 'No endpoint set'
    if (nt === 'trello') return c.endpoint ? `${String(c.method ?? 'GET')} ${String(c.endpoint)}` : 'No endpoint set'
    if (nt === 'monday') return c.query ? String(c.query).slice(0, 40) : 'No query set'
    if (nt === 'clickup') return c.endpoint ? `${String(c.method ?? 'GET')} ${String(c.endpoint)}` : 'No endpoint set'
    if (nt === 'amplitude') return `${String(c.operation ?? 'track')}`
    if (nt === 'mixpanel') return `${String(c.operation ?? 'track')}`
    if (nt === 'segment') return `${String(c.operation ?? 'track')}`
    if (nt === 'sendgrid') return c.endpoint ? `${String(c.method ?? 'POST')} ${String(c.endpoint)}` : 'No endpoint set'
    if (nt === 'braintree') return c.endpoint ? `${String(c.environment ?? 'sandbox')} ${String(c.endpoint)}` : 'No endpoint set'
    if (nt === 'paypal') return c.endpoint ? `${String(c.environment ?? 'sandbox')} ${String(c.endpoint)}` : 'No endpoint set'
    if (nt === 'razorpay') return c.endpoint ? `${String(c.method ?? 'GET')} ${String(c.endpoint)}` : 'No endpoint set'
    if (nt === 'firebase') return c.endpoint ? `${String(c.service ?? 'firestore')} ${String(c.endpoint)}` : 'No endpoint set'
    if (nt === 'supabase') return c.endpoint ? `${String(c.method ?? 'GET')} ${String(c.endpoint)}` : 'No endpoint set'
    if (nt === 'mailchimp') return c.endpoint ? `${String(c.server ?? 'us1')} ${String(c.endpoint)}` : 'No endpoint set'
    if (nt === 'activecampaign') return c.endpoint ? `${String(c.method ?? 'GET')} ${String(c.endpoint)}` : 'No endpoint set'
    if (nt === 'klaviyo') return c.endpoint ? `${String(c.method ?? 'GET')} ${String(c.endpoint)}` : 'No endpoint set'
    if (nt === 'resend') return c.endpoint ? `${String(c.method ?? 'POST')} ${String(c.endpoint)}` : 'No endpoint set'
    if (nt === 'contentful') return c.endpoint ? `${String(c.api_type ?? 'delivery')} ${String(c.endpoint)}` : 'No endpoint set'
    if (nt === 'algolia') return c.endpoint ? String(c.endpoint) : 'No endpoint set'
    if (nt === 'postmark') return c.endpoint ? `${String(c.method ?? 'POST')} ${String(c.endpoint)}` : 'No endpoint set'
    if (nt === 'vonage') return `${String(c.operation ?? 'sms')}${c.to ? ' → '+String(c.to) : ''}`
    if (nt === 'telegram') return c.chat_id ? `${String(c.operation ?? 'sendMessage')} → ${String(c.chat_id)}` : 'No chat ID set'
    if (nt === 'replicate') return c.version ? `${String(c.operation ?? 'run')} ${String(c.version).slice(0, 12)}…` : 'No model version'
    if (nt === 'mistral') return c.model ? `${String(c.operation ?? 'chat')} · ${String(c.model)}` : `${String(c.operation ?? 'chat')}`
    if (nt === 'whatsapp') return c.to ? `${String(c.message_type ?? 'text')} → ${String(c.to)}` : 'No recipient'
    if (nt === 'googledocs') return c.document_id ? `${String(c.operation ?? 'get')} ${String(c.document_id).slice(0, 16)}…` : String(c.operation ?? 'get')
    if (nt === 'perplexity') return c.model ? String(c.model) : 'sonar-small-online'
    if (nt === 'cohere') return `${String(c.operation ?? 'chat')}${c.model ? ' · '+String(c.model) : ''}`
    if (nt === 'googledrive') return `${String(c.operation ?? 'list')}${c.file_id ? ' '+String(c.file_id).slice(0, 12)+'…' : ''}`
    if (nt === 'woocommerce') return c.site_url ? `${String(c.method ?? 'GET')} ${String(c.endpoint ?? '/products')}` : 'No site URL'
    if (nt === 'pinecone') return `${String(c.operation ?? 'query')}${c.namespace ? ' ['+String(c.namespace)+']' : ''}`
    if (nt === 'togetherai') return c.model ? `${String(c.operation ?? 'chat')} · ${String(c.model).split('/').pop()}` : String(c.operation ?? 'chat')
    if (nt === 'awss3') return c.bucket ? `${String(c.operation ?? 'list')} ${String(c.bucket)}${c.key ? '/'+String(c.key) : ''}` : 'No bucket'
    if (nt === 'huggingface') return c.model ? `${String(c.operation ?? 'inference')} · ${String(c.model)}` : 'No model'
    if (nt === 'groq') return c.model ? `${String(c.operation ?? 'chat')} · ${String(c.model)}` : String(c.operation ?? 'chat')
    if (nt === 'openrouter') return c.model ? `${String(c.operation ?? 'chat')} · ${String(c.model).split('/').pop()}` : String(c.operation ?? 'chat')
    if (nt === 'qdrant') return c.collection ? `${String(c.operation ?? 'search')} [${String(c.collection)}]` : 'No collection'
    if (nt === 'cloudinary') return `${String(c.operation ?? 'upload')}${c.public_id ? ' · '+String(c.public_id) : ''}`
    if (nt === 'gcal') return `${String(c.operation ?? 'list_events')}${c.calendar_id && c.calendar_id !== 'primary' ? ' ['+String(c.calendar_id)+']' : ''}`
    if (nt === 'docusign') return `${String(c.operation ?? 'list_envelopes')}${c.account_id ? ' · '+String(c.account_id).slice(0, 8)+'…' : ''}`
    if (nt === 'xero') return `${String(c.method ?? 'GET')} ${String(c.endpoint ?? '/Contacts')}`
    if (nt === 'calendly') return String(c.operation ?? 'get_current_user')
    if (nt === 'apify') return c.actor_id ? `${String(c.operation ?? 'run_actor')} · ${String(c.actor_id)}` : String(c.operation ?? 'run_actor')
    if (nt === 'ganalytics') return c.property_id ? `${String(c.operation ?? 'run_report')} · ${String(c.property_id)}` : 'No property ID'
    if (nt === 'neon') return `${String(c.operation ?? 'list_projects')}${c.project_id ? ' · '+String(c.project_id).slice(0, 10)+'…' : ''}`
    if (nt === 'copper') return `${String(c.operation ?? 'list')} ${String(c.resource ?? 'people')}`
    if (nt === 'deepseek') return c.model ? String(c.model) : 'deepseek-chat'
    if (nt === 'qwen') return c.model ? String(c.model) : 'qwen-max'
    if (nt === 'zhipu') return c.model ? String(c.model) : 'glm-4'
    if (nt === 'moonshot') return c.model ? String(c.model) : 'moonshot-v1-8k'
    if (nt === 'doubao') return c.endpoint_id ? String(c.endpoint_id) : 'No endpoint ID'
    if (nt === 'minimax') return c.model ? String(c.model) : 'abab6.5s-chat'
    if (nt === 'ernie') return c.model ? String(c.model) : 'ernie-4.0-8k'
    if (nt === 'hunyuan') return c.model ? String(c.model) : 'hunyuan-standard'
    return ''
  })()

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
      <div className="flow-node-header">
        <span>{icon}</span>
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
            const colors: Record<string, string> = {
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
              deepseek: '#4d6bfe',
              qwen: '#6200ea',
              zhipu: '#00897b',
              moonshot: '#1a237e',
              doubao: '#0078ff',
              minimax: '#ff6f00',
              ernie: '#2979ff',
              hunyuan: '#00bcd4',
            }
            return nt ? (colors[nt] ?? '#30363d') : '#30363d'
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
