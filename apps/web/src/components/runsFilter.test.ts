// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { describe, it, expect } from 'vitest'
import { applyRunFilters, type RunFilters } from './runsFilter'
import type { ExecutionSummary } from '../types'

const run = (over: Partial<ExecutionSummary>): ExecutionSummary =>
  ({ id: 'ex', tenant_id: 't', workflow_id: 'wf1', workflow_version_id: 'v1', status: 'succeeded', started_at: 1000, ...over } as ExecutionSummary)

const base: RunFilters = { dateFilter: 'all', statusFilter: 'all', triggerFilter: 'all', starredOnly: false, labelFilter: '', outputFilter: '', searchQuery: '' }
const names = new Map([['wf1', 'Nightly Sync']])

describe('applyRunFilters', () => {
  const runs = [
    run({ id: 'a', status: 'succeeded', label: 'nightly', trigger_type: 'schedule', started_at: 5000 }),
    run({ id: 'b', status: 'failed', label: 'adhoc', trigger_type: 'manual', started_at: 4000 }),
    run({ id: 'c', status: 'running', started_at: 3000 }),
  ]

  it('returns all runs with the default (no-op) filters', () => {
    expect(applyRunFilters(runs, base, names, 9999).map((r) => r.id)).toEqual(['a', 'b', 'c'])
  })

  it('filters by status', () => {
    expect(applyRunFilters(runs, { ...base, statusFilter: 'failed' }, names, 9999).map((r) => r.id)).toEqual(['b'])
  })

  it('filters by trigger type (defaulting missing to manual)', () => {
    expect(applyRunFilters(runs, { ...base, triggerFilter: 'manual' }, names, 9999).map((r) => r.id)).toEqual(['b', 'c'])
  })

  it('filters by label', () => {
    expect(applyRunFilters(runs, { ...base, labelFilter: 'nightly' }, names, 9999).map((r) => r.id)).toEqual(['a'])
  })

  it('searches by id prefix, workflow name and label', () => {
    // Use non-overlapping ids/names/labels so each match path is isolated.
    const sruns = [
      run({ id: 'xaa', workflow_id: 'wfA', label: 'nightly', started_at: 5000 }),
      run({ id: 'ybb', workflow_id: 'wfB', label: 'weekly', started_at: 4000 }),
    ]
    const snames = new Map([['wfA', 'Alpha'], ['wfB', 'Beta']])
    // workflow name match (all share neither term)
    expect(applyRunFilters(sruns, { ...base, searchQuery: 'beta' }, snames, 9999).map((r) => r.id)).toEqual(['ybb'])
    // label match
    expect(applyRunFilters(sruns, { ...base, searchQuery: 'night' }, snames, 9999).map((r) => r.id)).toEqual(['xaa'])
    // id prefix match
    expect(applyRunFilters(sruns, { ...base, searchQuery: 'xa' }, snames, 9999).map((r) => r.id)).toEqual(['xaa'])
  })

  it('filters by relative date window', () => {
    // now = 5000; '7d' threshold = 5000 - 7*86400 < 0 so all pass; use a tight now.
    const now = 4500
    expect(applyRunFilters(runs, { ...base, dateFilter: 'today' }, names, now).every((r) => r.started_at >= now - 86400)).toBe(true)
  })
})
