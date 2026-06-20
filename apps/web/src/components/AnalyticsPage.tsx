// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useEffect, useRef, useState } from 'react'
import { ThemeToggleIcon } from './uiIcons'
import { useAuth } from '../AuthContext'
import { useLocale } from '../useLocale'
import * as api from '../api/client'
import type { ExecutionSummary, WorkflowRecord } from '../types'
import { useTheme } from '../useTheme'

interface Props {
  onBack: () => void
}

function dayKey(ts: number): string {
  const d = new Date(ts * 1000)
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
}

function formatDate(key: string): string {
  const [, m, d] = key.split('-')
  return `${m}/${d}`
}

interface DayBucket { date: string; total: number; succeeded: number; failed: number }

// Approximate pricing per 1M tokens (input/output). Source: public pricing pages, may be out of date.
const TOKEN_PRICES: Record<string, { input: number; output: number }> = {
  'gpt-4o':           { input: 2.50, output: 10.00 },
  'gpt-4o-mini':      { input: 0.15, output: 0.60 },
  'o1':               { input: 15.00, output: 60.00 },
  'o1-mini':          { input: 3.00, output: 12.00 },
  'gemini-2.0-flash': { input: 0.075, output: 0.30 },
  'gemini-1.5-pro':   { input: 1.25, output: 5.00 },
  'gemini-1.5-flash': { input: 0.075, output: 0.30 },
  'claude-opus-4-7':          { input: 15.00, output: 75.00 },
  'claude-opus-4-8':          { input: 15.00, output: 75.00 },
  'claude-sonnet-4-6':        { input: 3.00,  output: 15.00 },
  'claude-haiku-4-5-20251001':{ input: 0.80,  output: 4.00 },
}

function estimateTokenCost(model: string, prompt: number, completion: number): number | null {
  const prices = TOKEN_PRICES[model]
  if (!prices) return null
  return (prompt / 1_000_000) * prices.input + (completion / 1_000_000) * prices.output
}

