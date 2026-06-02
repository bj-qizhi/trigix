// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useEffect, useRef, useState } from 'react'
import { useAuth } from '../AuthContext'
import { useLocale } from '../useLocale'
import { useTheme } from '../useTheme'
import * as api from '../api/client'
import type { ExecutionSummary, ScheduleSummary } from '../types'

interface Props {
  onBack: () => void
  onOpenWorkflow?: (workflowId: string) => void
  onOpenExecution?: (executionId: string) => void
}

function formatDuration(secs: number, zh: boolean): string {
  if (secs <= 0) return zh ? '即将运行' : 'imminent'
  if (secs < 60) return zh ? `${secs}秒后` : `in ${secs}s`
  if (secs < 3600) {
    const m = Math.floor(secs / 60)
    const s = secs % 60
    return zh ? `${m}分${s}秒后` : `in ${m}m ${s}s`
  }
  const h = Math.floor(secs / 3600)
  const m = Math.floor((secs % 3600) / 60)
  return zh ? `${h}小时${m}分后` : `in ${h}h ${m}m`
}

function formatSchedule(s: ScheduleSummary, zh: boolean): string {
  if (s.cron_expression) return s.cron_expression
  if (s.interval_secs === 0) return zh ? '已禁用' : 'Disabled'
  const secs = s.interval_secs
  if (secs % 3600 === 0) return zh ? `每 ${secs / 3600} 小时` : `Every ${secs / 3600}h`
  if (secs % 60 === 0) return zh ? `每 ${secs / 60} 分钟` : `Every ${secs / 60}m`
  return zh ? `每 ${secs} 秒` : `Every ${secs}s`
}

function formatAge(secs: number, zh = false): string {
  const diff = Math.floor(Date.now() / 1000) - secs
  if (diff < 5) return zh ? '刚刚' : 'just now'
  if (diff < 60) return zh ? `${diff}秒前` : `${diff}s ago`
  const m = Math.floor(diff / 60)
  if (m < 60) return zh ? `${m}分钟前` : `${m}m ago`
  return zh ? `${Math.floor(m / 60)}小时前` : `${Math.floor(m / 60)}h ago`
}

function useWorkflowNames(tenantId: string): Map<string, string> {
  const [names, setNames] = useState(new Map<string, string>())
  useEffect(() => {
    api.listWorkflows(tenantId, 'project-1')
      .then((wfs) => setNames(new Map(wfs.map((w) => [w.id, w.name]))))
      .catch(() => {})
  }, [tenantId])
  return names
}

// Build a map: workflowId → last N executions (newest-first)
function buildRecentByWorkflow(runs: ExecutionSummary[], n = 7): Map<string, ExecutionSummary[]> {
  const m = new Map<string, ExecutionSummary[]>()
  for (const r of runs) {
    const list = m.get(r.workflow_id) ?? []
    if (list.length < n) list.push(r)
    m.set(r.workflow_id, list)
  }
  return m
}

const STATUS_COLOR: Record<string, string> = {
  succeeded: 'var(--success, #16a34a)',
  failed: 'var(--danger-text, #dc2626)',
  cancelled: 'var(--muted)',
  running: 'var(--node-http, #0ea5e9)',
  waiting_approval: '#b45309',
}

function RunSparkline({ runs }: { runs: ExecutionSummary[] }) {
  if (runs.length === 0) return <span style={{ color: 'var(--muted)', fontSize: 11 }}>—</span>
  return (
    <span style={{ display: 'inline-flex', gap: 2, alignItems: 'center' }}>
      {runs.map((r) => (
        <span
          key={r.id}
          title={`${r.status} · ${formatAge(r.started_at)}`}
          style={{
            width: 8, height: 8, borderRadius: '50%',
            background: STATUS_COLOR[r.status] ?? 'var(--muted)',
            display: 'inline-block', flexShrink: 0,
            opacity: 0.9,
          }}
        />
      ))}
    </span>
  )
}

