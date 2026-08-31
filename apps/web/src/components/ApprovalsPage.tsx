// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { Fragment, useEffect, useRef, useState } from 'react'
import { useAuth } from '../AuthContext'
import * as api from '../api/client'
import type { DesktopApprovalSummary, DesktopEvidenceRecord, ExecutionSummary } from '../types'
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
  const [desktopPending, setDesktopPending] = useState<DesktopApprovalSummary[]>([])
  const [wfNames, setWfNames] = useState<Record<string, string>>({})
  const [loading, setLoading] = useState(true)
  const [commentFor, setCommentFor] = useState<string | null>(null)
  const [commentText, setCommentText] = useState('')
  const [acting, setActing] = useState<string | null>(null)
  const [desktopDecision, setDesktopDecision] = useState<{ commandId: string; decision: 'approve' | 'reject' } | null>(null)
  const [loadError, setLoadError] = useState('')
  const [evidenceExecutionId, setEvidenceExecutionId] = useState('')
  const [evidence, setEvidence] = useState<DesktopEvidenceRecord[]>([])
  const [evidenceLoading, setEvidenceLoading] = useState(false)
  const toast = useToast()
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null)

  const load = async () => {
    try {
      setLoadError('')
      const [result, desktopApprovals] = await Promise.all([
        api.listExecutionsPage(auth!.tenantId, { status: 'waiting_approval', limit: 100 }),
        auth?.role === 'admin'
          ? api.listDesktopApprovals(auth.tenantId).catch(() => {
            setLoadError(zh ? '桌面审批暂时无法刷新。' : 'Desktop approvals could not be refreshed.')
            return []
          })
          : Promise.resolve([]),
      ])
      const execs = result.data
      setPending(execs)
      setDesktopPending(desktopApprovals)
      // fetch workflow names we don't have yet
      const missingWfIds = [...new Set(execs.map((e) => e.workflow_id))].filter((id) => !wfNames[id])
      if (missingWfIds.length > 0) {
        const workflows = await api.listWorkflows(auth!.tenantId, auth!.projectId)
        const nameMap: Record<string, string> = {}
        for (const wf of workflows) nameMap[wf.id] = wf.name
        setWfNames((prev) => ({ ...prev, ...nameMap }))
      }
    } catch {
      setLoadError(zh ? '无法刷新审批队列，请检查连接并重试。' : 'Could not refresh approvals. Check the connection and retry.')
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

  const handleDesktopDecision = async () => {
    if (!desktopDecision) return
    setActing(desktopDecision.commandId)
    try {
      await api.decideDesktopApproval(auth!.tenantId, desktopDecision.commandId, desktopDecision.decision)
      toast.success(desktopDecision.decision === 'approve'
        ? (zh ? '桌面命令已批准' : 'Desktop command approved')
        : (zh ? '桌面命令已拒绝' : 'Desktop command rejected'))
      setDesktopDecision(null)
      await load()
    } catch (error) {
      toast.error(String(error))
    } finally {
      setActing(null)
    }
  }

  const loadEvidence = async () => {
    const executionId = evidenceExecutionId.trim()
    if (!executionId) return
    setEvidenceLoading(true)
    try {
      setEvidence(await api.listDesktopEvidence(auth!.tenantId, executionId))
    } catch (error) {
      toast.error(String(error))
    } finally {
      setEvidenceLoading(false)
    }
  }

  const exportEvidence = async () => {
    const executionId = evidenceExecutionId.trim()
    if (!executionId) return
    try {
      const exported = await api.exportDesktopEvidence(auth!.tenantId, executionId)
      const blob = new Blob([JSON.stringify(exported, null, 2)], { type: 'application/json' })
      const url = URL.createObjectURL(blob)
      const link = document.createElement('a')
      link.href = url
      link.download = `desktop-evidence-${executionId}.json`
      link.click()
      URL.revokeObjectURL(url)
    } catch (error) {
      toast.error(String(error))
    }
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', background: 'var(--bg)' }}>
      {/* Topbar */}
      <header className="topbar" style={{ display: 'flex', alignItems: 'center', gap: 12, padding: '0 16px' }}>
        <button className="btn btn-sm" onClick={onBack}>
          ← {zh ? '返回' : 'Back'}
        </button>
        <h1 className="topbar-title" style={{ fontWeight: 600, fontSize: 14, margin: 0 }}>
          {zh ? '审批与桌面证据' : 'Approvals & desktop evidence'}
        </h1>
        {!loading && (
          <span style={{
            background: pending.length > 0 ? 'var(--approval-text)' : 'var(--muted)',
            color: pending.length > 0 ? 'var(--bg)' : 'var(--fg)',
            borderRadius: 12,
            padding: '2px 10px',
            fontSize: 12,
            fontWeight: 700,
          }}>
            {pending.length + desktopPending.length} {zh ? '待审批' : 'pending'}
          </span>
        )}
        <span style={{ marginLeft: 'auto', fontSize: 11, color: 'var(--muted)' }}>
          {zh ? '每 10 秒自动刷新' : 'Auto-refreshes every 10s'}
        </span>
      </header>

      <div style={{ flex: 1, overflow: 'auto', padding: 24 }}>
        <div role="status" aria-live="polite" style={{ minHeight: 20, color: loadError ? 'var(--danger-text)' : 'var(--muted)', fontSize: 12, marginBottom: 12 }}>
          {loadError || (!loading ? (zh ? `桌面命令 ${desktopPending.length} 条，工作流 ${pending.length} 条待审批` : `${desktopPending.length} Desktop and ${pending.length} Workflow approvals pending`) : '')}
        </div>
        {!loading && <section aria-labelledby="desktop-approvals-heading" style={{ marginBottom: 28 }}>
          <h2 id="desktop-approvals-heading" style={{ fontSize: 16, margin: '0 0 6px' }}>{zh ? '桌面命令审批' : 'Desktop command approvals'}</h2>
          <p style={{ color: 'var(--muted)', fontSize: 12, margin: '0 0 12px' }}>
            {zh ? '批准仅授权这一条命令和当前租约；界面不会显示输入文本、凭据或屏幕内容。' : 'Approval authorizes only this command and lease. Typed text, credentials, and screen content are never shown here.'}
          </p>
          {desktopPending.length === 0 ? <div style={{ border: '1px solid var(--border)', padding: 16, borderRadius: 6, color: 'var(--muted)' }}>{zh ? '没有待审批的桌面命令。' : 'No Desktop commands are waiting for approval.'}</div> : (
            <div style={{ display: 'grid', gap: 10 }}>
              {desktopPending.map((item) => <article key={item.command_id} style={{ border: '1px solid var(--border)', borderRadius: 6, padding: 14 }}>
                <div style={{ display: 'flex', gap: 12, alignItems: 'flex-start', flexWrap: 'wrap' }}>
                  <div style={{ flex: 1, minWidth: 240 }}>
                    <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
                      <strong>{item.action_kind}</strong>
                      <span className="badge" aria-label={`${item.risk} risk`} style={{ color: item.risk === 'high' || item.risk === 'critical' ? 'var(--danger-text)' : 'var(--approval-text)' }}>{item.risk.toUpperCase()}</span>
                    </div>
                    <div style={{ color: 'var(--muted)', fontSize: 12, marginTop: 5 }}>{item.reason}</div>
                    <dl style={{ display: 'grid', gridTemplateColumns: 'max-content 1fr', gap: '3px 10px', fontSize: 11, margin: '10px 0 0' }}>
                      <dt>{zh ? '设备' : 'Device'}</dt><dd style={{ margin: 0, fontFamily: 'monospace' }}>{item.device_id}</dd>
                      <dt>{zh ? '执行' : 'Execution'}</dt><dd style={{ margin: 0, fontFamily: 'monospace' }}>{item.execution_id}</dd>
                      <dt>{zh ? '到期' : 'Expires'}</dt><dd style={{ margin: 0 }}>{new Date(item.expires_at_unix_ms).toLocaleString()}</dd>
                    </dl>
                  </div>
                  <div style={{ display: 'flex', gap: 6 }}>
                    <button className="btn btn-sm btn-primary" disabled={acting === item.command_id} onClick={() => setDesktopDecision({ commandId: item.command_id, decision: 'approve' })}>{zh ? '审查并批准' : 'Review & approve'}</button>
                    <button className="btn btn-sm btn-danger" disabled={acting === item.command_id} onClick={() => setDesktopDecision({ commandId: item.command_id, decision: 'reject' })}>{zh ? '审查并拒绝' : 'Review & reject'}</button>
                  </div>
                </div>
                {desktopDecision?.commandId === item.command_id && <div role="alertdialog" aria-modal="false" aria-labelledby={`desktop-decision-${item.command_id}`} style={{ marginTop: 12, borderTop: '1px solid var(--border)', paddingTop: 12 }}>
                  <strong id={`desktop-decision-${item.command_id}`}>{desktopDecision.decision === 'approve' ? (zh ? '确认批准此命令？' : 'Approve this command?') : (zh ? '确认拒绝此命令？' : 'Reject this command?')}</strong>
                  <div style={{ color: 'var(--muted)', fontSize: 12, margin: '5px 0 10px' }}>{zh ? '决策会记录操作者，并且不能用于其他命令。' : 'The decision records the operator and cannot authorize another command.'}</div>
                  <button autoFocus className={`btn btn-sm ${desktopDecision.decision === 'approve' ? 'btn-primary' : 'btn-danger'}`} onClick={() => void handleDesktopDecision()} disabled={acting === item.command_id}>{zh ? '确认' : 'Confirm'}</button>
                  <button className="btn btn-sm" style={{ marginLeft: 6 }} onClick={() => setDesktopDecision(null)}>{zh ? '取消' : 'Cancel'}</button>
                </div>}
              </article>)}
            </div>
          )}
        </section>}

        {!loading && <section aria-labelledby="desktop-evidence-heading" style={{ marginBottom: 28 }}>
          <h2 id="desktop-evidence-heading" style={{ fontSize: 16, margin: '0 0 6px' }}>{zh ? '执行证据与诊断' : 'Execution evidence & diagnostics'}</h2>
          <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap', marginBottom: 10 }}>
            <label style={{ flex: '1 1 300px', display: 'flex', alignItems: 'center', gap: 8 }}><span style={{ fontSize: 12, whiteSpace: 'nowrap' }}>{zh ? '执行 ID' : 'Execution ID'}</span><input style={{ width: '100%' }} value={evidenceExecutionId} onChange={(event) => setEvidenceExecutionId(event.target.value.slice(0, 128))} placeholder={zh ? '输入执行 ID' : 'Enter an Execution ID'} /></label>
            <button className="btn btn-sm" disabled={evidenceLoading || !evidenceExecutionId.trim()} onClick={() => void loadEvidence()}>{evidenceLoading ? (zh ? '加载中…' : 'Loading…') : (zh ? '查看证据' : 'View evidence')}</button>
            <button className="btn btn-sm" disabled={!evidenceExecutionId.trim()} onClick={() => void exportEvidence()}>{zh ? '导出安全元数据' : 'Export safe metadata'}</button>
          </div>
          {evidence.length > 0 && <div role="region" aria-label={zh ? '桌面证据结果' : 'Desktop evidence results'} style={{ overflowX: 'auto' }}><table className="data-table" style={{ width: '100%' }}><thead><tr><th>{zh ? '类型' : 'Kind'}</th><th>{zh ? '应用' : 'Application'}</th><th>{zh ? '选择器策略' : 'Selector strategy'}</th><th>{zh ? '回退' : 'Fallback'}</th><th>{zh ? '结果' : 'Outcome'}</th><th>{zh ? '脱敏区域' : 'Redactions'}</th><th>{zh ? '保留至' : 'Retained until'}</th></tr></thead><tbody>{evidence.map((record) => <tr key={record.evidence_id}><td>{record.kind}</td><td>{record.application_id}</td><td>{record.selector_strategy}</td><td>{record.selector_fallback_used ? `${zh ? '是' : 'yes'} · ${record.selector_fallback_depth}` : (zh ? '否' : 'no')}</td><td>{record.outcome}</td><td>{record.redacted_regions}</td><td>{new Date(record.expires_at_unix_ms).toLocaleString()}</td></tr>)}</tbody></table></div>}
        </section>}

        {!loading && <h2 style={{ fontSize: 16, margin: '0 0 12px' }}>{zh ? '工作流审批' : 'Workflow approvals'}</h2>}
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
                  <Fragment key={exec.id}>
                    <tr style={{ verticalAlign: 'middle' }}>
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
                  </Fragment>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>
    </div>
  )
}
