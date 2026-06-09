// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useState } from 'react'
import { useLocale } from '../useLocale'
import type { WorkflowGraph } from '../types'

export interface Template {
  id: string
  name: string
  name_zh?: string
  description: string
  description_zh?: string
  category: string
  category_zh?: string
  graph: WorkflowGraph
}

export const TEMPLATES: Template[] = [
  // ── Starter (runs immediately, no credentials) ─────────────────────────────
  {
    id: 'quick-start-offline',
    name: 'Quick Start (no setup)',
    name_zh: '快速开始（无需配置）',
    description: 'Runs immediately with no API keys or credentials. Builds a small dataset, sums a field, and branches on the total. Open it, hit Run, and see a workflow execute end to end.',
    description_zh: '无需任何 API 密钥或凭证，开箱即跑。构造一组示例数据、对字段求和、按总额分支。打开后点"运行"即可看到一个工作流端到端执行。',
    category: 'Starter',
    category_zh: '入门',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: {} },
        { id: 'sample', type: 'transform', config: { template: { orders: [{ item: 'Keyboard', amount: 40 }, { item: 'Mouse', amount: 25 }, { item: 'Monitor', amount: 43 }] } } },
        { id: 'total', type: 'aggregate', config: { items: '{{sample.orders}}', operation: 'sum', field: 'amount' } },
        { id: 'check', type: 'condition', config: { field: '{{total.result}}', operator: 'gt', value: '100' } },
      ],
      edges: [
        { source: 'trigger', target: 'sample' },
        { source: 'sample', target: 'total' },
        { source: 'total', target: 'check' },
      ],
    },
  },
  // ── Sales ──────────────────────────────────────────────────────────────────
  {
    id: 'ai-lead-scorer',
    name: 'AI Lead Scorer',
    name_zh: 'AI 线索评分',
    description: 'Fetch a lead from your CRM, score it with OpenAI, then notify sales on Slack for hot leads.',
    description_zh: '从 CRM 获取销售线索，用 OpenAI 打分（1-10），高分线索自动推送 Slack 通知销售团队。',
    category: 'Sales',
    category_zh: '销售',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: {} },
        { id: 'fetch_lead', type: 'http', config: { method: 'GET', url: 'https://api.example.com/leads/{{input.lead_id}}' } },
        { id: 'score_lead', type: 'openai', config: { model: 'gpt-4o-mini', api_key: '{{credential.openai_key}}', system_prompt: 'You are a B2B lead scoring expert. Respond with JSON only: {"score":1-10,"reason":"..."}', prompt_template: 'Score this lead: {{fetch_lead}}', max_tokens: 200, temperature: 0.3 } },
        { id: 'check_score', type: 'condition', config: { field: 'score', operator: 'gte', value: '7', source: '{{score_lead.content}}' } },
        { id: 'notify_sales', type: 'slack', config: { webhook_url: '{{credential.slack_webhook}}', text: 'Hot lead (score ≥ 7)! {{fetch_lead.body.name}} ({{fetch_lead.body.email}})\nModel output: {{score_lead.content}}', username: 'LeadBot' } },
      ],
      edges: [
        { source: 'trigger', target: 'fetch_lead' },
        { source: 'fetch_lead', target: 'score_lead' },
        { source: 'score_lead', target: 'check_score' },
        { source: 'check_score', target: 'notify_sales', condition_label: 'true' },
      ],
    },
  },
  {
    id: 'customer-onboarding',
    name: 'Customer Onboarding Flow',
    name_zh: '新客户开通流程',
    description: 'Triggered by a new signup webhook: create account, send welcome email, notify the team.',
    description_zh: '新用户注册 Webhook 触发：自动建账号、发送欢迎邮件、Slack 通知团队，含错误捕获分支。',
    category: 'Sales',
    category_zh: '销售',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: { webhook_secret: '' } },
        { id: 'create_account', type: 'http', config: { method: 'POST', url: 'https://api.example.com/accounts', auth_token: '{{credential.api_key}}', body: '{"name":"{{input.company}}","email":"{{input.email}}","plan":"{{input.plan}}"}' } },
        { id: 'gate', type: 'condition', config: { field: 'body.id', operator: 'exists', source: '{{create_account}}' } },
        { id: 'welcome_email', type: 'email', config: { to: '{{input.email}}', subject: 'Welcome to the platform, {{input.name}}!', body: 'Hi {{input.name}},\n\nYour account is ready. Get started at https://app.example.com.\n\nYour account ID: {{create_account.body.id}}\n\nBest,\nThe Team', api_key: '{{credential.sendgrid_key}}', from: 'welcome@example.com' } },
        { id: 'notify_team', type: 'slack', config: { webhook_url: '{{credential.slack_webhook}}', text: 'New customer: *{{input.company}}* ({{input.email}}) signed up for {{input.plan}}. Account ID: {{create_account.body.id}}', username: 'SalesBot' } },
        { id: 'on_error', type: 'catch', config: {} },
        { id: 'alert_error', type: 'slack', config: { webhook_url: '{{credential.slack_webhook}}', text: 'Failed to onboard {{input.email}}: {{on_error.error}}' } },
      ],
      edges: [
        { source: 'trigger', target: 'create_account' },
        { source: 'create_account', target: 'gate' },
        { source: 'gate', target: 'welcome_email', condition_label: 'true' },
        { source: 'welcome_email', target: 'notify_team' },
        { source: 'create_account', target: 'on_error', condition_label: 'error' },
        { source: 'on_error', target: 'alert_error' },
      ],
    },
  },

  // ── Marketing ──────────────────────────────────────────────────────────────
  {
    id: 'content-pipeline',
    name: 'Content Generation Pipeline',
    name_zh: 'AI 内容生成流水线',
    description: 'Generate a blog post with OpenAI, transform it into your CMS format, then publish via HTTP.',
    description_zh: '用 OpenAI 自动撰写博客文章，转换为 CMS 格式后通过 HTTP 接口发布，适合内容营销团队。',
    category: 'Marketing',
    category_zh: '营销',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: {} },
        { id: 'generate_draft', type: 'openai', config: { model: 'gpt-4o', api_key: '{{credential.openai_key}}', system_prompt: 'You are a professional content writer. Write engaging, SEO-optimized blog posts.', prompt_template: 'Write a 500-word blog post about: {{input.topic}}\nTarget audience: {{input.audience}}', max_tokens: 1500, temperature: 0.7 } },
        { id: 'format_post', type: 'transform', config: { template: { title: '{{input.topic}}', content: '{{generate_draft.content}}', tags: '{{input.tags}}', status: 'draft' } } },
        { id: 'publish', type: 'http', config: { method: 'POST', url: 'https://api.yourcms.com/posts', auth_token: '{{credential.cms_key}}', body: '{{format_post}}' } },
      ],
      edges: [
        { source: 'trigger', target: 'generate_draft' },
        { source: 'generate_draft', target: 'format_post' },
        { source: 'format_post', target: 'publish' },
      ],
    },
  },
  {
    id: 'email-newsletter',
    name: 'Email Newsletter Sender',
    name_zh: '邮件营销自动发送',
    description: 'Generate a personalized newsletter with AI, then send it via SendGrid to a list of recipients.',
    description_zh: '每周一早 9 点自动触发：拉取最新话题，用 OpenAI 生成通讯内容，SendGrid 批量发送邮件。',
    category: 'Marketing',
    category_zh: '营销',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: { cron_expression: '0 0 9 * * Mon *' } },
        { id: 'fetch_topics', type: 'http', config: { method: 'GET', url: 'https://api.example.com/newsletter/topics' } },
        { id: 'write_content', type: 'openai', config: { model: 'gpt-4o', api_key: '{{credential.openai_key}}', system_prompt: 'You are an expert newsletter writer. Write engaging, concise content.', prompt_template: 'Write a weekly newsletter section covering these topics:\n{{fetch_topics.body.topics}}\nKeep it to 200 words.', max_tokens: 600, temperature: 0.7 } },
        { id: 'send_email', type: 'email', config: { to: '{{input.recipient_list}}', subject: 'Your Weekly Update — {{input.issue_date}}', body: '{{write_content.content}}', api_key: '{{credential.sendgrid_key}}', from: 'newsletter@example.com' } },
      ],
      edges: [
        { source: 'trigger', target: 'fetch_topics' },
        { source: 'fetch_topics', target: 'write_content' },
        { source: 'write_content', target: 'send_email' },
      ],
    },
  },

  // ── Data ───────────────────────────────────────────────────────────────────
  {
    id: 'data-aggregation',
    name: 'Data Aggregation Pipeline',
    name_zh: '数据聚合流水线',
    description: 'Fetch a dataset, filter by criteria, aggregate statistics, and store the results.',
    description_zh: '从 API 拉取数据集，按条件过滤，汇总统计指标，将报表结果写回存储接口。',
    category: 'Data',
    category_zh: '数据',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: {} },
        { id: 'fetch_data', type: 'http', config: { method: 'GET', url: 'https://api.example.com/data?from={{input.from}}&to={{input.to}}', auth_token: '{{credential.api_key}}' } },
        { id: 'filter_active', type: 'filter', config: { items: '{{fetch_data.body.records}}', field: 'status', operator: 'equals', value: 'active' } },
        { id: 'compute_total', type: 'aggregate', config: { items: '{{filter_active.items}}', operation: 'sum', field: 'amount' } },
        { id: 'build_report', type: 'transform', config: { template: { total: '{{compute_total.result}}', count: '{{filter_active.count}}', period: { from: '{{input.from}}', to: '{{input.to}}' } } } },
        { id: 'store_report', type: 'http', config: { method: 'POST', url: 'https://api.example.com/reports', body: '{{build_report}}' } },
      ],
      edges: [
        { source: 'trigger', target: 'fetch_data' },
        { source: 'fetch_data', target: 'filter_active' },
        { source: 'filter_active', target: 'compute_total' },
        { source: 'compute_total', target: 'build_report' },
        { source: 'build_report', target: 'store_report' },
      ],
    },
  },
  {
    id: 'database-etl',
    name: 'Database ETL Pipeline',
    name_zh: '数据库 ETL 同步',
    description: 'Query a source database, transform the rows, and write results to a destination or API.',
    description_zh: '每天定时从源数据库抽取增量数据，字段映射转换后批量写入目标接口，并通过 Slack 确认完成。',
    category: 'Data',
    category_zh: '数据',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: { interval_secs: 86400 } },
        { id: 'extract', type: 'database', config: { url: '{{credential.source_db}}', query: "SELECT id, name, email, created_at FROM users WHERE created_at > NOW() - INTERVAL '1 day'" } },
        { id: 'transform', type: 'map', config: { items: '{{extract.rows}}', item_template: { id: '{{item.id}}', email: '{{item.email}}', display_name: '{{item.name}}', synced_at: '{{input.run_time}}' } } },
        { id: 'load', type: 'http', config: { method: 'POST', url: 'https://api.destination.com/users/bulk', auth_token: '{{credential.dest_api_key}}', body: '{{transform.items}}' } },
        { id: 'notify', type: 'slack', config: { webhook_url: '{{credential.slack_webhook}}', text: 'ETL complete: {{transform.count}} users synced.' } },
      ],
      edges: [
        { source: 'trigger', target: 'extract' },
        { source: 'extract', target: 'transform' },
        { source: 'transform', target: 'load' },
        { source: 'load', target: 'notify' },
      ],
    },
  },
  {
    id: 'webhook-to-database',
    name: 'Webhook → Database Logger',
    name_zh: 'Webhook 事件入库',
    description: 'Accept incoming webhook events, validate them, and insert records into PostgreSQL.',
    description_zh: '接收入站 Webhook，校验 event_type 字段，插入 PostgreSQL events 表，返回入库 ID。',
    category: 'Data',
    category_zh: '数据',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: { webhook_secret: '' } },
        { id: 'validate', type: 'assert', config: { condition: '{{input.event_type}}', message: 'Missing event_type in payload' } },
        { id: 'insert', type: 'database', config: { url: '{{credential.pg_url}}', query: "INSERT INTO events (type, payload, received_at) VALUES ('{{input.event_type}}', '{{input}}', NOW()) RETURNING id" } },
        { id: 'ack', type: 'transform', config: { template: { ok: true, event_id: '{{insert.rows.0.id}}', event_type: '{{input.event_type}}' } } },
      ],
      edges: [
        { source: 'trigger', target: 'validate' },
        { source: 'validate', target: 'insert' },
        { source: 'insert', target: 'ack' },
      ],
    },
  },
  {
    id: 'graphql-sync',
    name: 'GraphQL Data Sync',
    name_zh: 'GraphQL 数据同步',
    description: 'Query a GraphQL API on a schedule, extract key fields, and push updates to your backend.',
    description_zh: '每小时定时查询 GraphQL API，提取商品节点并过滤有效价格，同步到内部服务接口。',
    category: 'Data',
    category_zh: '数据',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: { interval_secs: 3600 } },
        { id: 'fetch_data', type: 'graphql', config: { url: 'https://api.example.com/graphql', query: 'query GetProducts($after: String) {\n  products(first: 50, after: $after) {\n    nodes { id sku title price }\n    pageInfo { hasNextPage endCursor }\n  }\n}', variables: '{"after": null}', bearer_token: '{{credential.api_token}}' } },
        { id: 'extract_nodes', type: 'extract', config: { source: '{{fetch_data}}', path: 'data.products.nodes' } },
        { id: 'filter_active', type: 'filter', config: { items: '{{extract_nodes.value}}', field: 'price', operator: 'gt', value: '0' } },
        { id: 'sync', type: 'http', config: { method: 'POST', url: 'https://api.internal.com/products/sync', auth_token: '{{credential.internal_key}}', body: '{{filter_active.items}}' } },
      ],
      edges: [
        { source: 'trigger', target: 'fetch_data' },
        { source: 'fetch_data', target: 'extract_nodes' },
        { source: 'extract_nodes', target: 'filter_active' },
        { source: 'filter_active', target: 'sync' },
      ],
    },
  },

  // ── Reliability ────────────────────────────────────────────────────────────
  {
    id: 'error-resilient-fetch',
    name: 'Error-Resilient API Call',
    name_zh: '带错误恢复的 API 调用',
    description: 'Call an external API with automatic error routing — on failure, alert on Slack instead of crashing.',
    description_zh: '调用外部 API 时自动路由错误分支，失败不崩溃，而是通过 Slack 告警并继续执行后续逻辑。',
    category: 'Reliability',
    category_zh: '可靠性',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: {} },
        { id: 'call_api', type: 'http', config: { method: 'POST', url: 'https://api.example.com/process', body: '{{input}}', max_retries: 2, timeout_secs: 30 } },
        { id: 'handle_success', type: 'transform', config: { template: { ok: true, result: '{{call_api}}' } } },
        { id: 'catch_error', type: 'catch', config: { source: 'call_api' } },
        { id: 'alert_slack', type: 'slack', config: { webhook_url: '{{credential.slack_webhook}}', text: 'API call failed!\nError: {{catch_error.error}}\nInput: {{input}}', username: 'ErrorBot' } },
      ],
      edges: [
        { source: 'trigger', target: 'call_api' },
        { source: 'call_api', target: 'handle_success' },
        { source: 'call_api', target: 'catch_error', condition_label: 'error' },
        { source: 'catch_error', target: 'alert_slack' },
      ],
    },
  },
  {
    id: 'data-quality-check',
    name: 'Data Quality Monitor',
    name_zh: '数据质量监控',
    description: 'Run daily data quality assertions on your database and alert via Slack on failures.',
    description_zh: '每天早 6 点自动检查数据库空值率，断言通过则推送成功通知，断言失败则触发告警分支。',
    category: 'Reliability',
    category_zh: '可靠性',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: { cron_expression: '0 0 6 * * * *' } },
        { id: 'check_nulls', type: 'database', config: { url: '{{credential.pg_url}}', query: 'SELECT COUNT(*) as null_count FROM orders WHERE customer_id IS NULL' } },
        { id: 'extract_count', type: 'extract', config: { source: '{{check_nulls}}', path: 'rows.0.null_count' } },
        { id: 'assert_quality', type: 'assert', config: { condition: '{{extract_count.value}} == 0', message: 'Data quality failure: {{extract_count.value}} orders missing customer_id' } },
        { id: 'alert', type: 'slack', config: { webhook_url: '{{credential.slack_webhook}}', text: 'Data quality check passed — no null customer IDs in orders.' } },
        { id: 'on_failure', type: 'catch', config: {} },
        { id: 'alert_failure', type: 'slack', config: { webhook_url: '{{credential.slack_webhook}}', text: 'DATA QUALITY ALERT: {{on_failure.error}}\nCheck the orders table immediately.' } },
      ],
      edges: [
        { source: 'trigger', target: 'check_nulls' },
        { source: 'check_nulls', target: 'extract_count' },
        { source: 'extract_count', target: 'assert_quality' },
        { source: 'assert_quality', target: 'alert' },
        { source: 'assert_quality', target: 'on_failure', condition_label: 'error' },
        { source: 'on_failure', target: 'alert_failure' },
      ],
    },
  },

  // ── Compliance ─────────────────────────────────────────────────────────────
  {
    id: 'human-approval-flow',
    name: 'Human Approval Gate',
    name_zh: '人工审批节点',
    description: 'AI analyzes a request, pauses for human review, then proceeds or rejects based on the decision.',
    description_zh: 'AI 自动分析请求风险等级，暂停等待人工审核，根据审批结果决定是否继续执行敏感操作。',
    category: 'Compliance',
    category_zh: '合规',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: {} },
        { id: 'analyze', type: 'agent', config: { model: 'claude-sonnet-4-6', system_prompt: 'Analyze this request and provide a risk assessment with recommendation.', prompt_template: 'Request details: {{input}}\n\nProvide: risk level (low/medium/high), summary, and recommendation.', max_tokens: 500 } },
        { id: 'gate', type: 'approval', config: {} },
        { id: 'execute_action', type: 'http', config: { method: 'POST', url: 'https://api.example.com/execute', body: '{{input}}', auth_token: '{{credential.api_key}}' } },
        { id: 'notify_complete', type: 'slack', config: { webhook_url: '{{credential.slack_webhook}}', text: 'Action approved and executed.\nAnalysis: {{analyze.text}}\nResult: {{execute_action}}' } },
      ],
      edges: [
        { source: 'trigger', target: 'analyze' },
        { source: 'analyze', target: 'gate' },
        { source: 'gate', target: 'execute_action' },
        { source: 'execute_action', target: 'notify_complete' },
      ],
    },
  },

  // ── Reporting ──────────────────────────────────────────────────────────────
  {
    id: 'scheduled-report',
    name: 'Scheduled Daily Report',
    name_zh: '每日定时汇报',
    description: 'Runs on a schedule, fetches data, summarizes with AI, and emails the report to your team.',
    description_zh: '每天定时拉取业务指标，AI 提炼 3-5 条关键结论，通过邮件发送给团队，免去人工汇总。',
    category: 'Reporting',
    category_zh: '报表',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: { interval_secs: 86400 } },
        { id: 'fetch_metrics', type: 'http', config: { method: 'GET', url: 'https://api.example.com/metrics/today', auth_token: '{{credential.api_key}}' } },
        { id: 'summarize', type: 'openai', config: { model: 'gpt-4o-mini', api_key: '{{credential.openai_key}}', system_prompt: 'You are a business analyst. Write concise, actionable daily summaries.', prompt_template: 'Summarize these daily metrics in 3-5 bullet points:\n{{fetch_metrics}}', max_tokens: 400, temperature: 0.4 } },
        { id: 'send_report', type: 'email', config: { to: '{{input.report_recipients}}', subject: 'Daily Report — {{input.date}}', body: '{{summarize.content}}', api_key: '{{credential.sendgrid_key}}' } },
      ],
      edges: [
        { source: 'trigger', target: 'fetch_metrics' },
        { source: 'fetch_metrics', target: 'summarize' },
        { source: 'summarize', target: 'send_report' },
      ],
    },
  },

  // ── AI / Vision ────────────────────────────────────────────────────────────
  {
    id: 'multi-model-analysis',
    name: 'Multi-Model Analysis',
    name_zh: '多模型并行分析',
    description: 'Run the same prompt through both Gemini and OpenAI in parallel, then merge results with Fan-In.',
    description_zh: '同一段文本同时发给 Gemini 和 OpenAI，Fan-Out 并行执行，Fan-In 汇总后对比两个模型的分析结果。',
    category: 'AI',
    category_zh: 'AI',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: {} },
        { id: 'fan_out_1', type: 'fan_out', config: {} },
        { id: 'gemini_1', type: 'gemini', config: { model: 'gemini-2.0-flash', api_key: '{{credential.gemini_key}}', system_prompt: 'You are a concise analyst. Respond with structured JSON.', prompt_template: 'Analyze: {{input.text}}', max_tokens: 512, temperature: 0.3 } },
        { id: 'openai_1', type: 'openai', config: { model: 'gpt-4o-mini', api_key: '{{credential.openai_key}}', system_prompt: 'You are a concise analyst. Respond with structured JSON.', prompt_template: 'Analyze: {{input.text}}', max_tokens: 512, temperature: 0.3 } },
        { id: 'fan_in_1', type: 'fan_in', config: {} },
        { id: 'merge', type: 'transform', config: { template: { gemini: '{{gemini_1.content}}', openai: '{{openai_1.content}}', combined: true } } },
      ],
      edges: [
        { source: 'trigger', target: 'fan_out_1' },
        { source: 'fan_out_1', target: 'gemini_1' },
        { source: 'fan_out_1', target: 'openai_1' },
        { source: 'gemini_1', target: 'fan_in_1' },
        { source: 'openai_1', target: 'fan_in_1' },
        { source: 'fan_in_1', target: 'merge' },
      ],
    },
  },
  {
    id: 'claude-content-pipeline',
    name: 'Claude Content Generation Pipeline',
    name_zh: 'Claude 内容创作流水线',
    description: 'Use Claude to draft content, review it, then post to Slack. Demonstrates multi-step Claude usage with structured prompts.',
    description_zh: '先用 Claude Sonnet 起草内容，再用 Claude Haiku 编辑评分，高质量内容自动推送 Slack，展示多步骤 Claude 协作。',
    category: 'AI',
    category_zh: 'AI',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: {} },
        { id: 'draft', type: 'claude', config: { model: 'claude-sonnet-4-6', api_key: '{{credential.anthropic_key}}', system_prompt: 'You are a professional content writer. Write clear, engaging content.', prompt_template: 'Write a {{input.format}} about {{input.topic}} for {{input.audience}}. Target length: {{input.length}} words.', max_tokens: 2048 } },
        { id: 'review', type: 'claude', config: { model: 'claude-haiku-4-5-20251001', api_key: '{{credential.anthropic_key}}', system_prompt: 'You are an editor. Be concise and constructive.', prompt_template: 'Review this content and rate it 1-10 for clarity and engagement. Provide brief feedback.\n\nContent:\n{{draft.content}}', max_tokens: 512 } },
        { id: 'gate', type: 'condition', config: { field: 'content', operator: 'exists', source: '{{draft}}' } },
        { id: 'post', type: 'slack', config: { webhook_url: '{{credential.slack_webhook}}', text: '*New {{input.format}}: {{input.topic}}*\n\n{{draft.content}}\n\n_Editor review: {{review.content}}_', username: 'ContentBot' } },
      ],
      edges: [
        { source: 'trigger', target: 'draft' },
        { source: 'draft', target: 'review' },
        { source: 'review', target: 'gate' },
        { source: 'gate', target: 'post', condition_label: 'true' },
      ],
    },
  },

  // ── Engineering ────────────────────────────────────────────────────────────
  {
    id: 'code-review-bot',
    name: 'AI Code Review Bot',
    name_zh: 'AI 代码审查机器人',
    description: 'Accept a code snippet via webhook, run AI review with Gemini, and post feedback to Slack.',
    description_zh: '通过 Webhook 接收代码片段，Gemini 自动审查 bug/安全/可读性，将 Markdown 格式反馈推送 Slack。',
    category: 'Engineering',
    category_zh: '工程',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: { webhook_secret: '' } },
        { id: 'review', type: 'gemini', config: { model: 'gemini-2.0-flash', api_key: '{{credential.gemini_key}}', system_prompt: 'You are a senior software engineer doing a code review. Be concise and actionable. Focus on bugs, security issues, and readability. Respond in markdown.', prompt_template: 'Please review this code:\n```\n{{input.code}}\n```\nLanguage: {{input.language}}\nContext: {{input.context}}', max_tokens: 1000 } },
        { id: 'post_feedback', type: 'slack', config: { webhook_url: '{{credential.slack_webhook}}', text: '*Code Review for {{input.pr_title}}*\n\n{{review.content}}', username: 'ReviewBot' } },
      ],
      edges: [
        { source: 'trigger', target: 'review' },
        { source: 'review', target: 'post_feedback' },
      ],
    },
  },
  {
    id: 'github-pr-review',
    name: 'GitHub PR Review Bot',
    name_zh: 'GitHub PR 自动评审',
    description: 'Triggered by a webhook: validate PR metadata, run Claude AI review, post comment back to GitHub.',
    description_zh: '监听 GitHub PR Webhook，校验元数据，Claude 自动生成审查意见，回调 GitHub Issues API 写评论。',
    category: 'Engineering',
    category_zh: '工程',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: { webhook_secret: '{{credential.github_webhook_secret}}' } },
        { id: 'validate', type: 'validate', config: { source: '{{input}}', schema: '{"required":["action","pull_request"]}', fail_on_invalid: true } },
        { id: 'review', type: 'claude', config: { model: 'claude-sonnet-4-6', api_key: '{{credential.claude_api_key}}', system_prompt: 'You are a code reviewer. Provide concise, constructive feedback.', prompt_template: 'Review this pull request:\nTitle: {{input.pull_request.title}}\nDescription: {{input.pull_request.body}}\n\nProvide a brief code review summary and key concerns.' } },
        { id: 'post_comment', type: 'github', config: { token: '{{credential.github_token}}', method: 'POST', endpoint: '/repos/{{input.repository.full_name}}/issues/{{input.pull_request.number}}/comments', body: '{"body": "🤖 AI Review:\\n\\n{{review.content}}"}' } },
        { id: 'notify', type: 'slack', config: { webhook_url: '{{credential.slack_webhook}}', text: 'PR reviewed: *{{input.pull_request.title}}* — {{post_comment.status}}', username: 'PRBot' } },
      ],
      edges: [
        { source: 'trigger', target: 'validate' },
        { source: 'validate', target: 'review' },
        { source: 'review', target: 'post_comment' },
        { source: 'post_comment', target: 'notify' },
      ],
    },
  },

  // ── Automation ─────────────────────────────────────────────────────────────
  {
    id: 'slack-command-bot',
    name: 'Slack Command Bot',
    name_zh: 'Slack 指令机器人',
    description: 'Receive a Slack slash-command via webhook, process the payload, and reply with a Slack message.',
    description_zh: '接收 Slack Slash Command，提取指令文本，OpenAI 生成简洁回复，自动发回对话频道。',
    category: 'Automation',
    category_zh: '自动化',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: { webhook_secret: '' } },
        { id: 'parse_command', type: 'extract', config: { source: '{{input}}', path: 'text' } },
        { id: 'respond', type: 'openai', config: { model: 'gpt-4o-mini', api_key: '{{credential.openai_key}}', system_prompt: 'You are a helpful Slack bot. Keep answers concise and friendly.', prompt_template: '{{parse_command.value}}', max_tokens: 300 } },
        { id: 'reply', type: 'slack', config: { webhook_url: '{{credential.slack_webhook}}', text: '{{respond.content}}', username: 'WorkflowBot' } },
      ],
      edges: [
        { source: 'trigger', target: 'parse_command' },
        { source: 'parse_command', target: 'respond' },
        { source: 'respond', target: 'reply' },
      ],
    },
  },
  {
    id: 'webhook-fanout',
    name: 'Outbound Webhook Fan-out',
    name_zh: 'Webhook 并行广播',
    description: 'Fan-out an event to multiple downstream webhooks in parallel, aggregate responses.',
    description_zh: '将单个事件并行广播给多个下游 Webhook，Fan-In 汇总所有响应，实现事件扇出分发。',
    category: 'Automation',
    category_zh: '自动化',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: {} },
        { id: 'fan_out', type: 'fan_out', config: {} },
        { id: 'webhook_a', type: 'webhook', config: { url: '{{credential.webhook_a_url}}', body_template: '{"event": "{{input.event}}", "data": "{{input.data}}"}' } },
        { id: 'webhook_b', type: 'webhook', config: { url: '{{credential.webhook_b_url}}', body_template: '{"event": "{{input.event}}", "data": "{{input.data}}"}' } },
        { id: 'webhook_c', type: 'webhook', config: { url: '{{credential.webhook_c_url}}', body_template: '{"event": "{{input.event}}", "data": "{{input.data}}"}' } },
        { id: 'fan_in', type: 'fan_in', config: {} },
        { id: 'summarize', type: 'transform', config: { template: { delivered: 3, event: '{{input.event}}' } } },
      ],
      edges: [
        { source: 'trigger', target: 'fan_out' },
        { source: 'fan_out', target: 'webhook_a' },
        { source: 'fan_out', target: 'webhook_b' },
        { source: 'fan_out', target: 'webhook_c' },
        { source: 'webhook_a', target: 'fan_in' },
        { source: 'webhook_b', target: 'fan_in' },
        { source: 'webhook_c', target: 'fan_in' },
        { source: 'fan_in', target: 'summarize' },
      ],
    },
  },

  // ── 国内大模型专属模板 ────────────────────────────────────────────────────
  {
    id: 'deepseek-code-review',
    name: 'DeepSeek Code Review Bot',
    name_zh: 'DeepSeek 代码审查机器人',
    description: 'Receive code via webhook, run DeepSeek-R1 reasoning review, post results to Slack.',
    description_zh: '通过 Webhook 接收代码，DeepSeek-R1 深度推理审查，结果推送 Slack，适合国内无法访问 GPT 的团队。',
    category: 'AI',
    category_zh: 'AI',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: { webhook_secret: '' } },
        { id: 'review', type: 'deepseek', config: { model: 'deepseek-reasoner', api_key: '{{credential.deepseek_key}}', system_prompt: '你是一名资深后端工程师，做代码审查时关注：安全漏洞、性能问题、可读性。用中文回复，Markdown 格式。', prompt_template: '请审查以下代码：\n```{{input.language}}\n{{input.code}}\n```\n上下文：{{input.context}}', max_tokens: 1500 } },
        { id: 'post', type: 'slack', config: { webhook_url: '{{credential.slack_webhook}}', text: '*DeepSeek 代码审查：{{input.pr_title}}*\n\n{{review.content}}', username: 'DeepSeekBot' } },
      ],
      edges: [
        { source: 'trigger', target: 'review' },
        { source: 'review', target: 'post' },
      ],
    },
  },
  {
    id: 'qwen-customer-service',
    name: 'Qwen Customer Service Reply',
    name_zh: '通义千问客服自动回复',
    description: 'Receive customer messages via webhook, generate reply with Qwen, send back via HTTP.',
    description_zh: '接收客服消息，通义千问 qwen-max 生成专业回复，通过 HTTP 接口自动发回，降低人工客服压力。',
    category: 'AI',
    category_zh: 'AI',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: { webhook_secret: '' } },
        { id: 'classify', type: 'qwen', config: { model: 'qwen-turbo', api_key: '{{credential.dashscope_key}}', system_prompt: '你是客服分类助手。将问题分类为：退款、配送、产品咨询、其他。只输出分类词。', prompt_template: '{{input.message}}', max_tokens: 20 } },
        { id: 'reply', type: 'qwen', config: { model: 'qwen-max', api_key: '{{credential.dashscope_key}}', system_prompt: '你是一名专业、友善的电商客服。用简洁中文回复，不超过150字。', prompt_template: '客户问题类型：{{classify.content}}\n客户原文：{{input.message}}\n订单信息：{{input.order_info}}', max_tokens: 300, temperature: 0.5 } },
        { id: 'send_reply', type: 'http', config: { method: 'POST', url: '{{input.reply_url}}', auth_token: '{{credential.cs_platform_key}}', body: '{"session_id":"{{input.session_id}}","message":"{{reply.content}}"}' } },
      ],
      edges: [
        { source: 'trigger', target: 'classify' },
        { source: 'classify', target: 'reply' },
        { source: 'reply', target: 'send_reply' },
      ],
    },
  },
  {
    id: 'cn-multi-model-compare',
    name: 'Chinese LLM Comparison',
    name_zh: '国内大模型对比分析',
    description: 'Send the same prompt to DeepSeek and Qwen in parallel, merge and compare outputs.',
    description_zh: '同一问题并行发给 DeepSeek 和通义千问，Fan-In 汇总后对比两个模型的回答质量，辅助模型选型决策。',
    category: 'AI',
    category_zh: 'AI',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: {} },
        { id: 'fan_out', type: 'fan_out', config: {} },
        { id: 'deepseek', type: 'deepseek', config: { model: 'deepseek-chat', api_key: '{{credential.deepseek_key}}', system_prompt: '请简洁、准确地回答。', prompt_template: '{{input.question}}', max_tokens: 512, temperature: 0.3 } },
        { id: 'qwen', type: 'qwen', config: { model: 'qwen-max', api_key: '{{credential.dashscope_key}}', system_prompt: '请简洁、准确地回答。', prompt_template: '{{input.question}}', max_tokens: 512, temperature: 0.3 } },
        { id: 'fan_in', type: 'fan_in', config: {} },
        { id: 'compare', type: 'transform', config: { template: { question: '{{input.question}}', deepseek: '{{deepseek.content}}', qwen: '{{qwen.content}}' } } },
      ],
      edges: [
        { source: 'trigger', target: 'fan_out' },
        { source: 'fan_out', target: 'deepseek' },
        { source: 'fan_out', target: 'qwen' },
        { source: 'deepseek', target: 'fan_in' },
        { source: 'qwen', target: 'fan_in' },
        { source: 'fan_in', target: 'compare' },
      ],
    },
  },
  {
    id: 'ernie-content-audit',
    name: 'ERNIE Content Moderation',
    name_zh: '文心一言内容审核',
    description: 'Webhook receives UGC content, ERNIE audits it, safe content passes through, risky content is flagged.',
    description_zh: '接收用户生成内容（UGC），文心一言自动审核合规性，安全内容放行，风险内容走拦截分支并记录。',
    category: 'Compliance',
    category_zh: '合规',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: { webhook_secret: '' } },
        { id: 'audit', type: 'ernie', config: { model: 'ernie-speed-128k', api_key: '{{credential.ernie_client_id}}', secret_key: '{{credential.ernie_client_secret}}', system_prompt: '你是内容审核助手。判断内容是否违规（色情/暴力/政治敏感/广告）。只输出 JSON：{"safe":true/false,"reason":"...","category":"..."}', prompt_template: '审核以下内容：{{input.content}}', max_tokens: 200, temperature: 0.1 } },
        { id: 'check', type: 'condition', config: { field: 'safe', equals: 'true', source: '{{audit.content}}' } },
        { id: 'approve', type: 'http', config: { method: 'POST', url: '{{input.callback_url}}', body: '{"id":"{{input.content_id}}","status":"approved"}' } },
        { id: 'reject', type: 'http', config: { method: 'POST', url: '{{input.callback_url}}', body: '{"id":"{{input.content_id}}","status":"rejected","reason":"{{audit.content}}"}' } },
      ],
      edges: [
        { source: 'trigger', target: 'audit' },
        { source: 'audit', target: 'check' },
        { source: 'check', target: 'approve', condition_label: 'true' },
        { source: 'check', target: 'reject', condition_label: 'false' },
      ],
    },
  },
  {
    id: 'moonshot-doc-summary',
    name: 'Long Document Summarizer',
    name_zh: '长文档摘要（Kimi 128K）',
    description: 'Feed a long document to Moonshot (128K context), get a structured summary, store to database.',
    description_zh: '利用 Kimi 128K 超长上下文处理合同/报告等长文档，生成结构化摘要并写入数据库，适合法务/研究场景。',
    category: 'AI',
    category_zh: 'AI',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: { webhook_secret: '' } },
        { id: 'summarize', type: 'moonshot', config: { model: 'moonshot-v1-128k', api_key: '{{credential.moonshot_key}}', system_prompt: '你是专业文档分析师。输出结构化摘要：核心结论、关键数据、风险点、行动建议，每项不超过3条。', prompt_template: '文档标题：{{input.title}}\n\n文档内容：\n{{input.content}}', max_tokens: 1000, temperature: 0.3 } },
        { id: 'store', type: 'database', config: { url: '{{credential.pg_url}}', query: "INSERT INTO doc_summaries (doc_id, title, summary, created_at) VALUES ('{{input.doc_id}}', '{{input.title}}', '{{summarize.content}}', NOW())" } },
        { id: 'notify', type: 'slack', config: { webhook_url: '{{credential.slack_webhook}}', text: '📄 文档摘要完成：*{{input.title}}*\n\n{{summarize.content}}', username: 'KimiBot' } },
      ],
      edges: [
        { source: 'trigger', target: 'summarize' },
        { source: 'summarize', target: 'store' },
        { source: 'store', target: 'notify' },
      ],
    },
  },
  {
    id: 'wecom-alert-bot',
    name: 'WeCom Alert Bot',
    name_zh: '企业微信告警机器人',
    description: 'Monitor metrics on a schedule, analyze anomalies with Hunyuan, send WeCom robot alerts.',
    description_zh: '每5分钟拉取系统指标，腾讯混元分析异常原因，通过企业微信群机器人 Webhook 推送告警卡片。',
    category: 'Reliability',
    category_zh: '可靠性',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: { interval_secs: 300 } },
        { id: 'fetch_metrics', type: 'http', config: { method: 'GET', url: '{{input.metrics_url}}', auth_token: '{{credential.monitor_key}}' } },
        { id: 'check_threshold', type: 'condition', config: { field: 'body.value', operator: 'gt', value: '{{input.threshold}}', source: '{{fetch_metrics}}' } },
        { id: 'analyze', type: 'hunyuan', config: { model: 'hunyuan-standard', api_key: '{{credential.hunyuan_key}}', system_prompt: '你是运维告警分析助手，用50字内中文描述告警原因和建议。', prompt_template: '指标：{{fetch_metrics.body.metric_name}}，当前值：{{fetch_metrics.body.value}}，阈值：{{input.threshold}}', max_tokens: 150 } },
        { id: 'send_wecom', type: 'http', config: { method: 'POST', url: '{{credential.wecom_webhook}}', body: '{"msgtype":"markdown","markdown":{"content":"## ⚠️ 告警通知\n**指标**：{{fetch_metrics.body.metric_name}}\n**当前值**：{{fetch_metrics.body.value}}\n**分析**：{{analyze.content}}"}}' } },
      ],
      edges: [
        { source: 'trigger', target: 'fetch_metrics' },
        { source: 'fetch_metrics', target: 'check_threshold' },
        { source: 'check_threshold', target: 'analyze', condition_label: 'true' },
        { source: 'analyze', target: 'send_wecom' },
      ],
    },
  },

  // ── 电商 & 支付 ────────────────────────────────────────────────────────────
  {
    id: 'shopify-order-notify',
    name: 'Shopify Order Processor',
    name_zh: 'Shopify 新订单处理',
    description: 'Receive Shopify order webhooks, validate order data, send confirmation email, and notify the fulfillment team on Slack.',
    description_zh: '接收 Shopify 订单 Webhook，校验订单数据，自动发送确认邮件给客户，并通过 Slack 通知履单团队。',
    category: 'Ecommerce',
    category_zh: '电商',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: { webhook_secret: '{{credential.shopify_webhook_secret}}' } },
        { id: 'validate', type: 'validate', config: { source: '{{input}}', schema: '{"required":["id","email","total_price","line_items"]}', fail_on_invalid: true } },
        { id: 'confirm_email', type: 'email', config: { to: '{{input.email}}', subject: '订单确认 #{{input.order_number}} — 感谢您的购买！', body: '您好 {{input.billing_address.name}}，\n\n感谢您的订单 #{{input.order_number}}，金额 {{input.total_price}} {{input.currency}}。\n\n我们将尽快为您发货。', api_key: '{{credential.sendgrid_key}}', from: 'orders@yourshop.com' } },
        { id: 'notify_team', type: 'slack', config: { webhook_url: '{{credential.slack_webhook}}', text: '🛒 新订单 #{{input.order_number}}\n客户：{{input.email}}\n金额：{{input.total_price}} {{input.currency}}\n商品数：{{input.line_items.length}}', username: 'ShopifyBot' } },
        { id: 'catch_error', type: 'catch', config: {} },
        { id: 'alert_error', type: 'slack', config: { webhook_url: '{{credential.slack_webhook}}', text: '❌ 订单处理失败：{{catch_error.error}}' } },
      ],
      edges: [
        { source: 'trigger', target: 'validate' },
        { source: 'validate', target: 'confirm_email' },
        { source: 'confirm_email', target: 'notify_team' },
        { source: 'validate', target: 'catch_error', condition_label: 'error' },
        { source: 'catch_error', target: 'alert_error' },
      ],
    },
  },
  {
    id: 'stripe-payment-handler',
    name: 'Stripe Payment Success Handler',
    name_zh: 'Stripe 支付成功处理',
    description: 'Process Stripe payment_intent.succeeded webhooks: update the database, send receipt email, notify Slack.',
    description_zh: '监听 Stripe payment_intent.succeeded 事件，更新数据库订单状态，发送收据邮件，Slack 通知财务团队。',
    category: 'Ecommerce',
    category_zh: '电商',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: { webhook_secret: '{{credential.stripe_webhook_secret}}' } },
        { id: 'check_event', type: 'condition', config: { field: 'type', operator: 'equals', value: 'payment_intent.succeeded', source: '{{input}}' } },
        { id: 'update_order', type: 'database', config: { url: '{{credential.pg_url}}', query: "UPDATE orders SET status='paid', paid_at=NOW() WHERE stripe_payment_id='{{input.data.object.id}}' RETURNING id, customer_email, amount" } },
        { id: 'send_receipt', type: 'email', config: { to: '{{update_order.rows.0.customer_email}}', subject: '收款确认 — ¥{{input.data.object.amount_received}}', body: '您好，\n\n我们已收到您的付款 ¥{{input.data.object.amount_received}}（订单 ID: {{update_order.rows.0.id}}）。\n\n如有疑问请联系 support@example.com。', api_key: '{{credential.sendgrid_key}}', from: 'finance@example.com' } },
        { id: 'notify_finance', type: 'slack', config: { webhook_url: '{{credential.slack_webhook}}', text: '💰 收款成功 ¥{{input.data.object.amount_received}}\n订单 ID: {{update_order.rows.0.id}}\nStripe ID: {{input.data.object.id}}', username: 'StripeBot' } },
      ],
      edges: [
        { source: 'trigger', target: 'check_event' },
        { source: 'check_event', target: 'update_order', condition_label: 'true' },
        { source: 'update_order', target: 'send_receipt' },
        { source: 'send_receipt', target: 'notify_finance' },
      ],
    },
  },
  {
    id: 'inventory-low-alert',
    name: 'Low Inventory Alert',
    name_zh: '库存预警 & 补货申请',
    description: 'Run daily to query product inventory, find items below threshold, and send alerts via Slack and email.',
    description_zh: '每天定时查询商品库存，过滤低于阈值的 SKU，并行发送 Slack 告警和邮件补货申请，支持自定义阈值。',
    category: 'Ecommerce',
    category_zh: '电商',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: { cron_expression: '0 0 8 * * *' } },
        { id: 'query_stock', type: 'database', config: { url: '{{credential.pg_url}}', query: 'SELECT sku, name, stock_qty, reorder_point FROM products WHERE stock_qty <= reorder_point ORDER BY stock_qty ASC' } },
        { id: 'check_empty', type: 'condition', config: { field: 'count', operator: 'gt', value: '0', source: '{{query_stock}}' } },
        { id: 'format_list', type: 'map', config: { items: '{{query_stock.rows}}', item_template: '{{item.sku}}: {{item.name}} (剩余 {{item.stock_qty}}, 补货点 {{item.reorder_point}})' } },
        { id: 'fan_out', type: 'fan_out', config: {} },
        { id: 'slack_alert', type: 'slack', config: { webhook_url: '{{credential.slack_webhook}}', text: '⚠️ 库存预警：{{query_stock.count}} 个 SKU 低于补货点\n{{format_list.items}}', username: 'StockBot' } },
        { id: 'email_alert', type: 'email', config: { to: '{{input.supply_manager_email}}', subject: '库存预警：{{query_stock.count}} 个 SKU 需要补货', body: '以下商品库存不足，请及时补货：\n\n{{format_list.items}}', api_key: '{{credential.sendgrid_key}}', from: 'alerts@example.com' } },
        { id: 'fan_in', type: 'fan_in', config: {} },
      ],
      edges: [
        { source: 'trigger', target: 'query_stock' },
        { source: 'query_stock', target: 'check_empty' },
        { source: 'check_empty', target: 'format_list', condition_label: 'true' },
        { source: 'format_list', target: 'fan_out' },
        { source: 'fan_out', target: 'slack_alert' },
        { source: 'fan_out', target: 'email_alert' },
        { source: 'slack_alert', target: 'fan_in' },
        { source: 'email_alert', target: 'fan_in' },
      ],
    },
  },
  {
    id: 'refund-approval',
    name: 'Smart Refund Approval',
    name_zh: '智能退款审批流程',
    description: 'Auto-approve small refunds, route large ones to human approval, then process via Stripe and notify the customer.',
    description_zh: '小额退款（≤100元）自动审批，大额退款路由人工审核，审批通过后调 Stripe 退款接口并发邮件通知客户。',
    category: 'Ecommerce',
    category_zh: '电商',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: { webhook_secret: '' } },
        { id: 'check_amount', type: 'condition', config: { field: 'amount', operator: 'lt', value: '100', source: '{{input}}' } },
        { id: 'auto_approve', type: 'transform', config: { template: { approved: true, method: 'auto', reason: '金额低于自动审批阈值' } } },
        { id: 'human_review', type: 'approval', config: {} },
        { id: 'process_refund', type: 'http', config: { method: 'POST', url: 'https://api.stripe.com/v1/refunds', auth_token: '{{credential.stripe_secret_key}}', body: 'payment_intent={{input.payment_intent_id}}&amount={{input.amount}}' } },
        { id: 'notify_customer', type: 'email', config: { to: '{{input.customer_email}}', subject: '退款已处理', body: '您好，\n\n您的退款申请（金额：¥{{input.amount}}）已成功处理，款项将在 3-5 个工作日内到账。', api_key: '{{credential.sendgrid_key}}', from: 'support@example.com' } },
      ],
      edges: [
        { source: 'trigger', target: 'check_amount' },
        { source: 'check_amount', target: 'auto_approve', condition_label: 'true' },
        { source: 'check_amount', target: 'human_review', condition_label: 'false' },
        { source: 'auto_approve', target: 'process_refund' },
        { source: 'human_review', target: 'process_refund' },
        { source: 'process_refund', target: 'notify_customer' },
      ],
    },
  },

  // ── HR & 办公协同 ──────────────────────────────────────────────────────────
  {
    id: 'employee-onboarding',
    name: 'Employee Onboarding Flow',
    name_zh: '新员工入职自动化',
    description: 'Triggered by HR system: create accounts, send welcome email, invite to Slack, create calendar onboarding event.',
    description_zh: '人事系统触发：自动创建系统账号、发送欢迎邮件、发送 Slack 欢迎消息、创建入职培训日历事件，全程无需人工操作。',
    category: 'HR',
    category_zh: 'HR',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: { webhook_secret: '' } },
        { id: 'create_account', type: 'http', config: { method: 'POST', url: 'https://api.example.com/users', auth_token: '{{credential.admin_api_key}}', body: '{"email":"{{input.email}}","name":"{{input.name}}","department":"{{input.department}}","role":"{{input.role}}"}' } },
        { id: 'fan_out', type: 'fan_out', config: {} },
        { id: 'welcome_email', type: 'email', config: { to: '{{input.email}}', subject: '欢迎加入 {{input.company_name}}！', body: '亲爱的 {{input.name}}，\n\n欢迎加入我们！您的账号已创建，用户名为 {{input.email}}。\n\n入职日期：{{input.start_date}}\n部门：{{input.department}}\n直属上司：{{input.manager_name}}\n\n期待与您共事！', api_key: '{{credential.sendgrid_key}}', from: 'hr@example.com' } },
        { id: 'slack_welcome', type: 'slack', config: { webhook_url: '{{credential.slack_webhook}}', text: '🎉 欢迎新同事 *{{input.name}}* 加入 {{input.department}} 部门！入职日期：{{input.start_date}}。大家热烈欢迎 👏', username: 'HRBot' } },
        { id: 'create_calendar', type: 'gcal', config: { access_token: '{{credential.gcal_token}}', operation: 'create_event', calendar_id: 'primary', summary: '{{input.name}} 入职培训', start_time: '{{input.start_date}}T09:00:00', end_time: '{{input.start_date}}T17:00:00' } },
        { id: 'fan_in', type: 'fan_in', config: {} },
        { id: 'notify_manager', type: 'slack', config: { webhook_url: '{{credential.slack_webhook}}', text: '✅ {{input.name}} 的入职准备已完成：账号已创建、欢迎邮件已发送、日历事件已创建。', username: 'HRBot' } },
      ],
      edges: [
        { source: 'trigger', target: 'create_account' },
        { source: 'create_account', target: 'fan_out' },
        { source: 'fan_out', target: 'welcome_email' },
        { source: 'fan_out', target: 'slack_welcome' },
        { source: 'fan_out', target: 'create_calendar' },
        { source: 'welcome_email', target: 'fan_in' },
        { source: 'slack_welcome', target: 'fan_in' },
        { source: 'create_calendar', target: 'fan_in' },
        { source: 'fan_in', target: 'notify_manager' },
      ],
    },
  },
  {
    id: 'leave-request-approval',
    name: 'Leave Request Approval',
    name_zh: '请假审批流程',
    description: 'Employee submits leave request via webhook; short leaves auto-approve, long ones go to manager approval, then update the HR system.',
    description_zh: '员工通过表单提交请假申请：3 天以内自动审批，3 天以上路由主管审批，审批结果同步 HR 系统并发邮件通知。',
    category: 'HR',
    category_zh: 'HR',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: { webhook_secret: '' } },
        { id: 'validate', type: 'validate', config: { source: '{{input}}', schema: '{"required":["employee_id","start_date","end_date","reason"]}', fail_on_invalid: true } },
        { id: 'calc_days', type: 'math', config: { operation: 'eval', expression: '{{input.days}}' } },
        { id: 'check_duration', type: 'condition', config: { field: 'days', operator: 'lte', value: '3', source: '{{input}}' } },
        { id: 'auto_approve', type: 'transform', config: { template: { approved: true, approver: 'system', note: '自动审批（≤3天）' } } },
        { id: 'manager_approval', type: 'approval', config: {} },
        { id: 'update_hr', type: 'http', config: { method: 'PUT', url: 'https://api.hr-system.com/leaves/{{input.request_id}}', auth_token: '{{credential.hr_api_key}}', body: '{"status":"approved","start_date":"{{input.start_date}}","end_date":"{{input.end_date}}"}' } },
        { id: 'notify_employee', type: 'email', config: { to: '{{input.employee_email}}', subject: '请假申请已批准', body: '您好 {{input.employee_name}}，\n\n您的请假申请（{{input.start_date}} 至 {{input.end_date}}，共 {{input.days}} 天）已批准。\n\n祝休假愉快！', api_key: '{{credential.sendgrid_key}}', from: 'hr@example.com' } },
      ],
      edges: [
        { source: 'trigger', target: 'validate' },
        { source: 'validate', target: 'calc_days' },
        { source: 'calc_days', target: 'check_duration' },
        { source: 'check_duration', target: 'auto_approve', condition_label: 'true' },
        { source: 'check_duration', target: 'manager_approval', condition_label: 'false' },
        { source: 'auto_approve', target: 'update_hr' },
        { source: 'manager_approval', target: 'update_hr' },
        { source: 'update_hr', target: 'notify_employee' },
      ],
    },
  },
  {
    id: 'interview-reminder',
    name: 'Interview Scheduler & Reminder',
    name_zh: '面试日程安排与提醒',
    description: 'When a candidate confirms an interview, create a calendar event, send confirmation emails to both parties, and set a Slack reminder.',
    description_zh: '候选人确认面试后，自动创建 Google Calendar 事件，向候选人和面试官分别发确认邮件，Slack 提前30分钟提醒面试官。',
    category: 'HR',
    category_zh: 'HR',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: { webhook_secret: '' } },
        { id: 'create_event', type: 'gcal', config: { access_token: '{{credential.gcal_token}}', operation: 'create_event', calendar_id: 'primary', summary: '面试：{{input.candidate_name}} — {{input.position}}', start_time: '{{input.interview_time}}', end_time: '{{input.interview_end_time}}' } },
        { id: 'fan_out', type: 'fan_out', config: {} },
        { id: 'email_candidate', type: 'email', config: { to: '{{input.candidate_email}}', subject: '面试确认 — {{input.position}} 职位', body: '您好 {{input.candidate_name}}，\n\n您的面试已安排：\n时间：{{input.interview_time}}\n地点/链接：{{input.location}}\n面试官：{{input.interviewer_name}}\n\n请准时参加，祝面试顺利！', api_key: '{{credential.sendgrid_key}}', from: 'recruit@example.com' } },
        { id: 'email_interviewer', type: 'email', config: { to: '{{input.interviewer_email}}', subject: '面试提醒 — {{input.candidate_name}}（{{input.position}}）', body: '您好，\n\n面试安排如下：\n时间：{{input.interview_time}}\n候选人：{{input.candidate_name}}\n应聘职位：{{input.position}}\n简历链接：{{input.resume_url}}', api_key: '{{credential.sendgrid_key}}', from: 'recruit@example.com' } },
        { id: 'slack_remind', type: 'slack', config: { webhook_url: '{{credential.slack_webhook}}', text: '📅 面试提醒：{{input.interviewer_name}} 将在 {{input.interview_time}} 面试候选人 {{input.candidate_name}}（{{input.position}}）', username: 'Recruit Bot' } },
        { id: 'fan_in', type: 'fan_in', config: {} },
      ],
      edges: [
        { source: 'trigger', target: 'create_event' },
        { source: 'create_event', target: 'fan_out' },
        { source: 'fan_out', target: 'email_candidate' },
        { source: 'fan_out', target: 'email_interviewer' },
        { source: 'fan_out', target: 'slack_remind' },
        { source: 'email_candidate', target: 'fan_in' },
        { source: 'email_interviewer', target: 'fan_in' },
        { source: 'slack_remind', target: 'fan_in' },
      ],
    },
  },

  // ── AI RAG & 知识库 ────────────────────────────────────────────────────────
  {
    id: 'doc-vectorize-pinecone',
    name: 'Document Vectorization Pipeline',
    name_zh: '文档向量化入库（Pinecone）',
    description: 'Split a document into chunks, generate OpenAI embeddings for each chunk, and upsert into Pinecone for semantic search.',
    description_zh: '将文档分块，对每个分块用 OpenAI text-embedding-3-small 生成向量，批量 upsert 到 Pinecone，支撑后续语义搜索。',
    category: 'AI',
    category_zh: 'AI',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: {} },
        { id: 'split_chunks', type: 'split', config: { source: '{{input.content}}', delimiter: '\n\n' } },
        { id: 'gen_embeddings', type: 'http', config: { method: 'POST', url: 'https://api.openai.com/v1/embeddings', auth_token: '{{credential.openai_key}}', body: '{"model":"text-embedding-3-small","input":{{split_chunks.parts}}}' } },
        { id: 'build_vectors', type: 'code', config: { script: 'let parts = nodes.split_chunks.parts;\nlet data = nodes.gen_embeddings.body.data;\nlet doc_id = input.doc_id;\nlet out = [];\nfor i in 0..parts.len() {\n  out.push(#{ id: doc_id + "-" + i, values: data[i].embedding, metadata: #{ doc_id: doc_id, chunk_index: i, text: parts[i] } });\n}\n#{ vectors: out, count: parts.len() }' } },
        { id: 'upsert_pinecone', type: 'pinecone', config: { api_key: '{{credential.pinecone_key}}', index_host: '{{credential.pinecone_host}}', operation: 'upsert', vectors: '{{build_vectors.vectors}}', namespace: '{{input.namespace}}' } },
        { id: 'confirm', type: 'transform', config: { template: { ok: true, doc_id: '{{input.doc_id}}', chunks_stored: '{{split_chunks.count}}', namespace: '{{input.namespace}}' } } },
      ],
      edges: [
        { source: 'trigger', target: 'split_chunks' },
        { source: 'split_chunks', target: 'gen_embeddings' },
        { source: 'gen_embeddings', target: 'build_vectors' },
        { source: 'build_vectors', target: 'upsert_pinecone' },
        { source: 'upsert_pinecone', target: 'confirm' },
      ],
    },
  },
  {
    id: 'rag-qa-bot',
    name: 'RAG Question-Answer Bot',
    name_zh: 'RAG 语义问答机器人',
    description: 'Embed the user query, retrieve relevant chunks from Pinecone, then use Claude to generate a grounded answer.',
    description_zh: '对用户问题生成向量，从 Pinecone 检索最相关的 Top-5 分块，Claude 基于上下文生成有来源依据的答案。',
    category: 'AI',
    category_zh: 'AI',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: {} },
        { id: 'embed_query', type: 'http', config: { method: 'POST', url: 'https://api.openai.com/v1/embeddings', auth_token: '{{credential.openai_key}}', body: '{"model":"text-embedding-3-small","input":"{{input.question}}"}' } },
        { id: 'extract_vec', type: 'extract', config: { source: '{{embed_query}}', path: 'body.data.0.embedding' } },
        { id: 'search_pinecone', type: 'pinecone', config: { api_key: '{{credential.pinecone_key}}', index_host: '{{credential.pinecone_host}}', operation: 'query', vector: '{{extract_vec.value}}', top_k: 5, include_metadata: true, namespace: '{{input.namespace}}' } },
        { id: 'extract_context', type: 'map', config: { items: '{{search_pinecone.body.matches}}', item_template: '{{item.metadata.text}}' } },
        { id: 'join_context', type: 'join', config: { items: '{{extract_context.items}}', delimiter: '\n\n---\n\n' } },
        { id: 'answer', type: 'claude', config: { model: 'claude-sonnet-4-6', api_key: '{{credential.anthropic_key}}', system_prompt: '你是一个专业的问答助手。请基于提供的上下文内容回答问题，如果上下文不包含相关信息，请明确说明。', prompt_template: '上下文：\n{{join_context.result}}\n\n问题：{{input.question}}', max_tokens: 1000, temperature: 0.3 } },
        { id: 'result', type: 'transform', config: { template: { answer: '{{answer.content}}', question: '{{input.question}}', sources_count: '{{extract_context.count}}' } } },
      ],
      edges: [
        { source: 'trigger', target: 'embed_query' },
        { source: 'embed_query', target: 'extract_vec' },
        { source: 'extract_vec', target: 'search_pinecone' },
        { source: 'search_pinecone', target: 'extract_context' },
        { source: 'extract_context', target: 'join_context' },
        { source: 'join_context', target: 'answer' },
        { source: 'answer', target: 'result' },
      ],
    },
  },
  {
    id: 'kb-sync-qdrant',
    name: 'Knowledge Base Sync to Qdrant',
    name_zh: '知识库同步到 Qdrant',
    description: 'Periodically fetch updated knowledge base articles, embed them, and upsert into Qdrant for vector search.',
    description_zh: '每天定时拉取知识库更新文章，调 OpenAI 生成嵌入向量，批量 upsert 到 Qdrant，保持向量库实时同步。',
    category: 'AI',
    category_zh: 'AI',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: { cron_expression: '0 0 2 * * *' } },
        { id: 'fetch_articles', type: 'http', config: { method: 'GET', url: 'https://api.example.com/kb/articles?updated_since={{input.last_sync}}', auth_token: '{{credential.kb_api_key}}' } },
        { id: 'check_count', type: 'condition', config: { field: 'body.count', operator: 'gt', value: '0', source: '{{fetch_articles}}' } },
        { id: 'prep_inputs', type: 'code', config: { script: 'let arts = nodes.fetch_articles.body.articles;\nlet out = [];\nfor a in arts {\n  out.push(a.content);\n}\n#{ contents: out }' } },
        { id: 'embed_articles', type: 'http', config: { method: 'POST', url: 'https://api.openai.com/v1/embeddings', auth_token: '{{credential.openai_key}}', body: '{"model":"text-embedding-3-small","input":{{prep_inputs.contents}}}' } },
        { id: 'build_points', type: 'code', config: { script: 'let arts = nodes.fetch_articles.body.articles;\nlet data = nodes.embed_articles.body.data;\nlet out = [];\nfor i in 0..arts.len() {\n  let a = arts[i];\n  out.push(#{ id: a.id, vector: data[i].embedding, payload: #{ title: a.title, url: a.url, updated_at: a.updated_at } });\n}\n#{ points: out, count: arts.len() }' } },
        { id: 'upsert_qdrant', type: 'qdrant', config: { url: '{{credential.qdrant_url}}', api_key: '{{credential.qdrant_key}}', collection: 'knowledge_base', operation: 'upsert', points: '{{build_points.points}}' } },
        { id: 'notify', type: 'slack', config: { webhook_url: '{{credential.slack_webhook}}', text: '📚 知识库同步完成：{{fetch_articles.body.count}} 篇文章已更新向量索引', username: 'KBBot' } },
      ],
      edges: [
        { source: 'trigger', target: 'fetch_articles' },
        { source: 'fetch_articles', target: 'check_count' },
        { source: 'check_count', target: 'prep_inputs', condition_label: 'true' },
        { source: 'prep_inputs', target: 'embed_articles' },
        { source: 'embed_articles', target: 'build_points' },
        { source: 'build_points', target: 'upsert_qdrant' },
        { source: 'upsert_qdrant', target: 'notify' },
      ],
    },
  },

  // ── DevOps & 监控告警 ──────────────────────────────────────────────────────
  {
    id: 'api-health-monitor',
    name: 'API Health Monitor',
    name_zh: 'API 健康巡检告警',
    description: 'Every 5 minutes, check multiple API endpoints, route failures to PagerDuty and Slack, auto-recover on success.',
    description_zh: '每 5 分钟巡检关键 API 端点，响应异常时同时触发 PagerDuty 告警和 Slack 通知，恢复正常后自动发送恢复通知。',
    category: 'DevOps',
    category_zh: 'DevOps',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: { interval_secs: 300 } },
        { id: 'check_api', type: 'http', config: { method: 'GET', url: '{{input.health_check_url}}', timeout_secs: 10 } },
        { id: 'catch_failure', type: 'catch', config: {} },
        { id: 'check_status', type: 'condition', config: { field: 'status', operator: 'lt', value: '400', source: '{{check_api}}' } },
        { id: 'fan_out_alert', type: 'fan_out', config: {} },
        { id: 'pagerduty_alert', type: 'pagerduty', config: { routing_key: '{{credential.pd_routing_key}}', summary: '{{input.service_name}} API 健康检查失败', event_action: 'trigger', severity: 'critical', source: '{{input.health_check_url}}', dedup_key: 'health-{{input.service_name}}' } },
        { id: 'slack_alert', type: 'slack', config: { webhook_url: '{{credential.slack_webhook}}', text: '🚨 *{{input.service_name}}* 健康检查失败！\nURL: {{input.health_check_url}}\n错误: {{catch_failure.error}}\n时间: {{input.run_time}}', username: 'MonitorBot' } },
        { id: 'fan_in_alert', type: 'fan_in', config: {} },
        { id: 'ok_notify', type: 'slack', config: { webhook_url: '{{credential.slack_webhook}}', text: '✅ {{input.service_name}} 健康检查正常（状态码 {{check_api.status}}）', username: 'MonitorBot' } },
      ],
      edges: [
        { source: 'trigger', target: 'check_api' },
        { source: 'check_api', target: 'check_status' },
        { source: 'check_api', target: 'catch_failure', condition_label: 'error' },
        { source: 'catch_failure', target: 'fan_out_alert' },
        { source: 'check_status', target: 'fan_out_alert', condition_label: 'false' },
        { source: 'fan_out_alert', target: 'pagerduty_alert' },
        { source: 'fan_out_alert', target: 'slack_alert' },
        { source: 'pagerduty_alert', target: 'fan_in_alert' },
        { source: 'slack_alert', target: 'fan_in_alert' },
        { source: 'check_status', target: 'ok_notify', condition_label: 'true' },
      ],
    },
  },
  {
    id: 'jira-bug-triage',
    name: 'AI Bug Triage & Assignment',
    name_zh: 'AI Bug 自动分类分配',
    description: 'Receive bug reports via webhook, use Claude to classify severity and suggest assignee, create Jira issue, notify on Slack.',
    description_zh: '通过 Webhook 接收 Bug 报告，Claude 自动判断严重程度并建议负责人，自动在 Jira 创建工单并设置优先级，Slack 通知研发团队。',
    category: 'DevOps',
    category_zh: 'DevOps',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: { webhook_secret: '' } },
        { id: 'classify', type: 'claude', config: { model: 'claude-haiku-4-5-20251001', api_key: '{{credential.anthropic_key}}', system_prompt: 'You are a bug triage assistant. Respond with JSON only: {"severity":"critical|high|medium|low","component":"frontend|backend|infra|data","suggested_assignee":"...","summary":"one sentence"}', prompt_template: 'Triage this bug:\nTitle: {{input.title}}\nDescription: {{input.description}}\nEnvironment: {{input.environment}}', max_tokens: 300, temperature: 0.1 } },
        { id: 'create_issue', type: 'jira', config: { base_url: '{{credential.jira_base_url}}', email: '{{credential.jira_email}}', token: '{{credential.jira_token}}', endpoint: '/rest/api/3/issue', method: 'POST', body: '{"fields":{"project":{"key":"{{input.project_key}}"},"summary":"[{{classify.content.severity}}] {{input.title}}","description":{"type":"doc","version":1,"content":[{"type":"paragraph","content":[{"type":"text","text":"{{input.description}}"}]}]},"issuetype":{"name":"Bug"},"priority":{"name":"{{classify.content.severity}}"}}}' } },
        { id: 'notify_team', type: 'slack', config: { webhook_url: '{{credential.slack_webhook}}', text: '🐛 新 Bug 工单已创建\n*[{{classify.content.severity}}]* {{input.title}}\nJira: {{create_issue.body.key}}\n建议负责人：{{classify.content.suggested_assignee}}\n摘要：{{classify.content.summary}}', username: 'BugBot' } },
      ],
      edges: [
        { source: 'trigger', target: 'classify' },
        { source: 'classify', target: 'create_issue' },
        { source: 'create_issue', target: 'notify_team' },
      ],
    },
  },
  {
    id: 'deploy-notification',
    name: 'Deployment Notification',
    name_zh: '部署通知流水线',
    description: 'Triggered by CI/CD webhook: announce deployment to Slack, update GitHub commit status, email the team lead.',
    description_zh: 'CI/CD 完成触发：更新 GitHub commit 状态标记，向 Slack 发布部署公告，发邮件通知团队负责人，部署失败时发告警。',
    category: 'DevOps',
    category_zh: 'DevOps',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: { webhook_secret: '' } },
        { id: 'check_status', type: 'condition', config: { field: 'status', operator: 'equals', value: 'success', source: '{{input}}' } },
        { id: 'update_github', type: 'github', config: { token: '{{credential.github_token}}', method: 'POST', endpoint: '/repos/{{input.repo}}/statuses/{{input.sha}}', body: '{"state":"success","description":"Deployed to {{input.environment}}","context":"deploy/{{input.environment}}"}' } },
        { id: 'slack_success', type: 'slack', config: { webhook_url: '{{credential.slack_webhook}}', text: '🚀 *部署成功*\n服务：{{input.service}}\n环境：{{input.environment}}\n版本：{{input.version}}\n提交：{{input.sha}}\n耗时：{{input.duration_secs}}s', username: 'DeployBot' } },
        { id: 'slack_failure', type: 'slack', config: { webhook_url: '{{credential.slack_webhook}}', text: '❌ *部署失败*\n服务：{{input.service}}\n环境：{{input.environment}}\n错误：{{input.error}}\n日志：{{input.log_url}}', username: 'DeployBot' } },
        { id: 'email_lead', type: 'email', config: { to: '{{input.team_lead_email}}', subject: '✅ {{input.service}} 已部署到 {{input.environment}}（v{{input.version}}）', body: '部署详情：\n\n服务：{{input.service}}\n环境：{{input.environment}}\n版本：{{input.version}}\nGit SHA：{{input.sha}}\n\n部署日志：{{input.log_url}}', api_key: '{{credential.sendgrid_key}}', from: 'devops@example.com' } },
      ],
      edges: [
        { source: 'trigger', target: 'check_status' },
        { source: 'check_status', target: 'update_github', condition_label: 'true' },
        { source: 'update_github', target: 'slack_success' },
        { source: 'slack_success', target: 'email_lead' },
        { source: 'check_status', target: 'slack_failure', condition_label: 'false' },
      ],
    },
  },

  // ── CRM & 客户管理 ─────────────────────────────────────────────────────────
  {
    id: 'crm-lead-sync',
    name: 'CRM Lead Sync (HubSpot + Salesforce)',
    name_zh: 'CRM 线索双向同步',
    description: 'Sync new leads between HubSpot and Salesforce in real-time, notify the sales rep on Slack.',
    description_zh: '新线索提交触发，同时创建 HubSpot 联系人和 Salesforce 潜在客户记录，Fan-In 确认同步完成后通知销售代表。',
    category: 'CRM',
    category_zh: 'CRM',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: { webhook_secret: '' } },
        { id: 'validate', type: 'validate', config: { source: '{{input}}', schema: '{"required":["email","first_name","last_name","company"]}', fail_on_invalid: true } },
        { id: 'fan_out', type: 'fan_out', config: {} },
        { id: 'create_hubspot', type: 'hubspot', config: { token: '{{credential.hubspot_token}}', endpoint: '/crm/v3/objects/contacts', method: 'POST', body: '{"properties":{"email":"{{input.email}}","firstname":"{{input.first_name}}","lastname":"{{input.last_name}}","company":"{{input.company}}","phone":"{{input.phone}}"}}' } },
        { id: 'create_salesforce', type: 'salesforce', config: { token: '{{credential.sf_token}}', instance_url: '{{credential.sf_instance_url}}', endpoint: '/services/data/v58.0/sobjects/Lead', method: 'POST', body: '{"FirstName":"{{input.first_name}}","LastName":"{{input.last_name}}","Email":"{{input.email}}","Company":"{{input.company}}","Phone":"{{input.phone}}","LeadSource":"{{input.source}}"}' } },
        { id: 'fan_in', type: 'fan_in', config: {} },
        { id: 'notify_sales', type: 'slack', config: { webhook_url: '{{credential.slack_webhook}}', text: '👤 新线索同步完成\n姓名：{{input.first_name}} {{input.last_name}}\n公司：{{input.company}}\n邮箱：{{input.email}}\nHubSpot ID：{{create_hubspot.id}}\n来源：{{input.source}}', username: 'CRMBot' } },
      ],
      edges: [
        { source: 'trigger', target: 'validate' },
        { source: 'validate', target: 'fan_out' },
        { source: 'fan_out', target: 'create_hubspot' },
        { source: 'fan_out', target: 'create_salesforce' },
        { source: 'create_hubspot', target: 'fan_in' },
        { source: 'create_salesforce', target: 'fan_in' },
        { source: 'fan_in', target: 'notify_sales' },
      ],
    },
  },
  {
    id: 'churn-risk-alert',
    name: 'Customer Churn Risk Alert',
    name_zh: '客户流失风险预警',
    description: 'Daily: identify inactive customers via DB query, score churn risk with AI, alert CS team for high-risk accounts.',
    description_zh: '每天查询 30 天无登录客户，Claude 根据使用数据评分流失风险，高风险客户推送给客户成功团队优先跟进。',
    category: 'CRM',
    category_zh: 'CRM',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: { cron_expression: '0 0 9 * * *' } },
        { id: 'query_inactive', type: 'database', config: { url: '{{credential.pg_url}}', query: "SELECT id, email, company, plan, last_login, usage_score FROM customers WHERE last_login < NOW() - INTERVAL '30 days' AND status='active' ORDER BY usage_score ASC LIMIT 20" } },
        { id: 'check_count', type: 'condition', config: { field: 'count', operator: 'gt', value: '0', source: '{{query_inactive}}' } },
        { id: 'analyze_risk', type: 'claude', config: { model: 'claude-haiku-4-5-20251001', api_key: '{{credential.anthropic_key}}', system_prompt: '根据客户数据评估流失风险。输出 JSON：{"high_risk":[],"medium_risk":[]}, 每项包含 email 和 reason。', prompt_template: '以下客户近30天未登录，请评估流失风险：\n{{query_inactive.rows}}', max_tokens: 800, temperature: 0.2 } },
        { id: 'notify_cs', type: 'slack', config: { webhook_url: '{{credential.slack_webhook}}', text: '⚠️ 客户流失风险预警（今日）\n\n*高风险客户：*\n{{analyze_risk.content.high_risk}}\n\n请优先联系以上客户。', username: 'ChurnBot' } },
        { id: 'email_cs_team', type: 'email', config: { to: '{{input.cs_team_email}}', subject: '【流失预警】{{query_inactive.count}} 位客户需要跟进', body: 'AI 分析结果：\n\n{{analyze_risk.content}}\n\n详情请查看 CRM 系统。', api_key: '{{credential.sendgrid_key}}', from: 'alerts@example.com' } },
      ],
      edges: [
        { source: 'trigger', target: 'query_inactive' },
        { source: 'query_inactive', target: 'check_count' },
        { source: 'check_count', target: 'analyze_risk', condition_label: 'true' },
        { source: 'analyze_risk', target: 'notify_cs' },
        { source: 'notify_cs', target: 'email_cs_team' },
      ],
    },
  },
  {
    id: 'nps-low-score-followup',
    name: 'NPS Low Score Follow-up',
    name_zh: 'NPS 低分自动跟进',
    description: 'When NPS score < 7 is submitted, draft a personalized follow-up email with Claude, require approval, then send.',
    description_zh: 'NPS 评分低于 7 分时触发，Claude 根据反馈内容起草个性化挽留邮件，主管审批后自动发送，同时更新 CRM 记录。',
    category: 'CRM',
    category_zh: 'CRM',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: { webhook_secret: '' } },
        { id: 'check_score', type: 'condition', config: { field: 'score', operator: 'lt', value: '7', source: '{{input}}' } },
        { id: 'draft_email', type: 'claude', config: { model: 'claude-sonnet-4-6', api_key: '{{credential.anthropic_key}}', system_prompt: '你是客户成功经理，撰写真诚、个性化的挽留邮件，不超过150字，不要模板化。', prompt_template: '客户：{{input.customer_name}}（{{input.company}}）\nNPS 评分：{{input.score}}/10\n反馈内容：{{input.feedback}}\n\n请起草一封针对性的跟进邮件。', max_tokens: 400, temperature: 0.6 } },
        { id: 'review_gate', type: 'approval', config: {} },
        { id: 'send_email', type: 'email', config: { to: '{{input.customer_email}}', subject: '感谢您的反馈，我们想了解更多', body: '{{draft_email.content}}', api_key: '{{credential.sendgrid_key}}', from: 'success@example.com' } },
        { id: 'update_crm', type: 'hubspot', config: { token: '{{credential.hubspot_token}}', endpoint: '/crm/v3/objects/contacts/{{input.hubspot_contact_id}}', method: 'PATCH', body: '{"properties":{"nps_score":"{{input.score}}","nps_followup_sent":"true","last_nps_date":"{{input.submitted_at}}"}}' } },
        { id: 'notify_cs', type: 'slack', config: { webhook_url: '{{credential.slack_webhook}}', text: '📧 NPS 跟进邮件已发送给 {{input.customer_name}}（{{input.company}}，评分 {{input.score}}/10）', username: 'NPSBot' } },
      ],
      edges: [
        { source: 'trigger', target: 'check_score' },
        { source: 'check_score', target: 'draft_email', condition_label: 'true' },
        { source: 'draft_email', target: 'review_gate' },
        { source: 'review_gate', target: 'send_email' },
        { source: 'send_email', target: 'update_crm' },
        { source: 'update_crm', target: 'notify_cs' },
      ],
    },
  },

  // ── 社交媒体 & 内容 ────────────────────────────────────────────────────────
  {
    id: 'content-multiplatform-publish',
    name: 'Multi-Platform Content Publisher',
    name_zh: '多平台内容一键分发',
    description: 'Generate platform-adapted content variations with Claude, then fan-out to Slack, Discord, Teams, and a webhook simultaneously.',
    description_zh: '输入原始内容，Claude 自动改写为适合各平台风格的版本，Fan-Out 同时推送到 Slack、Discord、Teams 和自定义 Webhook。',
    category: 'Marketing',
    category_zh: '营销',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: {} },
        { id: 'adapt_content', type: 'claude', config: { model: 'claude-haiku-4-5-20251001', api_key: '{{credential.anthropic_key}}', system_prompt: '你是社交媒体运营专家。将内容改写为适合各平台的版本，输出 JSON：{"slack":"...","discord":"...","teams":"..."}', prompt_template: '原始内容：\n{{input.content}}\n\n请分别适配 Slack（支持 Markdown *bold*）、Discord（支持 **bold** 和 emoji）、Teams（简洁正式）。', max_tokens: 600, temperature: 0.5 } },
        { id: 'fan_out', type: 'fan_out', config: {} },
        { id: 'post_slack', type: 'slack', config: { webhook_url: '{{credential.slack_webhook}}', text: '{{adapt_content.content.slack}}', username: 'ContentBot' } },
        { id: 'post_discord', type: 'discord', config: { webhook_url: '{{credential.discord_webhook}}', content: '{{adapt_content.content.discord}}' } },
        { id: 'post_teams', type: 'teams', config: { webhook_url: '{{credential.teams_webhook}}', text: '{{adapt_content.content.teams}}', title: '{{input.title}}' } },
        { id: 'fan_in', type: 'fan_in', config: {} },
        { id: 'record', type: 'transform', config: { template: { published: true, platforms: 3, content_id: '{{input.content_id}}', published_at: '{{input.run_time}}' } } },
      ],
      edges: [
        { source: 'trigger', target: 'adapt_content' },
        { source: 'adapt_content', target: 'fan_out' },
        { source: 'fan_out', target: 'post_slack' },
        { source: 'fan_out', target: 'post_discord' },
        { source: 'fan_out', target: 'post_teams' },
        { source: 'post_slack', target: 'fan_in' },
        { source: 'post_discord', target: 'fan_in' },
        { source: 'post_teams', target: 'fan_in' },
        { source: 'fan_in', target: 'record' },
      ],
    },
  },
  {
    id: 'content-localization',
    name: 'AI Content Localization',
    name_zh: 'AI 内容多语言本地化',
    description: 'Translate marketing content into Chinese, Japanese, and Spanish using Claude, then push localized versions to CMS.',
    description_zh: '将英文营销内容用 Claude 并行翻译为中文、日语、西班牙语，保留格式和品牌语气，同步推送到多语言 CMS 端点。',
    category: 'Marketing',
    category_zh: '营销',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: {} },
        { id: 'fan_out', type: 'fan_out', config: {} },
        { id: 'translate_zh', type: 'claude', config: { model: 'claude-haiku-4-5-20251001', api_key: '{{credential.anthropic_key}}', system_prompt: '你是专业营销文案翻译，将内容翻译为简体中文，保持品牌语气，自然流畅。只输出译文，不加说明。', prompt_template: '{{input.content}}', max_tokens: 2000 } },
        { id: 'translate_ja', type: 'claude', config: { model: 'claude-haiku-4-5-20251001', api_key: '{{credential.anthropic_key}}', system_prompt: 'プロのマーケティングコピーライターとして、ブランドのトーンを保ちながら自然な日本語に翻訳してください。翻訳文のみ出力してください。', prompt_template: '{{input.content}}', max_tokens: 2000 } },
        { id: 'translate_es', type: 'claude', config: { model: 'claude-haiku-4-5-20251001', api_key: '{{credential.anthropic_key}}', system_prompt: 'Como redactor de marketing profesional, traduce el contenido al español manteniendo el tono de la marca. Solo devuelve la traducción.', prompt_template: '{{input.content}}', max_tokens: 2000 } },
        { id: 'fan_in', type: 'fan_in', config: {} },
        { id: 'publish_cms', type: 'http', config: { method: 'POST', url: 'https://api.yourcms.com/localized-content', auth_token: '{{credential.cms_key}}', body: '{"slug":"{{input.slug}}","translations":{"zh":"{{translate_zh.content}}","ja":"{{translate_ja.content}}","es":"{{translate_es.content}}"}}' } },
      ],
      edges: [
        { source: 'trigger', target: 'fan_out' },
        { source: 'fan_out', target: 'translate_zh' },
        { source: 'fan_out', target: 'translate_ja' },
        { source: 'fan_out', target: 'translate_es' },
        { source: 'translate_zh', target: 'fan_in' },
        { source: 'translate_ja', target: 'fan_in' },
        { source: 'translate_es', target: 'fan_in' },
        { source: 'fan_in', target: 'publish_cms' },
      ],
    },
  },

  // ── 数据报表 & BI ──────────────────────────────────────────────────────────
  {
    id: 'weekly-business-report',
    name: 'Weekly Business Report',
    name_zh: '周报自动生成 & 发送',
    description: 'Every Monday: fetch key metrics from multiple sources in parallel, summarize with AI, email to leadership.',
    description_zh: '每周一 9 点触发：并行拉取销售、用户、收入多项指标，Claude 提炼关键趋势和行动建议，生成周报邮件发送给管理层。',
    category: 'Reporting',
    category_zh: '报表',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: { cron_expression: '0 0 9 * * 1' } },
        { id: 'fan_out', type: 'fan_out', config: {} },
        { id: 'fetch_sales', type: 'database', config: { url: '{{credential.pg_url}}', query: "SELECT SUM(amount) as revenue, COUNT(*) as orders FROM orders WHERE created_at >= NOW() - INTERVAL '7 days'" } },
        { id: 'fetch_users', type: 'database', config: { url: '{{credential.pg_url}}', query: "SELECT COUNT(*) as new_users, COUNT(CASE WHEN last_login >= NOW() - INTERVAL '7 days' THEN 1 END) as active_users FROM users" } },
        { id: 'fetch_support', type: 'http', config: { method: 'GET', url: 'https://api.zendesk.com/api/v2/tickets/count.json?status=solved&period=last_7_days', auth_token: '{{credential.zendesk_token}}' } },
        { id: 'fan_in', type: 'fan_in', config: {} },
        { id: 'summarize', type: 'claude', config: { model: 'claude-sonnet-4-6', api_key: '{{credential.anthropic_key}}', system_prompt: '你是商业分析师，撰写简洁有力的周报摘要，包含关键亮点、问题和行动建议，用中文，不超过300字。', prompt_template: '本周数据：\n销售：{{fetch_sales.rows.0}}\n用户：{{fetch_users.rows.0}}\n支持工单：{{fetch_support.body.count}}\n\n请生成管理层周报。', max_tokens: 600, temperature: 0.4 } },
        { id: 'send_report', type: 'email', config: { to: '{{input.leadership_emails}}', subject: '【周报】{{input.week_label}} 业务数据摘要', body: '{{summarize.content}}\n\n---\n原始数据：\n· 本周收入：¥{{fetch_sales.rows.0.revenue}}\n· 本周订单：{{fetch_sales.rows.0.orders}}\n· 新增用户：{{fetch_users.rows.0.new_users}}\n· 活跃用户：{{fetch_users.rows.0.active_users}}', api_key: '{{credential.sendgrid_key}}', from: 'report@example.com' } },
      ],
      edges: [
        { source: 'trigger', target: 'fan_out' },
        { source: 'fan_out', target: 'fetch_sales' },
        { source: 'fan_out', target: 'fetch_users' },
        { source: 'fan_out', target: 'fetch_support' },
        { source: 'fetch_sales', target: 'fan_in' },
        { source: 'fetch_users', target: 'fan_in' },
        { source: 'fetch_support', target: 'fan_in' },
        { source: 'fan_in', target: 'summarize' },
        { source: 'summarize', target: 'send_report' },
      ],
    },
  },
  {
    id: 'ga-weekly-summary',
    name: 'Google Analytics Weekly Summary',
    name_zh: 'Google Analytics 周报',
    description: 'Query GA4 traffic data weekly, extract key metrics, generate insights with AI, and email the marketing team.',
    description_zh: '每周自动查询 GA4 流量报告，提取 PV、用户数、跳出率等核心指标，Claude 生成营销洞察和优化建议，发送周报邮件。',
    category: 'Reporting',
    category_zh: '报表',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: { cron_expression: '0 0 8 * * 1' } },
        { id: 'fetch_ga', type: 'ganalytics', config: { access_token: '{{credential.gcal_token}}', property_id: '{{credential.ga_property_id}}', operation: 'run_report', date_ranges: [{ startDate: '7daysAgo', endDate: 'today' }], dimensions: [{ name: 'sessionSource' }], metrics: [{ name: 'sessions' }, { name: 'newUsers' }, { name: 'bounceRate' }, { name: 'averageSessionDuration' }] } },
        { id: 'top_sources', type: 'filter', config: { items: '{{fetch_ga.body.rows}}', field: 'metricValues.0.value', operator: 'gt', value: '10' } },
        { id: 'insights', type: 'claude', config: { model: 'claude-haiku-4-5-20251001', api_key: '{{credential.anthropic_key}}', system_prompt: '你是数字营销分析师，用简洁中文分析网站流量数据，给出3条关键洞察和2条优化建议。', prompt_template: 'GA4 本周数据：\n总会话：{{fetch_ga.body.totals}}\n流量来源 Top：{{top_sources.items}}\n\n请提供营销洞察。', max_tokens: 500, temperature: 0.4 } },
        { id: 'send_report', type: 'email', config: { to: '{{input.marketing_email}}', subject: '【GA 周报】网站流量分析 — {{input.week_label}}', body: 'Google Analytics 本周洞察：\n\n{{insights.content}}\n\n---\n数据摘要：\n· 总会话数：{{fetch_ga.body.totals.0.metricValues.0.value}}\n· 新用户数：{{fetch_ga.body.totals.0.metricValues.1.value}}', api_key: '{{credential.sendgrid_key}}', from: 'report@example.com' } },
      ],
      edges: [
        { source: 'trigger', target: 'fetch_ga' },
        { source: 'fetch_ga', target: 'top_sources' },
        { source: 'top_sources', target: 'insights' },
        { source: 'insights', target: 'send_report' },
      ],
    },
  },
  {
    id: 'db-daily-snapshot',
    name: 'Daily Database Snapshot & Diff',
    name_zh: '数据库每日快照对比',
    description: 'Daily snapshot of key table counts, compare with previous day, alert on significant changes.',
    description_zh: '每天拍摄关键表行数快照并写入快照表，与前日数据对比，变化超过 10% 时触发 Slack 告警，辅助数据漂移监控。',
    category: 'Reporting',
    category_zh: '报表',
    graph: {
      workflow_version_id: 'template',
      nodes: [
        { id: 'trigger', type: 'trigger', config: { cron_expression: '0 0 1 * * *' } },
        { id: 'snapshot_today', type: 'database', config: { url: '{{credential.pg_url}}', query: "SELECT 'orders' as table_name, COUNT(*) as cnt FROM orders UNION ALL SELECT 'users', COUNT(*) FROM users UNION ALL SELECT 'products', COUNT(*) FROM products" } },
        { id: 'snapshot_yesterday', type: 'database', config: { url: '{{credential.pg_url}}', query: "SELECT table_name, row_count as cnt FROM db_snapshots WHERE snapshot_date = CURRENT_DATE - 1" } },
        { id: 'store_snapshot', type: 'database', config: { url: '{{credential.pg_url}}', query: "INSERT INTO db_snapshots (snapshot_date, table_name, row_count) SELECT CURRENT_DATE, table_name, cnt FROM json_populate_recordset(null::record, '{{snapshot_today.rows}}')" } },
        { id: 'analyze_diff', type: 'claude', config: { model: 'claude-haiku-4-5-20251001', api_key: '{{credential.anthropic_key}}', system_prompt: '分析数据库行数变化，输出 JSON：{"has_anomaly":true/false,"anomalies":[],"summary":""}。变化超10%视为异常。', prompt_template: '今日快照：{{snapshot_today.rows}}\n昨日快照：{{snapshot_yesterday.rows}}', max_tokens: 300, temperature: 0.1 } },
        { id: 'check_anomaly', type: 'condition', config: { field: 'has_anomaly', equals: 'true', source: '{{analyze_diff.content}}' } },
        { id: 'alert_anomaly', type: 'slack', config: { webhook_url: '{{credential.slack_webhook}}', text: '📊 数据库快照异常告警\n\n{{analyze_diff.content.summary}}\n\n异常项：{{analyze_diff.content.anomalies}}', username: 'DBMonitor' } },
        { id: 'daily_summary', type: 'slack', config: { webhook_url: '{{credential.slack_webhook}}', text: '✅ 数据库快照正常\n{{analyze_diff.content.summary}}', username: 'DBMonitor' } },
      ],
      edges: [
        { source: 'trigger', target: 'snapshot_today' },
        { source: 'snapshot_today', target: 'snapshot_yesterday' },
        { source: 'snapshot_yesterday', target: 'store_snapshot' },
        { source: 'store_snapshot', target: 'analyze_diff' },
        { source: 'analyze_diff', target: 'check_anomaly' },
        { source: 'check_anomaly', target: 'alert_anomaly', condition_label: 'true' },
        { source: 'check_anomaly', target: 'daily_summary', condition_label: 'false' },
      ],
    },
  },
]

