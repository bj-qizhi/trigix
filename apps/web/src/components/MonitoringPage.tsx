// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useEffect, useRef, useState } from 'react'
import { useAuth } from '../AuthContext'
import { useLocale } from '../useLocale'
import { useTheme } from '../useTheme'
import * as api from '../api/client'
import type { ExecutionSummary } from '../types'
import logoWordmark from '../assets/logo-wordmark.svg'

interface Props {
  onBack: () => void
  onOpenExecution: (id: string) => void
  onOpenWorkflow?: (id: string) => void
}

const LIVE = new Set(['running', 'waiting_approval'])

function useWorkflowNames(tenantId: string): Map<string, string> {
  const [names, setNames] = useState(new Map<string, string>())
  useEffect(() => {
    api.listWorkflows(tenantId, 'project-1')
      .then((wfs) => setNames(new Map(wfs.map((w) => [w.id, w.name]))))
      .catch(() => {})
  }, [tenantId])
  return names
}

function formatAge(secs: number, zh = false): string {
  const diff = Math.floor(Date.now() / 1000) - secs
  if (diff < 5) return zh ? '刚刚' : 'just now'
  if (diff < 60) return zh ? `${diff}秒前` : `${diff}s ago`
  const m = Math.floor(diff / 60)
  if (m < 60) return zh ? `${m}分钟前` : `${m}m ago`
  return zh ? `${Math.floor(m / 60)}小时前` : `${Math.floor(m / 60)}h ago`
}

function formatElapsed(secs: number, zh = false): string {
  const diff = Math.floor(Date.now() / 1000) - secs
  if (diff < 60) return zh ? `${diff}s` : `${diff}s`
  const m = Math.floor(diff / 60)
  if (m < 60) return zh ? `${m}分${diff % 60}秒` : `${m}m ${diff % 60}s`
  return zh ? `${Math.floor(m / 60)}小时+` : `${Math.floor(m / 60)}h+`
}

// Build a 24-bucket hourly count array from a list of summaries
function buildHourlyCounts(runs: ExecutionSummary[]): number[] {
  const buckets = Array(24).fill(0)
  const nowSecs = Math.floor(Date.now() / 1000)
  for (const r of runs) {
    const hoursAgo = Math.floor((nowSecs - r.started_at) / 3600)
    if (hoursAgo >= 0 && hoursAgo < 24) {
      buckets[23 - hoursAgo]++
    }
  }
  return buckets
}

