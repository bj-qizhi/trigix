// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useTheme } from '../useTheme'
import { useLocale } from '../useLocale'
import { useEffect, useRef, useState } from 'react'
import { useAuth } from '../AuthContext'
import * as api from '../api/client'
import type { ExecutionSummary, ExecutionRecord, NodeExecutionRecord } from '../types'
import logoWordmark from '../assets/logo-wordmark.svg'

const LIVE_STATUSES = new Set(['running', 'waiting_approval'])

function useWorkflowNames(tenantId: string): Map<string, string> {
  const [names, setNames] = useState<Map<string, string>>(new Map())
  useEffect(() => {
    api.listWorkflows(tenantId, 'project-1').then((wfs) => {
      setNames(new Map(wfs.map((w) => [w.id, w.name])))
    }).catch(() => {})
  }, [tenantId])
  return names
}

interface Props {
  onBack: () => void
  onOpenExecution: (executionId: string) => void
  onOpenWorkflow?: (workflowId: string) => void
  initialWorkflowFilter?: string
}

function formatTs(secs: number): string {
  return new Date(secs * 1000).toLocaleString()
}

function formatAge(secs: number, zh = false): string {
  const diff = Math.floor(Date.now() / 1000) - secs
  if (zh) {
    if (diff < 5) return '< 5秒'
    if (diff < 60) return `${diff}秒前`
    const mins = Math.floor(diff / 60)
    if (mins < 60) return `${mins}分钟前`
    return `${Math.floor(mins / 60)}小时前`
  }
  if (diff < 5) return '< 5s'
  if (diff < 60) return `${diff}s ago`
  const mins = Math.floor(diff / 60)
  if (mins < 60) return `${mins}m ago`
  return `${Math.floor(mins / 60)}h ago`
}

const PAGE_SIZE = 50

