// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import type { ExecutionSummary } from '../types'

// Pure run-list filtering pulled out of RunsPage: applies the date / status /
// trigger / starred / label / output / search filters in order. Kept as a pure
// function so the page component stays a thin shell and the logic is testable.

export interface RunFilters {
  dateFilter: 'all' | 'today' | '7d' | '30d'
  statusFilter: string
  triggerFilter: string
  starredOnly: boolean
  labelFilter: string
  outputFilter: string
  searchQuery: string
}

export function applyRunFilters(
  runs: ExecutionSummary[],
  f: RunFilters,
  workflowNames: Map<string, string>,
  nowSecs: number,
): ExecutionSummary[] {
  const dateThreshold = f.dateFilter === 'today' ? nowSecs - 86400
    : f.dateFilter === '7d' ? nowSecs - 7 * 86400
    : f.dateFilter === '30d' ? nowSecs - 30 * 86400
    : 0
  const byDate      = dateThreshold > 0 ? runs.filter((r) => r.started_at >= dateThreshold) : runs
  const byStatus    = f.statusFilter === 'all' ? byDate : byDate.filter((r) => r.status === f.statusFilter)
  const byTrigger   = f.triggerFilter === 'all' ? byStatus : byStatus.filter((r) => (r.trigger_type ?? 'manual') === f.triggerFilter)
  const byStarred   = f.starredOnly ? byTrigger.filter((r) => r.starred) : byTrigger
  const byLabel     = f.labelFilter ? byStarred.filter((r) => r.label === f.labelFilter) : byStarred
  const byOutput    = f.outputFilter ? byLabel.filter((r) => (r as unknown as Record<string, unknown>).output_json != null && String((r as unknown as Record<string, unknown>).output_json).toLowerCase().includes(f.outputFilter.toLowerCase())) : byLabel
  return f.searchQuery
    ? byOutput.filter((r) => {
        const q = f.searchQuery.toLowerCase()
        return r.id.toLowerCase().startsWith(q) ||
          (workflowNames.get(r.workflow_id) ?? '').toLowerCase().includes(q) ||
          (r.label ?? '').toLowerCase().includes(q)
      })
    : byOutput
}