const CATEGORY_COLORS: Record<string, string> = {
  Sales: 'var(--node-agent)',
  Marketing: 'var(--node-openai)',
  Data: 'var(--node-http)',
  Reliability: 'var(--node-catch)',
  Compliance: 'var(--node-approval)',
  Reporting: 'var(--node-email)',
  Vision: 'var(--node-gemini)',
  Automation: 'var(--node-slack)',
  Engineering: 'var(--node-code)',
  AI: 'var(--node-claude)',
  Integration: 'var(--node-github)',
  Ecommerce: '#16a34a',
  HR: '#7c3aed',
  DevOps: '#0369a1',
  CRM: '#c2410c',
}

interface Props {
  onImport: (template: Template) => void
  onClose: () => void
}

export function TemplatesModal({ onImport, onClose }: Props) {
  const { locale } = useLocale()
  const zh = locale === 'zh'

  const [search, setSearch] = useState('')
  const [categoryFilter, setCategoryFilter] = useState('')

  const allCategories = Array.from(new Set(TEMPLATES.map((t) => t.category))).sort()

  const filtered = TEMPLATES.filter((tpl) => {
    const q = search.trim().toLowerCase()
    const name = zh && tpl.name_zh ? tpl.name_zh : tpl.name
    const desc = zh && tpl.description_zh ? tpl.description_zh : tpl.description
    const matchesSearch = !q ||
      name.toLowerCase().includes(q) ||
      desc.toLowerCase().includes(q) ||
      tpl.category.toLowerCase().includes(q) ||
      (tpl.category_zh ?? '').includes(q) ||
      tpl.graph.nodes.some((n) => n.type.includes(q))
    const matchesCategory = !categoryFilter || tpl.category === categoryFilter
    return matchesSearch && matchesCategory
  })

  const catLabel = (cat: string) => {
    if (!zh) return cat
    const tpl = TEMPLATES.find((t) => t.category === cat)
    return tpl?.category_zh ?? cat
  }

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div
        className="modal"
        style={{ width: 760, maxWidth: '95vw', maxHeight: '85vh', overflow: 'hidden', display: 'flex', flexDirection: 'column', gap: 0, padding: 0 }}
        onClick={(e) => e.stopPropagation()}
      >
        {/* header */}
        <div style={{ padding: '20px 24px 14px', borderBottom: '1px solid var(--border)', flexShrink: 0 }}>
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 10 }}>
            <div>
              <h2 style={{ margin: 0, fontSize: 16 }}>
                {zh ? '工作流模板库' : 'Workflow Templates'}
              </h2>
              <p style={{ margin: '4px 0 0', fontSize: 12, color: 'var(--muted)' }}>
                {zh
                  ? '从预置模板快速开始 — 替换凭证和 URL 即可激活'
                  : 'Start from a pre-built template — configure credentials and URLs to activate.'}
              </p>
            </div>
            <button className="btn btn-sm" onClick={onClose}>✕</button>
          </div>
          <div style={{ display: 'flex', gap: 8, alignItems: 'center', flexWrap: 'wrap' }}>
            <div style={{ position: 'relative', flex: 1, minWidth: 180 }}>
              <input
                autoFocus
                placeholder={zh ? '搜索模板…' : 'Search templates…'}
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                style={{ width: '100%', fontSize: 13, paddingRight: search ? 28 : 10, boxSizing: 'border-box' }}
              />
              {search && (
                <button
                  onClick={() => setSearch('')}
                  style={{ position: 'absolute', right: 6, top: '50%', transform: 'translateY(-50%)', background: 'none', border: 'none', color: 'var(--muted)', cursor: 'pointer', fontSize: 14, padding: 0, lineHeight: 1 }}
                >
                  ✕
                </button>
              )}
            </div>
            <button
              onClick={() => setCategoryFilter('')}
              style={{ padding: '3px 10px', borderRadius: 12, border: '1px solid', fontSize: 12, cursor: 'pointer', borderColor: !categoryFilter ? 'var(--link)' : 'var(--border)', background: !categoryFilter ? 'var(--link)' : 'transparent', color: !categoryFilter ? '#fff' : 'var(--muted)' }}
            >
              {zh ? '全部' : 'All'}
            </button>
            {allCategories.map((cat) => (
              <button
                key={cat}
                onClick={() => setCategoryFilter(categoryFilter === cat ? '' : cat)}
                style={{
                  padding: '3px 10px', borderRadius: 12, border: '1px solid', fontSize: 12, cursor: 'pointer',
                  borderColor: categoryFilter === cat ? (CATEGORY_COLORS[cat] ?? 'var(--link)') : 'var(--border)',
                  background: categoryFilter === cat ? `${CATEGORY_COLORS[cat] ?? 'var(--link)'}22` : 'transparent',
                  color: categoryFilter === cat ? (CATEGORY_COLORS[cat] ?? 'var(--link)') : 'var(--muted)',
                }}
              >
                {catLabel(cat)}
              </button>
            ))}
            {(search || categoryFilter) && (
              <span style={{ fontSize: 12, color: 'var(--muted)', marginLeft: 4 }}>
                {filtered.length} / {TEMPLATES.length}
              </span>
            )}
          </div>
        </div>

        {/* grid */}
        <div style={{ overflow: 'auto', padding: '14px 24px 24px', display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12, flex: 1, alignContent: 'start' }}>
          {filtered.length === 0 && (
            <div style={{ gridColumn: '1 / -1', color: 'var(--muted)', fontSize: 13, textAlign: 'center', padding: '32px 0' }}>
              {zh ? `没有匹配"${search}"的模板。` : `No templates match "${search}".`}
            </div>
          )}
          {filtered.map((tpl) => {
            const displayName = zh && tpl.name_zh ? tpl.name_zh : tpl.name
            const displayDesc = zh && tpl.description_zh ? tpl.description_zh : tpl.description
            const displayCat = zh && tpl.category_zh ? tpl.category_zh : tpl.category
            return (
              <div
                key={tpl.id}
                style={{
                  background: 'var(--panel)', border: '1px solid var(--border)', borderRadius: 8,
                  padding: '14px 16px', cursor: 'pointer', display: 'flex', flexDirection: 'column', gap: 8,
                  transition: 'border-color 0.15s',
                }}
                onClick={() => onImport(tpl)}
                onMouseEnter={(e) => (e.currentTarget.style.borderColor = 'var(--accent)')}
                onMouseLeave={(e) => (e.currentTarget.style.borderColor = 'var(--border)')}
              >
                <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                  <span
                    style={{
                      fontSize: 10, fontWeight: 700, padding: '2px 7px', borderRadius: 20,
                      background: `${CATEGORY_COLORS[tpl.category] ?? 'var(--muted)'}22`,
                      color: CATEGORY_COLORS[tpl.category] ?? 'var(--muted)',
                      letterSpacing: '0.04em', textTransform: 'uppercase',
                    }}
                  >
                    {displayCat}
                  </span>
                  <span style={{ fontSize: 11, color: 'var(--muted)' }}>
                    {tpl.graph.nodes.length}{zh ? ' 个节点' : ' nodes'}
                  </span>
                </div>
                <div style={{ fontWeight: 600, fontSize: 13 }}>{displayName}</div>
                <div style={{ fontSize: 12, color: 'var(--muted)', lineHeight: 1.5 }}>{displayDesc}</div>
                <div style={{ display: 'flex', gap: 4, flexWrap: 'wrap', marginTop: 4 }}>
                  {Array.from(new Set(tpl.graph.nodes.map((n) => n.type))).map((type) => (
                    <span
                      key={type}
                      style={{
                        fontSize: 10, padding: '1px 5px', borderRadius: 4,
                        background: 'var(--bg)', border: '1px solid var(--border)', color: 'var(--muted)',
                      }}
                    >
                      {type}
                    </span>
                  ))}
                </div>
              </div>
            )
          })}
        </div>
      </div>
    </div>
  )
}