export function MonitoringPage({ onBack, onOpenExecution, onOpenWorkflow }: Props) {
  const { auth } = useAuth()
  const { locale, t } = useLocale()
  const { theme, toggle: toggleTheme } = useTheme()
  const zh = locale === 'zh'
  const names = useWorkflowNames(auth!.tenantId)

  const [all, setAll] = useState<ExecutionSummary[]>([])
  const [stats, setStats] = useState<api.ExecutionStats | null>(null)
  const [queueDepth, setQueueDepth] = useState<number | null>(null)
  const [queueDepthHistory, setQueueDepthHistory] = useState<number[]>([])
  const [loading, setLoading] = useState(true)
  const [cancelling, setCancelling] = useState<string | null>(null)
  const [retrying, setRetrying] = useState<string | null>(null)
  const [approving, setApproving] = useState<string | null>(null)
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null)
  const [failThreshold, setFailThreshold] = useState<number>(() => {
    try { return Number(localStorage.getItem('af:mon:fail-threshold') ?? '20') } catch { return 20 }
  })
  const [showThresholdEdit, setShowThresholdEdit] = useState(false)
  const [thresholdInput, setThresholdInput] = useState(String(failThreshold))

  const load = async () => {
    try {
      const [page, s, qd] = await Promise.all([
        api.listExecutionsPage(auth!.tenantId, { limit: 200 }),
        api.getExecutionStats(auth!.tenantId),
        api.getQueueDepth(),
      ])
      setAll(page.data)
      setStats(s)
      setQueueDepth(qd.queue_depth)
      if (qd.queue_depth !== null) {
        setQueueDepthHistory((prev) => [...prev.slice(-29), qd.queue_depth as number])
      }
    } catch { /* ignore */ } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    load()
    pollRef.current = setInterval(load, 5000)
    return () => { if (pollRef.current) clearInterval(pollRef.current) }
  }, [])

  const nowSecs = Math.floor(Date.now() / 1000)
  const todayCutoff = nowSecs - 86400

  const liveRuns = all.filter((r) => LIVE.has(r.status))
  const awaitingApproval = all.filter((r) => r.status === 'waiting_approval')
  const recentFailed = all.filter((r) => r.status === 'failed' && r.started_at >= todayCutoff)
  const todayRuns = all.filter((r) => r.started_at >= todayCutoff)
  const todaySucceeded = todayRuns.filter((r) => r.status === 'succeeded').length
  const successRate = todayRuns.length > 0
    ? Math.round((todaySucceeded / todayRuns.length) * 100)
    : null

  const hourlyCounts = buildHourlyCounts(all.filter((r) => r.started_at >= todayCutoff))
  const maxCount = Math.max(...hourlyCounts, 1)

  const handleCancel = async (id: string) => {
    setCancelling(id)
    try { await api.cancelExecution(auth!.tenantId, id); load() } catch { /* skip */ } finally { setCancelling(null) }
  }

  const handleRetry = async (r: ExecutionSummary) => {
    setRetrying(r.id)
    try {
      const newRec = await api.retryExecution(auth!.tenantId, r.id)
      onOpenExecution(newRec.id)
    } catch { /* skip */ } finally { setRetrying(null) }
  }

  const handleApprove = async (id: string) => {
    setApproving(id)
    try { await api.approveExecution(id); load() } catch { /* skip */ } finally { setApproving(null) }
  }

  const StatusDot = ({ status }: { status: string }) => {
    const color = status === 'running' ? 'var(--node-http, #0ea5e9)'
      : status === 'waiting_approval' ? 'var(--node-delay, #b45309)'
      : status === 'succeeded' ? 'var(--success, #16a34a)'
      : status === 'failed' ? 'var(--danger-text, #dc2626)'
      : 'var(--muted)'
    return (
      <span style={{
        display: 'inline-block', width: 8, height: 8, borderRadius: '50%',
        background: color, flexShrink: 0,
        animation: LIVE.has(status) ? 'pulse 1.5s infinite' : undefined,
      }} />
    )
  }

  const wfLink = (id: string) => {
    const name = names.get(id) ?? id.slice(-8)
    if (onOpenWorkflow) {
      return (
        <span
          style={{ color: 'var(--link)', cursor: 'pointer', textDecoration: 'underline', fontWeight: 600 }}
          onClick={(e) => { e.stopPropagation(); onOpenWorkflow(id) }}
        >
          {name}
        </span>
      )
    }
    return <span style={{ fontWeight: 600 }}>{name}</span>
  }

  const cardStyle: React.CSSProperties = {
    background: 'var(--panel)',
    border: '1px solid var(--border)',
    borderRadius: 8,
    padding: '12px 16px',
    display: 'flex',
    flexDirection: 'column',
    gap: 4,
  }

  return (
    <div className="app">
      <header className="topbar">
        <img src={logoWordmark} alt="Trigix" style={{ height: '28px' }} />
        <span className="topbar-sep">|</span>
        <span className="topbar-title">{zh ? '监控中心' : 'Monitoring'}</span>

        <div className="topbar-actions" style={{ flex: 1 }}>
          <button className="btn btn-sm" onClick={load}>{zh ? '↺ 刷新' : '↺ Refresh'}</button>
          <span style={{ display: 'inline-flex', alignItems: 'center', gap: 4, fontSize: 11, color: 'var(--link)', fontStyle: 'italic' }}>
            <span style={{ width: 6, height: 6, borderRadius: '50%', background: 'var(--link)', animation: 'pulse 2s infinite', flexShrink: 0 }} />
            {zh ? '自动刷新 5s' : 'auto-refresh 5s'}
          </span>
        </div>

        <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
          <button className="btn btn-sm" onClick={toggleTheme}>{theme === 'dark' ? '☀' : '◑'}</button>
          <button className="btn btn-sm" onClick={onBack}>← {t('nav.back')}</button>
        </div>
      </header>

      <main className="list-page">
        {/* Stats bar */}
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(5, 1fr)', gap: 10, marginBottom: 20 }}>
          <div style={cardStyle}>
            <span style={{ fontSize: 26, fontWeight: 700, color: 'var(--node-http, #0ea5e9)', display: 'flex', alignItems: 'center', gap: 8 }}>
              {liveRuns.length}
              {liveRuns.length > 0 && <span style={{ width: 8, height: 8, borderRadius: '50%', background: 'var(--node-http, #0ea5e9)', animation: 'pulse 1.5s infinite' }} />}
            </span>
            <span style={{ fontSize: 11, color: 'var(--muted)', textTransform: 'uppercase', letterSpacing: '0.04em' }}>{zh ? '运行中' : 'Live Running'}</span>
          </div>
          <div style={cardStyle}>
            <span style={{ fontSize: 26, fontWeight: 700, color: awaitingApproval.length > 0 ? 'var(--node-delay, #b45309)' : 'var(--muted)' }}>
              {awaitingApproval.length}
            </span>
            <span style={{ fontSize: 11, color: 'var(--muted)', textTransform: 'uppercase', letterSpacing: '0.04em' }}>{zh ? '等待审批' : 'Awaiting Approval'}</span>
          </div>
          <div style={cardStyle}>
            <span style={{ fontSize: 26, fontWeight: 700, color: recentFailed.length > 0 ? 'var(--danger-text, #dc2626)' : 'var(--muted)' }}>
              {recentFailed.length}
            </span>
            <span style={{ fontSize: 11, color: 'var(--muted)', textTransform: 'uppercase', letterSpacing: '0.04em' }}>{zh ? '今日失败' : 'Failed Today'}</span>
          </div>
          <div style={cardStyle}>
            <span style={{ fontSize: 26, fontWeight: 700, color: successRate === null ? 'var(--muted)' : successRate >= 90 ? 'var(--success, #16a34a)' : successRate >= 70 ? '#d97706' : 'var(--danger-text, #dc2626)' }}>
              {successRate !== null ? `${successRate}%` : '—'}
            </span>
            <span style={{ fontSize: 11, color: 'var(--muted)', textTransform: 'uppercase', letterSpacing: '0.04em' }}>{zh ? '今日成功率' : 'Success Rate Today'}</span>
          </div>
          <div style={cardStyle}>
            <span style={{ fontSize: 26, fontWeight: 700, color: queueDepth === null ? 'var(--muted)' : queueDepth > 50 ? 'var(--danger-text, #dc2626)' : queueDepth > 10 ? '#d97706' : 'var(--node-http, #0ea5e9)' }}>
              {queueDepth !== null ? queueDepth : '—'}
            </span>
            <span style={{ fontSize: 11, color: 'var(--muted)', textTransform: 'uppercase', letterSpacing: '0.04em' }}>{zh ? '队列深度' : 'Queue Depth'}</span>
            {queueDepth === null ? (
              <span style={{ fontSize: 10, color: 'var(--muted)', fontStyle: 'italic' }}>{zh ? '无 Redis' : 'no Redis'}</span>
            ) : queueDepthHistory.length > 1 && (() => {
              const max = Math.max(...queueDepthHistory, 1)
              const w = 80
              const h = 24
              const pts = queueDepthHistory.map((v, i) => {
                const x = (i / (queueDepthHistory.length - 1)) * w
                const y = h - (v / max) * h
                return `${x},${y}`
              }).join(' ')
              const color = queueDepth > 50 ? '#dc2626' : queueDepth > 10 ? '#d97706' : '#0ea5e9'
              return (
                <svg width={w} height={h} style={{ display: 'block', marginTop: 4 }}>
                  <polyline points={pts} fill="none" stroke={color} strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
                </svg>
              )
            })()}
          </div>
        </div>

        {/* Alert threshold banner */}
        {(() => {
          const failRate = todayRuns.length > 0 ? Math.round((recentFailed.length / todayRuns.length) * 100) : 0
          const isBreached = failRate >= failThreshold && todayRuns.length >= 5
          return (
            <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: 16, padding: '8px 14px', borderRadius: 8, background: isBreached ? 'rgba(220,38,38,0.08)' : 'var(--panel)', border: `1px solid ${isBreached ? 'var(--danger-text, #dc2626)' : 'var(--border)'}` }}>
              <span style={{ fontSize: 13, fontWeight: 600, color: isBreached ? 'var(--danger-text, #dc2626)' : 'var(--muted)' }}>
                {isBreached ? `⚠ ${zh ? '失败率告警' : 'Failure Rate Alert'}: ${failRate}% ≥ ${failThreshold}%` : `✓ ${zh ? '失败率正常' : 'Failure rate normal'}: ${failRate}% / ${failThreshold}% ${zh ? '阈值' : 'threshold'}`}
              </span>
              <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginLeft: 'auto' }}>
                {showThresholdEdit ? (
                  <>
                    <input
                      type="number"
                      min={1}
                      max={100}
                      value={thresholdInput}
                      onChange={(e) => setThresholdInput(e.target.value)}
                      style={{ width: 60, padding: '2px 6px', fontSize: 12, borderRadius: 4, border: '1px solid var(--border)', background: 'var(--surface)' }}
                      onKeyDown={(e) => {
                        if (e.key === 'Enter') {
                          const v = Math.max(1, Math.min(100, Number(thresholdInput) || 20))
                          setFailThreshold(v)
                          try { localStorage.setItem('af:mon:fail-threshold', String(v)) } catch { /* ignore */ }
                          setShowThresholdEdit(false)
                        } else if (e.key === 'Escape') {
                          setShowThresholdEdit(false)
                          setThresholdInput(String(failThreshold))
                        }
                      }}
                      autoFocus
                    />
                    <span style={{ fontSize: 12, color: 'var(--muted)' }}>%</span>
                    <button
                      className="btn btn-sm btn-primary"
                      style={{ fontSize: 11 }}
                      onClick={() => {
                        const v = Math.max(1, Math.min(100, Number(thresholdInput) || 20))
                        setFailThreshold(v)
                        try { localStorage.setItem('af:mon:fail-threshold', String(v)) } catch { /* ignore */ }
                        setShowThresholdEdit(false)
                      }}
                    >✓</button>
                    <button className="btn btn-sm" style={{ fontSize: 11 }} onClick={() => { setShowThresholdEdit(false); setThresholdInput(String(failThreshold)) }}>✕</button>
                  </>
                ) : (
                  <button
                    className="btn btn-sm"
                    style={{ fontSize: 11 }}
                    onClick={() => { setThresholdInput(String(failThreshold)); setShowThresholdEdit(true) }}
                    title={zh ? '编辑告警阈值' : 'Edit alert threshold'}
                  >
                    ✎ {zh ? `阈值 ${failThreshold}%` : `Threshold ${failThreshold}%`}
                  </button>
                )}
              </div>
            </div>
          )
        })()}

        {/* Two-column live + failures */}
        <div style={{ display: 'grid', gridTemplateColumns: '3fr 2fr', gap: 16, marginBottom: 20 }}>
          {/* Live executions */}
          <div style={{ background: 'var(--panel)', border: '1px solid var(--border)', borderRadius: 8, overflow: 'hidden' }}>
            <div style={{ padding: '10px 14px', borderBottom: '1px solid var(--border)', display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
              <span style={{ fontWeight: 600, fontSize: 13 }}>
                {zh ? '实时执行' : 'Live Executions'}
                {liveRuns.length > 0 && (
                  <span style={{ marginLeft: 8, background: 'var(--node-http, #0ea5e9)', color: '#fff', fontSize: 11, borderRadius: 10, padding: '1px 7px', fontWeight: 700 }}>
                    {liveRuns.length}
                  </span>
                )}
              </span>
              {liveRuns.length > 0 && (
                <button
                  className="btn btn-sm btn-danger"
                  style={{ fontSize: 11 }}
                  onClick={async () => {
                    if (!window.confirm(zh ? '取消所有运行中的执行？' : 'Cancel all live executions?')) return
                    await api.cancelAllRunningExecutions(auth!.tenantId)
                    load()
                  }}
                >
                  {zh ? '✕ 全部取消' : '✕ Cancel All'}
                </button>
              )}
            </div>
            {loading ? (
              <div style={{ padding: '2rem', textAlign: 'center', color: 'var(--muted)', fontSize: 13 }}>{zh ? '加载中…' : 'Loading…'}</div>
            ) : liveRuns.length === 0 ? (
              <div style={{ padding: '2rem', textAlign: 'center', color: 'var(--muted)', fontSize: 13 }}>
                <div style={{ fontSize: '1.5rem', marginBottom: 6 }}>✓</div>
                {zh ? '当前没有运行中的执行' : 'No executions currently running'}
              </div>
            ) : (
              <table className="workflow-table" style={{ margin: 0 }}>
                <thead>
                  <tr>
                    <th>{zh ? '工作流' : 'Workflow'}</th>
                    <th>{zh ? '状态' : 'Status'}</th>
                    <th>{zh ? '进度' : 'Progress'}</th>
                    <th>{zh ? '已运行' : 'Elapsed'}</th>
                    <th></th>
                  </tr>
                </thead>
                <tbody>
                  {liveRuns.map((r) => {
                    const nc = r.node_count ?? 0
                    const cc = r.completed_node_count ?? 0
                    const pct = nc > 0 ? Math.round((cc / nc) * 100) : null
                    return (
                    <tr key={r.id} style={{ cursor: 'pointer' }} onClick={() => onOpenExecution(r.id)}>
                      <td>{wfLink(r.workflow_id)}</td>
                      <td>
                        <span style={{ display: 'inline-flex', alignItems: 'center', gap: 5 }}>
                          <StatusDot status={r.status} />
                          <span style={{ fontSize: 12 }}>
                            {r.status === 'waiting_approval' ? (zh ? '待审批' : 'approval') : (zh ? '运行中' : 'running')}
                          </span>
                        </span>
                      </td>
                      <td style={{ minWidth: 90 }}>
                        {pct !== null ? (
                          <div title={`${cc}/${nc} nodes`} style={{ display: 'flex', alignItems: 'center', gap: 5 }}>
                            <div style={{ flex: 1, height: 6, borderRadius: 3, background: 'var(--border)', overflow: 'hidden' }}>
                              <div style={{ height: '100%', width: `${pct}%`, background: 'var(--node-http, #0ea5e9)', borderRadius: 3, transition: 'width 0.4s ease' }} />
                            </div>
                            <span style={{ fontSize: 10, color: 'var(--muted)', flexShrink: 0 }}>{cc}/{nc}</span>
                          </div>
                        ) : (
                          <div style={{ display: 'flex', gap: 3 }}>
                            {[0,1,2].map((i) => (
                              <div key={i} style={{ width: 5, height: 5, borderRadius: '50%', background: 'var(--node-http, #0ea5e9)', opacity: 0.5, animation: `pulse ${1 + i * 0.3}s infinite` }} />
                            ))}
                          </div>
                        )}
                      </td>
                      <td style={{ fontSize: 12, color: 'var(--muted)', fontVariantNumeric: 'tabular-nums' }}>
                        {formatElapsed(r.started_at, zh)}
                      </td>
                      <td onClick={(e) => e.stopPropagation()}>
                        <div style={{ display: 'flex', gap: 4 }}>
                          {r.status === 'waiting_approval' && (
                            <button
                              className="btn btn-sm btn-primary"
                              style={{ fontSize: 11 }}
                              disabled={approving === r.id}
                              onClick={() => handleApprove(r.id)}
                            >
                              {approving === r.id ? '…' : (zh ? '✓ 批准' : '✓ OK')}
                            </button>
                          )}
                          <button
                            className="btn btn-sm btn-danger"
                            style={{ fontSize: 11 }}
                            disabled={cancelling === r.id}
                            onClick={() => handleCancel(r.id)}
                          >
                            {cancelling === r.id ? '…' : '✕'}
                          </button>
                        </div>
                      </td>
                    </tr>
                  )})}
                </tbody>
              </table>
            )}
          </div>

          {/* Recent failures */}
          <div style={{ background: 'var(--panel)', border: '1px solid var(--border)', borderRadius: 8, overflow: 'hidden' }}>
            <div style={{ padding: '10px 14px', borderBottom: '1px solid var(--border)' }}>
              <span style={{ fontWeight: 600, fontSize: 13 }}>
                {zh ? '近期失败' : 'Recent Failures'}
                {recentFailed.length > 0 && (
                  <span style={{ marginLeft: 8, background: 'var(--danger-text, #dc2626)', color: '#fff', fontSize: 11, borderRadius: 10, padding: '1px 7px', fontWeight: 700 }}>
                    {recentFailed.length}
                  </span>
                )}
              </span>
            </div>
            {loading ? (
              <div style={{ padding: '2rem', textAlign: 'center', color: 'var(--muted)', fontSize: 13 }}>{zh ? '加载中…' : 'Loading…'}</div>
            ) : recentFailed.length === 0 ? (
              <div style={{ padding: '2rem', textAlign: 'center', color: 'var(--muted)', fontSize: 13 }}>
                <div style={{ fontSize: '1.5rem', marginBottom: 6 }}>✓</div>
                {zh ? '今日没有失败的执行' : 'No failures today'}
              </div>
            ) : (
              <table className="workflow-table" style={{ margin: 0 }}>
                <thead>
                  <tr>
                    <th>{zh ? '工作流' : 'Workflow'}</th>
                    <th>{zh ? '时间' : 'When'}</th>
                    <th></th>
                  </tr>
                </thead>
                <tbody>
                  {recentFailed.slice(0, 12).map((r) => (
                    <tr key={r.id} style={{ cursor: 'pointer' }} onClick={() => onOpenExecution(r.id)}>
                      <td>{wfLink(r.workflow_id)}</td>
                      <td style={{ fontSize: 12, color: 'var(--muted)' }}>{formatAge(r.started_at, zh)}</td>
                      <td onClick={(e) => e.stopPropagation()}>
                        <button
                          className="btn btn-sm btn-primary"
                          style={{ fontSize: 11 }}
                          disabled={retrying === r.id}
                          onClick={() => handleRetry(r)}
                        >
                          {retrying === r.id ? '…' : (zh ? '↺ 重试' : '↺')}
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </div>
        </div>

        {/* Stuck executions warning */}
        {(() => {
          const STUCK_THRESHOLD_SECS = 5 * 60
          const stuck = liveRuns.filter((r) => r.status === 'running' && (nowSecs - r.started_at) >= STUCK_THRESHOLD_SECS)
          if (stuck.length === 0) return null
          return (
            <div style={{ marginBottom: 16, background: 'rgba(217,119,6,0.07)', border: '1px solid #d97706', borderRadius: 8, overflow: 'hidden' }}>
              <div style={{ padding: '8px 14px', borderBottom: '1px solid rgba(217,119,6,0.2)', display: 'flex', alignItems: 'center', gap: 8 }}>
                <span style={{ fontSize: 15 }}>⚠</span>
                <span style={{ fontWeight: 600, fontSize: 13, color: '#d97706' }}>
                  {zh ? `${stuck.length} 个执行可能卡住（运行超过 5 分钟）` : `${stuck.length} execution${stuck.length !== 1 ? 's' : ''} possibly stuck (running > 5m)`}
                </span>
              </div>
              <table className="workflow-table" style={{ margin: 0 }}>
                <thead>
                  <tr>
                    <th>{zh ? '工作流' : 'Workflow'}</th>
                    <th>{zh ? '已运行' : 'Elapsed'}</th>
                    <th></th>
                  </tr>
                </thead>
                <tbody>
                  {stuck.map((r) => {
                    const elapsed = nowSecs - r.started_at
                    const mins = Math.floor(elapsed / 60)
                    const secs = elapsed % 60
                    const label = mins > 0 ? `${mins}m ${secs}s` : `${secs}s`
                    return (
                      <tr key={r.id} style={{ cursor: 'pointer' }} onClick={() => onOpenExecution(r.id)}>
                        <td>{wfLink(r.workflow_id)}</td>
                        <td style={{ fontSize: 12, color: '#d97706', fontVariantNumeric: 'tabular-nums', fontWeight: 600 }}>{label}</td>
                        <td onClick={(e) => e.stopPropagation()}>
                          <button
                            className="btn btn-sm btn-danger"
                            style={{ fontSize: 11 }}
                            disabled={cancelling === r.id}
                            onClick={() => handleCancel(r.id)}
                          >
                            {cancelling === r.id ? '…' : (zh ? '✕ 强制取消' : '✕ Force Cancel')}
                          </button>
                        </td>
                      </tr>
                    )
                  })}
                </tbody>
              </table>
            </div>
          )
        })()}

        {/* 24h Activity Chart */}
        <div style={{ background: 'var(--panel)', border: '1px solid var(--border)', borderRadius: 8, padding: '14px 16px' }}>
          <div style={{ fontWeight: 600, fontSize: 13, marginBottom: 12 }}>
            {zh ? '过去 24 小时执行量' : '24h Execution Activity'}
            <span style={{ marginLeft: 8, fontWeight: 400, color: 'var(--muted)', fontSize: 12 }}>
              {zh ? `(共 ${todayRuns.length} 次)` : `(${todayRuns.length} total)`}
            </span>
          </div>
          <div style={{ display: 'flex', alignItems: 'flex-end', gap: 3, height: 64 }}>
            {hourlyCounts.map((count, i) => {
              const pct = count / maxCount
              const isCurrentHour = i === 23
              return (
                <div
                  key={i}
                  title={`${i === 23 ? (zh ? '最近1小时' : 'last hour') : `${23 - i}h ago`}: ${count}`}
                  style={{
                    flex: 1,
                    height: `${Math.max(pct * 100, count > 0 ? 8 : 4)}%`,
                    background: isCurrentHour
                      ? 'var(--accent, #2563eb)'
                      : count === 0
                        ? 'var(--border)'
                        : 'var(--node-http, #0ea5e9)',
                    borderRadius: '2px 2px 0 0',
                    opacity: isCurrentHour ? 1 : 0.7 + 0.3 * (i / 23),
                    minHeight: 2,
                    transition: 'height 0.3s ease',
                  }}
                />
              )
            })}
          </div>
          <div style={{ display: 'flex', justifyContent: 'space-between', marginTop: 4, fontSize: 10, color: 'var(--muted)' }}>
            <span>{zh ? '24h 前' : '24h ago'}</span>
            <span>{zh ? '12h 前' : '12h ago'}</span>
            <span>{zh ? '现在' : 'now'}</span>
          </div>
        </div>

        {/* Per-workflow health table */}
        {stats && (
          <div style={{ marginTop: 16, background: 'var(--panel)', border: '1px solid var(--border)', borderRadius: 8, overflow: 'hidden' }}>
            <div style={{ padding: '10px 14px', borderBottom: '1px solid var(--border)', fontWeight: 600, fontSize: 13 }}>
              {zh ? '系统总览' : 'System Overview'}
            </div>
            <div style={{ padding: '12px 16px', display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(160px, 1fr))', gap: 12 }}>
              {[
                { label: zh ? '总执行次数' : 'Total Executions', value: stats.total, color: 'var(--text)' },
                { label: zh ? '运行中' : 'Running', value: stats.running, color: 'var(--node-http, #0ea5e9)' },
                { label: zh ? '等待审批' : 'Waiting', value: stats.waiting_approval, color: '#b45309' },
                { label: zh ? '成功' : 'Succeeded', value: stats.succeeded, color: 'var(--success, #16a34a)' },
                { label: zh ? '失败' : 'Failed', value: stats.failed, color: 'var(--danger-text, #dc2626)' },
                { label: zh ? '已取消' : 'Cancelled', value: stats.cancelled, color: 'var(--muted)' },
                {
                  label: zh ? '平均耗时' : 'Avg Duration',
                  value: stats.avg_duration_secs !== null
                    ? (stats.avg_duration_secs < 60 ? `${stats.avg_duration_secs.toFixed(1)}s` : `${(stats.avg_duration_secs / 60).toFixed(1)}m`)
                    : '—',
                  color: 'var(--text)',
                  raw: true,
                },
              ].map((item) => (
                <div key={item.label} style={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
                  <span style={{ fontSize: 18, fontWeight: 700, color: item.color }}>
                    {item.raw ? item.value : item.value as number}
                  </span>
                  <span style={{ fontSize: 11, color: 'var(--muted)', textTransform: 'uppercase', letterSpacing: '0.04em' }}>{item.label}</span>
                </div>
              ))}
            </div>
            {stats.by_trigger && Object.keys(stats.by_trigger).length > 0 && (
              <div style={{ padding: '0 16px 12px', borderTop: '1px solid var(--border)', marginTop: 4, paddingTop: 12 }}>
                <div style={{ fontSize: 11, color: 'var(--muted)', textTransform: 'uppercase', letterSpacing: '0.04em', marginBottom: 8 }}>
                  {zh ? '触发来源分布' : 'Trigger Sources'}
                </div>
                <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
                  {Object.entries(stats.by_trigger).map(([trigger, count]) => {
                    const icon = trigger === 'webhook' ? '⇅' : trigger === 'schedule' ? '⏱' : trigger === 'retry' ? '↺' : '▶'
                    return (
                      <span key={trigger} style={{
                        fontSize: 12,
                        background: 'var(--surface, var(--bg))',
                        border: '1px solid var(--border)',
                        borderRadius: 12,
                        padding: '2px 10px',
                        display: 'inline-flex',
                        alignItems: 'center',
                        gap: 4,
                      }}>
                        {icon} {trigger}
                        <span style={{ fontWeight: 700 }}>{count}</span>
                      </span>
                    )
                  })}
                </div>
              </div>
            )}
          </div>
        )}
      </main>
    </div>
  )
}
