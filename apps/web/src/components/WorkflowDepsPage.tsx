// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useEffect, useMemo, useState } from 'react'
import { useAuth } from '../AuthContext'
import * as api from '../api/client'
import type { WorkflowRecord } from '../types'
import { useLocale } from '../useLocale'

interface Props {
  onBack: () => void
  onOpenWorkflow: (id: string) => void
}

export function WorkflowDepsPage({ onBack, onOpenWorkflow }: Props) {
  const { auth } = useAuth()
  const { locale } = useLocale()
  const zh = locale === 'zh'
  const [deps, setDeps] = useState<api.WorkflowDepsResponse | null>(null)
  const [workflows, setWorkflows] = useState<WorkflowRecord[]>([])
  const [loading, setLoading] = useState(true)
  const [search, setSearch] = useState('')
  const [selectedId, setSelectedId] = useState<string | null>(null)

  useEffect(() => {
    if (!auth) return
    setLoading(true)
    Promise.all([
      api.getWorkflowDeps(auth.tenantId),
      api.listWorkflows(auth.tenantId, auth.workspaceId, auth.projectId),
    ]).then(([d, wfs]) => {
      setDeps(d)
      setWorkflows(wfs)
    }).catch(() => {}).finally(() => setLoading(false))
  }, [auth])

  const nameMap = useMemo(() => {
    const m = new Map<string, string>()
    workflows.forEach((w) => m.set(w.id, w.name))
    return m
  }, [workflows])

  // Compute which workflows have deps
  const allIds = useMemo(() => {
    if (!deps) return new Set<string>()
    const s = new Set<string>()
    deps.edges.forEach((e) => { s.add(e.from_workflow_id); s.add(e.to_workflow_id) })
    return s
  }, [deps])

  const filteredWorkflows = useMemo(() => {
    return workflows.filter((w) => {
      if (!allIds.has(w.id)) return false
      if (!search) return true
      return w.name.toLowerCase().includes(search.toLowerCase())
    })
  }, [workflows, allIds, search])

  // For selected workflow, compute callers (who calls it) and callees (who it calls)
  const selectedDeps = useMemo(() => {
    if (!selectedId || !deps) return null
    const callers = deps.edges.filter((e) => e.to_workflow_id === selectedId)
    const callees = deps.edges.filter((e) => e.from_workflow_id === selectedId)
    return { callers, callees }
  }, [selectedId, deps])

  // Compute in-degree and out-degree for each workflow
  const degreeMap = useMemo(() => {
    const m = new Map<string, { in: number; out: number }>()
    if (!deps) return m
    deps.edges.forEach((e) => {
      const from = m.get(e.from_workflow_id) ?? { in: 0, out: 0 }
      from.out++
      m.set(e.from_workflow_id, from)
      const to = m.get(e.to_workflow_id) ?? { in: 0, out: 0 }
      to.in++
      m.set(e.to_workflow_id, to)
    })
    return m
  }, [deps])

  return (
    <div className="app">
      <header className="topbar">
        <button className="btn btn-sm" onClick={onBack}>← {zh ? '返回' : 'Back'}</button>
        <span className="topbar-logo" style={{ fontSize: 15 }}>
          {zh ? '工作流依赖图' : 'Workflow Dependency Graph'}
        </span>
        <span style={{ fontSize: 12, color: 'var(--muted)', marginLeft: 8 }}>
          {deps ? (zh ? `${deps.edges.length} 条依赖` : `${deps.edges.length} dep${deps.edges.length !== 1 ? 's' : ''}`) : ''}
        </span>
      </header>

      {loading && <div style={{ padding: 32, textAlign: 'center', color: 'var(--muted)' }}>{zh ? '加载中…' : 'Loading…'}</div>}

      {!loading && deps && (
        <div style={{ display: 'flex', height: 'calc(100vh - 48px)', overflow: 'hidden' }}>
          {/* Left: workflow list */}
          <div style={{ width: 280, flexShrink: 0, borderRight: '1px solid var(--border)', display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
            <div style={{ padding: '10px 12px', borderBottom: '1px solid var(--border)' }}>
              <input
                placeholder={zh ? '搜索工作流…' : 'Search workflows…'}
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                style={{ width: '100%', fontSize: 12, padding: '4px 8px', boxSizing: 'border-box' }}
              />
            </div>
            {filteredWorkflows.length === 0 && (
              <div style={{ padding: 24, textAlign: 'center', color: 'var(--muted)', fontSize: 13 }}>
                {deps.edges.length === 0
                  ? (zh ? '没有检测到工作流依赖。使用 SubWorkflow 或 ForEach 节点来建立依赖。' : 'No workflow dependencies detected. Use SubWorkflow or ForEach nodes to create dependencies.')
                  : (zh ? '无匹配结果' : 'No matches')
                }
              </div>
            )}
            <div style={{ overflowY: 'auto', flex: 1 }}>
              {filteredWorkflows.map((w) => {
                const deg = degreeMap.get(w.id)
                const isSelected = w.id === selectedId
                return (
                  <div
                    key={w.id}
                    onClick={() => setSelectedId(isSelected ? null : w.id)}
                    style={{
                      padding: '10px 14px',
                      cursor: 'pointer',
                      background: isSelected ? 'var(--panel)' : undefined,
                      borderBottom: '1px solid var(--border)',
                      borderLeft: isSelected ? '3px solid var(--link)' : '3px solid transparent',
                    }}
                  >
                    <div style={{ fontSize: 13, fontWeight: 500, marginBottom: 4, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{w.name}</div>
                    <div style={{ display: 'flex', gap: 8, fontSize: 11, color: 'var(--text-secondary)' }}>
                      {deg?.out != null && deg.out > 0 && (
                        <span style={{ color: 'var(--link)' }}>→ {deg.out} {zh ? '调用' : 'call' + (deg.out !== 1 ? 's' : '')}</span>
                      )}
                      {deg?.in != null && deg.in > 0 && (
                        <span style={{ color: 'var(--success-text)' }}>← {deg.in} {zh ? '被调用' : 'caller' + (deg.in !== 1 ? 's' : '')}</span>
                      )}
                      <span className={`badge badge-${w.status}`}>{w.status}</span>
                    </div>
                  </div>
                )
              })}
            </div>
          </div>

          {/* Right: detail panel or graph overview */}
          <div style={{ flex: 1, overflowY: 'auto', padding: 24 }}>
            {selectedId && selectedDeps ? (
              <>
                <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 20 }}>
                  <h2 style={{ margin: 0, fontSize: 18 }}>{nameMap.get(selectedId) ?? selectedId.slice(0, 8)}</h2>
                  <button className="btn btn-sm btn-primary" onClick={() => onOpenWorkflow(selectedId)}>
                    {zh ? '打开编辑器' : 'Open Editor'}
                  </button>
                  <button className="btn btn-sm" onClick={() => setSelectedId(null)}>✕</button>
                </div>

                {selectedDeps.callees.length > 0 && (
                  <section style={{ marginBottom: 24 }}>
                    <h3 style={{ fontSize: 13, fontWeight: 600, color: 'var(--text-secondary)', marginBottom: 12 }}>
                      {zh ? '调用的工作流' : 'Calls these workflows'}
                    </h3>
                    <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
                      {selectedDeps.callees.map((e, i) => (
                        <div key={i} style={{ display: 'flex', alignItems: 'center', gap: 12, padding: '10px 14px', background: 'var(--bg-secondary)', borderRadius: 8, border: '1px solid var(--border)' }}>
                          <span style={{ fontSize: 18 }}>→</span>
                          <div style={{ flex: 1 }}>
                            <div style={{ fontSize: 13, fontWeight: 500 }}>
                              {nameMap.get(e.to_workflow_id) ?? e.to_workflow_id.slice(0, 8) + '…'}
                            </div>
                            <div style={{ fontSize: 11, color: 'var(--text-secondary)' }}>
                              {zh ? '通过' : 'via'} <code style={{ fontSize: 11 }}>{e.node_type}</code>
                            </div>
                          </div>
                          <button className="btn btn-sm" onClick={() => { setSelectedId(e.to_workflow_id); }}>
                            {zh ? '查看' : 'View'}
                          </button>
                          <button className="btn btn-sm btn-primary" onClick={() => onOpenWorkflow(e.to_workflow_id)}>
                            {zh ? '打开' : 'Open'}
                          </button>
                        </div>
                      ))}
                    </div>
                  </section>
                )}

                {selectedDeps.callers.length > 0 && (
                  <section style={{ marginBottom: 24 }}>
                    <h3 style={{ fontSize: 13, fontWeight: 600, color: 'var(--text-secondary)', marginBottom: 12 }}>
                      {zh ? '被以下工作流调用' : 'Called by these workflows'}
                    </h3>
                    <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
                      {selectedDeps.callers.map((e, i) => (
                        <div key={i} style={{ display: 'flex', alignItems: 'center', gap: 12, padding: '10px 14px', background: 'var(--bg-secondary)', borderRadius: 8, border: '1px solid var(--border)' }}>
                          <span style={{ fontSize: 18 }}>←</span>
                          <div style={{ flex: 1 }}>
                            <div style={{ fontSize: 13, fontWeight: 500 }}>
                              {nameMap.get(e.from_workflow_id) ?? e.from_workflow_id.slice(0, 8) + '…'}
                            </div>
                            <div style={{ fontSize: 11, color: 'var(--text-secondary)' }}>
                              {zh ? '通过' : 'via'} <code style={{ fontSize: 11 }}>{e.node_type}</code>
                            </div>
                          </div>
                          <button className="btn btn-sm" onClick={() => setSelectedId(e.from_workflow_id)}>
                            {zh ? '查看' : 'View'}
                          </button>
                          <button className="btn btn-sm btn-primary" onClick={() => onOpenWorkflow(e.from_workflow_id)}>
                            {zh ? '打开' : 'Open'}
                          </button>
                        </div>
                      ))}
                    </div>
                  </section>
                )}

                {selectedDeps.callers.length === 0 && selectedDeps.callees.length === 0 && (
                  <div style={{ color: 'var(--muted)', fontSize: 13 }}>
                    {zh ? '此工作流没有依赖关系。' : 'This workflow has no dependencies.'}
                  </div>
                )}
              </>
            ) : (
              <>
                <h2 style={{ margin: '0 0 20px', fontSize: 18 }}>{zh ? '所有依赖关系' : 'All Dependencies'}</h2>
                {deps.edges.length === 0 ? (
                  <div style={{ color: 'var(--muted)', fontSize: 14, padding: '24px 0' }}>
                    {zh ? '暂无工作流之间的依赖关系。' : 'No workflow dependencies found.'}
                  </div>
                ) : (
                  <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 13 }}>
                    <thead>
                      <tr style={{ borderBottom: '1px solid var(--border)' }}>
                        <th style={{ textAlign: 'left', padding: '6px 10px', color: 'var(--text-secondary)', fontWeight: 500 }}>{zh ? '调用方' : 'Caller'}</th>
                        <th style={{ textAlign: 'center', padding: '6px 10px', width: 60 }}></th>
                        <th style={{ textAlign: 'left', padding: '6px 10px', color: 'var(--text-secondary)', fontWeight: 500 }}>{zh ? '被调用方' : 'Callee'}</th>
                        <th style={{ textAlign: 'left', padding: '6px 10px', color: 'var(--text-secondary)', fontWeight: 500 }}>{zh ? '节点类型' : 'Node Type'}</th>
                      </tr>
                    </thead>
                    <tbody>
                      {deps.edges.map((e, i) => (
                        <tr key={i} style={{ borderBottom: '1px solid var(--border)', cursor: 'pointer' }} onClick={() => setSelectedId(e.from_workflow_id)}>
                          <td style={{ padding: '8px 10px' }}>
                            <span style={{ color: 'var(--link)', textDecoration: 'underline' }}>
                              {nameMap.get(e.from_workflow_id) ?? e.from_workflow_id.slice(0, 8) + '…'}
                            </span>
                          </td>
                          <td style={{ padding: '8px 10px', textAlign: 'center', color: 'var(--muted)' }}>→</td>
                          <td style={{ padding: '8px 10px' }}>
                            <span
                              style={{ color: 'var(--link)', textDecoration: 'underline', cursor: 'pointer' }}
                              onClick={(ev) => { ev.stopPropagation(); setSelectedId(e.to_workflow_id) }}
                            >
                              {nameMap.get(e.to_workflow_id) ?? e.to_workflow_id.slice(0, 8) + '…'}
                            </span>
                          </td>
                          <td style={{ padding: '8px 10px' }}><span className="badge">{e.node_type}</span></td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                )}
              </>
            )}
          </div>
        </div>
      )}
    </div>
  )
}
