// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useEffect, useRef, useState } from 'react'
import { IconTestTube, ThemeToggleIcon, IconCards, IconGraph } from './uiIcons'
import { useAuth } from '../AuthContext'
import * as api from '../api/client'
import { friendlyError } from '../errorMessage'
import { SkeletonRows } from './Skeleton'
import type { ExecutionRecord, AuditEvent } from '../types'
import { useTheme } from '../useTheme'
import { useLocale } from '../useLocale'
import { JsonTree } from './JsonTree'
import { prettyJson, CopyButton, NodeResultCard, StatCard, ExecutionGraph, ExecutionTimeline, NoteEditor, LabelEditor } from './executiondetail/ExecutionDetailParts'

interface Props {
  executionId: string
  onBack: () => void
  onOpenWorkflow: (workflowId: string, initialInput?: string) => void
  onRetry: (newExecutionId: string) => void
  onOpenExecution?: (executionId: string) => void
}

function formatTs(secs: number): string {
  return new Date(secs * 1000).toLocaleString()
}

function formatDuration(started: number, finished?: number, zh = false): string {
  if (!finished) return zh ? '进行中…' : 'in progress…'
  const secs = finished - started
  if (secs < 60) return `${secs}s`
  return `${Math.floor(secs / 60)}m ${secs % 60}s`
}


