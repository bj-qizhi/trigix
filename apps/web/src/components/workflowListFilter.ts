// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import type { WorkflowRecord, ExecutionSummary } from '../types'

export interface WorkflowStats { total: number; succeeded: number; failed: number; running: number; lastAt: number }

// Per-workflow run stats aggregated from execution summaries.
export function computeWorkflowStats(execSummaries: ExecutionSummary[]): Map<string, WorkflowStats> {
  const map = new Map<string, WorkflowStats>()
  for (const ex of execSummaries) {
    const cur = map.get(ex.workflow_id) ?? { total: 0, succeeded: 0, failed: 0, running: 0, lastAt: 0 }
    cur.total++
    if (ex.status === 'succeeded') cur.succeeded++
    else if (ex.status === 'failed') cur.failed++
    else if (ex.status === 'running' || ex.status === 'waiting_approval') cur.running++
    if (ex.started_at > cur.lastAt) cur.lastAt = ex.started_at
    map.set(ex.workflow_id, cur)
  }
  return map
}

// Pure workflow-list filtering + sorting pulled out of WorkflowList. Applies the
// search / tag / status / run-today / folder filters, then floats pinned items
// to the top and sorts each group by the chosen key.

export interface WorkflowFilters {
  search: string
  tagFilter: string
  statusFilter: 'all' | 'published' | 'draft' | 'archived'
  folderFilter: string
  runTodayOnly: boolean
  sortBy: 'name' | 'status' | 'runs' | 'recent' | 'created' | 'modified'
}

export type WorkflowStatsMap = Map<string, { lastAt?: number; total?: number }>

export function filterAndSortWorkflows(
  workflows: WorkflowRecord[],
  f: WorkflowFilters,
  statsByWorkflow: WorkflowStatsMap,
  todayStart: number,
): WorkflowRecord[] {
  const base = workflows.filter((wf) => {
    const q = f.search.trim().toLowerCase()
    const matchesSearch = !q || wf.name.toLowerCase().includes(q) || (wf.description?.toLowerCase().includes(q)) || (wf.tags ?? []).some((t) => t.toLowerCase().includes(q))
    const matchesTag = !f.tagFilter || (wf.tags ?? []).includes(f.tagFilter)
    const matchesStatus = f.statusFilter === 'all' || wf.status === f.statusFilter
    const matchesRunToday = !f.runTodayOnly || (statsByWorkflow.get(wf.id)?.lastAt ?? 0) >= todayStart
    const matchesFolder = !f.folderFilter || wf.folder === f.folderFilter
    return matchesSearch && matchesTag && matchesStatus && matchesRunToday && matchesFolder
  })
  // Pinned always float to top; within each group apply sort
  const pinned = base.filter((w) => w.pinned)
  const unpinned = base.filter((w) => !w.pinned)
  const cmp = (a: WorkflowRecord, b: WorkflowRecord): number => {
    if (f.sortBy === 'name') return a.name.localeCompare(b.name)
    if (f.sortBy === 'status') return a.status.localeCompare(b.status)
    if (f.sortBy === 'runs') return (statsByWorkflow.get(b.id)?.total ?? 0) - (statsByWorkflow.get(a.id)?.total ?? 0)
    if (f.sortBy === 'recent') return (statsByWorkflow.get(b.id)?.lastAt ?? 0) - (statsByWorkflow.get(a.id)?.lastAt ?? 0)
    if (f.sortBy === 'created') return (b.created_at ?? 0) - (a.created_at ?? 0)
    if (f.sortBy === 'modified') return (b.updated_at ?? 0) - (a.updated_at ?? 0)
    return 0
  }
  return [...pinned.sort(cmp), ...unpinned.sort(cmp)]
}
