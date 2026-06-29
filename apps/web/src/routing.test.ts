// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { describe, it, expect } from 'vitest'
import { pageToPath, pathToPage, type Page } from './routing'

describe('pageToPath', () => {
  it('maps the workflow list to root', () => {
    expect(pageToPath({ name: 'list' })).toBe('/')
  })

  it('maps static pages to their path (incl. the kebab/aliased ones)', () => {
    expect(pageToPath({ name: 'credentials' })).toBe('/credentials')
    expect(pageToPath({ name: 'apikeys' })).toBe('/api-keys')
    expect(pageToPath({ name: 'custom-nodes' })).toBe('/custom-nodes')
    expect(pageToPath({ name: 'workflow-deps' })).toBe('/workflow-deps')
  })

  it('encodes id-carrying pages', () => {
    expect(pageToPath({ name: 'editor', workflowId: 'wf 1' })).toBe('/workflows/wf%201')
    expect(pageToPath({ name: 'execution', executionId: 'ex1' })).toBe('/executions/ex1')
  })

  it('encodes the runs filter and the execution back-target as query params', () => {
    expect(pageToPath({ name: 'runs' })).toBe('/runs')
    expect(pageToPath({ name: 'runs', workflowFilter: 'wf1' })).toBe('/runs?workflow=wf1')
    expect(pageToPath({ name: 'execution', executionId: 'ex1', fromRuns: true })).toBe('/executions/ex1?from=runs')
  })

  it('drops the transient editor initialInput from the URL', () => {
    expect(pageToPath({ name: 'editor', workflowId: 'wf1', initialInput: '{"a":1}' })).toBe('/workflows/wf1')
  })
})

describe('pathToPage', () => {
  it('parses root and static paths (trailing slash tolerant)', () => {
    expect(pathToPage('/')).toEqual({ name: 'list' })
    expect(pathToPage('/credentials')).toEqual({ name: 'credentials' })
    expect(pathToPage('/api-keys')).toEqual({ name: 'apikeys' })
    expect(pathToPage('/monitoring/')).toEqual({ name: 'monitoring' })
  })

  it('parses id-carrying paths', () => {
    expect(pathToPage('/workflows/wf%201')).toEqual({ name: 'editor', workflowId: 'wf 1' })
    expect(pathToPage('/executions/ex1')).toEqual({ name: 'execution', executionId: 'ex1', fromRuns: false })
    expect(pathToPage('/executions/ex1', '?from=runs')).toEqual({ name: 'execution', executionId: 'ex1', fromRuns: true })
  })

  it('parses the runs filter query', () => {
    expect(pathToPage('/runs')).toEqual({ name: 'runs' })
    expect(pathToPage('/runs', '?workflow=wf1')).toEqual({ name: 'runs', workflowFilter: 'wf1' })
  })

  it('falls back to the list for unknown paths', () => {
    expect(pathToPage('/nope/deep/link')).toEqual({ name: 'list' })
  })
})

describe('round-trip', () => {
  const pages: Page[] = [
    { name: 'list' },
    { name: 'analytics' },
    { name: 'workflow-deps' },
    { name: 'apikeys' },
    { name: 'editor', workflowId: 'abc-123' },
    { name: 'execution', executionId: 'ex9', fromRuns: true },
    { name: 'runs', workflowFilter: 'wf7' },
  ]
  it('pathToPage(pageToPath(p)) is stable', () => {
    for (const p of pages) {
      const path = pageToPath(p)
      const [pathname, search] = path.split('?')
      expect(pathToPage(pathname, search ? `?${search}` : '')).toEqual(
        p.name === 'execution' ? p : { ...p },
      )
    }
  })
})