export function SchedulesPage({ onBack, onOpenWorkflow, onOpenExecution }: Props) {
  const { auth } = useAuth()
  const { locale, toggle: toggleLocale, t } = useLocale()
  const { theme, toggle: toggleTheme } = useTheme()
  const zh = locale === 'zh'
  const workflowNames = useWorkflowNames(auth!.tenantId)

  const [schedules, setSchedules] = useState<ScheduleSummary[]>([])
  const [recentRuns, setRecentRuns] = useState<Map<string, ExecutionSummary[]>>(new Map())
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [toggling, setToggling] = useState<string | null>(null)
  const [running, setRunning] = useState<string | null>(null)
  const [runToast, setRunToast] = useState<string | null>(null)
  const [countdown, setCountdown] = useState<Record<string, number>>({})
  const toastTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const load = () => {
    setLoading(true)
    setError(null)
    Promise.all([
      api.listSchedules(auth!.tenantId),
      api.listExecutionsPage(auth!.tenantId, { limit: 200 }),
    ]).then(([scheds, { data: runs }]) => {
      setSchedules(scheds)
      const c: Record<string, number> = {}
      scheds.forEach((s) => { c[s.workflow_version_id] = s.secs_until_next_run })
      setCountdown(c)
      // runs are newest-first; group by workflow
      setRecentRuns(buildRecentByWorkflow(runs))
    }).catch((e: unknown) => setError(String(e)))
      .finally(() => setLoading(false))
  }

  useEffect(() => {
    load()
    const intervalId = setInterval(() => {
      setCountdown((prev) => {
        const next = { ...prev }
        Object.keys(next).forEach((k) => { next[k] = Math.max(0, next[k] - 1) })
        return next
      })
    }, 1000)
    return () => clearInterval(intervalId)
  }, [])

  const handleTogglePause = async (s: ScheduleSummary) => {
    setToggling(s.workflow_version_id)
    try {
      if (s.paused) await api.resumeSchedule(s.workflow_version_id)
      else await api.pauseSchedule(s.workflow_version_id)
      load()
    } catch { /* ignore */ } finally {
      setToggling(null)
    }
  }

  const handleRunNow = async (s: ScheduleSummary) => {
    setRunning(s.workflow_version_id)
    try {
      const rec = await api.startExecutionFromWorkflow(auth!.tenantId, s.workflow_id, '{}', undefined, 'manual (run now)')
      if (toastTimerRef.current) clearTimeout(toastTimerRef.current)
      setRunToast(rec.id)
      toastTimerRef.current = setTimeout(() => setRunToast(null), 5000)
      load()
    } catch (e) {
      alert(String(e))
    } finally {
      setRunning(null)
    }
  }

  const active = schedules.filter((s) => !s.paused)
  const paused = schedules.filter((s) => s.paused)

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
        <span className="topbar-logo">aiworkflow</span>
        <span className="topbar-sep">|</span>
        <span className="topbar-title">{zh ? '计划任务' : 'Schedules'}</span>

        <div className="topbar-actions" style={{ flex: 1 }}>
          <button className="btn btn-sm" onClick={load}>↺ {zh ? '刷新' : 'Refresh'}</button>
          {active.length > 0 && (
            <button
              className="btn btn-sm"
              disabled={toggling !== null}
              onClick={async () => {
                if (!window.confirm(zh ? `暂停全部 ${active.length} 个活跃计划？` : `Pause all ${active.length} active schedule${active.length !== 1 ? 's' : ''}?`)) return
                for (const s of active) {
                  setToggling(s.workflow_version_id)
                  await api.pauseSchedule(s.workflow_version_id).catch(() => {})
                }
                setToggling(null)
                load()
              }}
              title={zh ? '暂停所有活跃计划' : 'Pause all active schedules'}
            >
              ⏸ {zh ? `全部暂停 (${active.length})` : `Pause All (${active.length})`}
            </button>
          )}
          {paused.length > 0 && (
            <button
              className="btn btn-sm btn-primary"
              disabled={toggling !== null}
              onClick={async () => {
                for (const s of paused) {
                  setToggling(s.workflow_version_id)
                  await api.resumeSchedule(s.workflow_version_id).catch(() => {})
                }
                setToggling(null)
                load()
              }}
              title={zh ? '恢复所有已暂停计划' : 'Resume all paused schedules'}
            >
              ▶ {zh ? `全部恢复 (${paused.length})` : `Resume All (${paused.length})`}
            </button>
          )}
        </div>

        <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
          <button className="btn btn-sm" onClick={toggleTheme}>{theme === 'dark' ? '☀' : '◑'}</button>
          <button className="btn btn-sm" onClick={toggleLocale}>{locale === 'zh' ? 'EN' : '中'}</button>
          <button className="btn btn-sm" onClick={onBack}>← {t('nav.back')}</button>
        </div>
      </header>

      <main className="list-page">
        {/* Summary cards */}
        {!loading && schedules.length > 0 && (
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: 10, marginBottom: 20 }}>
            <div style={cardStyle}>
              <span style={{ fontSize: 22, fontWeight: 700 }}>{schedules.length}</span>
              <span style={{ fontSize: 11, color: 'var(--muted)', textTransform: 'uppercase', letterSpacing: '0.04em' }}>{zh ? '全部' : 'Total'}</span>
            </div>
            <div style={cardStyle}>
              <span style={{ fontSize: 22, fontWeight: 700, color: 'var(--success, #16a34a)', display: 'flex', alignItems: 'center', gap: 6 }}>
                {active.length}
                {active.length > 0 && <span style={{ width: 8, height: 8, borderRadius: '50%', background: 'var(--success, #16a34a)', animation: 'pulse 2s infinite' }} />}
              </span>
              <span style={{ fontSize: 11, color: 'var(--muted)', textTransform: 'uppercase', letterSpacing: '0.04em' }}>{zh ? '活跃' : 'Active'}</span>
            </div>
            <div style={cardStyle}>
              <span style={{ fontSize: 22, fontWeight: 700, color: paused.length > 0 ? '#b45309' : 'var(--muted)' }}>{paused.length}</span>
              <span style={{ fontSize: 11, color: 'var(--muted)', textTransform: 'uppercase', letterSpacing: '0.04em' }}>{zh ? '已暂停' : 'Paused'}</span>
            </div>
          </div>
        )}

        {/* Run Now toast */}
        {runToast && (
          <div style={{
            display: 'flex', alignItems: 'center', gap: 10,
            padding: '10px 14px', marginBottom: 12,
            background: 'rgba(22,163,74,0.08)', border: '1px solid var(--success, #16a34a)',
            borderRadius: 8, fontSize: 13,
          }}>
            <span style={{ color: 'var(--success, #16a34a)', fontWeight: 600 }}>✓ {zh ? '执行已启动' : 'Execution started'}</span>
            {onOpenExecution && (
              <button className="btn btn-sm" style={{ fontSize: 12 }} onClick={() => { onOpenExecution(runToast); setRunToast(null) }}>
                {zh ? '查看详情' : 'View →'}
              </button>
            )}
            <button style={{ marginLeft: 'auto', background: 'none', border: 'none', cursor: 'pointer', color: 'var(--muted)', fontSize: 16 }} onClick={() => setRunToast(null)}>×</button>
          </div>
        )}

        {loading && (
          <div style={{ padding: '3rem', textAlign: 'center', color: 'var(--muted)' }}>
            {zh ? '加载中…' : 'Loading…'}
          </div>
        )}

        {error && (
          <div style={{ padding: '1rem 2rem', color: 'var(--danger-text, #dc2626)', background: 'var(--danger-bg, #fee2e2)', borderRadius: 6, margin: '0 0 1rem' }}>
            {error}
          </div>
        )}

        {!loading && !error && schedules.length === 0 && (
          <div style={{ padding: '4rem', textAlign: 'center', color: 'var(--muted)' }}>
            <div style={{ fontSize: '2.5rem', marginBottom: '1rem' }}>⏱</div>
            <div style={{ fontWeight: 600, fontSize: '1.1rem', marginBottom: '0.5rem' }}>
              {zh ? '没有计划任务' : 'No scheduled workflows'}
            </div>
            <div style={{ fontSize: '0.9rem' }}>
              {zh
                ? '在工作流编辑器的触发节点中设置计划触发器。'
                : 'Set up schedule triggers in the Trigger node inside the workflow editor.'}
            </div>
          </div>
        )}

        {!loading && schedules.length > 0 && (
          <div>
            <table className="workflow-table">
              <thead>
                <tr>
                  <th style={{ minWidth: 140 }}>{zh ? '工作流' : 'Workflow'}</th>
                  <th>{zh ? '计划' : 'Schedule'}</th>
                  <th>{zh ? '下次运行' : 'Next Run'}</th>
                  <th>{zh ? '近期运行' : 'Recent Runs'}</th>
                  <th>{zh ? '最后运行' : 'Last Run'}</th>
                  <th>{zh ? '状态' : 'Status'}</th>
                  <th>{zh ? '操作' : 'Actions'}</th>
                </tr>
              </thead>
              <tbody>
                {schedules.map((s) => {
                  const wfName = workflowNames.get(s.workflow_id) ?? s.workflow_id.slice(0, 8) + '…'
                  const secs = countdown[s.workflow_version_id] ?? s.secs_until_next_run
                  const isToggling = toggling === s.workflow_version_id
                  const isRunning = running === s.workflow_version_id
                  const recent = recentRuns.get(s.workflow_id) ?? []
                  const lastRun = recent[0]
                  return (
                    <tr key={s.workflow_version_id} style={{ opacity: s.paused ? 0.65 : 1 }}>
                      <td>
                        <div style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
                          {onOpenWorkflow ? (
                            <button
                              style={{ fontWeight: 600, textAlign: 'left', background: 'none', border: 'none', cursor: 'pointer', color: 'var(--accent)', padding: 0, fontSize: 'inherit' }}
                              onClick={() => onOpenWorkflow(s.workflow_id)}
                            >
                              {wfName}
                            </button>
                          ) : (
                            <span style={{ fontWeight: 600 }}>{wfName}</span>
                          )}
                          <span style={{ fontSize: 11, color: 'var(--muted)', fontFamily: 'monospace' }}>
                            v:{s.workflow_version_id.slice(0, 10)}…
                          </span>
                        </div>
                      </td>
                      <td>
                        <code style={{ background: 'var(--panel)', border: '1px solid var(--border)', padding: '2px 6px', borderRadius: 4, fontSize: 12 }}>
                          {formatSchedule(s, zh)}
                        </code>
                      </td>
                      <td>
                        {s.paused ? (
                          <span style={{ color: 'var(--muted)', fontSize: 13 }}>—</span>
                        ) : (
                          <span style={{
                            color: secs < 60 ? 'var(--success-text, #16a34a)' : 'var(--text)',
                            fontVariantNumeric: 'tabular-nums',
                            fontSize: 13,
                            fontWeight: secs < 60 ? 700 : 400,
                          }}>
                            {formatDuration(secs, zh)}
                          </span>
                        )}
                      </td>
                      <td>
                        <RunSparkline runs={recent} />
                      </td>
                      <td>
                        {lastRun ? (
                          <div style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
                            <span
                              style={{ cursor: onOpenExecution ? 'pointer' : 'default' }}
                              onClick={onOpenExecution ? () => onOpenExecution(lastRun.id) : undefined}
                            >
                              <span className={`badge badge-${lastRun.status}`} style={{ fontSize: 11 }}>{lastRun.status}</span>
                            </span>
                            <span style={{ fontSize: 11, color: 'var(--muted)' }}>{formatAge(lastRun.started_at, zh)}</span>
                          </div>
                        ) : (
                          <span style={{ color: 'var(--muted)', fontSize: 12 }}>{zh ? '无记录' : 'No runs'}</span>
                        )}
                      </td>
                      <td>
                        <span style={{
                          display: 'inline-flex', alignItems: 'center', gap: 5,
                          padding: '2px 8px', borderRadius: 12, fontSize: 11, fontWeight: 600,
                          background: s.paused ? 'rgba(180,83,9,0.1)' : 'rgba(22,163,74,0.1)',
                          color: s.paused ? '#b45309' : 'var(--success, #16a34a)',
                        }}>
                          {s.paused
                            ? (zh ? '⏸ 已暂停' : '⏸ Paused')
                            : <><span style={{ width: 6, height: 6, borderRadius: '50%', background: 'var(--success, #16a34a)', animation: 'pulse 2s infinite', flexShrink: 0 }} />{zh ? '活跃' : 'Active'}</>
                          }
                        </span>
                      </td>
                      <td>
                        <div style={{ display: 'flex', gap: 5 }}>
                          <button
                            className="btn btn-sm btn-primary"
                            disabled={isRunning || s.paused}
                            onClick={() => handleRunNow(s)}
                            title={s.paused ? (zh ? '恢复后方可立即运行' : 'Resume first to run now') : (zh ? '立即运行一次' : 'Trigger one run now')}
                            style={{ fontSize: 11 }}
                          >
                            {isRunning ? '…' : (zh ? '▶ 立即运行' : '▶ Run Now')}
                          </button>
                          <button
                            className="btn btn-sm"
                            disabled={isToggling}
                            onClick={() => handleTogglePause(s)}
                            style={{ fontSize: 11, ...(s.paused ? {} : { color: '#b45309', borderColor: '#b45309' }) }}
                            title={s.paused ? (zh ? '恢复' : 'Resume') : (zh ? '暂停' : 'Pause')}
                          >
                            {isToggling ? '…' : s.paused ? '▶' : '⏸'}
                          </button>
                        </div>
                      </td>
                    </tr>
                  )
                })}
              </tbody>
            </table>
          </div>
        )}
      </main>
    </div>
  )
}