export function ExecutionDetailPage({ executionId, onBack, onOpenWorkflow, onRetry, onOpenExecution }: Props) {
  const { auth } = useAuth()
  const { theme, toggle: toggleTheme } = useTheme()
  const { locale, toggle: toggleLocale, t } = useLocale()
  const zh = locale === 'zh'
  const [record, setRecord]         = useState<ExecutionRecord | null>(null)
  const [loading, setLoading]       = useState(true)
  const [error, setError]           = useState<string | null>(null)
  const [cancelling, setCancelling]   = useState(false)
  const [deleting, setDeleting]       = useState(false)
  const [retrying, setRetrying]       = useState(false)
  const [showReplay, setShowReplay]   = useState(false)
  const [replayInput, setReplayInput] = useState('')
  const [replaying, setReplaying]     = useState(false)
  const [approving, setApproving]     = useState(false)
  const [rejecting, setRejecting]     = useState(false)
  const [approvalComment, setApprovalComment] = useState('')
  const [nodeFilter, setNodeFilter]   = useState<string>('all')
  const [nodeTypeFilter, setNodeTypeFilter] = useState<string>('all')
  const [nodeSearch, setNodeSearch]   = useState('')
  const [nodeView, setNodeView]       = useState<'cards' | 'log' | 'graph'>('cards')
  const logBottomRef                  = useRef<HTMLDivElement>(null)
  const [auditEvents, setAuditEvents] = useState<AuditEvent[]>([])

  const load = (quiet = false) => {
    if (!quiet) setLoading(true)
    api.getExecution(auth!.tenantId, executionId)
      .then(setRecord)
      .catch((e: unknown) => setError(friendlyError(e, zh)))
      .finally(() => setLoading(false))
  }

  useEffect(() => { load() }, [executionId])
  useEffect(() => {
    api.listAuditLog(auth!.tenantId, 50, executionId).then(setAuditEvents).catch(() => {})
  }, [executionId])

  // Subscribe via SSE while live; fall back to polling if EventSource unavailable
  useEffect(() => {
    if (!record) return
    if (record.status !== 'running' && record.status !== 'waiting_approval') return

    const token = auth?.token ?? ''
    const url = `/v1/executions/${executionId}/events?tenant_id=${encodeURIComponent(auth!.tenantId)}&token=${encodeURIComponent(token)}`

    if (typeof EventSource !== 'undefined') {
      const es = new EventSource(url)
      es.addEventListener('update', (e: MessageEvent) => {
        try {
          const updated = JSON.parse(e.data as string) as import('../types').ExecutionRecord
          setRecord(updated)
          if (updated.status !== 'running' && updated.status !== 'waiting_approval') {
            es.close()
          }
        } catch { /* ignore parse errors */ }
      })
      es.onerror = () => { es.close(); load(true) }
      return () => es.close()
    }

    // Fallback: polling
    const timer = setInterval(() => load(true), 1500)
    return () => clearInterval(timer)
  }, [record?.id, record?.status])

  const isLive      = record?.status === 'running' || record?.status === 'waiting_approval'
  const isWaiting   = record?.status === 'waiting_approval'
  const isRetryable = record?.status === 'failed' || record?.status === 'cancelled'

  const handleRetry = async () => {
    if (!record || !isRetryable) return
    setRetrying(true)
    try {
      const newExec = await api.retryExecution(auth!.tenantId, executionId)
      onRetry(newExec.id)
    } catch (e) {
      setError(friendlyError(e, zh))
      setRetrying(false)
    }
  }

  const handleOpenReplay = () => {
    setReplayInput(prettyJson(record?.input_json ?? null) || '{}')
    setShowReplay(true)
  }

  const handleReplay = async () => {
    if (!record || replaying) return
    let parsed: unknown
    try { parsed = JSON.parse(replayInput) } catch { return setError(zh ? '回放输入不是有效的 JSON' : 'Replay input is not valid JSON') }
    setReplaying(true)
    try {
      const newExec = await api.startExecutionFromVersion(
        auth!.tenantId,
        record.workflow_version_id,
        JSON.stringify(parsed),
      )
      setShowReplay(false)
      onRetry(newExec.id)
    } catch (e) {
      setError(friendlyError(e, zh))
    } finally {
      setReplaying(false)
    }
  }

  const handleCancel = async () => {
    if (!record || !isLive) return
    setCancelling(true)
    try {
      await api.cancelExecution(auth!.tenantId, executionId)
      load(true)
    } catch (e) {
      setError(friendlyError(e, zh))
    } finally {
      setCancelling(false)
    }
  }

  const handleDelete = async () => {
    if (!record || isLive) return
    if (!window.confirm(zh ? '永久删除此执行记录？' : 'Permanently delete this execution record?')) return
    setDeleting(true)
    try {
      await api.deleteExecution(auth!.tenantId, executionId)
      onBack()
    } catch (e) {
      setError(friendlyError(e, zh))
      setDeleting(false)
    }
  }

  const handleApprove = async () => {
    if (!record || !isWaiting) return
    setApproving(true)
    try {
      await api.approveExecution(executionId, approvalComment || undefined)
      setApprovalComment('')
      load(true)
    } catch (e) {
      setError(friendlyError(e, zh))
    } finally {
      setApproving(false)
    }
  }

  const handleReject = async () => {
    if (!record || !isWaiting) return
    setRejecting(true)
    try {
      await api.rejectExecution(executionId, approvalComment || undefined)
      setApprovalComment('')
      load(true)
    } catch (e) {
      setError(friendlyError(e, zh))
    } finally {
      setRejecting(false)
    }
  }

  return (
    <div className="app">
      <header className="topbar">
        <button className="btn btn-sm btn-icon" onClick={onBack} title={zh ? '返回运行列表' : 'Back to runs'}>←</button>
        <span className="topbar-sep">|</span>
        <span className="topbar-title" style={{ fontFamily: 'monospace', fontSize: 13 }}>
          run:{executionId.slice(-12)}
        </span>
        {record && (
          <>
            <span className={`badge badge-${record.status}`}>{record.status}</span>
            {record.dry_run && (
              <span style={{ fontSize: 10, padding: '1px 5px', background: 'var(--link)', color: '#fff', borderRadius: 3, fontWeight: 600 }}>
                DRY
              </span>
            )}
          </>
        )}
        {isLive && (
          <>
            <span style={{ fontSize: 11, color: 'var(--link)', animation: 'pulse 1.5s infinite' }}>
              {zh ? '实时' : 'live'}
            </span>
            {(record?.node_count ?? 0) > 0 && (() => {
              const nc = record!.node_count!
              const cc = record!.completed_node_count ?? 0
              const pct = Math.round((cc / nc) * 100)
              return (
                <div style={{ display: 'flex', alignItems: 'center', gap: 5 }}>
                  <div style={{ width: 80, height: 5, borderRadius: 3, background: 'var(--border)', overflow: 'hidden' }}>
                    <div style={{ height: '100%', width: `${pct}%`, background: 'var(--node-http, #0ea5e9)', borderRadius: 3, transition: 'width 0.4s ease' }} />
                  </div>
                  <span style={{ fontSize: 10, color: 'var(--muted)' }}>{cc}/{nc}</span>
                </div>
              )
            })()}
          </>
        )}
        <div className="topbar-actions">
          {isRetryable && (
            <button
              className="btn btn-sm btn-primary"
              disabled={retrying}
              onClick={handleRetry}
              title={zh ? '使用相同输入重试此执行' : 'Retry this execution with the same input'}
            >
              {retrying ? (zh ? '重试中…' : 'Retrying…') : t('exec.retry')}
            </button>
          )}
          {record && !isLive && (
            <button
              className="btn btn-sm btn-secondary"
              onClick={handleOpenReplay}
              title={zh ? '修改输入后重新运行' : 'Re-run with modified input'}
            >
              {t('exec.replay')}
            </button>
          )}
          {isWaiting && (
            <>
              <input
                value={approvalComment}
                onChange={(e) => setApprovalComment(e.target.value)}
                placeholder={zh ? '备注（可选）' : 'Comment (optional)'}
                style={{ fontSize: 12, padding: '2px 6px', width: 180 }}
              />
              <button
                className="btn btn-sm btn-primary"
                disabled={approving || rejecting}
                onClick={handleApprove}
                title={zh ? '批准并继续执行' : 'Approve and continue this execution'}
              >
                {approving ? (zh ? '批准中…' : 'Approving…') : t('exec.approve')}
              </button>
              <button
                className="btn btn-sm btn-danger"
                disabled={approving || rejecting}
                onClick={handleReject}
                title={zh ? '拒绝并终止执行' : 'Reject and fail this execution'}
              >
                {rejecting ? (zh ? '拒绝中…' : 'Rejecting…') : t('exec.reject')}
              </button>
            </>
          )}
          {isLive && !isWaiting && (
            <button
              className="btn btn-sm btn-danger"
              disabled={cancelling}
              onClick={handleCancel}
              title={zh ? '取消此执行' : 'Cancel this execution'}
            >
              {cancelling ? (zh ? '取消中…' : 'Cancelling…') : t('exec.cancel')}
            </button>
          )}
          {record && !isLive && (
            <button
              className="btn btn-sm btn-danger"
              disabled={deleting}
              onClick={handleDelete}
              title={zh ? '永久删除此执行记录' : 'Delete this execution record permanently'}
            >
              {deleting ? (zh ? '删除中…' : 'Deleting…') : t('exec.delete')}
            </button>
          )}
          {record && (
            <button
              className="btn btn-sm"
              onClick={() => {
                const blob = new Blob([JSON.stringify(record, null, 2)], { type: 'application/json' })
                const url = URL.createObjectURL(blob)
                const a = document.createElement('a')
                a.href = url
                a.download = `execution-${record.id.slice(-12)}.json`
                a.click()
                URL.revokeObjectURL(url)
              }}
              title={zh ? '下载执行记录（JSON）' : 'Download execution record as JSON'}
            >
              {t('exec.export')}
            </button>
          )}
          {record && record.workflow_version_id && (
            <button
              className="btn btn-sm"
              title={zh ? '复制为 cURL 命令' : 'Copy as cURL command'}
              onClick={() => {
                const inputData = record.input_json ? JSON.parse(record.input_json) : {}
                const curl = `curl -s -X POST \\
  '${window.location.origin}/v1/workflows/version/${record.workflow_version_id}/execute?tenant_id=${auth!.tenantId}' \\
  -H 'Content-Type: application/json' \\
  -H 'Authorization: Bearer ${auth!.token}' \\
  -d '${JSON.stringify({ input_json: JSON.stringify(inputData) })}'`
                navigator.clipboard.writeText(curl)
              }}
            >
              ⎘ cURL
            </button>
          )}
          {record && (
            <button
              className="btn btn-sm btn-icon"
              title={record.starred ? (zh ? '取消收藏' : 'Unstar execution') : (zh ? '收藏此执行' : 'Star execution')}
              style={{ fontSize: 18, color: record.starred ? '#f59e0b' : 'var(--muted)', lineHeight: 1 }}
              onClick={async () => {
                const fn = record.starred ? api.unstarExecution : api.starExecution
                await fn(auth!.tenantId, record.id).catch(() => null)
                setRecord((r) => r ? { ...r, starred: !r.starred } : r)
              }}
            >
              {record.starred ? '⭐' : '☆'}
            </button>
          )}
          {record && (
            <>
              <button
                className="btn btn-sm"
                onClick={() => onOpenWorkflow(record.workflow_id)}
                title={zh ? '打开工作流编辑器' : 'Open workflow editor'}
              >
                {zh ? '打开工作流 →' : 'Open Workflow →'}
              </button>
              <button
                className="btn btn-sm"
                onClick={() => onOpenWorkflow(record.workflow_id, record.input_json ?? undefined)}
                title={zh ? '在编辑器中预填本次执行的输入' : "Open workflow editor with this execution's input pre-filled"}
              >
                {zh ? '↺ 带输入重运行' : '↺ Re-run with Input'}
              </button>
            </>
          )}
          <button className="btn btn-sm" onClick={toggleTheme} title={zh ? '切换深色/浅色主题' : 'Toggle dark/light theme'}>{theme === 'dark' ? <ThemeToggleIcon dark /> : <ThemeToggleIcon dark={false} />}</button>
          <button className="btn btn-sm" onClick={toggleLocale} title="切换语言 / Switch language">{locale === 'zh' ? 'EN' : '中'}</button>
        </div>
      </header>

      <main className="list-page">
        {loading && <SkeletonRows rows={6} />}
        {error && <p style={{ color: 'var(--danger-text)' }}>{error}</p>}

        {record && (
          <>
            {/* ── Summary ── */}
            <section style={{ marginBottom: 28 }}>
              <h1 style={{ marginBottom: 16 }}>{zh ? '执行摘要' : 'Execution Summary'}</h1>
              <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(200px, 1fr))', gap: 12 }}>
                <StatCard label={t('exec.status')}>
                  <span className={`badge badge-${record.status}`}>{record.status}</span>
                </StatCard>
                <StatCard label={t('exec.id')}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
                    <span style={{ fontFamily: 'monospace', fontSize: 12 }}>{record.id.slice(-16)}</span>
                    <CopyButton text={record.id} />
                  </div>
                </StatCard>
                <StatCard label={t('exec.started')}>
                  {formatTs(record.started_at)}
                </StatCard>
                <StatCard label={t('exec.finished')}>
                  {record.finished_at ? formatTs(record.finished_at) : '—'}
                </StatCard>
                <StatCard label={t('exec.duration')}>
                  {formatDuration(record.started_at, record.finished_at, zh)}
                </StatCard>
                <StatCard label={t('exec.workflow')}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
                    <span style={{ fontFamily: 'monospace', fontSize: 12 }}>{record.workflow_id.slice(-12)}</span>
                    <CopyButton text={record.workflow_id} />
                  </div>
                </StatCard>
                <StatCard label={zh ? '版本 ID' : 'Version ID'}>
                  <span style={{ fontFamily: 'monospace', fontSize: 12 }}>
                    {record.workflow_version_id.slice(-12)}
                  </span>
                </StatCard>
                <LabelEditor
                  tenantId={auth!.tenantId}
                  executionId={executionId}
                  label={record.label ?? null}
                  onSaved={(newLabel) => setRecord((r) => r ? { ...r, label: newLabel ?? undefined } : r)}
                />
                {record.trigger_type && (
                  <StatCard label={zh ? '触发方式' : 'Trigger'}>
                    <span style={{ fontSize: 12 }}>{record.trigger_type}</span>
                  </StatCard>
                )}
                {record.retried_from && (
                  <StatCard label={zh ? '重试自' : 'Retried from'}>
                    <button
                      className="btn btn-sm"
                      style={{ fontSize: 11, padding: '2px 6px' }}
                      onClick={() => onOpenExecution?.(record.retried_from!)}
                      title={record.retried_from}
                    >
                      ↺ {record.retried_from.slice(-12)}
                    </button>
                  </StatCard>
                )}
                <NoteEditor
                  tenantId={auth!.tenantId}
                  executionId={executionId}
                  note={record.note ?? null}
                  onSaved={(newNote) => setRecord((r) => r ? { ...r, note: newNote } : r)}
                />
              </div>
            </section>

            {/* ── Workflow Output ── */}
            {record.output_json && record.status === 'succeeded' && (
              <section style={{ marginBottom: 28 }}>
                <h2 style={{ marginBottom: 10, display: 'flex', alignItems: 'center' }}>
                  {t('exec.output')}
                  <CopyButton text={prettyJson(record.output_json)} />
                </h2>
                <div style={{
                  background: 'var(--panel)', border: '1px solid var(--success-text)',
                  borderRadius: 'var(--radius)', padding: '10px 12px',
                  maxHeight: 320, overflowY: 'auto',
                }}>
                  <JsonTree raw={record.output_json} />
                </div>
              </section>
            )}

            {/* ── Input ── */}
            <section style={{ marginBottom: 28 }}>
              <h2 style={{ marginBottom: 10 }}>{zh ? '输入' : 'Input'}</h2>
              <pre style={{
                background: 'var(--panel)', border: '1px solid var(--border)',
                borderRadius: 'var(--radius)', padding: '10px 12px',
                fontSize: 12, fontFamily: 'monospace', overflowX: 'auto',
                color: 'var(--muted)', lineHeight: 1.5, maxHeight: 180, overflowY: 'auto',
              }}>
                {prettyJson(record.input_json ?? null) || '{}'}
              </pre>
            </section>

            {/* ── Approval notice ── */}
            {isWaiting && (
              <section style={{ marginBottom: 28 }}>
                <div style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: 14,
                  padding: '14px 18px',
                  background: 'rgba(8,145,178,0.08)',
                  border: '1px solid var(--approval-text)',
                  borderRadius: 8,
                }}>
                  <span style={{
                    width: 10, height: 10, borderRadius: '50%',
                    background: 'var(--approval-text)',
                    animation: 'pulse 1s infinite', flexShrink: 0,
                  }} />
                  <div>
                    <div style={{ fontWeight: 600, color: 'var(--approval-text)', marginBottom: 4 }}>
                      {zh ? '等待审批' : 'Waiting for approval'}
                    </div>
                    <div style={{ fontSize: 12, color: 'var(--muted)' }}>
                      {zh ? '审批节点已暂停。批准后继续执行，拒绝则终止执行。' : 'An approval node is paused. Approve to continue or Reject to fail the execution.'}
                    </div>
                  </div>
                  <div style={{ display: 'flex', flexDirection: 'column', gap: 8, marginLeft: 'auto' }}>
                    <input
                      value={approvalComment}
                      onChange={(e) => setApprovalComment(e.target.value)}
                      placeholder={zh ? '备注（可选）' : 'Comment (optional)'}
                      style={{ fontSize: 12, padding: '4px 8px', borderRadius: 4, border: '1px solid var(--border)', background: 'var(--bg)', color: 'var(--fg)' }}
                    />
                    <div style={{ display: 'flex', gap: 8 }}>
                      <button
                        className="btn btn-primary"
                        disabled={approving || rejecting}
                        onClick={handleApprove}
                      >
                        {approving ? (zh ? '批准中…' : 'Approving…') : (zh ? '✓ 批准' : '✓ Approve')}
                      </button>
                      <button
                        className="btn btn-danger"
                        disabled={approving || rejecting}
                        onClick={handleReject}
                      >
                        {rejecting ? (zh ? '拒绝中…' : 'Rejecting…') : (zh ? '✕ 拒绝' : '✕ Reject')}
                      </button>
                    </div>
                  </div>
                </div>
              </section>
            )}

            {/* ── Dry-run banner ── */}
            {record.dry_run && (
              <section style={{ marginBottom: 16 }}>
                <div style={{
                  display: 'flex', alignItems: 'center', gap: 10, padding: '10px 16px',
                  background: 'rgba(59,130,246,0.07)',
                  border: '1px solid var(--link)',
                  borderRadius: 8, fontSize: 13, color: 'var(--link)',
                }}>
                  <span style={{ fontSize: 18 }}><IconTestTube size={16} /></span>
                  <span>{zh ? '演练模式 — 外部 API 调用已跳过，节点输出仅包含模拟数据。' : 'Dry run — external API calls were skipped. Node outputs contain mock data only.'}</span>
                </div>
              </section>
            )}

            {/* ── Failure banner ── */}
            {record.status === 'failed' && (() => {
              const failed = record.node_results.find((nr) => nr.status === 'failed')
              if (!failed) return null
              return (
                <section style={{ marginBottom: 28 }}>
                  <div style={{
                    display: 'flex', gap: 14, padding: '14px 18px',
                    background: 'rgba(220,38,38,0.07)',
                    border: '1px solid var(--danger-text)',
                    borderRadius: 8,
                  }}>
                    <span style={{ fontSize: 22, flexShrink: 0, marginTop: 2 }}>✕</span>
                    <div style={{ flex: 1 }}>
                      <div style={{ fontWeight: 600, color: 'var(--danger-text)', marginBottom: 6 }}>
                        {zh
                          ? <>执行在 <code style={{ fontSize: 13 }}>{failed.node_id}</code> 处失败</>
                          : <>Execution failed at <code style={{ fontSize: 13 }}>{failed.node_id}</code></>}
                        {failed.node_type && <span style={{ fontWeight: 400, fontSize: 12, color: 'var(--muted)', marginLeft: 6 }}>({failed.node_type})</span>}
                      </div>
                      {failed.error && (
                        <pre style={{
                          margin: 0, fontSize: 11, fontFamily: 'monospace',
                          color: 'var(--danger-text)', whiteSpace: 'pre-wrap', wordBreak: 'break-all',
                          background: 'rgba(220,38,38,0.05)', borderRadius: 4, padding: '6px 8px',
                        }}>
                          {failed.error}
                        </pre>
                      )}
                    </div>
                  </div>
                </section>
              )
            })()}

            {/* ── Long-running warning ── */}
            {record.status === 'running' && (() => {
              const elapsedSecs = Math.floor(Date.now() / 1000) - record.started_at
              if (elapsedSecs < 300) return null
              const mins = Math.floor(elapsedSecs / 60)
              return (
                <section style={{ marginBottom: 20 }}>
                  <div style={{
                    display: 'flex', gap: 12, padding: '12px 16px',
                    background: 'rgba(217,119,6,0.08)',
                    border: '1px solid #d97706',
                    borderRadius: 8,
                    alignItems: 'center',
                  }}>
                    <span style={{ fontSize: 20 }}>⚠</span>
                    <span style={{ fontSize: 13, color: '#d97706' }}>
                      {zh
                        ? <>此执行已运行 <strong>{mins} 分钟</strong>。如果疑似卡住，可以取消。</>
                        : <>This execution has been running for <strong>{mins} minute{mins !== 1 ? 's' : ''}</strong>. If it appears stuck, you can cancel it.</>}
                    </span>
                  </div>
                </section>
              )
            })()}

            {/* ── Execution Timeline ── */}
            {record.node_results.length > 0 && record.finished_at && (
              <ExecutionTimeline
                nodeResults={record.node_results}
                startedAt={record.started_at}
                finishedAt={record.finished_at}
                onClickNode={(nodeId) => {
                  const el = document.getElementById(`node-result-${nodeId}`)
                  if (el) { el.scrollIntoView({ behavior: 'smooth', block: 'center' }); el.style.outline = '2px solid var(--link)'; setTimeout(() => { el.style.outline = '' }, 1800) }
                }}
              />
            )}

            {/* ── Node Results ── */}
            <section>
              {(() => {
                const TOKEN_PRICES: Record<string, { input: number; output: number }> = {
                  'gpt-4o': { input: 2.50, output: 10.00 }, 'gpt-4o-mini': { input: 0.15, output: 0.60 },
                  'o1': { input: 15.00, output: 60.00 }, 'o1-mini': { input: 3.00, output: 12.00 },
                  'gemini-2.0-flash': { input: 0.075, output: 0.30 }, 'gemini-1.5-pro': { input: 1.25, output: 5.00 },
                  'claude-opus-4-7': { input: 15.00, output: 75.00 }, 'claude-opus-4-8': { input: 15.00, output: 75.00 }, 'claude-sonnet-4-6': { input: 3.00, output: 15.00 },
                  'claude-haiku-4-5-20251001': { input: 0.80, output: 4.00 },
                }
                let totalCost = 0; let hasAi = false
                for (const nr of record.node_results) {
                  if (!['openai','gemini','claude'].includes(nr.node_type)) continue
                  if (!nr.output_json) continue
                  try {
                    const out = JSON.parse(nr.output_json)
                    const usage = out.usage as { prompt_tokens?: number; input_tokens?: number; completion_tokens?: number; output_tokens?: number } | undefined
                    if (!usage) continue
                    const prompt = usage.prompt_tokens ?? usage.input_tokens ?? 0
                    const completion = usage.completion_tokens ?? usage.output_tokens ?? 0
                    const model = (out.model as string) ?? ''
                    const prices = TOKEN_PRICES[model]
                    if (prices) { totalCost += (prompt / 1e6) * prices.input + (completion / 1e6) * prices.output; hasAi = true }
                  } catch { /* ignore */ }
                }
                if (!hasAi) return null
                return (
                  <div style={{ fontSize: 11, color: 'var(--muted)', marginBottom: 6 }}>
                    {zh ? '预估 AI 费用' : 'Est. AI cost'}: <strong style={{ color: 'var(--fg)' }}>~${totalCost.toFixed(5)}</strong>
                  </div>
                )
              })()}
              <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: 10 }}>
                <h2 style={{ margin: 0 }}>
                  {zh ? '节点结果' : 'Node Results'}
                  <span style={{ fontSize: 12, fontWeight: 400, color: 'var(--muted)', marginLeft: 8 }}>
                    {zh
                      ? `${record.node_results.length} 个节点`
                      : `${record.node_results.length} node${record.node_results.length !== 1 ? 's' : ''}`}
                  </span>
                </h2>
                <div style={{ display: 'flex', gap: 6, marginLeft: 'auto', alignItems: 'center' }}>
                  <button
                    className={`btn btn-sm${nodeView === 'cards' ? ' btn-primary' : ''}`}
                    title={zh ? '卡片视图' : 'Card view'}
                    onClick={() => setNodeView('cards')}
                    style={{ fontSize: 11 }}
                  >
                    <IconCards aria-hidden style={{ verticalAlign: '-2px', marginRight: 3 }} />{zh ? '卡片' : 'Cards'}
                  </button>
                  <button
                    className={`btn btn-sm${nodeView === 'log' ? ' btn-primary' : ''}`}
                    title={zh ? '日志视图' : 'Log console view'}
                    onClick={() => setNodeView('log')}
                    style={{ fontSize: 11 }}
                  >
                    {zh ? '日志' : 'Log'}
                  </button>
                  <button
                    className={`btn btn-sm${nodeView === 'graph' ? ' btn-primary' : ''}`}
                    title={zh ? '图形视图' : 'Graph view'}
                    onClick={() => setNodeView('graph')}
                    style={{ fontSize: 11 }}
                  >
                    <IconGraph aria-hidden style={{ verticalAlign: '-2px', marginRight: 3 }} />{zh ? '图' : 'Graph'}
                  </button>
                  <input
                    placeholder={zh ? '搜索节点或输出…' : 'Search nodes or output…'}
                    value={nodeSearch}
                    onChange={(e) => setNodeSearch(e.target.value)}
                    style={{ fontSize: 12, padding: '2px 6px', width: 160 }}
                  />
                  <select
                    value={nodeFilter}
                    onChange={(e) => setNodeFilter(e.target.value)}
                    style={{ fontSize: 12, padding: '2px 6px' }}
                  >
                    <option value="all">{zh ? '全部状态' : 'All statuses'}</option>
                    <option value="succeeded">{zh ? '成功' : 'Succeeded'}</option>
                    <option value="failed">{zh ? '失败' : 'Failed'}</option>
                    <option value="skipped">{zh ? '跳过' : 'Skipped'}</option>
                    <option value="running">{zh ? '运行中' : 'Running'}</option>
                  </select>
                  {(() => {
                    const types = Array.from(new Set(record.node_results.map(nr => nr.node_type))).sort()
                    if (types.length < 3) return null
                    return (
                      <select
                        value={nodeTypeFilter}
                        onChange={(e) => setNodeTypeFilter(e.target.value)}
                        style={{ fontSize: 12, padding: '2px 6px' }}
                      >
                        <option value="all">{zh ? '全部类型' : 'All types'}</option>
                        {types.map(t => <option key={t} value={t}>{t}</option>)}
                      </select>
                    )
                  })()}
                </div>
              </div>

              {record.node_results.length === 0 ? (
                <p style={{ color: 'var(--muted)' }}>{zh ? '暂无节点结果。' : 'No node results yet.'}</p>
              ) : nodeView === 'log' ? (
                /* ── Log Console ── */
                (() => {
                  const logLines = [...record.node_results].sort((a, b) => {
                    // sort by retry_count then node_id as stable key
                    return a.node_id.localeCompare(b.node_id)
                  })
                  const statusIcon = (s: string) =>
                    s === 'succeeded' ? '✓' : s === 'failed' ? '✗' : s === 'skipped' ? '⤳' : s === 'running' ? '⟳' : '?'
                  const statusColor = (s: string) =>
                    s === 'succeeded' ? '#22c55e' : s === 'failed' ? '#ef4444' : s === 'skipped' ? '#6b7280' : s === 'running' ? '#3b82f6' : '#9ca3af'

                  const allText = logLines.map((nr) => {
                    const dur = nr.duration_ms != null ? (nr.duration_ms < 1000 ? `${nr.duration_ms}ms` : `${(nr.duration_ms / 1000).toFixed(2)}s`) : ''
                    const retry = (nr.retry_count ?? 0) > 0 ? ` ↺${nr.retry_count}` : ''
                    const errPart = nr.error ? ` | ERROR: ${nr.error}` : ''
                    return `[${nr.node_type}] ${nr.node_id}: ${nr.status}${retry}${dur ? ` ← ${dur}` : ''}${errPart}`
                  }).join('\n')

                  return (
                    <div style={{ position: 'relative' }}>
                      <button
                        className="btn btn-sm"
                        style={{ position: 'absolute', top: 8, right: 8, fontSize: 11, zIndex: 1 }}
                        onClick={() => navigator.clipboard?.writeText(allText)}
                        title={zh ? '复制日志' : 'Copy log'}
                      >
                        ⎘ {zh ? '复制' : 'Copy'}
                      </button>
                      <div style={{
                        background: theme === 'dark' ? '#0d1117' : '#1a1a2e',
                        borderRadius: 6,
                        fontFamily: 'monospace',
                        fontSize: 12,
                        padding: '12px 14px',
                        overflowY: 'auto',
                        maxHeight: 480,
                        lineHeight: 1.7,
                      }}>
                        {logLines.map((nr, i) => {
                          const dur = nr.duration_ms != null
                            ? (nr.duration_ms < 1000 ? `${nr.duration_ms}ms` : `${(nr.duration_ms / 1000).toFixed(2)}s`)
                            : null
                          const retry = (nr.retry_count ?? 0) > 0 ? nr.retry_count : null
                          return (
                            <div key={nr.node_id} style={{ display: 'flex', gap: 8, alignItems: 'flex-start', borderBottom: i < logLines.length - 1 ? '1px solid rgba(255,255,255,0.04)' : undefined, padding: '2px 0' }}>
                              <span style={{ color: '#64748b', userSelect: 'none', minWidth: 24, textAlign: 'right', flexShrink: 0 }}>{i + 1}</span>
                              <span style={{ color: statusColor(nr.status), fontWeight: 700, flexShrink: 0, width: 14 }}>{statusIcon(nr.status)}</span>
                              <span style={{ color: '#93c5fd', flexShrink: 0 }}>[{nr.node_type}]</span>
                              <span style={{ color: '#e2e8f0', flexShrink: 0 }}>{nr.node_id}</span>
                              <span style={{ color: '#94a3b8' }}>:</span>
                              <span style={{ color: statusColor(nr.status) }}>{nr.status}</span>
                              {retry && <span style={{ color: '#fb923c', flexShrink: 0 }}>↺{retry}</span>}
                              {dur && <span style={{ color: '#64748b' }}>← {dur}</span>}
                              {nr.error && (
                                <span style={{ color: '#f87171', wordBreak: 'break-all' }}>| {nr.error}</span>
                              )}
                            </div>
                          )
                        })}
                        <div ref={logBottomRef} />
                      </div>
                    </div>
                  )
                })()
              ) : nodeView === 'graph' ? (
                /* ── Graph view ── */
                <ExecutionGraph record={record} />
              ) : (
                /* ── Cards view ── */
                <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
                  {(() => {
                    const q = nodeSearch.trim().toLowerCase()
                    const filtered = record.node_results.filter((nr) => {
                      const matchesStatus = nodeFilter === 'all' || nr.status === nodeFilter
                      const matchesType = nodeTypeFilter === 'all' || nr.node_type === nodeTypeFilter
                      const matchesSearch = !q || nr.node_id.toLowerCase().includes(q) ||
                        (nr.output_json?.toLowerCase().includes(q)) ||
                        (nr.error?.toLowerCase().includes(q))
                      return matchesStatus && matchesType && matchesSearch
                    })
                    if (filtered.length === 0) {
                      return (
                        <p style={{ color: 'var(--muted)', margin: 0 }}>
                          {zh
                            ? `当前筛选条件下无匹配节点${nodeSearch ? `（"${nodeSearch}"）` : ''}。`
                            : `No nodes match the current filters${nodeSearch ? ` ("${nodeSearch}")` : ''}.`}
                        </p>
                      )
                    }
                    return filtered.map((nr) => <NodeResultCard key={nr.node_id} nr={nr} />)
                  })()}
                </div>
              )}
            </section>

            {/* ── Audit Trail ── */}
            {auditEvents.length > 0 && (
              <section>
                <h2 style={{ marginBottom: 10 }}>{zh ? '审计追踪' : 'Audit Trail'}</h2>
                <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 12 }}>
                  <tbody>
                    {auditEvents.map((ev) => (
                      <tr key={ev.id} style={{ borderBottom: '1px solid var(--border)' }}>
                        <td style={{ padding: '5px 8px', color: 'var(--muted)', whiteSpace: 'nowrap', fontFamily: 'monospace', fontSize: 11 }}>
                          {new Date(ev.timestamp * 1000).toLocaleTimeString()}
                        </td>
                        <td style={{ padding: '5px 8px', fontWeight: 600, color: 'var(--link)', whiteSpace: 'nowrap' }}>
                          {ev.action}
                        </td>
                        <td style={{ padding: '5px 8px', color: 'var(--muted)', maxWidth: 300, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                          {ev.detail ?? '—'}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </section>
            )}
          </>
        )}
      </main>

      {/* ── Replay modal ── */}
      {showReplay && record && (
        <div className="modal-overlay" onClick={() => setShowReplay(false)}>
          <div className="modal" style={{ maxWidth: 560 }} onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h2>{zh ? '修改输入后回放' : 'Replay with Modified Input'}</h2>
              <button className="btn btn-sm btn-icon" onClick={() => setShowReplay(false)}>✕</button>
            </div>
            <div className="modal-body">
              <p style={{ fontSize: 12, color: 'var(--muted)', marginBottom: 8 }}>
                {zh
                  ? <>编辑输入 JSON 并重新运行相同工作流版本（<code>{record.workflow_version_id.slice(-12)}</code>）。</>
                  : <>Edit the input JSON and re-run the same workflow version (<code>{record.workflow_version_id.slice(-12)}</code>).</>}
              </p>
              <textarea
                rows={12}
                style={{ width: '100%', fontFamily: 'monospace', fontSize: 12 }}
                value={replayInput}
                onChange={(e) => setReplayInput(e.target.value)}
                spellCheck={false}
              />
              {(() => {
                try {
                  const parsed = JSON.parse(replayInput)
                  const orig = (() => { try { return JSON.parse(record.input_json || '{}') } catch { return {} } })()
                  const allKeys = new Set([...Object.keys(orig), ...Object.keys(parsed)])
                  const diffs: Array<{ key: string; type: 'added' | 'removed' | 'changed'; from?: string; to?: string }> = []
                  for (const key of allKeys) {
                    const origVal = JSON.stringify(orig[key])
                    const newVal = JSON.stringify(parsed[key])
                    if (!(key in orig)) diffs.push({ key, type: 'added', to: newVal })
                    else if (!(key in parsed)) diffs.push({ key, type: 'removed', from: origVal })
                    else if (origVal !== newVal) diffs.push({ key, type: 'changed', from: origVal, to: newVal })
                  }
                  if (diffs.length === 0) return <div style={{ fontSize: 11, color: 'var(--muted)', marginTop: 4 }}>✓ {zh ? '与原始输入相同' : 'Same as original input'}</div>
                  return (
                    <div style={{ marginTop: 8, fontSize: 11, fontFamily: 'monospace', background: 'var(--surface)', border: '1px solid var(--border)', borderRadius: 4, padding: '6px 8px', maxHeight: 120, overflowY: 'auto' }}>
                      {diffs.map(d => (
                        <div key={d.key} style={{ color: d.type === 'added' ? '#22c55e' : d.type === 'removed' ? '#ef4444' : '#f59e0b' }}>
                          {d.type === 'added' && `+ ${d.key}: ${d.to}`}
                          {d.type === 'removed' && `- ${d.key}: ${d.from}`}
                          {d.type === 'changed' && `~ ${d.key}: ${d.from} → ${d.to}`}
                        </div>
                      ))}
                    </div>
                  )
                } catch {
                  return <div style={{ color: 'var(--danger-text)', fontSize: 11, marginTop: 4 }}>⚠ {zh ? 'JSON 格式无效' : 'Invalid JSON'}</div>
                }
              })()}
            </div>
            <div className="modal-footer">
              <button className="btn btn-secondary" onClick={() => setShowReplay(false)}>{zh ? '取消' : 'Cancel'}</button>
              <button
                className="btn btn-primary"
                disabled={replaying}
                onClick={handleReplay}
              >
                {replaying ? (zh ? '启动中…' : 'Starting…') : (zh ? '▶ 运行' : '▶ Run')}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
