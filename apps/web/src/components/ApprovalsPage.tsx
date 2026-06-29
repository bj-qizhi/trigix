// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useEffect, useRef, useState } from 'react'
import { useAuth } from '../AuthContext'
import * as api from '../api/client'
import type { ExecutionSummary } from '../types'
import { useLocale } from '../useLocale'
import { useToast } from '../toast'
import { SkeletonRows } from './Skeleton'
import { IconCheck, IconX } from './uiIcons'

interface Props {
  onBack: () => void
  onOpenExecution?: (id: string) => void
  onOpenWorkflow?: (id: string) => void
}

function formatWait(startedAt: number): string {
  const secs = Math.floor(Date.now() / 1000) - startedAt
  if (secs < 60) return `${secs}s`
  if (secs < 3600) return `${Math.floor(secs / 60)}m ${secs % 60}s`
  return `${Math.floor(secs / 3600)}h ${Math.floor((secs % 3600) / 60)}m`
}

function formatAge(startedAt: number, zh = false): string {
  const diff = Math.floor(Date.now() / 1000) - startedAt
  if (zh) {
    if (diff < 60) return `${diff}秒前`
    if (diff < 3600) return `${Math.floor(diff / 60)}分钟前`
    return `${Math.floor(diff / 3600)}小时前`
  }
  if (diff < 60) return `${diff}s ago`
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`
  return `${Math.floor(diff / 3600)}h ago`
}

export function ApprovalsPage({ onBack, onOpenExecution, onOpenWorkflow }: Props) {
  const { auth } = useAuth()
  const { locale } = useLocale()
  const zh = locale === 'zh'

  const [pending, setPending] = useState<ExecutionSummary[]>([])
  const [wfNames, setWfNames] = useState<Record<string, string>>({})
  const [loading, setLoading] = useState(true)
  const [commentFor, setCommentFor] = useState<string | null>(null)
  const [commentText, setCommentText] = useState('')
  const [acting, setActing] = useState<string | null>(null)
  const toast = useToast()
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null)

  const load = async () => {
    try {
      const result = await api.listExecutionsPage(auth!.tenantId, { status: 'waiting_approval', limit: 100 })
      const execs = result.data
      setPending(execs)
      // fetch workflow names we don't have yet
      const missingWfIds = [...new Set(execs.map((e) => e.workflow_id))].filter((id) => !wfNames[id])
      if (missingWfIds.length > 0) {
        const workflows = await api.listWorkflows(auth!.tenantId, auth!.projectId)
        const nameMap: Record<string, string> = {}
        for (const wf of workflows) nameMap[wf.id] = wf.name
        setWfNames((prev) => ({ ...prev, ...nameMap }))
      }
    } catch {
      // silently continue on refresh failure
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    load()
    timerRef.current = setInterval(load, 10_000)
    return () => {
      if (timerRef.current) clearInterval(timerRef.current)
    }
  }, [])

  const handleApprove = async (execId: string) => {
    setActing(execId)
    try {
      await api.approveExecution(execId, commentText || undefined)
      toast.success(zh ? '已批准执行' : 'Execution approved')
      setCommentFor(null)
      setCommentText('')
      await load()
    } catch (e) {
      toast.error(String(e))
    } finally {
      setActing(null)
    }
  }

  const handleReject = async (execId: string) => {
    setActing(execId)
    try {
      await api.rejectExecution(execId, commentText || undefined)
      toast.success(zh ? '已拒绝执行' : 'Execution rejected')
      setCommentFor(null)
      setCommentText('')
      await load()
    } catch (e) {
      toast.error(String(e))
    } finally {
      setActing(null)
    }
  }

  const openComment = (execId: string) => {
    if (commentFor === execId) {
      setCommentFor(null)
      setCommentText('')
    } else {
      setCommentFor(execId)
      setCommentText('')
    }
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', background: 'var(--bg)' }}>
      {/* Topbar */}
      <header className="topbar" style={{ display: 'flex', alignItems: 'center', gap: 12, padding: '0 16px' }}>
        <button className="btn btn-sm" onClick={onBack}>
          ← {zh ? '返回' : 'Back'}
        </button>
        <span className="topbar-title" style={{ fontWeight: 600 }}>
          {zh ? '审批队列' : 'Approval Queue'}
        </span>
        {!loading && (
          <span style={{
            background: pending.length > 0 ? 'var(--approval-text)' : 'var(--muted)',
            color: pending.length > 0 ? 'var(--bg)' : 'var(--fg)',
            borderRadius: 12,
            padding: '2px 10px',
            fontSize: 12,
            fontWeight: 700,
          }}>
            {pending.length} {zh ? '待审批' : 'pending'}
          </span>
        )}
        <span style={{ marginLeft: 'auto', fontSize: 11, color: 'var(--muted)' }}>
          {zh ? '每 10 秒自动刷新' : 'Auto-refreshes every 10s'}
        </span>
      </header>

      <div style={{ flex: 1, overflow: 'auto', padding: 24 }}>
        {loading ? (
          <SkeletonRows rows={5} />
        ) : pending.length === 0 ? (
          <div style={{ textAlign: 'center', paddingTop: 80 }}>
            <div style={{ fontSize: 48, marginBottom: 16 }}>✓</div>
            <div style={{ fontSize: 18, fontWeight: 600, color: 'var(--success-text)' }}>
              {zh ? '暂无待审批的执行' : 'No pending approvals'}
            </div>
            <div style={{ color: 'var(--muted)', marginTop: 8 }}>
              {zh ? '当执行到达人工审批节点时，将会出现在这里' : 'Executions will appear here when they reach a human approval node'}
            </div>
          </div>
        ) : (
          <div>
            <div style={{ marginBottom: 16, color: 'var(--muted)', fontSize: 13 }}>
              {zh
                ? `${pending.length} 个执行正在等待您的审批决策。`
                : `${pending.length} execution${pending.length !== 1 ? 's' : ''} awaiting your approval decision.`}
            </div>
            <table className="data-table" style={{ width: '100%' }}>
              <thead>
                <tr>
                  <th>{zh ? '工作流' : 'Workflow'}</th>
                  <th>{zh ? '执行 ID' : 'Execution ID'}</th>
                  <th>{zh ? '开始时间' : 'Started'}</th>
                  <th>{zh ? '等待时长' : 'Waiting'}</th>
                  <th>{zh ? '触发方式' : 'Trigger'}</th>
                  <th>{zh ? '标签' : 'Label'}</th>
                  <th style={{ textAlign: 'right' }}>{zh ? '操作' : 'Actions'}</th>
                </tr>
              </thead>
              <tbody>
                {pending.map((exec) => (
                  <>
                    <tr key={exec.id} style={{ verticalAlign: 'middle' }}>
                      <td>
                        <span
                          style={{ color: 'var(--link)', cursor: onOpenWorkflow ? 'pointer' : 'default', fontWeight: 500 }}
                          onClick={() => onOpenWorkflow?.(exec.workflow_id)}
                          title={exec.workflow_id}
                        >
                          {wfNames[exec.workflow_id] ?? exec.workflow_id.slice(0, 12) + '…'}
                        </span>
                      </td>
                      <td>
                        <span
                          style={{ fontFamily: 'monospace', fontSize: 12, color: 'var(--link)', cursor: onOpenExecution ? 'pointer' : 'default' }}
                          onClick={() => onOpenExecution?.(exec.id)}
                          title={exec.id}
                        >
                          {exec.id.slice(0, 16)}…
                        </span>
                      </td>
                      <td style={{ fontSize: 12, color: 'var(--muted)' }}>
                        <span title={new Date(exec.started_at * 1000).toLocaleString()}>
                          {formatAge(exec.started_at, zh)}
                        </span>
                      </td>
                      <td>
                        <span style={{
                          fontFamily: 'monospace',
                          fontSize: 12,
                          color: Date.now() / 1000 - exec.started_at > 3600 ? 'var(--danger-text)' : Date.now() / 1000 - exec.started_at > 600 ? '#d97706' : 'var(--fg)',
                          fontWeight: Date.now() / 1000 - exec.started_at > 600 ? 600 : 400,
                        }}>
                          {formatWait(exec.started_at)}
                        </span>
                      </td>
                      <td>
                        {exec.trigger_type
                          ? <span className={`badge badge-${exec.trigger_type}`} style={{ fontSize: 10 }}>{exec.trigger_type}</span>
                          : <span style={{ color: 'var(--muted)', fontSize: 12 }}>—</span>}
                      </td>
                      <td style={{ fontSize: 12, color: 'var(--muted)', maxWidth: 120, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                        {exec.label ?? '—'}
                      </td>
                      <td>
                        <div style={{ display: 'flex', gap: 6, justifyContent: 'flex-end', alignItems: 'center' }}>
                          <button
                            className="btn btn-sm"
                            style={{ fontSize: 11 }}
                            onClick={() => openComment(exec.id)}
                          >
                            {commentFor === exec.id ? (zh ? '▲ 收起' : '▲ Hide') : (zh ? '备注' : 'Comment')}
                          </button>
                          <button
                            className="btn btn-sm btn-primary"
                            style={{ fontSize: 11 }}
                            disabled={acting === exec.id}
                            onClick={() => {
                              if (commentFor === exec.id) {
                                handleApprove(exec.id)
                              } else {
                                setCommentFor(null)
                                handleApprove(exec.id)
                              }
                            }}
                          >
                            {acting === exec.id ? '…' : <><IconCheck aria-hidden style={{ verticalAlign: '-2px', marginRight: 3 }} />{zh ? '批准' : 'Approve'}</>}
                          </button>
                          <button
                            className="btn btn-sm btn-danger"
                            style={{ fontSize: 11 }}
                            disabled={acting === exec.id}
                            onClick={() => {
                              if (commentFor === exec.id) {
                                handleReject(exec.id)
                              } else {
                                setCommentFor(null)
                                handleReject(exec.id)
                              }
                            }}
                          >
                            {acting === exec.id ? '…' : <><IconX aria-hidden style={{ verticalAlign: '-2px', marginRight: 3 }} />{zh ? '拒绝' : 'Reject'}</>}
                          </button>
                        </div>
                      </td>
                    </tr>
                    {commentFor === exec.id && (
                      <tr key={`${exec.id}-comment`} style={{ background: 'var(--surface)' }}>
                        <td colSpan={7} style={{ padding: '8px 16px' }}>
                          <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
                            <input
                              autoFocus
                              placeholder={zh ? '审批备注（可选）…' : 'Approval comment (optional)…'}
                              value={commentText}
                              onChange={(e) => setCommentText(e.target.value)}
                              onKeyDown={(e) => {
                                if (e.key === 'Enter' && !e.shiftKey) handleApprove(exec.id)
                                if (e.key === 'Escape') { setCommentFor(null); setCommentText('') }
                              }}
                              style={{ flex: 1, fontSize: 12, padding: '4px 8px' }}
                            />
                            <button className="btn btn-sm btn-primary" onClick={() => handleApprove(exec.id)} disabled={acting === exec.id}>
                              <IconCheck aria-hidden style={{ verticalAlign: '-2px', marginRight: 3 }} />{zh ? '批准' : 'Approve'}
                            </button>
                            <button className="btn btn-sm btn-danger" onClick={() => handleReject(exec.id)} disabled={acting === exec.id}>
                              <IconX aria-hidden style={{ verticalAlign: '-2px', marginRight: 3 }} />{zh ? '拒绝' : 'Reject'}
                            </button>
                          </div>
                          <div style={{ fontSize: 11, color: 'var(--muted)', marginTop: 4 }}>
                            {zh ? 'Enter 批准 · Escape 取消' : 'Enter to approve · Escape to cancel'}
                          </div>
                        </td>
                      </tr>
                    )}
                  </>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>
    </div>
  )
}