export function AnalyticsPage({ onBack }: Props) {
  const { auth } = useAuth()
  const { theme, toggle: toggleTheme } = useTheme()
  const { t, locale } = useLocale()
  const zh = locale === 'zh'
  const [allExecutions, setAllExecutions] = useState<ExecutionSummary[]>([])
  const [workflows, setWorkflows]   = useState<WorkflowRecord[]>([])
  const [loading, setLoading]       = useState(true)
  const [tokenUsage, setTokenUsage] = useState<api.TokenUsageSummary | null>(null)
  const [acquisition, setAcquisition] = useState<api.AcquisitionChannel[]>([])
  const [nodeTypeStats, setNodeTypeStats] = useState<api.NodeTypeStat[]>([])
  const [timeRange, setTimeRange] = useState<7 | 14 | 30 | 90>(14)
  const [showDeps, setShowDeps] = useState(false)
  const [deps, setDeps] = useState<api.WorkflowDepsResponse | null>(null)
  const [depsLoading, setDepsLoading] = useState(false)
  const [wfStatsAnalytics, setWfStatsAnalytics] = useState<api.WorkflowStatsAnalyticsResponse | null>(null)
  const [slaBreaches, setSlaBreaches] = useState<api.SlaBreachesResponse | null>(null)
  const [errorAnalysis, setErrorAnalysis] = useState<api.ErrorAnalysisResponse | null>(null)
  const [showExport, setShowExport] = useState(false)
  const exportRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!showExport) return
    const handler = (e: MouseEvent) => {
      if (exportRef.current && !exportRef.current.contains(e.target as Node)) setShowExport(false)
    }
    document.addEventListener('mousedown', handler)
    return () => document.removeEventListener('mousedown', handler)
  }, [showExport])

  useEffect(() => {
    Promise.all([
      api.listExecutions(auth!.tenantId),
      api.listWorkflows(auth!.tenantId, auth!.projectId),
      api.getTokenUsage(auth!.tenantId, 30),
      api.getNodeTypeAnalytics(auth!.tenantId),
    ])
      .then(([execs, wfs, usage, ntStats]) => {
        setAllExecutions(execs); setWorkflows(wfs); setTokenUsage(usage); setNodeTypeStats(ntStats)
      })
      .catch(() => { /* forbidden / no data — leave empty rather than reject unhandled */ })
      .finally(() => setLoading(false))
    api.getWorkflowStatsAnalytics(auth!.tenantId, 30).then(setWfStatsAnalytics).catch(() => {})
    api.getSlaBreaches(auth!.tenantId, 30).then(setSlaBreaches).catch(() => {})
    api.getErrorAnalysis(auth!.tenantId, 30).then(setErrorAnalysis).catch(() => {})
    // Admin-only; non-admins get 403 → leave empty (card hidden).
    api.getAcquisitionChannels().then(setAcquisition).catch(() => {})
  }, [])

  // ── Filter by selected time range ───────────────────────────────────────────
  const cutoff = Math.floor(Date.now() / 1000) - timeRange * 86400
  const executions = allExecutions.filter((e) => e.started_at >= cutoff)

  // ── Aggregate stats ──────────────────────────────────────────────────────────
  const total      = executions.length
  const succeeded  = executions.filter((e) => e.status === 'succeeded').length
  const failed     = executions.filter((e) => e.status === 'failed').length
  const running    = executions.filter((e) => e.status === 'running' || e.status === 'waiting_approval').length
  const successRate = total > 0 ? Math.round((succeeded / total) * 100) : 0

  // ── Daily buckets (last N days by timeRange) ────────────────────────────────
  const today = dayKey(Date.now() / 1000)
  const lastN: string[] = []
  for (let i = timeRange - 1; i >= 0; i--) {
    const d = new Date(); d.setDate(d.getDate() - i)
    const ts = d.getTime() / 1000
    lastN.push(dayKey(ts))
  }
  const bucketMap = new Map<string, DayBucket>()
  lastN.forEach((date) => bucketMap.set(date, { date, total: 0, succeeded: 0, failed: 0 }))
  for (const ex of executions) {
    const key = dayKey(ex.started_at)
    const b = bucketMap.get(key)
    if (b) {
      b.total++
      if (ex.status === 'succeeded') b.succeeded++
      else if (ex.status === 'failed') b.failed++
    }
  }
  const dailyBuckets = lastN.map((d) => bucketMap.get(d)!)
  const maxDaily = Math.max(...dailyBuckets.map((b) => b.total), 1)

  // Success-rate trend as SVG polyline points
  const ratePoints = dailyBuckets
    .map((b, i) => ({ x: i, rate: b.total > 0 ? b.succeeded / b.total : null }))
    .filter((p): p is { x: number; rate: number } => p.rate !== null)
  const trendPoints = ratePoints
    .map((p) => `${(p.x / (timeRange - 1)) * 200},${(1 - p.rate) * 40}`)
    .join(' ')

  // ── Top workflows by run count ───────────────────────────────────────────────
  const wfCountMap = new Map<string, { id: string; name: string; total: number; succeeded: number; failed: number }>()
  const wfNameMap = new Map(workflows.map((w) => [w.id, w.name]))
  for (const ex of executions) {
    const cur = wfCountMap.get(ex.workflow_id) ?? { id: ex.workflow_id, name: wfNameMap.get(ex.workflow_id) ?? ex.workflow_id.slice(0, 8), total: 0, succeeded: 0, failed: 0 }
    cur.total++
    if (ex.status === 'succeeded') cur.succeeded++
    else if (ex.status === 'failed') cur.failed++
    wfCountMap.set(ex.workflow_id, cur)
  }
  const topWorkflows = [...wfCountMap.values()].sort((a, b) => b.total - a.total).slice(0, 6)

  // ── Per-workflow average duration ───────────────────────────────────────────
  const wfDurationMap = new Map<string, { total: number; sumSecs: number; name: string }>()
  for (const ex of executions) {
    if (!ex.finished_at || ex.started_at >= ex.finished_at) continue
    const secs = ex.finished_at - ex.started_at
    const cur = wfDurationMap.get(ex.workflow_id) ?? { total: 0, sumSecs: 0, name: wfNameMap.get(ex.workflow_id) ?? ex.workflow_id.slice(0, 8) }
    cur.total++
    cur.sumSecs += secs
    wfDurationMap.set(ex.workflow_id, cur)
  }
  const wfDurations = [...wfDurationMap.entries()]
    .map(([id, { total, sumSecs, name }]) => ({ id, name, avg: sumSecs / total, total }))
    .filter((w) => w.total >= 2)
    .sort((a, b) => b.avg - a.avg)
    .slice(0, 8)
  const maxAvgDuration = Math.max(...wfDurations.map((w) => w.avg), 1)

  // ── 12-week contribution calendar heatmap ───────────────────────────────────
  const calWeeks = 12
  const calCells: Array<{ date: string; count: number; succeeded: number; failed: number }> = []
  const nowMs = Date.now()
  const calStart = new Date(nowMs - calWeeks * 7 * 86400 * 1000)
  calStart.setHours(0, 0, 0, 0)
  // align to Sunday
  calStart.setDate(calStart.getDate() - calStart.getDay())
  const calDayCount = Math.ceil((nowMs - calStart.getTime()) / (86400 * 1000)) + 1
  for (let i = 0; i < calDayCount; i++) {
    const d = new Date(calStart.getTime() + i * 86400 * 1000)
    calCells.push({ date: dayKey(d.getTime() / 1000), count: 0, succeeded: 0, failed: 0 })
  }
  for (const ex of allExecutions) {
    const k = dayKey(ex.started_at)
    const cell = calCells.find((c) => c.date === k)
    if (cell) {
      cell.count++
      if (ex.status === 'succeeded') cell.succeeded++
      if (ex.status === 'failed') cell.failed++
    }
  }
  const calMax = Math.max(1, ...calCells.map((c) => c.count))
  // Group into columns (weeks), each col = 7 days (Sun→Sat)
  const calCols: typeof calCells[] = []
  for (let i = 0; i < calCells.length; i += 7) calCols.push(calCells.slice(i, i + 7))

  // ── Hourly activity heatmap (day-of-week × hour) ────────────────────────────
  // grid[dow][hour] = count (dow: 0=Sun…6=Sat)
  const heatmap: number[][] = Array.from({ length: 7 }, () => Array(24).fill(0))
  for (const ex of executions) {
    const d = new Date(ex.started_at * 1000)
    heatmap[d.getDay()][d.getHours()]++
  }
  const heatmapMax = Math.max(1, ...heatmap.flatMap((r) => r))
  const DOW_LABELS = zh
    ? ['日', '一', '二', '三', '四', '五', '六']
    : ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat']

  // ── Recent failures ──────────────────────────────────────────────────────────
  const recentFailed = executions.filter((e) => e.status === 'failed').slice(0, 8)

  // ── Trigger type breakdown ───────────────────────────────────────────────────
  const triggerCounts: Record<string, number> = {}
  for (const ex of executions) {
    const t = ex.trigger_type ?? 'manual'
    triggerCounts[t] = (triggerCounts[t] ?? 0) + 1
  }
  const TRIGGER_COLORS: Record<string, string> = {
    manual:   'var(--link)',
    webhook:  '#7c3aed',
    schedule: '#d97706',
    retry:    '#dc2626',
  }

  const completedWithDuration = executions.filter((e) => e.finished_at && e.started_at)
  const avgDurationSecs = completedWithDuration.length > 0
    ? Math.round(completedWithDuration.reduce((sum, e) => sum + (e.finished_at! - e.started_at), 0) / completedWithDuration.length)
    : null
  const avgDurationLabel = avgDurationSecs === null ? '—'
    : avgDurationSecs < 60 ? `${avgDurationSecs}s`
    : `${Math.floor(avgDurationSecs / 60)}m ${avgDurationSecs % 60}s`

  // ── Latency percentiles (p50 / p95 / p99) ────────────────────────────────────
  const latencyPercentiles = (() => {
    if (completedWithDuration.length < 5) return null
    const sorted = completedWithDuration.map((e) => e.finished_at! - e.started_at).sort((a, b) => a - b)
    const pct = (p: number) => sorted[Math.min(Math.floor(sorted.length * p), sorted.length - 1)]
    return { p50: pct(0.5), p75: pct(0.75), p95: pct(0.95), p99: pct(0.99), min: sorted[0], max: sorted[sorted.length - 1] }
  })()
  const fmtSecs = (s: number) => s < 60 ? `${s}s` : `${Math.floor(s / 60)}m ${s % 60}s`
  const p95DurationSecs = latencyPercentiles?.p95 ?? null
  const p95Label = p95DurationSecs === null ? null : fmtSecs(p95DurationSecs)

  // ── Week-over-week change ─────────────────────────────────────────────────────
  const periodSecs = timeRange * 86400
  const prevCutoff = Math.floor(Date.now() / 1000) - periodSecs * 2
  const prevExecs = allExecutions.filter((e) => e.started_at >= prevCutoff && e.started_at < cutoff)
  const wow = total > 0 && prevExecs.length > 0
    ? Math.round(((total - prevExecs.length) / prevExecs.length) * 100)
    : null
  const wowLabel = wow === null ? null : (wow >= 0 ? `+${wow}%` : `${wow}%`)
  const wowColor = wow === null ? 'var(--muted)' : wow > 0 ? 'var(--success-text)' : 'var(--danger-text)'

  // ── Slowest executions ────────────────────────────────────────────────────────
  const slowestExecs = completedWithDuration
    .map((e) => ({ ...e, dur: e.finished_at! - e.started_at }))
    .sort((a, b) => b.dur - a.dur)
    .slice(0, 8)

  // ── Execution duration histogram ────────────────────────────────────────────
  // Buckets: <1s, 1-5s, 5-30s, 30s-2m, 2-10m, >10m
  const DURATION_BUCKETS = ['<1s', '1–5s', '5–30s', '30s–2m', '2–10m', '>10m']
  const durationBuckets = [0, 0, 0, 0, 0, 0]
  for (const ex of completedWithDuration) {
    const secs = ex.finished_at! - ex.started_at
    if (secs < 1)   durationBuckets[0]++
    else if (secs < 5)   durationBuckets[1]++
    else if (secs < 30)  durationBuckets[2]++
    else if (secs < 120) durationBuckets[3]++
    else if (secs < 600) durationBuckets[4]++
    else durationBuckets[5]++
  }
  const maxBucket = Math.max(1, ...durationBuckets)

  function formatAge(ts: number) {
    const diff = Math.floor(Date.now() / 1000 - ts)
    if (zh) {
      if (diff < 60) return `${diff}秒前`
      if (diff < 3600) return `${Math.floor(diff / 60)}分钟前`
      if (diff < 86400) return `${Math.floor(diff / 3600)}小时前`
      return `${Math.floor(diff / 86400)}天前`
    }
    if (diff < 60) return `${diff}s ago`
    if (diff < 3600) return `${Math.floor(diff / 60)}m ago`
    if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`
    return `${Math.floor(diff / 86400)}d ago`
  }

  const rateColor = successRate >= 90 ? 'var(--success-text)' : successRate >= 70 ? 'var(--warning-text)' : 'var(--danger-text)'

  function handleExportCSV() {
    const headers = ['id', 'workflow_id', 'workflow_name', 'status', 'trigger_type', 'started_at', 'finished_at', 'duration_secs']
    const rows = executions.map((e) => [
      e.id,
      e.workflow_id,
      wfNameMap.get(e.workflow_id) ?? '',
      e.status,
      e.trigger_type ?? 'manual',
      new Date(e.started_at * 1000).toISOString(),
      e.finished_at ? new Date(e.finished_at * 1000).toISOString() : '',
      e.finished_at ? String(e.finished_at - e.started_at) : '',
    ])
    const csv = [headers, ...rows].map((r) => r.map((v) => `"${String(v).replace(/"/g, '""')}"`).join(',')).join('\n')
    const url = URL.createObjectURL(new Blob([csv], { type: 'text/csv' }))
    const a = document.createElement('a')
    a.href = url; a.download = `executions-${timeRange}d-${new Date().toISOString().slice(0, 10)}.csv`; a.click()
    URL.revokeObjectURL(url); setShowExport(false)
  }

  function handleExportJSON() {
    const data = executions.map((e) => ({
      ...e,
      workflow_name: wfNameMap.get(e.workflow_id) ?? null,
      duration_secs: e.finished_at ? e.finished_at - e.started_at : null,
    }))
    const url = URL.createObjectURL(new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' }))
    const a = document.createElement('a')
    a.href = url; a.download = `executions-${timeRange}d-${new Date().toISOString().slice(0, 10)}.json`; a.click()
    URL.revokeObjectURL(url); setShowExport(false)
  }

  return (
    <div className="app">
      <header className="topbar">
        <button className="btn btn-sm btn-icon" onClick={onBack}>←</button>
        <span className="topbar-sep">|</span>
        <span className="topbar-title">{t('analytics.title')}</span>
        <div style={{ flex: 1 }} />
        <div ref={exportRef} style={{ position: 'relative' }}>
          <button className="btn btn-sm" onClick={() => setShowExport((v) => !v)} style={{ fontSize: 11 }}>
            ⬇ {zh ? '导出' : 'Export'}
          </button>
          {showExport && (
            <div style={{ position: 'absolute', right: 0, top: '100%', marginTop: 4, background: 'var(--surface)', border: '1px solid var(--border)', borderRadius: 'var(--radius)', boxShadow: '0 4px 16px rgba(0,0,0,.25)', zIndex: 200, minWidth: 150 }}>
              <button onClick={handleExportCSV} style={{ display: 'block', width: '100%', padding: '9px 14px', textAlign: 'left', background: 'none', border: 'none', cursor: 'pointer', color: 'var(--text)', fontSize: 13 }}>
                {zh ? '导出 CSV' : 'Export CSV'}
              </button>
              <button onClick={handleExportJSON} style={{ display: 'block', width: '100%', padding: '9px 14px', textAlign: 'left', background: 'none', border: 'none', cursor: 'pointer', color: 'var(--text)', fontSize: 13 }}>
                {'{ }'} {zh ? '导出 JSON' : 'Export JSON'}
              </button>
            </div>
          )}
        </div>
        <button
          className="btn btn-sm"
          onClick={() => {
            setShowDeps(true)
            if (!deps) {
              setDepsLoading(true)
              api.getWorkflowDeps(auth!.tenantId)
                .then(setDeps)
                .catch(() => {})
                .finally(() => setDepsLoading(false))
            }
          }}
          title={zh ? '显示工作流依赖图' : 'Show workflow dependency graph'}
          style={{ fontSize: 11 }}
        >
          ⇆ {zh ? '依赖图' : 'Deps'}
        </button>
        <button className="btn btn-sm" onClick={toggleTheme} title="Toggle dark/light theme">{theme === 'dark' ? <ThemeToggleIcon dark /> : <ThemeToggleIcon dark={false} />}</button>
      </header>

      <main className="list-page" style={{ maxWidth: 960, margin: '0 auto', width: '100%' }}>
        {/* ── Time range selector ───────────────────────────────────────── */}
        <div style={{ display: 'flex', gap: 6, marginBottom: 20, alignItems: 'center' }}>
          <span style={{ fontSize: 12, color: 'var(--muted)', marginRight: 4 }}>{zh ? '时间范围：' : 'Range:'}</span>
          {([7, 14, 30, 90] as const).map((n) => (
            <button
              key={n}
              onClick={() => setTimeRange(n)}
              className={`btn btn-sm${timeRange === n ? ' btn-primary' : ''}`}
              style={{ fontSize: 12, padding: '2px 10px' }}
            >
              {n}D
            </button>
          ))}
        </div>

        {loading ? (
          <p>Loading…</p>
        ) : (
          <>
            {/* ── Summary cards ─────────────────────────────────────────── */}
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(140px, 1fr))', gap: 12, marginBottom: 28 }}>
              {[
                { label: zh ? '总运行次数' : 'Total Runs', value: String(total), sub: wowLabel ? `${wowLabel} vs prev` : undefined, subColor: wowColor, color: 'var(--text)' },
                { label: zh ? '成功率' : 'Success Rate', value: `${successRate}%`, color: rateColor },
                { label: zh ? '失败' : 'Failed', value: String(failed), color: failed > 0 ? 'var(--danger-text)' : 'var(--muted)' },
                { label: zh ? '进行中' : 'In Progress', value: String(running), color: running > 0 ? 'var(--link)' : 'var(--muted)' },
                { label: zh ? '平均耗时' : 'Avg Duration', value: avgDurationLabel, color: 'var(--text)' },
                ...(p95Label ? [{ label: zh ? 'P95 耗时' : 'P95 Duration', value: p95Label, color: 'var(--text)', sub: zh ? '第95百分位' : '95th pct' }] : []),
              ].map(({ label, value, color, sub, subColor }) => (
                <div key={label} style={{ background: 'var(--surface)', border: '1px solid var(--border)', borderRadius: 8, padding: '16px 20px' }}>
                  <div style={{ fontSize: 12, color: 'var(--muted)', marginBottom: 6 }}>{label}</div>
                  <div style={{ fontSize: 26, fontWeight: 700, color }}>{value}</div>
                  {sub && <div style={{ fontSize: 11, color: subColor ?? 'var(--muted)', marginTop: 2 }}>{sub}</div>}
                </div>
              ))}
            </div>

            {/* ── Status breakdown bar ──────────────────────────────────── */}
            {total > 0 && (
              <div style={{ marginBottom: 28 }}>
                <div style={{ fontSize: 12, color: 'var(--muted)', marginBottom: 8, fontWeight: 600 }}>{t('analytics.status')}</div>
                <div style={{ display: 'flex', height: 12, borderRadius: 6, overflow: 'hidden', background: 'var(--panel)' }}>
                  {succeeded > 0 && <div style={{ width: `${(succeeded/total)*100}%`, background: 'var(--success-text)' }} title={`${succeeded} succeeded`} />}
                  {failed > 0 && <div style={{ width: `${(failed/total)*100}%`, background: 'var(--danger-text)' }} title={`${failed} failed`} />}
                  {running > 0 && <div style={{ width: `${(running/total)*100}%`, background: 'var(--link)', animation: 'pulse 1s infinite' }} title={`${running} running`} />}
                </div>
                <div style={{ display: 'flex', gap: 16, marginTop: 8, fontSize: 12, color: 'var(--muted)' }}>
                  <span><span style={{ color: 'var(--success-text)' }}>■</span> {succeeded} {zh ? '成功' : 'succeeded'}</span>
                  <span><span style={{ color: 'var(--danger-text)' }}>■</span> {failed} {zh ? '失败' : 'failed'}</span>
                  {running > 0 && <span><span style={{ color: 'var(--link)' }}>■</span> {running} {zh ? '运行中' : 'running'}</span>}
                </div>
              </div>
            )}

            {/* ── Trigger type breakdown ────────────────────────────────── */}
            {total > 0 && Object.keys(triggerCounts).length > 1 && (
              <div style={{ marginBottom: 28 }}>
                <div style={{ fontSize: 12, color: 'var(--muted)', marginBottom: 8, fontWeight: 600 }}>{t('analytics.triggers')}</div>
                <div style={{ display: 'flex', height: 12, borderRadius: 6, overflow: 'hidden', background: 'var(--panel)' }}>
                  {Object.entries(triggerCounts).map(([t, n]) => (
                    <div
                      key={t}
                      style={{ width: `${(n / total) * 100}%`, background: TRIGGER_COLORS[t] ?? 'var(--muted)' }}
                      title={`${t}: ${n}`}
                    />
                  ))}
                </div>
                <div style={{ display: 'flex', gap: 16, marginTop: 8, fontSize: 12, color: 'var(--muted)' }}>
                  {Object.entries(triggerCounts).map(([t, n]) => (
                    <span key={t}>
                      <span style={{ color: TRIGGER_COLORS[t] ?? 'var(--muted)' }}>■</span>{' '}
                      {n} {t}
                    </span>
                  ))}
                </div>
              </div>
            )}

            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 20, marginBottom: 28 }}>
              {/* ── Daily chart ────────────────────────────────────────── */}
              <div style={{ background: 'var(--surface)', border: '1px solid var(--border)', borderRadius: 8, padding: '16px 20px' }}>
                <div style={{ fontSize: 12, color: 'var(--muted)', marginBottom: 12, fontWeight: 600 }}>{zh ? `每日运行 — 最近 ${timeRange} 天` : `DAILY RUNS — LAST ${timeRange} DAYS`}</div>
                <div style={{ display: 'flex', alignItems: 'flex-end', gap: 4, height: 80 }}>
                  {dailyBuckets.map((b) => (
                    <div
                      key={b.date}
                      style={{ flex: 1, display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 2, height: '100%', justifyContent: 'flex-end' }}
                      title={`${b.date}: ${b.total} runs`}
                    >
                      <div style={{ width: '100%', display: 'flex', flexDirection: 'column', justifyContent: 'flex-end' }}>
                        {b.total > 0 && (
                          <>
                            {b.failed > 0 && <div style={{ height: `${(b.failed / maxDaily) * 60}px`, background: 'var(--danger-text)', borderRadius: '2px 2px 0 0' }} />}
                            {b.succeeded > 0 && <div style={{ height: `${(b.succeeded / maxDaily) * 60}px`, background: 'var(--success-text)', borderRadius: b.failed > 0 ? 0 : '2px 2px 0 0' }} />}
                          </>
                        )}
                        {b.total === 0 && <div style={{ height: 2, background: 'var(--border)', borderRadius: 1 }} />}
                      </div>
                    </div>
                  ))}
                </div>
                <div style={{ display: 'flex', justifyContent: 'space-between', marginTop: 4, fontSize: 10, color: 'var(--muted)' }}>
                  <span>{formatDate(lastN[0])}</span>
                  <span>{formatDate(today)}</span>
                </div>
                {ratePoints.length >= 3 && (
                  <div style={{ marginTop: 10 }}>
                    <div style={{ fontSize: 10, color: 'var(--muted)', marginBottom: 4 }}>{t('analytics.success.trend')}</div>
                    <svg width="100%" viewBox="0 0 200 44" preserveAspectRatio="none" style={{ height: 44, display: 'block' }}>
                      <defs>
                        <linearGradient id="trendGrad" x1="0" y1="0" x2="0" y2="1">
                          <stop offset="0%" stopColor="var(--success-text)" stopOpacity="0.2" />
                          <stop offset="100%" stopColor="var(--success-text)" stopOpacity="0" />
                        </linearGradient>
                      </defs>
                      <polyline points={trendPoints} fill="none" stroke="var(--success-text)" strokeWidth="1.5" strokeLinejoin="round" strokeLinecap="round" />
                      {ratePoints.map((p) => (
                        <circle key={p.x} cx={(p.x / (timeRange - 1)) * 200} cy={(1 - p.rate) * 40} r="2" fill="var(--success-text)" />
                      ))}
                    </svg>
                  </div>
                )}
              </div>

              {/* ── Top workflows ──────────────────────────────────────── */}
              <div style={{ background: 'var(--surface)', border: '1px solid var(--border)', borderRadius: 8, padding: '16px 20px' }}>
                <div style={{ fontSize: 12, color: 'var(--muted)', marginBottom: 12, fontWeight: 600 }}>{t('analytics.top.workflows')}</div>
                {topWorkflows.length === 0 ? (
                  <p style={{ fontSize: 13 }}>{zh ? '暂无执行记录。' : 'No executions yet.'}</p>
                ) : (
                  <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
                    {topWorkflows.map((wf) => {
                      const wfRate = Math.round((wf.succeeded / wf.total) * 100)
                      return (
                        <div key={wf.id}>
                          <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 12, marginBottom: 3 }}>
                            <span style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', flex: 1, marginRight: 8 }} title={wf.name}>{wf.name}</span>
                            <span style={{ color: 'var(--muted)', flexShrink: 0 }}>{wf.total} · <span style={{ color: wfRate >= 90 ? 'var(--success-text)' : wfRate >= 70 ? 'var(--warning-text)' : 'var(--danger-text)' }}>{wfRate}%</span></span>
                          </div>
                          <div style={{ height: 4, borderRadius: 2, background: 'var(--panel)', overflow: 'hidden', display: 'flex' }}>
                            <div style={{ width: `${(wf.succeeded/wf.total)*100}%`, background: 'var(--success-text)' }} />
                            <div style={{ width: `${(wf.failed/wf.total)*100}%`, background: 'var(--danger-text)' }} />
                          </div>
                        </div>
                      )
                    })}
                  </div>
                )}
              </div>
            </div>

            {/* ── Per-workflow avg duration ─────────────────────────────── */}
            {wfDurations.length >= 2 && (
              <div style={{ background: 'var(--surface)', border: '1px solid var(--border)', borderRadius: 8, padding: '16px 20px' }}>
                <div style={{ fontSize: 12, color: 'var(--muted)', marginBottom: 12, fontWeight: 600 }}>{t('analytics.avg.duration')}</div>
                <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
                  {wfDurations.map((wf) => {
                    const label = wf.avg < 60 ? `${wf.avg.toFixed(1)}s` : `${(wf.avg / 60).toFixed(1)}m`
                    const pct = Math.max(2, (wf.avg / maxAvgDuration) * 100)
                    return (
                      <div key={wf.id} style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                        <div style={{ width: 140, flexShrink: 0, fontSize: 11, color: 'var(--muted)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }} title={wf.name}>
                          {wf.name}
                        </div>
                        <div style={{ flex: 1, height: 14, background: 'var(--panel)', borderRadius: 3, overflow: 'hidden', position: 'relative' }}>
                          <div style={{ width: `${pct}%`, height: '100%', background: 'var(--link)', opacity: 0.75, borderRadius: 3 }} />
                        </div>
                        <div style={{ width: 48, flexShrink: 0, fontSize: 11, fontFamily: 'monospace', color: 'var(--fg)', textAlign: 'right' }}>
                          {label}
                        </div>
                        <div style={{ width: 28, flexShrink: 0, fontSize: 10, color: 'var(--muted)', textAlign: 'right' }}>
                          ×{wf.total}
                        </div>
                      </div>
                    )
                  })}
                </div>
              </div>
            )}

            {/* ── 12-week contribution calendar ────────────────────────── */}
            {allExecutions.length > 0 && (
              <div style={{ background: 'var(--surface)', border: '1px solid var(--border)', borderRadius: 8, padding: '16px 20px', overflowX: 'auto' }}>
                <div style={{ fontSize: 12, color: 'var(--muted)', marginBottom: 10, fontWeight: 600 }}>
                  {zh ? '12 周执行日历' : '12-WEEK EXECUTION CALENDAR'}
                </div>
                <div style={{ display: 'flex', gap: 3, alignItems: 'flex-start' }}>
                  {/* Day-of-week labels */}
                  <div style={{ display: 'flex', flexDirection: 'column', gap: 3, paddingTop: 18 }}>
                    {['', 'M', '', 'W', '', 'F', ''].map((l, i) => (
                      <div key={i} style={{ height: 12, width: 14, fontSize: 8, color: 'var(--muted)', display: 'flex', alignItems: 'center' }}>{l}</div>
                    ))}
                  </div>
                  {/* Columns (weeks) */}
                  {calCols.map((col, wi) => (
                    <div key={wi} style={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
                      {/* Month label on first col that starts a month */}
                      <div style={{ height: 14, fontSize: 8, color: 'var(--muted)', whiteSpace: 'nowrap' }}>
                        {col[0] && new Date(col[0].date + 'T00:00:00').getDate() <= 7
                          ? new Date(col[0].date + 'T00:00:00').toLocaleString('default', { month: 'short' })
                          : ''}
                      </div>
                      {col.map((cell, di) => {
                        const intensity = cell.count / calMax
                        const bg = cell.count === 0
                          ? 'var(--border)'
                          : cell.failed > 0 && cell.failed === cell.count
                            ? `rgba(239,68,68,${0.3 + intensity * 0.7})`
                            : `rgba(34,197,94,${0.2 + intensity * 0.8})`
                        const future = new Date(cell.date + 'T00:00:00').getTime() > nowMs
                        return (
                          <div
                            key={di}
                            title={`${cell.date}: ${cell.count} run${cell.count !== 1 ? 's' : ''} (${cell.succeeded} ok, ${cell.failed} failed)`}
                            style={{
                              width: 12, height: 12, borderRadius: 2,
                              background: future ? 'transparent' : bg,
                              cursor: cell.count > 0 ? 'default' : undefined,
                            }}
                          />
                        )
                      })}
                    </div>
                  ))}
                </div>
                {/* Legend */}
                <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginTop: 8, fontSize: 10, color: 'var(--muted)' }}>
                  <span>{zh ? '少' : 'Less'}</span>
                  {[0, 0.25, 0.5, 0.75, 1].map((v) => (
                    <div key={v} style={{ width: 12, height: 12, borderRadius: 2, background: v === 0 ? 'var(--border)' : `rgba(34,197,94,${0.2 + v * 0.8})` }} />
                  ))}
                  <span>{zh ? '多' : 'More'}</span>
                  <div style={{ width: 12, height: 12, borderRadius: 2, background: 'rgba(239,68,68,0.8)', marginLeft: 8 }} />
                  <span>{zh ? '全部失败' : 'All failed'}</span>
                </div>
              </div>
            )}

            {/* ── Hourly activity heatmap ──────────────────────────────── */}
            {total >= 5 && (
              <div style={{ background: 'var(--surface)', border: '1px solid var(--border)', borderRadius: 8, padding: '16px 20px', overflowX: 'auto' }}>
                <div style={{ fontSize: 12, color: 'var(--muted)', marginBottom: 10, fontWeight: 600 }}>{zh ? '按星期 & 小时活动分布' : 'ACTIVITY BY DAY & HOUR'}</div>
                <div style={{ display: 'grid', gridTemplateColumns: '32px repeat(24, 1fr)', gap: 2 }}>
                  {/* Hour labels row */}
                  <div />
                  {Array.from({ length: 24 }, (_, h) => (
                    <div key={h} style={{ fontSize: 8, color: 'var(--muted)', textAlign: 'center' }}>
                      {h % 6 === 0 ? `${h}h` : ''}
                    </div>
                  ))}
                  {/* Data rows */}
                  {heatmap.map((row, dow) => (
                    <>
                      <div key={`label-${dow}`} style={{ fontSize: 9, color: 'var(--muted)', display: 'flex', alignItems: 'center' }}>
                        {DOW_LABELS[dow]}
                      </div>
                      {row.map((count, hour) => {
                        const intensity = count / heatmapMax
                        return (
                          <div
                            key={`${dow}-${hour}`}
                            title={`${DOW_LABELS[dow]} ${hour}:00 — ${count} run${count !== 1 ? 's' : ''}`}
                            style={{
                              height: 12, borderRadius: 2,
                              background: count === 0 ? 'var(--panel)' : `rgba(37,99,235,${0.15 + intensity * 0.85})`,
                            }}
                          />
                        )
                      })}
                    </>
                  ))}
                </div>
                <div style={{ marginTop: 6, fontSize: 10, color: 'var(--muted)' }}>
                  {zh ? '颜色越深 = 运行越多' : 'lighter = fewer runs, darker = more runs'}
                </div>
              </div>
            )}

            {/* ── Latency percentiles ───────────────────────────────────── */}
            {latencyPercentiles && (
              <div style={{ background: 'var(--surface)', border: '1px solid var(--border)', borderRadius: 8, padding: '16px 20px' }}>
                <div style={{ fontSize: 12, color: 'var(--muted)', marginBottom: 12, fontWeight: 600 }}>
                  {zh ? '耗时百分位 (已完成执行)' : 'LATENCY PERCENTILES (COMPLETED EXECUTIONS)'}
                </div>
                <div style={{ display: 'grid', gridTemplateColumns: 'repeat(6, 1fr)', gap: 8 }}>
                  {([['Min', latencyPercentiles.min], ['P50', latencyPercentiles.p50], ['P75', latencyPercentiles.p75], ['P95', latencyPercentiles.p95], ['P99', latencyPercentiles.p99], ['Max', latencyPercentiles.max]] as [string, number][]).map(([label, val]) => {
                    const ratio = latencyPercentiles.max > 0 ? val / latencyPercentiles.max : 0
                    const color = ratio < 0.4 ? 'var(--success-text)' : ratio < 0.75 ? 'var(--warning-text, #d97706)' : 'var(--danger-text)'
                    return (
                      <div key={label} style={{ textAlign: 'center', padding: '8px 4px', background: 'var(--panel)', borderRadius: 6 }}>
                        <div style={{ fontSize: 10, color: 'var(--muted)', marginBottom: 4 }}>{label}</div>
                        <div style={{ fontSize: 16, fontWeight: 700, color }}>{fmtSecs(val)}</div>
                      </div>
                    )
                  })}
                </div>
              </div>
            )}

            {/* ── Duration histogram ─────────────────────────────────────── */}
            {completedWithDuration.length >= 5 && (
              <div style={{ background: 'var(--surface)', border: '1px solid var(--border)', borderRadius: 8, padding: '16px 20px' }}>
                <div style={{ fontSize: 12, color: 'var(--muted)', marginBottom: 12, fontWeight: 600 }}>
                  {zh ? '执行耗时分布' : 'EXECUTION DURATION DISTRIBUTION'}
                  <span style={{ fontWeight: 400, marginLeft: 8 }}>{completedWithDuration.length} {zh ? '条已完成' : 'completed runs'}</span>
                </div>
                <div style={{ display: 'flex', gap: 8, alignItems: 'flex-end', height: 80 }}>
                  {durationBuckets.map((count, i) => {
                    const pct = count / maxBucket
                    return (
                      <div key={i} style={{ flex: 1, display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 4 }}>
                        <span style={{ fontSize: 10, color: 'var(--muted)' }}>{count > 0 ? count : ''}</span>
                        <div
                          style={{
                            width: '100%',
                            height: Math.max(4, pct * 64),
                            background: 'var(--link)',
                            borderRadius: '3px 3px 0 0',
                            opacity: 0.4 + pct * 0.6,
                          }}
                          title={`${DURATION_BUCKETS[i]}: ${count} runs`}
                        />
                        <span style={{ fontSize: 10, color: 'var(--muted)', textAlign: 'center' }}>{DURATION_BUCKETS[i]}</span>
                      </div>
                    )
                  })}
                </div>
              </div>
            )}

            {/* ── Recent failures ────────────────────────────────────────── */}
            {recentFailed.length > 0 && (
              <div style={{ background: 'var(--surface)', border: '1px solid var(--border)', borderRadius: 8, padding: '16px 20px' }}>
                <div style={{ fontSize: 12, color: 'var(--muted)', marginBottom: 12, fontWeight: 600 }}>{t('analytics.recent.failures')}</div>
                <table className="workflow-table" style={{ marginBottom: 0 }}>
                  <thead>
                    <tr>
                      <th>{t('analytics.col.execution')}</th>
                      <th>{t('analytics.col.workflow')}</th>
                      <th>{t('analytics.col.when')}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {recentFailed.map((ex) => (
                      <tr key={ex.id}>
                        <td style={{ fontFamily: 'monospace', fontSize: 12 }}>
                          <span className="badge badge-failed">{ex.id.slice(0, 12)}…</span>
                        </td>
                        <td style={{ color: 'var(--muted)', fontSize: 12 }}>
                          {wfNameMap.get(ex.workflow_id) ?? ex.workflow_id.slice(0, 8)}
                        </td>
                        <td style={{ color: 'var(--muted)', fontSize: 12 }}>{formatAge(ex.started_at)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}

            {/* ── Top Errors Analysis ── */}
            {errorAnalysis && errorAnalysis.top_errors.length > 0 && (
              <div style={{ background: 'var(--surface)', border: '1px solid var(--border)', borderRadius: 8, padding: '16px 20px' }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 12 }}>
                  <div style={{ fontSize: 12, color: 'var(--muted)', fontWeight: 600 }}>
                    {zh ? '常见错误（近30天）' : 'TOP ERRORS (LAST 30 DAYS)'}
                  </div>
                  <span style={{ fontSize: 11, color: 'var(--muted)', marginLeft: 'auto' }}>
                    {zh ? `${errorAnalysis.distinct_error_types} 种错误类型，${errorAnalysis.total_failed_nodes} 个节点失败` : `${errorAnalysis.distinct_error_types} distinct types, ${errorAnalysis.total_failed_nodes} failed nodes total`}
                  </span>
                </div>
                <table style={{ width: '100%', fontSize: 12, borderCollapse: 'collapse' }}>
                  <thead>
                    <tr style={{ borderBottom: '1px solid var(--border)' }}>
                      <th style={{ textAlign: 'left', padding: '4px 8px', color: 'var(--text-secondary)', fontWeight: 500 }}>{zh ? '错误信息' : 'Error Message'}</th>
                      <th style={{ textAlign: 'left', padding: '4px 8px', color: 'var(--text-secondary)', fontWeight: 500 }}>{zh ? '节点类型' : 'Node Type'}</th>
                      <th style={{ textAlign: 'left', padding: '4px 8px', color: 'var(--text-secondary)', fontWeight: 500 }}>{zh ? '工作流' : 'Workflow'}</th>
                      <th style={{ textAlign: 'right', padding: '4px 8px', color: 'var(--text-secondary)', fontWeight: 500 }}>{zh ? '次数' : 'Count'}</th>
                      <th style={{ textAlign: 'right', padding: '4px 8px', color: 'var(--text-secondary)', fontWeight: 500 }}>{zh ? '最近出现' : 'Last Seen'}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {errorAnalysis.top_errors.map((err, i) => (
                      <tr key={i} style={{ borderBottom: '1px solid var(--border)' }}>
                        <td style={{ padding: '6px 8px', maxWidth: 280, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', color: 'var(--danger-text)', fontFamily: 'monospace', fontSize: 11 }} title={err.error_message}>
                          {err.error_message}
                        </td>
                        <td style={{ padding: '6px 8px' }}><span className="badge">{err.node_type}</span></td>
                        <td style={{ padding: '6px 8px', color: 'var(--text-secondary)' }}>{err.workflow_name || err.workflow_id.slice(0, 8)}</td>
                        <td style={{ padding: '6px 8px', textAlign: 'right', fontWeight: 700, color: err.count >= 5 ? 'var(--danger-text)' : undefined }}>{err.count}</td>
                        <td style={{ padding: '6px 8px', textAlign: 'right', color: 'var(--muted)', fontSize: 11 }}>{formatAge(err.last_seen)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}

            {/* ── Failure by Workflow ── */}
            {(() => {
              const failing = [...wfCountMap.values()]
                .filter((w) => w.failed > 0)
                .sort((a, b) => b.failed - a.failed)
                .slice(0, 8)
              if (failing.length === 0) return null
              const maxFails = Math.max(...failing.map((w) => w.failed), 1)
              return (
                <div style={{ marginBottom: 28 }}>
                  <h2 style={{ marginBottom: 12 }}>{zh ? '失败频率 · 按工作流' : 'Failure Frequency by Workflow'}</h2>
                  <table style={{ width: '100%', borderCollapse: 'collapse' }}>
                    <thead>
                      <tr style={{ fontSize: 11, color: 'var(--muted)', textAlign: 'left', borderBottom: '1px solid var(--border)' }}>
                        <th style={{ padding: '4px 8px' }}>{zh ? '工作流' : 'Workflow'}</th>
                        <th style={{ padding: '4px 8px', textAlign: 'right' }}>{zh ? '总执行' : 'Total'}</th>
                        <th style={{ padding: '4px 8px', textAlign: 'right', color: '#f85149' }}>{zh ? '失败' : 'Failed'}</th>
                        <th style={{ padding: '4px 8px', textAlign: 'right' }}>{zh ? '失败率' : 'Failure Rate'}</th>
                        <th style={{ padding: '4px 8px' }}></th>
                      </tr>
                    </thead>
                    <tbody>
                      {failing.map((w) => {
                        const rate = w.total > 0 ? Math.round((w.failed / w.total) * 100) : 0
                        const barW = Math.round((w.failed / maxFails) * 140)
                        const barColor = rate >= 50 ? '#f85149' : rate >= 20 ? '#d29922' : '#e78c4a'
                        return (
                          <tr key={w.id} style={{ borderBottom: '1px solid var(--border)', fontSize: 12 }}>
                            <td style={{ padding: '6px 8px', maxWidth: 200, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{w.name}</td>
                            <td style={{ padding: '6px 8px', textAlign: 'right', color: 'var(--muted)' }}>{w.total}</td>
                            <td style={{ padding: '6px 8px', textAlign: 'right', color: '#f85149', fontWeight: 600 }}>{w.failed}</td>
                            <td style={{ padding: '6px 8px', textAlign: 'right', color: barColor, fontWeight: 600 }}>{rate}%</td>
                            <td style={{ padding: '6px 8px' }}>
                              <div style={{ height: 8, width: barW, background: barColor, borderRadius: 4 }} />
                            </td>
                          </tr>
                        )
                      })}
                    </tbody>
                  </table>
                </div>
              )
            })()}

            {/* ── Slowest executions ── */}
            {slowestExecs.length >= 3 && (
              <div style={{ marginBottom: 28 }}>
                <h2 style={{ marginBottom: 12 }}>{zh ? '耗时最长的执行' : 'Slowest Executions'}</h2>
                <table style={{ width: '100%', borderCollapse: 'collapse' }}>
                  <thead>
                    <tr style={{ fontSize: 11, color: 'var(--muted)', textAlign: 'left', borderBottom: '1px solid var(--border)' }}>
                      <th style={{ padding: '4px 8px' }}>{zh ? '工作流' : 'Workflow'}</th>
                      <th style={{ padding: '4px 8px', textAlign: 'right' }}>{zh ? '耗时' : 'Duration'}</th>
                      <th style={{ padding: '4px 8px' }}>{zh ? '时间' : 'When'}</th>
                      <th style={{ padding: '4px 8px' }}></th>
                    </tr>
                  </thead>
                  <tbody>
                    {slowestExecs.map((ex) => {
                      const wfName = wfNameMap.get(ex.workflow_id) ?? ex.workflow_id.slice(0, 8)
                      const durLabel = ex.dur < 60 ? `${ex.dur}s` : ex.dur < 3600 ? `${Math.floor(ex.dur / 60)}m ${ex.dur % 60}s` : `${Math.floor(ex.dur / 3600)}h ${Math.floor((ex.dur % 3600) / 60)}m`
                      const vsAvg = avgDurationSecs ? Math.round(((ex.dur - avgDurationSecs) / avgDurationSecs) * 100) : null
                      return (
                        <tr key={ex.id} style={{ borderBottom: '1px solid var(--border)', fontSize: 12 }}>
                          <td style={{ padding: '5px 8px', maxWidth: 200, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{wfName}</td>
                          <td style={{ padding: '5px 8px', textAlign: 'right', fontWeight: 700, fontVariantNumeric: 'tabular-nums' }}>{durLabel}</td>
                          <td style={{ padding: '5px 8px', color: 'var(--muted)' }}>{formatAge(ex.started_at)}</td>
                          <td style={{ padding: '5px 8px', color: 'var(--muted)', fontSize: 11 }}>
                            {vsAvg !== null && vsAvg > 0 && (
                              <span style={{ color: vsAvg > 100 ? 'var(--danger-text)' : vsAvg > 50 ? '#d97706' : 'var(--muted)' }}>
                                +{vsAvg}% {zh ? '超过均值' : 'above avg'}
                              </span>
                            )}
                          </td>
                        </tr>
                      )
                    })}
                  </tbody>
                </table>
              </div>
            )}

            {/* ── Per-Workflow Performance (from backend aggregation) ── */}
            {wfStatsAnalytics && wfStatsAnalytics.rows.length > 0 && (
              <div style={{ marginBottom: 28 }}>
                <h2 style={{ marginBottom: 12 }}>
                  {zh ? '工作流性能排行（近 30 天）' : 'Workflow Performance (Last 30 Days)'}
                </h2>
                <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 12 }}>
                  <thead>
                    <tr style={{ fontSize: 11, color: 'var(--muted)', textAlign: 'left', borderBottom: '1px solid var(--border)' }}>
                      <th style={{ padding: '4px 8px' }}>{zh ? '工作流' : 'Workflow'}</th>
                      <th style={{ padding: '4px 8px', textAlign: 'right' }}>{zh ? '总执行' : 'Total'}</th>
                      <th style={{ padding: '4px 8px', textAlign: 'right', color: 'var(--success-text)' }}>{zh ? '成功' : 'OK'}</th>
                      <th style={{ padding: '4px 8px', textAlign: 'right', color: 'var(--danger-text)' }}>{zh ? '失败' : 'Fail'}</th>
                      <th style={{ padding: '4px 8px', textAlign: 'right' }}>{zh ? '成功率' : 'Rate'}</th>
                      <th style={{ padding: '4px 8px', textAlign: 'right' }}>{zh ? '均耗时' : 'Avg Dur'}</th>
                      <th style={{ padding: '4px 8px' }}>{zh ? '最后运行' : 'Last Run'}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {wfStatsAnalytics.rows.slice(0, 12).map((row) => {
                      const name = wfNameMap.get(row.workflow_id) ?? row.workflow_id.slice(0, 10)
                      const rate = row.total > 0 ? Math.round((row.succeeded / row.total) * 100) : 0
                      const rateCol = rate >= 90 ? 'var(--success-text)' : rate >= 70 ? '#d97706' : 'var(--danger-text)'
                      const avgDur = row.avg_duration_secs === null ? '—'
                        : row.avg_duration_secs < 60 ? `${row.avg_duration_secs.toFixed(1)}s`
                        : `${Math.floor(row.avg_duration_secs / 60)}m ${Math.round(row.avg_duration_secs % 60)}s`
                      return (
                        <tr key={row.workflow_id} style={{ borderBottom: '1px solid var(--border)' }}>
                          <td style={{ padding: '5px 8px', fontWeight: 600, maxWidth: 200, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{name}</td>
                          <td style={{ padding: '5px 8px', textAlign: 'right', fontVariantNumeric: 'tabular-nums' }}>{row.total}</td>
                          <td style={{ padding: '5px 8px', textAlign: 'right', color: 'var(--success-text)', fontVariantNumeric: 'tabular-nums' }}>{row.succeeded}</td>
                          <td style={{ padding: '5px 8px', textAlign: 'right', color: row.failed > 0 ? 'var(--danger-text)' : 'var(--muted)', fontVariantNumeric: 'tabular-nums' }}>{row.failed}</td>
                          <td style={{ padding: '5px 8px', textAlign: 'right', color: rateCol, fontWeight: 700 }}>{row.total > 0 ? `${rate}%` : '—'}</td>
                          <td style={{ padding: '5px 8px', textAlign: 'right', color: 'var(--muted)', fontVariantNumeric: 'tabular-nums' }}>{avgDur}</td>
                          <td style={{ padding: '5px 8px', color: 'var(--muted)' }}>
                            {row.last_run_at ? formatAge(row.last_run_at) : '—'}
                          </td>
                        </tr>
                      )
                    })}
                  </tbody>
                </table>
              </div>
            )}

            {/* ── SLA Compliance ── */}
            {completedWithDuration.length >= 3 && (() => {
              const SLA_TARGETS = [
                { label: zh ? '< 1s' : '< 1s', secs: 1 },
                { label: zh ? '< 5s' : '< 5s', secs: 5 },
                { label: zh ? '< 30s' : '< 30s', secs: 30 },
                { label: zh ? '< 2m' : '< 2m', secs: 120 },
                { label: zh ? '< 10m' : '< 10m', secs: 600 },
              ]
              const total90 = completedWithDuration.length
              return (
                <div style={{ marginBottom: 28 }}>
                  <h2 style={{ marginBottom: 12 }}>
                    {zh ? 'SLA 合规率（已完成执行）' : 'SLA Compliance (Completed Executions)'}
                  </h2>
                  <div style={{ display: 'flex', gap: 10, flexWrap: 'wrap' }}>
                    {SLA_TARGETS.map(({ label, secs }) => {
                      const within = completedWithDuration.filter((e) => (e.finished_at! - e.started_at) < secs).length
                      const pct = Math.round((within / total90) * 100)
                      const color = pct >= 95 ? '#3fb950' : pct >= 80 ? '#d29922' : '#f85149'
                      return (
                        <div key={label} style={{
                          background: 'var(--surface)', border: `1px solid ${color}44`,
                          borderRadius: 'var(--radius)', padding: '10px 16px', minWidth: 100, textAlign: 'center',
                        }}>
                          <div style={{ fontSize: 11, color: 'var(--muted)', marginBottom: 4 }}>
                            {zh ? '完成于' : 'Finish in'} {label}
                          </div>
                          <div style={{ fontSize: 22, fontWeight: 700, color }}>{pct}%</div>
                          <div style={{ fontSize: 10, color: 'var(--muted)', marginTop: 2 }}>
                            {within}/{total90}
                          </div>
                          <div style={{ marginTop: 6, height: 4, borderRadius: 2, background: 'var(--border)', overflow: 'hidden' }}>
                            <div style={{ height: '100%', width: `${pct}%`, background: color, borderRadius: 2, transition: 'width 0.4s' }} />
                          </div>
                        </div>
                      )
                    })}
                  </div>
                </div>
              )
            })()}

            {/* ── Per-workflow SLA breach report ── */}
            {slaBreaches && slaBreaches.total_workflows_with_sla > 0 && (
              <div style={{ marginBottom: 28 }}>
                <h2 style={{ marginBottom: 8 }}>
                  {zh ? '工作流 SLA 合规报告（过去 30 天）' : 'Per-Workflow SLA Compliance (Last 30 Days)'}
                </h2>
                <div style={{ display: 'flex', gap: 12, marginBottom: 16, flexWrap: 'wrap' }}>
                  {[
                    {
                      label: zh ? '合规率' : 'Compliance Rate',
                      value: `${slaBreaches.compliance_rate.toFixed(1)}%`,
                      color: slaBreaches.compliance_rate >= 95 ? '#3fb950' : slaBreaches.compliance_rate >= 80 ? '#d29922' : '#f85149',
                    },
                    {
                      label: zh ? '已完成' : 'Completed',
                      value: String(slaBreaches.total_completed),
                      color: 'var(--fg)',
                    },
                    {
                      label: zh ? 'SLA 超时次数' : 'SLA Breaches',
                      value: String(slaBreaches.breaches.length),
                      color: slaBreaches.breaches.length > 0 ? '#f85149' : '#3fb950',
                    },
                    {
                      label: zh ? '设定 SLA 的工作流' : 'Workflows w/ SLA',
                      value: String(slaBreaches.total_workflows_with_sla),
                      color: 'var(--muted)',
                    },
                  ].map(({ label, value, color }) => (
                    <div key={label} style={{ background: 'var(--surface)', border: '1px solid var(--border)', borderRadius: 'var(--radius)', padding: '10px 16px', minWidth: 130, textAlign: 'center' }}>
                      <div style={{ fontSize: 11, color: 'var(--muted)', marginBottom: 4 }}>{label}</div>
                      <div style={{ fontSize: 22, fontWeight: 700, color }}>{value}</div>
                    </div>
                  ))}
                </div>
                {slaBreaches.breaches.length > 0 && (
                  <table className="data-table" style={{ width: '100%' }}>
                    <thead>
                      <tr>
                        <th>{zh ? '工作流' : 'Workflow'}</th>
                        <th>{zh ? 'SLA 阈值' : 'SLA'}</th>
                        <th>{zh ? '实际耗时' : 'Elapsed'}</th>
                        <th>{zh ? '超时' : 'Overage'}</th>
                        <th>{zh ? '执行时间' : 'Started'}</th>
                      </tr>
                    </thead>
                    <tbody>
                      {slaBreaches.breaches.slice(0, 10).map((b) => (
                        <tr key={b.execution_id}>
                          <td style={{ fontWeight: 500 }}>{b.workflow_name}</td>
                          <td style={{ fontFamily: 'monospace', fontSize: 12 }}>{b.sla_seconds}s</td>
                          <td style={{ fontFamily: 'monospace', fontSize: 12, color: '#f85149', fontWeight: 600 }}>{b.elapsed_seconds}s</td>
                          <td style={{ fontFamily: 'monospace', fontSize: 12, color: '#f85149' }}>+{b.overage_seconds}s</td>
                          <td style={{ fontSize: 12, color: 'var(--muted)' }}>
                            {new Date(b.started_at * 1000).toLocaleString()}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                )}
                {slaBreaches.breaches.length === 0 && (
                  <div style={{ color: '#3fb950', fontSize: 13, padding: '12px 0', fontWeight: 500 }}>
                    ✓ {zh ? '所有执行均在 SLA 范围内' : 'All executions within SLA'}
                  </div>
                )}
              </div>
            )}

            {/* ── Token Usage ── */}
            {tokenUsage && tokenUsage.total_tokens > 0 && (
              <div style={{ marginBottom: 28 }}>
                <h2 style={{ marginBottom: 12 }}>{t('analytics.token.usage')}</h2>
                <div style={{ display: 'flex', gap: 12, flexWrap: 'wrap', marginBottom: 12 }}>
                  {[
                    { label: zh ? '总 Token' : 'Total Tokens', value: tokenUsage.total_tokens.toLocaleString(), color: 'var(--accent)' },
                    { label: zh ? '输入 Token' : 'Prompt Tokens', value: tokenUsage.prompt_tokens.toLocaleString(), color: 'var(--muted)' },
                    { label: zh ? '输出 Token' : 'Completion Tokens', value: tokenUsage.completion_tokens.toLocaleString(), color: 'var(--muted)' },
                  ].map(({ label, value, color }) => (
                    <div key={label} style={{ background: 'var(--surface)', border: '1px solid var(--border)', borderRadius: 'var(--radius)', padding: '12px 18px', minWidth: 140 }}>
                      <div style={{ fontSize: 11, color: 'var(--muted)', marginBottom: 4 }}>{label}</div>
                      <div style={{ fontSize: 22, fontWeight: 700, color }}>{value}</div>
                    </div>
                  ))}
                </div>
                {Object.entries(tokenUsage.by_model).length > 0 && (
                  <table className="workflow-table" style={{ maxWidth: 600 }}>
                    <thead><tr><th>{t('analytics.col.model')}</th><th>{t('analytics.col.prompt')}</th><th>{t('analytics.col.completion')}</th><th>{t('analytics.col.total')}</th><th title="Estimated cost based on public pricing (approximate)">{t('analytics.col.cost')}</th></tr></thead>
                    <tbody>
                      {Object.entries(tokenUsage.by_model)
                        .sort((a, b) => b[1].total_tokens - a[1].total_tokens)
                        .map(([model, usage]) => {
                          const cost = estimateTokenCost(model, usage.prompt_tokens, usage.completion_tokens)
                          return (
                            <tr key={model}>
                              <td style={{ fontFamily: 'monospace', fontSize: 12 }}>{model}</td>
                              <td style={{ fontSize: 12, color: 'var(--muted)' }}>{usage.prompt_tokens.toLocaleString()}</td>
                              <td style={{ fontSize: 12, color: 'var(--muted)' }}>{usage.completion_tokens.toLocaleString()}</td>
                              <td style={{ fontSize: 12, fontWeight: 600 }}>{usage.total_tokens.toLocaleString()}</td>
                              <td style={{ fontSize: 12, color: 'var(--muted)', fontFamily: 'monospace' }}>
                                {cost !== null ? `~$${cost.toFixed(4)}` : '—'}
                              </td>
                            </tr>
                          )
                        })}
                    </tbody>
                  </table>
                )}
              </div>
            )}

            {/* ── Acquisition channels ROI (admin-only) ── */}
            {acquisition.length > 0 && (() => {
              const totalSignups = acquisition.reduce((s, c) => s + c.signups, 0)
              const totalPaid = acquisition.reduce((s, c) => s + c.paid, 0)
              const fmtCur = (cents: number, currency: string) =>
                `${(cents / 100).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })} ${currency.toUpperCase()}`
              // Sum revenue per currency (never mix currencies).
              const totalsByCur: Record<string, number> = {}
              for (const c of acquisition) for (const r of c.revenue) totalsByCur[r.currency] = (totalsByCur[r.currency] ?? 0) + r.cents
              const totalRevenue = Object.entries(totalsByCur).map(([cur, cents]) => fmtCur(cents, cur)).join(' + ') || fmtCur(0, 'usd')
              const revCell = (rev: api.CurrencyRevenue[]) => (rev.length ? rev.map((r) => fmtCur(r.cents, r.currency)).join(' + ') : '—')
              const rate = (paid: number, signups: number) => (signups > 0 ? `${((paid / signups) * 100).toFixed(0)}%` : '—')
              return (
                <div style={{ marginBottom: 28 }}>
                  <h2 style={{ marginBottom: 4 }}>{zh ? '获客渠道 ROI' : 'Acquisition ROI'}</h2>
                  <div style={{ fontSize: 11, color: 'var(--muted)', marginBottom: 12 }}>
                    {zh
                      ? `按首触渠道:注册 → 付费转化 → 收入(首单+续费,分币种)(${totalSignups} 注册 / ${totalPaid} 付费 / ${totalRevenue})`
                      : `By first-touch channel: signup → paid → revenue (initial + recurring, per currency) (${totalSignups} signups / ${totalPaid} paid / ${totalRevenue})`}
                  </div>
                  <table className="workflow-table" style={{ maxWidth: 600 }}>
                    <thead>
                      <tr>
                        <th>{zh ? '渠道' : 'Channel'}</th>
                        <th>{zh ? '注册' : 'Signups'}</th>
                        <th>{zh ? '付费' : 'Paid'}</th>
                        <th>{zh ? '转化率' : 'Conv.'}</th>
                        <th>{zh ? '收入' : 'Revenue'}</th>
                      </tr>
                    </thead>
                    <tbody>
                      {acquisition.map((c) => (
                        <tr key={c.channel}>
                          <td style={{ fontFamily: 'monospace', fontSize: 12 }}>{c.channel}</td>
                          <td style={{ fontSize: 12 }}>{c.signups.toLocaleString()}</td>
                          <td style={{ fontSize: 12 }}>{c.paid.toLocaleString()}</td>
                          <td style={{ fontSize: 12, color: 'var(--muted)' }}>{rate(c.paid, c.signups)}</td>
                          <td style={{ fontSize: 12, fontWeight: 600 }}>{revCell(c.revenue)}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )
            })()}

            {nodeTypeStats.length > 0 && (
              <div style={{ marginBottom: 24 }}>
                <div style={{ fontSize: 11, fontWeight: 700, letterSpacing: 1, color: 'var(--muted)', marginBottom: 10 }}>
                  {zh ? '节点类型执行统计' : 'NODE TYPE EXECUTION STATS'}
                </div>
                <table style={{ width: '100%', borderCollapse: 'collapse' }}>
                  <thead>
                    <tr style={{ fontSize: 11, color: 'var(--muted)', textAlign: 'left', borderBottom: '1px solid var(--border)' }}>
                      <th style={{ padding: '4px 8px' }}>{t('analytics.col.node.type')}</th>
                      <th style={{ padding: '4px 8px' }}>{t('analytics.col.total')}</th>
                      <th style={{ padding: '4px 8px', color: '#3fb950' }}>{t('analytics.col.succeeded')}</th>
                      <th style={{ padding: '4px 8px', color: '#f85149' }}>{t('analytics.col.failed')}</th>
                      <th style={{ padding: '4px 8px', color: 'var(--muted)' }}>{t('analytics.col.skipped')}</th>
                      <th style={{ padding: '4px 8px' }}>{t('analytics.col.success.rate')}</th>
                      <th style={{ padding: '4px 8px', color: 'var(--muted)' }}>{zh ? '平均耗时' : 'Avg Duration'}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {nodeTypeStats.map((s) => {
                      const rate = s.total > 0 ? Math.round((s.succeeded / s.total) * 100) : 0
                      const avgDur = s.avg_duration_ms !== undefined
                        ? s.avg_duration_ms < 1000 ? `${s.avg_duration_ms}ms`
                        : s.avg_duration_ms < 60000 ? `${(s.avg_duration_ms / 1000).toFixed(1)}s`
                        : `${Math.floor(s.avg_duration_ms / 60000)}m`
                        : '—'
                      return (
                        <tr key={s.node_type} style={{ borderBottom: '1px solid var(--border)', fontSize: 12 }}>
                          <td style={{ padding: '4px 8px', fontFamily: 'monospace', fontWeight: 600 }}>{s.node_type}</td>
                          <td style={{ padding: '4px 8px' }}>{s.total}</td>
                          <td style={{ padding: '4px 8px', color: '#3fb950' }}>{s.succeeded}</td>
                          <td style={{ padding: '4px 8px', color: s.failed > 0 ? '#f85149' : 'var(--muted)' }}>{s.failed}</td>
                          <td style={{ padding: '4px 8px', color: 'var(--muted)' }}>{s.skipped}</td>
                          <td style={{ padding: '4px 8px', color: rate >= 90 ? '#3fb950' : rate >= 70 ? '#d29922' : '#f85149' }}>
                            {s.total > 0 ? `${rate}%` : '—'}
                          </td>
                          <td style={{ padding: '4px 8px', color: 'var(--muted)', fontFamily: 'monospace' }}>{avgDur}</td>
                        </tr>
                      )
                    })}
                  </tbody>
                </table>
              </div>
            )}

            {total === 0 && (
              <div className="empty-state">
                <p>{zh ? '暂无执行记录，运行工作流后查看分析。' : 'No executions yet. Run a workflow to see analytics.'}</p>
              </div>
            )}
          </>
        )}
      </main>

      {showDeps && (
        <div className="modal-backdrop" onClick={() => setShowDeps(false)}>
          <div className="modal" style={{ maxWidth: 640 }} onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h3>⇆ {zh ? '工作流依赖图' : 'Workflow Dependency Graph'}</h3>
              <button className="btn btn-sm" onClick={() => setShowDeps(false)}>✕</button>
            </div>
            <div style={{ padding: '16px 24px 20px' }}>
              {depsLoading && <p style={{ color: 'var(--muted)', textAlign: 'center', padding: '2rem' }}>{zh ? '加载中…' : 'Loading…'}</p>}
              {deps && deps.edges.length === 0 && (
                <div style={{ textAlign: 'center', color: 'var(--muted)', padding: '2rem' }}>
                  <div style={{ fontSize: '2rem', marginBottom: 8 }}>⇆</div>
                  <p>{zh ? '无跨工作流依赖。使用 SubWorkflow 或 ForEach 节点引用其他工作流时，此处将显示依赖关系。' : 'No cross-workflow dependencies found. Use SubWorkflow or ForEach nodes referencing other workflows to see dependencies here.'}</p>
                </div>
              )}
              {deps && deps.edges.length > 0 && (
                <>
                  <p style={{ fontSize: 13, color: 'var(--muted)', marginBottom: 16 }}>
                    {zh ? `共 ${deps.edges.length} 条依赖关系（通过 SubWorkflow 或 ForEach 节点）` : `${deps.edges.length} dependency relationship${deps.edges.length !== 1 ? 's' : ''} via SubWorkflow or ForEach nodes`}
                  </p>
                  <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 13 }}>
                    <thead>
                      <tr style={{ fontSize: 11, color: 'var(--muted)', borderBottom: '1px solid var(--border)' }}>
                        <th style={{ padding: '4px 8px', textAlign: 'left' }}>{zh ? '调用方工作流' : 'Caller Workflow'}</th>
                        <th style={{ padding: '4px 8px', textAlign: 'center' }}></th>
                        <th style={{ padding: '4px 8px', textAlign: 'left' }}>{zh ? '被调用工作流' : 'Called Workflow'}</th>
                        <th style={{ padding: '4px 8px', textAlign: 'left' }}>{zh ? '节点类型' : 'Node Type'}</th>
                      </tr>
                    </thead>
                    <tbody>
                      {deps.edges.map((edge, i) => {
                        const fromName = wfNameMap.get(edge.from_workflow_id) ?? edge.from_workflow_id.slice(0, 12)
                        const toName = wfNameMap.get(edge.to_workflow_id) ?? edge.to_workflow_id.slice(0, 12)
                        return (
                          <tr key={i} style={{ borderBottom: '1px solid var(--border)' }}>
                            <td style={{ padding: '6px 8px', fontWeight: 600 }}>{fromName}</td>
                            <td style={{ padding: '6px 8px', textAlign: 'center', color: 'var(--muted)' }}>→</td>
                            <td style={{ padding: '6px 8px', color: 'var(--link)' }}>{toName}</td>
                            <td style={{ padding: '6px 8px' }}>
                              <code style={{ background: 'var(--panel)', border: '1px solid var(--border)', borderRadius: 3, padding: '1px 5px', fontSize: 11 }}>{edge.node_type}</code>
                            </td>
                          </tr>
                        )
                      })}
                    </tbody>
                  </table>
                </>
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
