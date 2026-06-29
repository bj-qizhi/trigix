// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useState } from 'react'
import type { WorkflowVersionRecord } from '../../types'
import { SkeletonRows } from '../Skeleton'

// Version-history modal: lists a workflow's versions, diffs any two of them
// (nodes added/removed/config-changed, edges added/removed), and loads or rolls
// back to a version. Extracted verbatim from WorkflowEditor; the diff/compare
// selection is modal-local UI state owned here rather than by the editor.

export interface VersionHistoryModalProps {
  versions: WorkflowVersionRecord[]
  currentVersionId?: string
  loading: boolean
  rollingBack: string | null
  zh: boolean
  onClose: () => void
  onLoad: (versionId: string) => void
  onRollback: (versionId: string, versionNum: number) => void
}

export function VersionHistoryModal({
  versions, currentVersionId, loading, rollingBack, zh,
  onClose, onLoad, onRollback,
}: VersionHistoryModalProps) {
  const [diffVersionId, setDiffVersionId] = useState<string | null>(null)
  const [diffCompareId, setDiffCompareId] = useState<string | null>(null)
  const [showComparePicker, setShowComparePicker] = useState<string | null>(null)

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" style={{ width: 560, maxHeight: '80vh', display: 'flex', flexDirection: 'column', gap: 0, padding: 0 }} onClick={(e) => e.stopPropagation()}>
        <div style={{ padding: '18px 20px 14px', borderBottom: '1px solid var(--border)', display: 'flex', alignItems: 'center', justifyContent: 'space-between', flexShrink: 0 }}>
          <div>
            <h2 style={{ margin: 0, fontSize: 15 }}>{zh ? '版本历史' : 'Version History'}</h2>
            <p style={{ margin: '3px 0 0', fontSize: 12, color: 'var(--muted)' }}>
              {zh ? '加载某个版本，或与上一版本对比差异。' : 'Load a version or Diff it against the previous one.'}
            </p>
          </div>
          <button className="btn btn-sm" onClick={onClose}>✕</button>
        </div>
        <div style={{ overflowY: 'auto', flex: 1 }}>
          {loading && (
            <div style={{ padding: 16 }}><SkeletonRows rows={4} /></div>
          )}
          {!loading && versions.length === 0 && (
            <div style={{ padding: 24, color: 'var(--muted)', textAlign: 'center', fontSize: 13 }}>{zh ? '暂无版本。' : 'No versions yet.'}</div>
          )}
          {!loading && versions.map((ver, i) => {
            const prev = versions[i + 1]
            const isDiffOpen = diffVersionId === ver.id
            // For "Compare with..." — use diffCompareId if set for this version, else fall back to prev
            const compareTarget = (diffVersionId === ver.id && diffCompareId)
              ? versions.find((v) => v.id === diffCompareId) ?? prev
              : prev

            // Compute diff vs selected compare target
            const diffBase = compareTarget
            const diff = diffBase ? (() => {
              const nodeMapA = new Map(diffBase.graph.nodes.map(n => [n.id, n]))
              const nodeMapB = new Map(ver.graph.nodes.map(n => [n.id, n]))
              const addedNodes = ver.graph.nodes.filter(n => !nodeMapA.has(n.id))
              const removedNodes = prev.graph.nodes.filter(n => !nodeMapB.has(n.id))
              // Config-level changes for nodes that exist in both versions
              const modifiedNodes: { id: string; type: string; changedKeys: string[] }[] = []
              for (const [id, nodeB] of nodeMapB) {
                const nodeA = nodeMapA.get(id)
                if (!nodeA) continue
                const cfgA = (nodeA.config ?? {}) as Record<string, unknown>
                const cfgB = (nodeB.config ?? {}) as Record<string, unknown>
                const allKeys = new Set([...Object.keys(cfgA), ...Object.keys(cfgB)])
                const changedKeys = [...allKeys].filter(k => JSON.stringify(cfgA[k]) !== JSON.stringify(cfgB[k]))
                if (changedKeys.length > 0) modifiedNodes.push({ id, type: nodeB.type, changedKeys })
              }
              const edgeKey = (e: { source: string; target: string }) => `${e.source}→${e.target}`
              const edgesA = new Set(diffBase.graph.edges.map(edgeKey))
              const edgesB = new Set(ver.graph.edges.map(edgeKey))
              const addedEdges = ver.graph.edges.filter(e => !edgesA.has(edgeKey(e)))
              const removedEdges = diffBase.graph.edges.filter(e => !edgesB.has(edgeKey(e)))
              return { addedNodes, removedNodes, modifiedNodes, addedEdges, removedEdges }
            })() : null

            const hasDiff = diff && (diff.addedNodes.length + diff.removedNodes.length + diff.modifiedNodes.length + diff.addedEdges.length + diff.removedEdges.length) > 0

            return (
              <div key={ver.id} style={{ borderBottom: '1px solid var(--border)' }}>
                <div style={{
                  display: 'flex', alignItems: 'center', gap: 10,
                  padding: '10px 20px',
                  background: ver.id === currentVersionId ? 'var(--panel)' : 'transparent',
                }}>
                  <span style={{ fontWeight: 600, fontSize: 13, minWidth: 28 }}>v{ver.version}</span>
                  <span className={`badge badge-${ver.status}`}>{ver.status}</span>
                  {i === 0 && <span style={{ fontSize: 11, color: 'var(--muted)' }}>{zh ? '最新' : 'latest'}</span>}
                  <span style={{ fontSize: 11, color: 'var(--muted)', fontFamily: 'monospace' }}>{ver.id.slice(0, 12)}…</span>
                  {ver.message && <span style={{ fontSize: 11, color: 'var(--fg)', fontStyle: 'italic', flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>"{ver.message}"</span>}
                  {!ver.message && <span style={{ flex: 1 }} />}
                  {versions.length > 1 && (
                    <div style={{ display: 'flex', alignItems: 'center', gap: 4, position: 'relative' }}>
                      <button
                        className={`btn btn-sm${isDiffOpen ? ' btn-primary' : ''}`}
                        onClick={() => { setDiffVersionId(isDiffOpen ? null : ver.id); if (isDiffOpen) setShowComparePicker(null) }}
                        title={zh ? '与另一版本对比' : 'Diff vs another version'}
                      >
                        {isDiffOpen ? (zh ? '隐藏' : 'Hide') : (zh ? '差异' : 'Diff')}
                        {isDiffOpen && compareTarget ? ` v${compareTarget.version}` : ''}
                        {hasDiff && !isDiffOpen && <span style={{ marginLeft: 4, fontSize: 10, color: 'var(--warning-text)' }}>●</span>}
                      </button>
                      {isDiffOpen && (
                        <div style={{ position: 'relative' }}>
                          <button
                            className="btn btn-sm"
                            style={{ fontSize: 10 }}
                            onClick={() => setShowComparePicker(showComparePicker === ver.id ? null : ver.id)}
                            title={zh ? '选择对比版本' : 'Pick version to compare'}
                          >
                            {zh ? '对比…' : 'vs…'}
                          </button>
                          {showComparePicker === ver.id && (
                            <div style={{ position: 'absolute', top: '100%', right: 0, zIndex: 999, background: 'var(--surface)', border: '1px solid var(--border)', borderRadius: 6, boxShadow: '0 4px 16px rgba(0,0,0,0.2)', minWidth: 140 }}>
                              {versions.filter((v) => v.id !== ver.id).map((v) => (
                                <button
                                  key={v.id}
                                  className="btn btn-sm"
                                  style={{ display: 'block', width: '100%', textAlign: 'left', borderRadius: 0, padding: '6px 12px', fontWeight: v.id === diffCompareId ? 700 : 400 }}
                                  onClick={() => { setDiffCompareId(v.id); setShowComparePicker(null) }}
                                >
                                  v{v.version} <span style={{ fontSize: 10, color: 'var(--muted)' }}>{v.status}</span>
                                </button>
                              ))}
                            </div>
                          )}
                        </div>
                      )}
                    </div>
                  )}
                  <button
                    className="btn btn-sm"
                    disabled={ver.id === currentVersionId || rollingBack === ver.id}
                    title={`Create a new draft version from v${ver.version}`}
                    onClick={() => onRollback(ver.id, ver.version)}
                    style={{ opacity: rollingBack === ver.id ? 0.6 : 1 }}
                  >
                    {rollingBack === ver.id ? '…' : '↩'}
                  </button>
                  <button
                    className="btn btn-sm"
                    disabled={ver.id === currentVersionId}
                    onClick={() => onLoad(ver.id)}
                  >
                    {ver.id === currentVersionId ? (zh ? '当前' : 'Current') : (zh ? '加载' : 'Load')}
                  </button>
                </div>
                {isDiffOpen && diff && (
                  <div style={{ padding: '8px 20px 12px', background: 'var(--canvas-bg)', fontSize: 12 }}>
                    <p style={{ color: 'var(--muted)', margin: '0 0 8px', fontSize: 11 }}>
                      {zh ? `对比 v${ver.version} 与 v${compareTarget?.version ?? '?'}` : `Comparing v${ver.version} vs v${compareTarget?.version ?? '?'}`} — {
                        hasDiff
                          ? (zh ? `${diff.addedNodes.length + diff.removedNodes.length} 个节点，${diff.modifiedNodes.length} 处配置变更，${diff.addedEdges.length + diff.removedEdges.length} 条边` : `${diff.addedNodes.length + diff.removedNodes.length} node(s), ${diff.modifiedNodes.length} config change(s), ${diff.addedEdges.length + diff.removedEdges.length} edge(s)`)
                          : (zh ? '完全相同' : 'identical')
                      }
                    </p>
                    {!hasDiff && (
                      <p style={{ color: 'var(--muted)' }}>{zh ? '无变更。' : 'No changes.'}</p>
                    )}
                    {diff.addedNodes.map(n => (
                      <div key={n.id} style={{ color: 'var(--success-text)', fontFamily: 'monospace', marginBottom: 2 }}>
                        + node: {n.id} ({n.type})
                      </div>
                    ))}
                    {diff.removedNodes.map(n => (
                      <div key={n.id} style={{ color: 'var(--danger-text)', fontFamily: 'monospace', marginBottom: 2 }}>
                        − node: {n.id} ({n.type})
                      </div>
                    ))}
                    {diff.modifiedNodes.map(m => (
                      <div key={m.id} style={{ marginBottom: 4 }}>
                        <div style={{ color: 'var(--warning-text)', fontFamily: 'monospace' }}>
                          ~ node: {m.id} ({m.type}) — {zh ? `${m.changedKeys.length} 个字段已变更` : `${m.changedKeys.length} field(s) changed`}
                        </div>
                        <div style={{ paddingLeft: 16 }}>
                          {m.changedKeys.map(k => (
                            <div key={k} style={{ color: 'var(--muted)', fontFamily: 'monospace', fontSize: 11, marginBottom: 1 }}>
                              ↳ {k}
                            </div>
                          ))}
                        </div>
                      </div>
                    ))}
                    {diff.addedEdges.map((e, idx) => (
                      <div key={idx} style={{ color: 'var(--success-text)', fontFamily: 'monospace', marginBottom: 2 }}>
                        + edge: {e.source} → {e.target}{e.condition_label ? ` [${e.condition_label}]` : ''}
                      </div>
                    ))}
                    {diff.removedEdges.map((e, idx) => (
                      <div key={idx} style={{ color: 'var(--danger-text)', fontFamily: 'monospace', marginBottom: 2 }}>
                        − edge: {e.source} → {e.target}{e.condition_label ? ` [${e.condition_label}]` : ''}
                      </div>
                    ))}
                  </div>
                )}
              </div>
            )
          })}
        </div>
      </div>
    </div>
  )
}
