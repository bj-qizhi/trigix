// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { describe, it, expect } from 'vitest'
import { NODE_PREVIEW_TYPES, nodePreview } from './nodePreview'

const configuredPreview: Record<string, unknown> = {
  account_id: 'account-123456789',
  action_kind: 'click_element',
  actor_id: 'actor-1',
  algorithm: 'RS256',
  a: 2,
  api_type: 'preview',
  application_id: 'com.example.app',
  b: 3,
  base_id: 'base-1',
  browser_allowed_actions: ['navigate'],
  browser_allowed_hosts: ['example.com'],
  bucket: 'bucket-1',
  calendar_id: 'team-calendar',
  categories: ['one', 'two'],
  chat_id: 'chat-1',
  chunk_overlap: 50,
  chunk_size: 500,
  class: 'Document',
  collection: 'documents',
  command: 'uptime',
  container: 'container-1',
  content: 'A configured message that is deliberately longer than forty characters.',
  database: 'main',
  deployment: 'deployment-1',
  device_id: 'device-1',
  document_id: 'document-1234567890',
  domain: 'example.test',
  endpoint: '/v1/resources',
  endpoint_id: 'endpoint-1',
  environment: 'production',
  extract: 'html',
  field: 'status',
  file_id: 'file-1234567890',
  full_page: false,
  host: 'db.example.test',
  instance: 'tenant',
  instance_url: 'https://instance.example.test',
  items: '{{ items }}',
  kb: 'knowledge-base',
  key: 'record-key',
  lang: 'zho',
  mailbox: 'Archive',
  message_type: 'template',
  method: 'POST',
  mode: 'resume',
  model: 'configured-model',
  model_id: 'provider.model',
  msg_type: 'markdown',
  namespace: 'tenant-a',
  object: 'path/file.txt',
  operation: 'configured-operation',
  organization: 'org-1',
  path: '/folder/file.txt',
  pattern: 'a+',
  pdf_base64: 'cGRm',
  project: 'project-1',
  project_id: 'project-1234567890',
  property_id: 'property-1',
  public_id: 'asset-1',
  query: 'SELECT * FROM records WHERE enabled = true AND tenant_id = $1',
  queue_url: 'https://queue.example.test/tenant/jobs',
  range: 'Data!A1:D20',
  resource: 'companies',
  routing_key: 'workflow.completed',
  script: 'return input\n// configured',
  seconds: 5,
  secret: 'configured',
  selector: '#submit',
  separator: ',',
  server: 'eu1',
  service: 'storage',
  shop: 'store.example.test',
  site_url: 'https://store.example.test',
  source: '{{ input }}',
  spreadsheet_id: 'sheet-1234567890',
  statement: 'SELECT 1',
  subdomain: 'support',
  summary: 'Configured incident summary',
  table: 'records',
  template: 'Configured template text that is deliberately longer than forty characters.',
  text: 'Configured text that is deliberately longer than forty characters.',
  to: 'recipient@example.test',
  topic: 'events',
  topic_arn: 'arn:aws:sns:region:account:events',
  type: 'uuid',
  until: '2030-01-01T00:00:00Z',
  url: 'https://example.test/path',
  username: 'operator',
  version: 'provider/model-version-1234567890',
  voice: 'nova',
  wait_mode: 'duration',
  webhook_url: 'https://hooks.example.test/abc',
  workflow_id: 'workflow-1234567890',
}

