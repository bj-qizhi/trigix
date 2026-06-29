// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { describe, it, expect } from 'vitest'
import { nodePreview } from './nodePreview'

describe('nodePreview', () => {
  it('returns empty string for missing or unknown node types', () => {
    expect(nodePreview(undefined, {})).toBe('')
    expect(nodePreview('definitely-not-a-node', {})).toBe('')
  })

  it('renders configured values', () => {
    expect(nodePreview('http', { url: 'https://x.test' })).toBe('https://x.test')
    expect(nodePreview('condition', { field: 'amount' })).toBe('if amount')
    expect(nodePreview('sub_workflow', { workflow_id: 'wf-9' })).toBe('wf-9')
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
})