export function RunsPage({ onBack, onOpenExecution, onOpenWorkflow, initialWorkflowFilter }: Props) {
  const { auth } = useAuth()
  const { theme, toggle: toggleTheme } = useTheme()
  const { locale, toggle: toggleLocale, t } = useLocale()
  const zh = locale === 'zh'
  const workflowNames = useWorkflowNames(auth!.tenantId)
  const [runs, setRuns]               = useState<ExecutionSummary[]>([])
  const [total, setTotal]             = useState(0)
  const [loadedOffset, setLoadedOffset] = useState(0)
  const [loading, setLoading]         = useState(true)
  const [loadingMore, setLoadingMore] = useState(false)
  const [error, setError]             = useState<string | null>(null)
  const [searchQuery, setSearchQuery]     = useState(initialWorkflowFilter ?? '')
  const [labelFilter, setLabelFilter]   = useState('')
  const [outputFilter, setOutputFilter] = useState('')
  const [statusFilter, setStatusFilter]   = useState<string>('all')
  const [triggerFilter, setTriggerFilter] = useState<string>('all')
  const [starredOnly, setStarredOnly] = useState(false)
  const [acting, setActing]               = useState<Record<string, 'approving' | 'rejecting'>>({})
  const [cancellingAll, setCancellingAll] = useState(false)
  const [selected, setSelected]           = useState<Set<string>>(new Set())
  const [batchRetrying, setBatchRetrying] = useState(false)
  const [batchDeleting, setBatchDeleting] = useState(false)
  const [dateFilter, setDateFilter] = useState<'all' | 'today' | '7d' | '30d'>('all')
  const pollRef                           = useRef<ReturnType<typeof setInterval> | null>(null)

  const load = () => {
    setLoading(true)
    setError(null)
    setLoadedOffset(0)
    api.listExecutionsPage(auth!.tenantId, { limit: PAGE_SIZE, offset: 0 })
      .then(({ data, total: t }) => { setRuns(data); setTotal(t) })
      .catch((e: unknown) => setError(String(e)))
      .finally(() => setLoading(false))
  }

  const loadMore = () => {
    const nextOffset = loadedOffset + PAGE_SIZE
    setLoadingMore(true)
    api.listExecutionsPage(auth!.tenantId, { limit: PAGE_SIZE, offset: nextOffset })
      .then(({ data, total: t }) => { setRuns((prev) => [...prev, ...data]); setTotal(t); setLoadedOffset(nextOffset) })
      .catch((e: unknown) => setError(String(e)))
      .finally(() => setLoadingMore(false))
  }

  // Initial load
  useEffect(() => { load() }, [])

  // Subscribe via SSE for live updates; fall back to polling when unavailable.
  useEffect(() => {
    const token = auth?.token ?? ''
    const url = `/v1/executions/stream?tenant_id=${encodeURIComponent(auth!.tenantId)}&token=${encodeURIComponent(token)}`

    if (typeof EventSource !== 'undefined') {
      const es = new EventSource(url)
      es.addEventListener('update', (e: MessageEvent) => {
        try {
          const updated = JSON.parse(e.data as string) as ExecutionSummary[]
          setRuns(updated)
          setLoading(false)
        } catch { /* ignore parse errors */ }
      })
      es.onerror = () => {
        es.close()
        // Fall back to polling on SSE error
        if (!pollRef.current) {
          pollRef.current = setInterval(() => {
            api.listExecutions(auth!.tenantId).then(setRuns).catch(() => {})
          }, 4000)
        }
      }
      return () => { es.close(); if (pollRef.current) { clearInterval(pollRef.current); pollRef.current = null } }
    }

    // Fallback: poll every 4s
    pollRef.current = setInterval(() => {
      api.listExecutions(auth!.tenantId).then(setRuns).catch(() => {})
    }, 4000)
    return () => { if (pollRef.current) { clearInterval(pollRef.current); pollRef.current = null } }
  }, [auth?.tenantId])

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (colSettingsRef.current && !colSettingsRef.current.contains(e.target as Node)) setShowColSettings(false)
      if (presetsRef.current && !presetsRef.current.contains(e.target as Node)) setShowPresets(false)
    }
    document.addEventListener('mousedown', handler)
    return () => document.removeEventListener('mousedown', handler)
  }, [])

  const toggleCol = (col: string) => {
    setHiddenCols((prev) => {
      const next = new Set(prev)
      if (next.has(col)) next.delete(col); else next.add(col)
      try { localStorage.setItem('af:runs:hidden-cols', JSON.stringify([...next])) } catch { /* ignore */ }
      return next
    })
  }

  const pendingCount = runs.filter((r) => r.status === 'waiting_approval').length

  const handleApprove = async (e: React.MouseEvent, runId: string) => {
    e.stopPropagation()
    setActing((prev) => ({ ...prev, [runId]: 'approving' }))
    try {
      await api.approveExecution(runId)
      load()
    } catch (err) {
      alert(String(err))
    } finally {
      setActing((prev) => { const n = { ...prev }; delete n[runId]; return n })
    }
  }

  const handleReject = async (e: React.MouseEvent, runId: string) => {
    e.stopPropagation()
    setActing((prev) => ({ ...prev, [runId]: 'rejecting' }))
    try {
      await api.rejectExecution(runId)
      load()
    } catch (err) {
      alert(String(err))
    } finally {
      setActing((prev) => { const n = { ...prev }; delete n[runId]; return n })
    }
  }

  const handleCancelAll = async () => {
    if (!window.confirm(zh ? '取消所有运行中和等待中的执行？' : 'Cancel all running and waiting executions?')) return
    setCancellingAll(true)
    try {
      const { cancelled } = await api.cancelAllRunningExecutions(auth!.tenantId)
      alert(zh ? `已取消 ${cancelled} 个执行。` : `Cancelled ${cancelled} execution${cancelled !== 1 ? 's' : ''}.`)
      load()
    } catch (err) {
      alert(String(err))
    } finally {
      setCancellingAll(false)
    }
  }

  const [batchCancelling, setBatchCancelling] = useState(false)
  const [compareIds, setCompareIds] = useState<[string, string] | null>(null)
  const [compareData, setCompareData] = useState<[ExecutionRecord, ExecutionRecord] | null>(null)
  const [compareLoading, setCompareLoading] = useState(false)
  const [groupByWorkflow, setGroupByWorkflow] = useState(false)
  const [collapsedGroups, setCollapsedGroups] = useState<Set<string>>(new Set())
  const [viewMode, setViewMode] = useState<'table' | 'timeline'>('table')
  const [showColSettings, setShowColSettings] = useState(false)
  const [showPresets, setShowPresets] = useState(false)
  const [presetName, setPresetName] = useState('')
  const [filterPresets, setFilterPresets] = useState<Array<{ name: string; status: string; trigger: string; date: string; search: string; label: string; starred: boolean }>>(() => {
    try { return JSON.parse(localStorage.getItem('af:runs:presets') ?? '[]') } catch { return [] }
  })
  const presetsRef = useRef<HTMLDivElement>(null)
  const [hiddenCols, setHiddenCols] = useState<Set<string>>(() => {
    try { return new Set(JSON.parse(localStorage.getItem('af:runs:hidden-cols') ?? '[]') as string[]) } catch { return new Set() }
  })
  const colSettingsRef = useRef<HTMLDivElement>(null)

  const handleBatchCancel = async () => {
    const liveSelected = [...selected].filter((id) => {
      const run = runs.find((r) => r.id === id)
      return run && LIVE_STATUSES.has(run.status)
    })
    if (liveSelected.length === 0) return
    if (!window.confirm(zh ? `取消 ${liveSelected.length} 个运行中的执行？` : `Cancel ${liveSelected.length} running execution${liveSelected.length !== 1 ? 's' : ''}?`)) return
    setBatchCancelling(true)
    let ok = 0
    for (const id of liveSelected) {
      try { await api.cancelExecution(auth!.tenantId, id); ok++ } catch { /* skip */ }
    }
    setSelected(new Set())
    setBatchCancelling(false)
    if (ok > 0) load()
  }

  const handleBatchRetry = async () => {
    if (selected.size === 0) return
    if (!window.confirm(zh ? `重试 ${selected.size} 条选中的执行？` : `Retry ${selected.size} execution${selected.size !== 1 ? 's' : ''}?`)) return
    setBatchRetrying(true)
    let ok = 0
    for (const id of selected) {
      try { await api.retryExecution(auth!.tenantId, id); ok++ } catch { /* skip */ }
    }
    setSelected(new Set())
    setBatchRetrying(false)
    if (ok > 0) load()
  }

  const handleBatchDelete = async () => {
    const terminal = [...selected].filter((id) => {
      const r = runs.find((x) => x.id === id)
      return r && !LIVE_STATUSES.has(r.status)
    })
    if (terminal.length === 0) return
    if (!window.confirm(zh ? `永久删除 ${terminal.length} 条执行记录？此操作不可撤回。` : `Permanently delete ${terminal.length} execution record${terminal.length !== 1 ? 's' : ''}?`)) return
    setBatchDeleting(true)
    let ok = 0
    for (const id of terminal) {
      try { await api.deleteExecution(auth!.tenantId, id); ok++ } catch { /* skip */ }
    }
    setSelected(new Set())
    setBatchDeleting(false)
    if (ok > 0) load()
  }

  const handleCompare = async () => {
    const ids = [...selected].slice(0, 2) as [string, string]
    setCompareIds(ids)
    setCompareLoading(true)
    try {
      const [a, b] = await Promise.all([
        api.getExecution(auth!.tenantId, ids[0]),
        api.getExecution(auth!.tenantId, ids[1]),
      ])
      setCompareData([a, b])
    } catch {
      setCompareIds(null)
    } finally {
      setCompareLoading(false)
    }
  }

  const savePreset = () => {
    if (!presetName.trim()) return
    const preset = { name: presetName.trim(), status: statusFilter, trigger: triggerFilter, date: dateFilter, search: searchQuery, label: labelFilter, starred: starredOnly }
    const next = [...filterPresets.filter((p) => p.name !== preset.name), preset]
    setFilterPresets(next)
    try { localStorage.setItem('af:runs:presets', JSON.stringify(next)) } catch { /* ignore */ }
    setPresetName('')
    setShowPresets(false)
  }

  const loadPreset = (p: typeof filterPresets[0]) => {
    setStatusFilter(p.status)
    setTriggerFilter(p.trigger)
    setDateFilter(p.date as 'all' | 'today' | '7d' | '30d')
    setSearchQuery(p.search)
    setLabelFilter(p.label)
    setStarredOnly(p.starred)
    setShowPresets(false)
  }

  const deletePreset = (name: string) => {
    const next = filterPresets.filter((p) => p.name !== name)
    setFilterPresets(next)
    try { localStorage.setItem('af:runs:presets', JSON.stringify(next)) } catch { /* ignore */ }
  }

  const handleExportCsv = () => {
    const rows = filtered
    const header = ['ID', 'Status', 'Workflow', 'Label', 'Trigger', 'Started', 'Finished', 'Duration(s)']
    const escape = (s: string) => `"${s.replace(/"/g, '""')}"`
    const lines = [
      header.join(','),
      ...rows.map((r) => {
        const wfName = workflowNames.get(r.workflow_id) ?? r.workflow_id
        const dur = r.finished_at ? String(r.finished_at - r.started_at) : ''
        return [
          escape(r.id),
          escape(r.status),
          escape(wfName),
          escape(r.label ?? ''),
          escape(r.trigger_type ?? 'manual'),
          escape(new Date(r.started_at * 1000).toISOString()),
          r.finished_at ? escape(new Date(r.finished_at * 1000).toISOString()) : '',
          dur,
        ].join(',')
      }),
    ]
    const blob = new Blob([lines.join('\n')], { type: 'text/csv' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = `runs-export-${Date.now()}.csv`
    a.click()
    URL.revokeObjectURL(url)
  }

  const nowSecs = Math.floor(Date.now() / 1000)
  const liveCount = runs.filter((r) => LIVE_STATUSES.has(r.status)).length
  const allLabels = Array.from(new Set(runs.map((r) => r.label).filter(Boolean) as string[]))
  const statusCounts: Record<string, number> = runs.reduce((acc, r) => {
    acc[r.status] = (acc[r.status] ?? 0) + 1
    return acc
  }, {} as Record<string, number>)
  const dateThreshold = dateFilter === 'today' ? nowSecs - 86400
    : dateFilter === '7d' ? nowSecs - 7 * 86400
    : dateFilter === '30d' ? nowSecs - 30 * 86400
    : 0
  const byDate      = dateThreshold > 0 ? runs.filter((r) => r.started_at >= dateThreshold) : runs
  const byStatus    = statusFilter === 'all' ? byDate : byDate.filter((r) => r.status === statusFilter)
  const byTrigger   = triggerFilter === 'all' ? byStatus : byStatus.filter((r) => (r.trigger_type ?? 'manual') === triggerFilter)
  const byStarred   = starredOnly ? byTrigger.filter((r) => r.starred) : byTrigger
  const byLabel     = labelFilter ? byStarred.filter((r) => r.label === labelFilter) : byStarred
  const byOutput    = outputFilter ? byLabel.filter((r) => (r as Record<string, unknown>).output_json != null && String((r as Record<string, unknown>).output_json).toLowerCase().includes(outputFilter.toLowerCase())) : byLabel
  const filtered    = searchQuery
    ? byOutput.filter((r) => {
        const q = searchQuery.toLowerCase()
        return r.id.toLowerCase().startsWith(q) ||
          (workflowNames.get(r.workflow_id) ?? '').toLowerCase().includes(q) ||
          (r.label ?? '').toLowerCase().includes(q)
      })
    : byOutput

  // Stats for mini bar (from full `runs` list, not filtered)
  const todayCutoff = nowSecs - 86400
  const todayRuns = runs.filter((r) => r.started_at >= todayCutoff)
  const miniStats = {
    running: runs.filter((r) => LIVE_STATUSES.has(r.status)).length,
    todaySucceeded: todayRuns.filter((r) => r.status === 'succeeded').length,
    todayFailed: todayRuns.filter((r) => r.status === 'failed').length,
    todayTotal: todayRuns.length,
  }

  return (
    <div className="app">
      <header className="topbar">
        <img src={logoWordmark} alt="Velara" style={{ height: '28px' }} />
        <span className="topbar-sep">|</span>
        <span className="topbar-title">{zh ? '运行记录' : 'Run History'}</span>

        {/* Center: global actions */}
        <div className="topbar-actions" style={{ flex: 1 }}>
          <button className="btn btn-sm" onClick={load} title={zh ? '刷新' : 'Refresh'}>↺ {zh ? '刷新' : 'Refresh'}</button>
          {filtered.length > 0 && (
            <button className="btn btn-sm" onClick={handleExportCsv} title={`Export ${filtered.length} runs as CSV`}>
              ↓ {t('runs.export.csv')}
            </button>
          )}
          <button
            className={`btn btn-sm${groupByWorkflow ? ' btn-primary' : ''}`}
            onClick={() => { setGroupByWorkflow((v) => !v); setCollapsedGroups(new Set()); setViewMode('table') }}
            title={zh ? '按工作流分组' : 'Group by workflow'}
          >
            ⊞ {zh ? '分组' : 'Group'}
          </button>
          <button
            className={`btn btn-sm${viewMode === 'timeline' ? ' btn-primary' : ''}`}
            onClick={() => { setViewMode((v) => v === 'timeline' ? 'table' : 'timeline'); setGroupByWorkflow(false) }}
            title={zh ? '时间线视图' : 'Timeline view'}
          >
            ⏱ {zh ? '时间线' : 'Timeline'}
          </button>
          {liveCount > 0 && (
            <button
              className="btn btn-sm btn-danger"
              disabled={cancellingAll}
              onClick={handleCancelAll}
              title={zh ? `取消全部 ${liveCount} 个运行中/等待执行` : `Cancel all ${liveCount} running/waiting`}
            >
              {cancellingAll ? '…' : `✕ ${zh ? '取消全部' : 'Cancel All'} (${liveCount})`}
            </button>
          )}
          {/* Batch actions shown only when items selected */}
          {selected.size > 0 && (
            <>
              {[...selected].some((id) => { const r = runs.find((x) => x.id === id); return r && LIVE_STATUSES.has(r.status) }) && (
                <button className="btn btn-sm btn-danger" disabled={batchCancelling} onClick={handleBatchCancel}>
                  {batchCancelling ? '…' : `✕ ${zh ? '取消运行中' : 'Cancel'} (${[...selected].filter((id) => { const r = runs.find((x) => x.id === id); return r && LIVE_STATUSES.has(r.status) }).length})`}
                </button>
              )}
              <button className="btn btn-sm btn-primary" disabled={batchRetrying} onClick={handleBatchRetry}>
                {batchRetrying ? '…' : `↺ ${zh ? '重试' : 'Retry'} (${selected.size})`}
              </button>
              {(() => {
                const n = [...selected].filter((id) => { const r = runs.find((x) => x.id === id); return r && !LIVE_STATUSES.has(r.status) }).length
                return n > 0 ? (
                  <button className="btn btn-sm btn-danger" disabled={batchDeleting} onClick={handleBatchDelete}>
                    {batchDeleting ? '…' : `🗑 ${n}`}
                  </button>
                ) : null
              })()}
              {selected.size === 2 && (
                <button className="btn btn-sm" onClick={handleCompare}>⇌ {zh ? '对比' : 'Compare'}</button>
              )}
              <button className="btn btn-sm" onClick={() => setSelected(new Set())} title={zh ? '清除选择' : 'Clear selection'}>
                {zh ? `已选 ${selected.size}` : `${selected.size} sel.`} ✕
              </button>
            </>
          )}
        </div>

        {/* Right: utilities */}
        <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
          {/* Filter presets */}
          <div ref={presetsRef} style={{ position: 'relative' }}>
            <button
              className={`btn btn-sm${filterPresets.length > 0 ? ' btn-primary' : ''}`}
              onClick={() => setShowPresets((v) => !v)}
              title={zh ? '保存或加载过滤预设' : 'Save or load filter presets'}
              style={{ fontSize: 11 }}
            >
              ★ {zh ? '预设' : 'Presets'}{filterPresets.length > 0 ? ` (${filterPresets.length})` : ''}
            </button>
            {showPresets && (
              <div style={{
                position: 'absolute', top: 'calc(100% + 6px)', right: 0, background: 'var(--surface)',
                border: '1px solid var(--border)', borderRadius: 8, boxShadow: '0 8px 24px rgba(0,0,0,0.15)',
                zIndex: 200, minWidth: 240, padding: '8px 0',
              }}>
                <div style={{ padding: '4px 14px 8px', fontSize: 11, color: 'var(--muted)', fontWeight: 600 }}>
                  {zh ? '过滤预设' : 'FILTER PRESETS'}
                </div>
                {filterPresets.length === 0 && (
                  <div style={{ padding: '4px 14px', fontSize: 12, color: 'var(--muted)' }}>{zh ? '暂无预设' : 'No presets yet'}</div>
                )}
                {filterPresets.map((p) => (
                  <div key={p.name} style={{ display: 'flex', alignItems: 'center', padding: '3px 14px', gap: 6 }}>
                    <button
                      className="btn btn-sm"
                      style={{ flex: 1, textAlign: 'left', background: 'none', border: 'none', padding: '2px 0', fontSize: 13, cursor: 'pointer' }}
                      onClick={() => loadPreset(p)}
                    >
                      {p.name}
                    </button>
                    <button
                      className="btn btn-sm btn-icon"
                      style={{ fontSize: 11, opacity: 0.5, padding: '0 4px' }}
                      onClick={() => deletePreset(p.name)}
                      title={zh ? '删除预设' : 'Delete preset'}
                    >
                      ✕
                    </button>
                  </div>
                ))}
                <div style={{ borderTop: '1px solid var(--border)', margin: '6px 0 4px' }} />
                <div style={{ padding: '4px 14px 6px', fontSize: 11, color: 'var(--muted)', fontWeight: 600 }}>
                  {zh ? '保存当前过滤条件' : 'SAVE CURRENT FILTERS'}
                </div>
                <div style={{ display: 'flex', gap: 6, padding: '0 14px 6px' }}>
                  <input
                    placeholder={zh ? '预设名称…' : 'Preset name…'}
                    value={presetName}
                    onChange={(e) => setPresetName(e.target.value)}
                    onKeyDown={(e) => { if (e.key === 'Enter') savePreset(); if (e.key === 'Escape') setShowPresets(false) }}
                    style={{ flex: 1, fontSize: 12, padding: '3px 6px' }}
                    autoFocus
                  />
                  <button className="btn btn-sm btn-primary" disabled={!presetName.trim()} onClick={savePreset}>
                    {zh ? '保存' : 'Save'}
                  </button>
                </div>
              </div>
            )}
          </div>
          {/* Column visibility */}
          <div ref={colSettingsRef} style={{ position: 'relative' }}>
            <button
              className={`btn btn-sm${hiddenCols.size > 0 ? ' btn-primary' : ''}`}
              onClick={() => setShowColSettings((v) => !v)}
              title={zh ? '列显示设置' : 'Column settings'}
              style={{ fontSize: 11 }}
            >
              ⚙ {zh ? '列' : 'Cols'}
            </button>
            {showColSettings && (
              <div style={{
                position: 'absolute', top: 'calc(100% + 6px)', right: 0, background: 'var(--surface)',
                border: '1px solid var(--border)', borderRadius: 8, boxShadow: '0 8px 24px rgba(0,0,0,0.15)',
                zIndex: 200, minWidth: 180, padding: '8px 0',
              }}>
                <div style={{ padding: '4px 14px 8px', fontSize: 11, color: 'var(--muted)', fontWeight: 600 }}>
                  {zh ? '显示列' : 'VISIBLE COLUMNS'}
                </div>
                {[
                  { id: 'label', label: zh ? '标签' : 'Label' },
                  { id: 'trigger', label: zh ? '来源' : 'Trigger' },
                  { id: 'duration', label: zh ? '耗时' : 'Duration' },
                  { id: 'age', label: zh ? '时间' : 'Age' },
                  { id: 'id', label: 'ID' },
                ].map((col) => (
                  <label
                    key={col.id}
                    style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '4px 14px', cursor: 'pointer', fontSize: 13 }}
                    onMouseEnter={(e) => (e.currentTarget.style.background = 'var(--hover)')}
                    onMouseLeave={(e) => (e.currentTarget.style.background = 'none')}
                  >
                    <input type="checkbox" checked={!hiddenCols.has(col.id)} onChange={() => toggleCol(col.id)} />
                    {col.label}
                  </label>
                ))}
              </div>
            )}
          </div>
          <button className="btn btn-sm" onClick={toggleTheme}>{theme === 'dark' ? '☀' : '◑'}</button>
          <button className="btn btn-sm" onClick={toggleLocale}>{locale === 'zh' ? 'EN' : '中'}</button>
          <button className="btn btn-sm" onClick={onBack}>← {t('nav.back')}</button>
        </div>
      </header>

      <main className="list-page">
        <div className="list-header">
          <h1>{t('runs.title')}</h1>
          <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
            <input
              placeholder={zh ? '按 ID、工作流或标签搜索…' : 'Search by ID, workflow, or label…'}
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              style={{ fontSize: 12, padding: '4px 8px', width: 240 }}
            />
            {searchQuery && (
              <button className="btn btn-sm btn-icon" onClick={() => setSearchQuery('')} title={zh ? '清除搜索' : 'Clear search'}>✕</button>
            )}
            <input
              placeholder={zh ? '输出包含…' : 'Output contains…'}
              value={outputFilter}
              onChange={(e) => setOutputFilter(e.target.value)}
              style={{ fontSize: 12, padding: '4px 8px', width: 140 }}
              title={zh ? '按工作流输出内容过滤' : 'Filter by workflow output content'}
            />
            {outputFilter && (
              <button className="btn btn-sm btn-icon" onClick={() => setOutputFilter('')} title={zh ? '清除输出过滤' : 'Clear output filter'}>✕</button>
            )}
            <span style={{ color: 'var(--muted)', fontSize: 13 }}>
              {filtered.length}{(searchQuery || labelFilter || statusFilter !== 'all') ? ` / ${runs.length}` : ''}{total > runs.length ? ` (${zh ? '共' : 'total'} ${total})` : ''} {zh ? '条' : 'runs'}
            </span>
            <span style={{
              display: 'inline-flex', alignItems: 'center', gap: 4,
              fontSize: 11, color: 'var(--link)', fontStyle: 'italic',
            }}>
              <span style={{
                width: 6, height: 6, borderRadius: '50%',
                background: 'var(--link)', animation: 'pulse 2s infinite', flexShrink: 0,
              }} />
              {zh ? '实时' : 'live'}
            </span>
          </div>
        </div>

        {/* Mini stats bar */}
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: 10, marginBottom: 16 }}>
          {[
            {
              label: zh ? '运行中' : 'Running',
              value: miniStats.running,
              color: 'var(--node-http)',
              pulse: miniStats.running > 0,
            },
            {
              label: zh ? '今日成功' : 'Today OK',
              value: miniStats.todaySucceeded,
              color: 'var(--success)',
              pulse: false,
            },
            {
              label: zh ? '今日失败' : 'Today Fail',
              value: miniStats.todayFailed,
              color: 'var(--danger-text)',
              pulse: false,
            },
            {
              label: zh ? '今日合计' : 'Today Total',
              value: miniStats.todayTotal,
              color: 'var(--muted)',
              pulse: false,
            },
          ].map((card) => (
            <div key={card.label} style={{
              background: 'var(--panel)',
              border: '1px solid var(--border)',
              borderRadius: 8,
              padding: '10px 14px',
              display: 'flex',
              flexDirection: 'column',
              gap: 4,
            }}>
              <span style={{ fontSize: 22, fontWeight: 700, color: card.color, display: 'flex', alignItems: 'center', gap: 6 }}>
                {card.value}
                {card.pulse && card.value > 0 && (
                  <span style={{ width: 8, height: 8, borderRadius: '50%', background: card.color, animation: 'pulse 1.5s infinite', flexShrink: 0 }} />
                )}
              </span>
              <span style={{ fontSize: 11, color: 'var(--muted)', textTransform: 'uppercase', letterSpacing: '0.04em' }}>{card.label}</span>
            </div>
          ))}
        </div>

        {/* Date range filter */}
        <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap', marginBottom: 8 }}>
          {(['all', 'today', '7d', '30d'] as const).map((d) => (
            <button
              key={d}
              className={`btn btn-sm${dateFilter === d ? ' btn-primary' : ''}`}
              onClick={() => setDateFilter(d)}
              style={{ fontSize: 11 }}
            >
              {d === 'all' ? (zh ? '全部时间' : 'All Time') : d === 'today' ? (zh ? '今日' : 'Today') : d === '7d' ? (zh ? '7 天' : '7 Days') : (zh ? '30 天' : '30 Days')}
            </button>
          ))}
        </div>

        {/* Pending approvals banner */}
        {pendingCount > 0 && (
          <div style={{
            display: 'flex',
            alignItems: 'center',
            gap: 10,
            padding: '10px 14px',
            marginBottom: 16,
            background: 'rgba(8,145,178,0.08)',
            border: '1px solid var(--approval-text)',
            borderRadius: 8,
          }}>
            <span style={{
              display: 'inline-block',
              width: 8,
              height: 8,
              borderRadius: '50%',
              background: 'var(--approval-text)',
              animation: 'pulse 1s infinite',
              flexShrink: 0,
            }} />
            <span style={{ color: 'var(--approval-text)', fontWeight: 600, fontSize: 13 }}>
              {zh ? `${pendingCount} 条执行等待审批` : `${pendingCount} execution${pendingCount !== 1 ? 's' : ''} waiting for approval`}
            </span>
            <span style={{ color: 'var(--muted)', fontSize: 12 }}>
              {zh ? '— 在下表直接批准或拒绝' : '— approve or reject directly in the table below'}
            </span>
          </div>
        )}

        {/* Status filter */}
        <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap', marginBottom: 8 }}>
          {(['all', 'running', 'waiting_approval', 'succeeded', 'failed', 'cancelled'] as const).map((s) => {
            const count = s === 'all' ? runs.length : (statusCounts[s] ?? 0)
            if (s !== 'all' && count === 0) return null
            return (
              <button
                key={s}
                className={`btn btn-sm${statusFilter === s ? ' btn-primary' : ''}`}
                onClick={() => setStatusFilter(s)}
              >
                {s === 'waiting_approval' ? (zh ? '待审批' : 'waiting') : s === 'all' ? (zh ? '全部' : 'all') : s === 'running' ? (zh ? '运行中' : 'running') : s === 'succeeded' ? (zh ? '成功' : 'succeeded') : s === 'failed' ? (zh ? '失败' : 'failed') : s === 'cancelled' ? (zh ? '已取消' : 'cancelled') : s}
                {count > 0 && (
                  <span style={{ marginLeft: 4, fontSize: 10, opacity: 0.75 }}>
                    {count}
                  </span>
                )}
              </button>
            )
          })}
        </div>

        <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap', marginBottom: 8 }}>
          {(['all', 'manual', 'webhook', 'schedule', 'retry'] as const).map((t) => (
            <button
              key={t}
              className={`btn btn-sm${triggerFilter === t ? ' btn-primary' : ''}`}
              onClick={() => setTriggerFilter(t)}
              style={{ fontSize: 11 }}
            >
              {t === 'all' ? (zh ? '全部来源' : 'All Sources') : t === 'manual' ? (zh ? '▶ 手动' : '▶ Manual') : t === 'webhook' ? '⇅ Webhook' : t === 'schedule' ? (zh ? '⏱ 调度' : '⏱ Schedule') : (zh ? '↺ 重试' : '↺ Retry')}
            </button>
          ))}
          <button
            className={`btn btn-sm${starredOnly ? ' btn-primary' : ''}`}
            onClick={() => setStarredOnly((s) => !s)}
            title={zh ? '只显示已收藏' : 'Show starred only'}
            style={{ fontSize: 11 }}
          >
            ⭐ {zh ? '已收藏' : 'Starred'}
          </button>
        </div>

        {allLabels.length > 0 && (
          <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap', marginBottom: 12 }}>
            <button
              className={`btn btn-sm${!labelFilter ? ' btn-primary' : ''}`}
              onClick={() => setLabelFilter('')}
            >
              {zh ? '所有标签' : 'All Labels'}
            </button>
            {allLabels.map((l) => (
              <button
                key={l}
                className={`btn btn-sm${labelFilter === l ? ' btn-primary' : ''}`}
                onClick={() => setLabelFilter(l === labelFilter ? '' : l)}
              >
                {l}
              </button>
            ))}
          </div>
        )}

        {(loading || loadingMore) && <p>{zh ? '加载中…' : 'Loading…'}</p>}
        {error && <p style={{ color: 'var(--danger-text)' }}>{error}</p>}

        {!loading && !error && filtered.length === 0 && (
          <div className="empty-state">
            <p>
              {searchQuery || labelFilter || statusFilter !== 'all'
                ? (zh ? '无符合当前筛选条件的记录。' : 'No runs match the current filters.')
                : (zh ? '暂无执行记录，运行工作流后在此查看历史。' : 'No executions yet. Run a workflow to see history here.')}
            </p>
          </div>
        )}

        {!loading && filtered.length > 0 && groupByWorkflow && (() => {
          const groups = new Map<string, ExecutionSummary[]>()
          for (const r of filtered) {
            const list = groups.get(r.workflow_id) ?? []
            list.push(r)
            groups.set(r.workflow_id, list)
          }
          return (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
              {[...groups.entries()].map(([wfId, rows]) => {
                const wfName = workflowNames.get(wfId) ?? wfId.slice(0, 8) + '…'
                const collapsed = collapsedGroups.has(wfId)
                const succRate = rows.length > 0 ? Math.round(rows.filter((r) => r.status === 'succeeded').length / rows.length * 100) : 0
                const failCount = rows.filter((r) => r.status === 'failed').length
                return (
                  <div key={wfId} style={{ border: '1px solid var(--border)', borderRadius: 8, overflow: 'hidden' }}>
                    <button
                      onClick={() => setCollapsedGroups((prev) => {
                        const next = new Set(prev)
                        if (next.has(wfId)) next.delete(wfId); else next.add(wfId)
                        return next
                      })}
                      style={{
                        display: 'flex', alignItems: 'center', gap: 10, width: '100%',
                        padding: '8px 14px', background: 'var(--panel)', border: 'none',
                        cursor: 'pointer', textAlign: 'left',
                      }}
                      onMouseEnter={(e) => (e.currentTarget.style.filter = 'brightness(0.97)')}
                      onMouseLeave={(e) => (e.currentTarget.style.filter = 'none')}
                    >
                      <span style={{ fontSize: 12, color: 'var(--muted)', userSelect: 'none' }}>{collapsed ? '▶' : '▼'}</span>
                      {onOpenWorkflow ? (
                        <span
                          style={{ fontWeight: 600, fontSize: 13, color: 'var(--link)', cursor: 'pointer' }}
                          onClick={(e) => { e.stopPropagation(); onOpenWorkflow(wfId) }}
                        >{wfName}</span>
                      ) : (
                        <span style={{ fontWeight: 600, fontSize: 13 }}>{wfName}</span>
                      )}
                      <span style={{ fontSize: 11, color: 'var(--muted)', marginLeft: 4 }}>
                        {rows.length} {zh ? '次运行' : 'runs'}
                      </span>
                      <span style={{ fontSize: 11, color: succRate >= 90 ? 'var(--success-text)' : succRate >= 70 ? 'var(--warning-text)' : 'var(--danger-text)', fontWeight: 600, marginLeft: 4 }}>
                        {succRate}%
                      </span>
                      {failCount > 0 && (
                        <span style={{ fontSize: 11, color: 'var(--danger-text)' }}>{failCount} ✕</span>
                      )}
                    </button>
                    {!collapsed && (
                      <table className="workflow-table" style={{ marginBottom: 0 }}>
                        <thead>
                          <tr>
                            <th>{zh ? '开始时间' : 'Started'}</th>
                            <th>{zh ? '标签' : 'Label'}</th>
                            <th>{zh ? '状态' : 'Status'}</th>
                            <th>{zh ? '耗时' : 'Dur.'}</th>
                            <th>{zh ? '时间' : 'Age'}</th>
                          </tr>
                        </thead>
                        <tbody>
                          {rows.map((run) => (
                            <tr key={run.id} onClick={() => onOpenExecution(run.id)} style={{ cursor: 'pointer' }}>
                              <td style={{ fontSize: 12, color: 'var(--muted)', whiteSpace: 'nowrap' }}>{formatTs(run.started_at)}</td>
                              <td style={{ fontSize: 12, maxWidth: 180, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                                {run.label ? <span style={{ background: 'var(--panel)', border: '1px solid var(--border)', borderRadius: 4, padding: '1px 5px', fontSize: 11 }}>{run.label}</span> : <span style={{ color: 'var(--muted)' }}>—</span>}
                              </td>
                              <td><span className={`badge badge-${run.status}`}>{run.status}</span></td>
                              <td style={{ color: 'var(--muted)', fontSize: 12, whiteSpace: 'nowrap' }}>
                                {run.finished_at
                                  ? (() => { const s = run.finished_at - run.started_at; return s < 60 ? `${s}s` : `${Math.floor(s / 60)}m ${s % 60}s` })()
                                  : LIVE_STATUSES.has(run.status) ? <span style={{ color: 'var(--node-http)' }}>…</span> : '—'}
                              </td>
                              <td style={{ color: 'var(--muted)', fontSize: 12 }}>{formatAge(run.started_at, zh)}</td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    )}
                  </div>
                )
              })}
            </div>
          )
        })()}

        {!loading && filtered.length > 0 && viewMode === 'timeline' && (() => {
          const sorted = [...filtered].sort((a, b) => a.started_at - b.started_at)
          const minTs = sorted[0].started_at
          const maxTs = Math.max(...sorted.map((r) => r.finished_at ?? Math.floor(Date.now() / 1000)))
          const span = Math.max(maxTs - minTs, 1)
          const statusColors: Record<string, string> = {
            succeeded: '#3fb950', failed: '#f85149', running: '#58a6ff', cancelled: '#8b949e', waiting_approval: '#d29922',
          }
          const uniqueWfs = [...new Set(sorted.map((r) => r.workflow_id))]
          const rowHeight = 32
          const labelWidth = 140
          const chartWidth = 700
          const totalHeight = uniqueWfs.length * rowHeight + 40
          return (
            <div style={{ overflowX: 'auto', marginBottom: 16 }}>
              <div style={{ fontSize: 12, color: 'var(--muted)', marginBottom: 8, display: 'flex', gap: 16 }}>
                <span>{zh ? '开始时间' : 'Start'}: {new Date(minTs * 1000).toLocaleString()}</span>
                <span>→</span>
                <span>{new Date(maxTs * 1000).toLocaleString()}</span>
                <span style={{ marginLeft: 'auto', color: 'var(--muted)' }}>
                  {zh ? '跨度' : 'Span'}: {span < 60 ? `${span}s` : span < 3600 ? `${Math.round(span / 60)}m` : `${(span / 3600).toFixed(1)}h`}
                </span>
              </div>
              <div style={{ position: 'relative', width: labelWidth + chartWidth + 20 }}>
                {/* Header time axis */}
                <div style={{ display: 'flex', marginLeft: labelWidth + 4, marginBottom: 4 }}>
                  {[0, 25, 50, 75, 100].map((pct) => (
                    <div key={pct} style={{ position: 'absolute', left: labelWidth + (chartWidth * pct / 100), fontSize: 9, color: 'var(--muted)', transform: 'translateX(-50%)' }}>
                      {pct === 0 ? '' : `+${Math.round(span * pct / 100)}s`}
                    </div>
                  ))}
                </div>
                <div style={{ marginTop: 16 }}>
                  {uniqueWfs.map((wfId, rowIdx) => {
                    const rowRuns = sorted.filter((r) => r.workflow_id === wfId)
                    const wfName = workflowNames.get(wfId) ?? wfId.slice(0, 12)
                    void rowIdx
                    return (
                      <div key={wfId} style={{ display: 'flex', alignItems: 'center', height: rowHeight, borderBottom: '1px solid var(--border)' }}>
                        <div style={{ width: labelWidth, fontSize: 11, color: 'var(--text)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', paddingRight: 8, flexShrink: 0 }}
                          title={wfName}>
                          {wfName}
                        </div>
                        <div style={{ position: 'relative', width: chartWidth, height: rowHeight - 4, flexShrink: 0 }}>
                          {/* Grid lines */}
                          {[25, 50, 75].map((pct) => (
                            <div key={pct} style={{ position: 'absolute', left: `${pct}%`, top: 0, bottom: 0, borderLeft: '1px dashed var(--border)', opacity: 0.5 }} />
                          ))}
                          {rowRuns.map((run) => {
                            const startPct = (run.started_at - minTs) / span * 100
                            const endTs = run.finished_at ?? Math.floor(Date.now() / 1000)
                            const widthPct = Math.max((endTs - run.started_at) / span * 100, 0.3)
                            const color = statusColors[run.status] ?? '#8b949e'
                            const dur = endTs - run.started_at
                            return (
                              <div
                                key={run.id}
                                onClick={() => onOpenExecution(run.id)}
                                title={`${run.label ?? run.id.slice(-8)} — ${run.status} — ${dur}s`}
                                style={{
                                  position: 'absolute',
                                  left: `${startPct}%`,
                                  width: `${widthPct}%`,
                                  top: 4,
                                  height: rowHeight - 12,
                                  background: color,
                                  borderRadius: 3,
                                  cursor: 'pointer',
                                  opacity: 0.85,
                                  transition: 'opacity 0.1s',
                                  overflow: 'hidden',
                                }}
                                onMouseEnter={(e) => (e.currentTarget.style.opacity = '1')}
                                onMouseLeave={(e) => (e.currentTarget.style.opacity = '0.85')}
                              >
                                {widthPct > 5 && (
                                  <span style={{ position: 'absolute', left: 4, top: '50%', transform: 'translateY(-50%)', fontSize: 9, color: '#fff', whiteSpace: 'nowrap', fontWeight: 600 }}>
                                    {run.label ?? run.id.slice(-6)}
                                  </span>
                                )}
                              </div>
                            )
                          })}
                        </div>
                      </div>
                    )
                  })}
                </div>
                {/* Legend */}
                <div style={{ display: 'flex', gap: 12, marginTop: 10, flexWrap: 'wrap' }}>
                  {Object.entries(statusColors).map(([s, c]) => (
                    <div key={s} style={{ display: 'flex', alignItems: 'center', gap: 4, fontSize: 11 }}>
                      <div style={{ width: 10, height: 10, borderRadius: 2, background: c }} />
                      <span style={{ color: 'var(--muted)' }}>{s}</span>
                    </div>
                  ))}
                </div>
              </div>
            </div>
          )
        })()}

        {!loading && filtered.length > 0 && !groupByWorkflow && viewMode === 'table' && (
          <table className="workflow-table">
            <thead>
              <tr>
                <th style={{ width: 28 }}>
                  <input
                    type="checkbox"
                    title={zh ? '全选可重试行' : 'Select all retryable rows'}
                    checked={filtered.filter((r) => r.status === 'failed' || r.status === 'cancelled').every((r) => selected.has(r.id)) && filtered.some((r) => r.status === 'failed' || r.status === 'cancelled')}
                    onChange={(e) => {
                      const retryable = filtered.filter((r) => r.status === 'failed' || r.status === 'cancelled').map((r) => r.id)
                      setSelected(e.target.checked ? new Set(retryable) : new Set())
                    }}
                  />
                </th>
                <th>{t('runs.col.started')}</th>
                <th>{t('runs.col.workflow')}</th>
                {!hiddenCols.has('label') && <th>{t('runs.col.label')}</th>}
                {!hiddenCols.has('trigger') && <th>{t('runs.col.trigger')}</th>}
                <th>{t('runs.col.status')}</th>
                {!hiddenCols.has('duration') && <th>{locale === 'zh' ? '耗时' : 'Dur.'}</th>}
                {!hiddenCols.has('age') && <th>{locale === 'zh' ? '时间' : 'Age'}</th>}
                {!hiddenCols.has('id') && <th>{t('runs.col.id')}</th>}
                <th></th>
              </tr>
            </thead>
            <tbody>
              {filtered.map((run) => {
                const isPending = run.status === 'waiting_approval'
                const isActing  = !!acting[run.id]
                return (
                  <tr
                    key={run.id}
                    onClick={() => onOpenExecution(run.id)}
                    title={zh ? '查看执行详情' : 'View execution details'}
                    style={isPending ? { background: 'rgba(8,145,178,0.04)' } : selected.has(run.id) ? { background: 'rgba(37,99,235,0.06)' } : undefined}
                  >
                    <td onClick={(e) => e.stopPropagation()}>
                      {(run.status === 'failed' || run.status === 'cancelled') && (
                        <input
                          type="checkbox"
                          checked={selected.has(run.id)}
                          onChange={(e) => setSelected((prev) => {
                            const next = new Set(prev)
                            if (e.target.checked) next.add(run.id); else next.delete(run.id)
                            return next
                          })}
                        />
                      )}
                    </td>
                    <td style={{ fontSize: 12, color: 'var(--muted)', whiteSpace: 'nowrap' }}>
                      {formatTs(run.started_at)}
                    </td>
                    <td className="name" style={{ maxWidth: 180, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                      <span
                        title={run.workflow_id}
                        onClick={onOpenWorkflow ? (e) => { e.stopPropagation(); onOpenWorkflow(run.workflow_id) } : undefined}
                        style={onOpenWorkflow ? { color: 'var(--link)', cursor: 'pointer', textDecoration: 'underline' } : undefined}
                      >
                        {workflowNames.get(run.workflow_id) ?? run.workflow_id.slice(-12)}
                      </span>
                    </td>
                    {!hiddenCols.has('label') && (
                      <td>
                        {run.label ? (
                          <span
                            style={{
                              fontSize: 11,
                              background: 'var(--panel)',
                              border: '1px solid var(--border)',
                              borderRadius: 10,
                              padding: '2px 8px',
                              cursor: 'pointer',
                            }}
                            onClick={(e) => { e.stopPropagation(); setLabelFilter(run.label!) }}
                          >
                            {run.label}
                          </span>
                        ) : (
                          <span style={{ color: 'var(--muted)', fontSize: 12 }}>—</span>
                        )}
                      </td>
                    )}
                    {!hiddenCols.has('trigger') && (
                      <td>
                        {(() => {
                          const t = run.trigger_type ?? 'manual'
                          const icon = t === 'webhook' ? '⇅' : t === 'schedule' ? '⏱' : t === 'retry' ? '↺' : '▶'
                          return (
                            <span
                              style={{ fontSize: 11, color: 'var(--muted)', cursor: 'pointer' }}
                              onClick={(e) => { e.stopPropagation(); setTriggerFilter(triggerFilter === t ? 'all' : t) }}
                              title={`Filter by ${t}`}
                            >
                              {icon} {t}
                            </span>
                          )
                        })()}
                      </td>
                    )}
                    <td>
                      <span className={`badge badge-${run.status}`}>{run.status}</span>
                      {run.dry_run && (
                        <span style={{ marginLeft: 4, fontSize: 10, padding: '1px 4px', background: 'var(--link)', color: '#fff', borderRadius: 3, fontWeight: 600 }}>
                          DRY
                        </span>
                      )}
                      {LIVE_STATUSES.has(run.status) && (run.node_count ?? 0) > 0 && (() => {
                        const pct = Math.round(((run.completed_node_count ?? 0) / run.node_count!) * 100)
                        return (
                          <div style={{ marginTop: 3, height: 3, borderRadius: 2, background: 'var(--border)', overflow: 'hidden', width: 60 }} title={`${run.completed_node_count}/${run.node_count} nodes`}>
                            <div style={{ height: '100%', width: `${pct}%`, background: 'var(--node-http, #0ea5e9)', transition: 'width 0.4s ease' }} />
                          </div>
                        )
                      })()}
                    </td>
                    {!hiddenCols.has('duration') && (
                      <td style={{ color: 'var(--muted)', fontSize: 12, whiteSpace: 'nowrap' }}>
                        {run.finished_at
                          ? (() => {
                              const secs = run.finished_at - run.started_at
                              return secs < 60 ? `${secs}s` : `${Math.floor(secs / 60)}m ${secs % 60}s`
                            })()
                          : LIVE_STATUSES.has(run.status) ? <span style={{ color: 'var(--node-http)' }}>…</span> : '—'}
                      </td>
                    )}
                    {!hiddenCols.has('age') && (
                      <td style={{ color: 'var(--muted)', fontSize: 12 }}>
                        {formatAge(run.started_at, zh)}
                      </td>
                    )}
                    {!hiddenCols.has('id') && (
                      <td style={{ color: 'var(--muted)', fontSize: 11, fontFamily: 'monospace' }}>
                        {run.id.slice(-8)}
                      </td>
                    )}
                    <td onClick={(e) => e.stopPropagation()}>
                      <div style={{ display: 'flex', gap: 4, alignItems: 'center', flexWrap: 'wrap' }}>
                        <button
                          className="btn btn-sm btn-icon"
                          title={run.starred ? (zh ? '取消收藏' : 'Unstar') : (zh ? '收藏' : 'Star')}
                          style={{ fontSize: 14, color: run.starred ? '#f59e0b' : 'var(--muted)', lineHeight: 1 }}
                          onClick={async (e) => {
                            e.stopPropagation()
                            const fn = run.starred ? api.unstarExecution : api.starExecution
                            await fn(auth!.tenantId, run.id).catch(() => null)
                            setRuns((prev) => prev.map((r) => r.id === run.id ? { ...r, starred: !r.starred } : r))
                          }}
                        >
                          {run.starred ? '⭐' : '☆'}
                        </button>
                        {isPending && (
                          <>
                            <button
                              className="btn btn-sm btn-primary"
                              disabled={isActing}
                              onClick={(e) => handleApprove(e, run.id)}
                              title={zh ? '批准此执行' : 'Approve this execution'}
                            >
                              {acting[run.id] === 'approving' ? '…' : (zh ? '✓ 批准' : '✓ Approve')}
                            </button>
                            <button
                              className="btn btn-sm btn-danger"
                              disabled={isActing}
                              onClick={(e) => handleReject(e, run.id)}
                              title={zh ? '拒绝此执行' : 'Reject this execution'}
                            >
                              {acting[run.id] === 'rejecting' ? '…' : (zh ? '✕ 拒绝' : '✕ Reject')}
                            </button>
                          </>
                        )}
                      </div>
                    </td>
                  </tr>
                )
              })}
            </tbody>
          </table>
        )}

        {!loading && runs.length < total && statusFilter === 'all' && !labelFilter && (
          <div style={{ textAlign: 'center', padding: '16px 0' }}>
            <button className="btn btn-sm" onClick={loadMore} disabled={loadingMore}>
              {loadingMore ? (zh ? '加载中…' : 'Loading…') : (zh ? `加载更多（剩余 ${total - runs.length} 条）` : `Load More (${total - runs.length} remaining)`)}
            </button>
          </div>
        )}
      </main>

      {compareIds && (
        <div className="modal-backdrop" onClick={() => { setCompareIds(null); setCompareData(null) }}>
          <div className="modal" style={{ maxWidth: 960, width: '90vw' }} onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h3>{zh ? '执行对比' : 'Execution Comparison'}</h3>
              <button className="btn btn-sm" onClick={() => { setCompareIds(null); setCompareData(null) }}>✕</button>
            </div>
            {compareLoading && <p style={{ padding: 24 }}>{zh ? '加载执行详情…' : 'Loading execution details…'}</p>}
            {compareData && (
              <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 0, overflow: 'auto', maxHeight: '70vh' }}>
                {compareData.map((rec, i) => (
                  <div key={rec.id} style={{ padding: 16, borderRight: i === 0 ? '1px solid var(--border)' : undefined }}>
                    <div style={{ marginBottom: 12 }}>
                      <span className={`status-badge status-${rec.status}`}>{rec.status}</span>
                      <span style={{ fontSize: 12, color: 'var(--muted)', marginLeft: 8 }}>
                        {rec.id.slice(-16)}
                      </span>
                    </div>
                    <div style={{ fontSize: 12, marginBottom: 12, display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 8 }}>
                      <div>
                        <span style={{ color: 'var(--muted)' }}>{zh ? '开始：' : 'Started: '}</span>
                        {new Date(rec.started_at * 1000).toLocaleTimeString()}
                      </div>
                      <div>
                        <span style={{ color: 'var(--muted)' }}>{zh ? '耗时：' : 'Duration: '}</span>
                        {rec.finished_at ? `${rec.finished_at - rec.started_at}s` : '—'}
                      </div>
                      <div>
                        <span style={{ color: 'var(--muted)' }}>{zh ? '触发：' : 'Trigger: '}</span>
                        {rec.trigger_type ?? 'manual'}
                      </div>
                      <div>
                        <span style={{ color: 'var(--muted)' }}>{zh ? '节点：' : 'Nodes: '}</span>
                        {rec.node_results.length}
                      </div>
                    </div>
                    <div style={{ fontSize: 12, fontWeight: 600, marginBottom: 6, color: 'var(--muted)' }}>{zh ? '节点结果' : 'NODE RESULTS'}</div>
                    {rec.node_results.map((nr: NodeExecutionRecord) => (
                      <div key={nr.node_id} style={{
                        marginBottom: 6,
                        padding: '6px 8px',
                        background: 'var(--panel)',
                        border: '1px solid var(--border)',
                        borderRadius: 4,
                        borderLeft: `3px solid ${nr.status === 'succeeded' ? 'var(--success)' : nr.status === 'failed' ? 'var(--danger)' : 'var(--border)'}`,
                      }}>
                        <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 2 }}>
                          <span style={{ fontFamily: 'monospace', fontSize: 12 }}>{nr.node_id}</span>
                          <span style={{ fontSize: 11, color: nr.status === 'failed' ? 'var(--danger-text)' : 'var(--muted)' }}>{nr.status}</span>
                        </div>
                        {nr.duration_ms != null && (
                          <span style={{ fontSize: 11, color: 'var(--muted)' }}>{nr.duration_ms < 1000 ? `${nr.duration_ms}ms` : `${(nr.duration_ms / 1000).toFixed(1)}s`}</span>
                        )}
                        {nr.error && <div style={{ fontSize: 11, color: 'var(--danger-text)', marginTop: 2 }}>{nr.error}</div>}
                      </div>
                    ))}
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  )
}