describe('nodePreview', () => {
  it('returns empty string for missing or unknown node types', () => {
    expect(nodePreview(undefined, {})).toBe('')
    expect(nodePreview('definitely-not-a-node', {})).toBe('')
  })

  it('renders configured values', () => {
    expect(nodePreview('http', { url: 'https://x.test' })).toBe('https://x.test')
    expect(nodePreview('condition', { field: 'amount' })).toBe('if amount')
    expect(nodePreview('sub_workflow', { workflow_id: 'wf-9' })).toBe('wf-9')
    expect(nodePreview('desktop', { action_kind: 'click_element', device_id: 'device-1' })).toBe('click_element · device-1')
  })

  it('falls back to placeholders when config is empty', () => {
    expect(nodePreview('http', {})).toBe('No URL set')
    expect(nodePreview('condition', {})).toBe('No field set')
  })

  it('falls back to default models for LLM nodes', () => {
    expect(nodePreview('openai', {})).toBe('gpt-5.4-mini')
    expect(nodePreview('claude', {})).toBe('claude-sonnet-4-6')
    expect(nodePreview('openai', { model: 'gpt-4o' })).toBe('gpt-4o')
  })

  it('handles the multi-line block cases', () => {
    expect(nodePreview('delay', { seconds: 5 })).toBe('wait 5s')
    expect(nodePreview('delay', {})).toBe('No duration set')
    expect(nodePreview('filter', {})).toBe('No items set')
    expect(nodePreview('filter', { items: '{{x}}', field: 'status', operator: 'eq', value: 'ok' }))
      .toBe('status eq ok')
    expect(nodePreview('aggregate', { operation: 'sum', field: 'amount' })).toBe('sum(amount)')
  })

  it('strips the scheme from URL-ish previews', () => {
    expect(nodePreview('webhook', { url: 'https://hooks.test/abc' })).toBe('hooks.test/abc')
  })

  it('renders Browser node state without exposing input values', () => {
    expect(nodePreview('browser_start', {})).toBe('Creates an isolated session')
    expect(nodePreview('browser_navigate', { url: 'https://example.com/path' })).toBe('example.com/path')
    expect(nodePreview('browser_navigate', {})).toBe('No URL set')
    expect(nodePreview('browser_click', { selector: '#submit' })).toBe('click #submit')
    expect(nodePreview('browser_click', {})).toBe('No selector set')
    expect(nodePreview('browser_input', { selector: '#password', value: 'secret' })).toBe('input #password')
    expect(nodePreview('browser_input', { value: 'secret' })).toBe('No selector set')
    expect(nodePreview('browser_wait', {})).toBe('selector wait')
    expect(nodePreview('browser_wait', { wait_mode: 'duration' })).toBe('duration wait')
    expect(nodePreview('browser_extract', { selector: 'main' })).toBe('text main')
    expect(nodePreview('browser_extract', { selector: 'a', mode: 'attribute' })).toBe('attribute a')
    expect(nodePreview('browser_extract', {})).toBe('No selector set')
    expect(nodePreview('browser_screenshot', { full_page: false })).toBe('Viewport screenshot')
    expect(nodePreview('browser_screenshot', {})).toBe('Full-page screenshot')
    expect(nodePreview('browser_close', {})).toBe('Closes the session')
  })

  it('covers the previously-missing node previews', () => {
    expect(nodePreview('trigger', {})).toBe('Workflow entry point')
    expect(nodePreview('switch', {})).toBe('No field set')
    expect(nodePreview('switch', { field: 'kind' })).toBe('switch on kind')
    expect(nodePreview('regex', { pattern: 'ab+' })).toBe('match /ab+/')
    expect(nodePreview('regex', {})).toBe('No pattern set')
    expect(nodePreview('dedupe', { field: 'id' })).toBe('unique by id')
    expect(nodePreview('split', { separator: ',' })).toBe('split · ","')
    // None of the 10 newly-added previews fall through to empty.
    for (const nt of ['trigger', 'csv', 'dedupe', 'format', 'join', 'random', 'regex', 'rename', 'split', 'switch']) {
      expect(nodePreview(nt, {}), nt).not.toBe('')
    }
  })

  it('renders every registered node type for empty and configured states', () => {
    expect(NODE_PREVIEW_TYPES.length).toBeGreaterThan(150)
    expect(new Set(NODE_PREVIEW_TYPES).size).toBe(NODE_PREVIEW_TYPES.length)

    for (const nodeType of NODE_PREVIEW_TYPES) {
      expect(nodePreview(nodeType, {}), `${nodeType} empty preview`).not.toBe('')
      expect(nodePreview(nodeType, configuredPreview), `${nodeType} configured preview`).not.toBe('')
    }
  })
})
