// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { describe, it, expect } from 'vitest'
import { filterAndSortWorkflows, computeWorkflowStats, type WorkflowFilters, type WorkflowStatsMap } from './workflowListFilter'
import type { WorkflowRecord, ExecutionSummary } from '../types'

const wf = (over: Partial<WorkflowRecord>): WorkflowRecord =>
  ({ id: 'wf', name: 'WF', status: 'published', updated_at: 0, created_at: 0, ...over } as WorkflowRecord)
const ex = (over: Partial<ExecutionSummary>): ExecutionSummary =>
  ({ id: 'ex', tenant_id: 't', workflow_id: 'wf1', workflow_version_id: 'v1', status: 'succeeded', started_at: 1, ...over } as ExecutionSummary)

const base: WorkflowFilters = { search: '', tagFilter: '', statusFilter: 'all', folderFilter: '', runTodayOnly: false, sortBy: 'name' }
const noStats: WorkflowStatsMap = new Map()

describe('computeWorkflowStats', () => {
  it('aggregates per-workflow run counts and last-run time', () => {
    const stats = computeWorkflowStats([
      ex({ workflow_id: 'a', status: 'succeeded', started_at: 10 }),
      ex({ workflow_id: 'a', status: 'failed', started_at: 20 }),
      ex({ workflow_id: 'a', status: 'running', started_at: 5 }),
      ex({ workflow_id: 'b', status: 'waiting_approval', started_at: 3 }),
    ])
    expect(stats.get('a')).toEqual({ total: 3, succeeded: 1, failed: 1, running: 1, lastAt: 20 })
    expect(stats.get('b')).toEqual({ total: 1, succeeded: 0, failed: 0, running: 1, lastAt: 3 })
  })
})

describe('filterAndSortWorkflows', () => {
  const workflows = [
    wf({ id: 'a', name: 'Alpha', status: 'published', tags: ['prod'] }),
    wf({ id: 'b', name: 'Beta', status: 'draft', tags: ['team'] }),
    wf({ id: 'c', name: 'Gamma', status: 'archived' }),
  ]

  it('sorts by name by default', () => {
    expect(filterAndSortWorkflows(workflows, base, noStats, 0).map((w) => w.id)).toEqual(['a', 'b', 'c'])
  })

  it('floats pinned workflows to the top within the sort', () => {
    const wfs = [wf({ id: 'a', name: 'Alpha' }), wf({ id: 'z', name: 'Zeta', pinned: true })]
    expect(filterAndSortWorkflows(wfs, base, noStats, 0).map((w) => w.id)).toEqual(['z', 'a'])
  })

  it('filters by status, tag and search', () => {
    expect(filterAndSortWorkflows(workflows, { ...base, statusFilter: 'draft' }, noStats, 0).map((w) => w.id)).toEqual(['b'])
    expect(filterAndSortWorkflows(workflows, { ...base, tagFilter: 'prod' }, noStats, 0).map((w) => w.id)).toEqual(['a'])
    expect(filterAndSortWorkflows(workflows, { ...base, search: 'gam' }, noStats, 0).map((w) => w.id)).toEqual(['c'])
  })

  it('sorts by run count using the stats map', () => {
    const stats: WorkflowStatsMap = new Map([['a', { total: 1 }], ['b', { total: 9 }], ['c', { total: 3 }]])
    expect(filterAndSortWorkflows(workflows, { ...base, sortBy: 'runs' }, stats, 0).map((w) => w.id)).toEqual(['b', 'c', 'a'])
  })
})
