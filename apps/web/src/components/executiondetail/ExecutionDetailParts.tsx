// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! Presentational sub-components of the execution detail page.

import { useState, useCallback, useRef } from 'react'
import { IconX } from '../uiIcons'
import * as api from '../../api/client'
import type { ExecutionRecord, NodeExecutionRecord } from '../../types'
import { useLocale } from '../../useLocale'
import { JsonTree } from '../JsonTree'

export function prettyJson(raw: string | null): string {
  if (!raw) return ''
  try { return JSON.stringify(JSON.parse(raw), null, 2) } catch { return raw }
}

export function CopyButton({ text }: { text: string }) {
  const [copied, setCopied] = useState(false)
  const { locale } = useLocale()
  const timer = useRef<ReturnType<typeof setTimeout>>()
  const handleCopy = useCallback(() => {
    navigator.clipboard.writeText(text).then(() => {
      setCopied(true)
      clearTimeout(timer.current)
      timer.current = setTimeout(() => setCopied(false), 1500)
    }).catch(() => {})
  }, [text])
  return (
    <button
      className="btn btn-sm btn-icon"
      onClick={handleCopy}
      title={locale === 'zh' ? '复制到剪贴板' : 'Copy to clipboard'}
      style={{ fontSize: 10, padding: '2px 6px', marginLeft: 6 }}
    >
      {copied ? '✓' : '⎘'}
    </button>
  )
}

export function NodeResultCard({ nr }: { nr: NodeExecutionRecord }) {
  const [expanded, setExpanded] = useState(false)
  const { locale } = useLocale()
  const zh = locale === 'zh'
  const isLong = !!nr.output_json && (nr.output_json.length > 400 || nr.output_json.split('\n').length > 8)
  const body = nr.error
    ? <div style={{ padding: '8px 12px', color: 'var(--danger-text)', fontSize: 12, fontFamily: 'monospace' }}>{nr.error}</div>
    : nr.output_json
      ? (
        <div style={{ position: 'relative' }}>
          <div style={{
            padding: '4px 12px',
            maxHeight: expanded ? 'none' : 180, overflowY: expanded ? 'visible' : 'auto',
          }}>
            <JsonTree raw={nr.output_json} />
          </div>
          {isLong && !expanded && (
            <div style={{
              position: 'absolute', bottom: 0, left: 0, right: 0,
              background: 'linear-gradient(transparent, var(--surface))',
              height: 32, display: 'flex', alignItems: 'flex-end', justifyContent: 'center', paddingBottom: 4,
            }}>
              <button
                className="btn btn-sm"
                onClick={() => setExpanded(true)}
                style={{ fontSize: 10, padding: '1px 8px', opacity: 0.85 }}
              >
                {zh ? '▼ 展开' : '▼ expand'}
              </button>
            </div>
          )}
          {isLong && expanded && (
            <div style={{ padding: '2px 12px 6px', textAlign: 'right' }}>
              <button
                className="btn btn-sm"
                onClick={() => setExpanded(false)}
                style={{ fontSize: 10, padding: '1px 8px', opacity: 0.7 }}
              >
                {zh ? '▲ 收起' : '▲ collapse'}
              </button>
            </div>
          )}
        </div>
      )
      : <div style={{ padding: '8px 12px', color: 'var(--muted)', fontSize: 12 }}>{zh ? '无输出' : 'No output'}</div>

  return (
    <div id={`node-result-${nr.node_id}`} style={{
      background: 'var(--surface)', border: '1px solid var(--border)',
      borderRadius: 'var(--radius)', overflow: 'hidden',
      transition: 'outline 0.3s',
    }}>
      <div style={{
        display: 'flex', alignItems: 'center', gap: 8,
        padding: '8px 12px', borderBottom: '1px solid var(--border)',
        background: 'var(--panel)',
      }}>
        <code style={{ fontSize: 13, fontWeight: 600 }}>{nr.node_id}</code>
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>{nr.node_type}</span>
        <span className={`badge badge-${nr.status}`} style={{ marginLeft: 'auto' }}>
          {nr.status}
        </span>
        {nr.duration_ms != null && nr.status !== 'skipped' && (
          <span style={{ fontSize: 11, color: 'var(--muted)', fontFamily: 'monospace' }}>
            {nr.duration_ms < 1000
              ? `${nr.duration_ms}ms`
              : `${(nr.duration_ms / 1000).toFixed(1)}s`}
          </span>
        )}
        {(nr.retry_count ?? 0) > 0 && (
          <span
            style={{ fontSize: 11, color: '#d97706', fontFamily: 'monospace' }}
            title={zh ? `已重试 ${nr.retry_count} 次` : `Retried ${nr.retry_count} time${nr.retry_count !== 1 ? 's' : ''}`}
          >
            ↺{nr.retry_count}
          </span>
        )}
        {nr.output_json && <CopyButton text={prettyJson(nr.output_json)} />}
      </div>
      {body}
    </div>
  )
}

export function StatCard({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div style={{
      background: 'var(--panel)', border: '1px solid var(--border)',
      borderRadius: 'var(--radius)', padding: '10px 14px',
    }}>
      <div style={{ fontSize: 11, color: 'var(--muted)', fontWeight: 600, marginBottom: 4, textTransform: 'uppercase', letterSpacing: '0.05em' }}>
        {label}
      </div>
      <div style={{ fontSize: 13 }}>{children}</div>
    </div>
  )
}

interface NodeResult {
  node_id: string
  node_type: string
  status: string
  duration_ms?: number
  started_at_ms?: number
}

const STATUS_COLORS: Record<string, string> = {
  succeeded: '#16a34a',
  failed:    '#dc2626',
  skipped:   '#6b7280',
  running:   '#2563eb',
}

export function ExecutionGraph({ record }: { record: ExecutionRecord }) {
  const { locale } = useLocale()
  const zh = locale === 'zh'
  const nodes = record.graph?.nodes ?? []
  const edges = record.graph?.edges ?? []
  const resultMap: Record<string, string> = {}
  for (const nr of record.node_results) resultMap[nr.node_id] = nr.status
  const statusColor = (s?: string) => {
    if (s === 'succeeded') return '#22c55e'
    if (s === 'failed') return '#ef4444'
    if (s === 'skipped') return '#6b7280'
    if (s === 'running') return '#3b82f6'
    return '#374151'
  }
  if (nodes.length === 0) return <p style={{ color: 'var(--muted)' }}>{zh ? '无图形数据' : 'No graph data'}</p>

  // Assign each node a horizontal level via BFS from trigger/orphan nodes
  const inDegree: Record<string, number> = {}
  const children: Record<string, string[]> = {}
  for (const n of nodes) { inDegree[n.id] = 0; children[n.id] = [] }
  for (const e of edges) { inDegree[e.target] = (inDegree[e.target] ?? 0) + 1; children[e.source]?.push(e.target) }
  const levels: string[][] = []
  const queue = nodes.filter((n) => !inDegree[n.id]).map((n) => n.id)
  const visited = new Set<string>()
  while (queue.length > 0) {
    const next: string[] = []
    const level: string[] = []
    for (const id of queue) {
      if (visited.has(id)) continue
      visited.add(id)
      level.push(id)
      for (const ch of children[id] ?? []) { if (!visited.has(ch)) next.push(ch) }
    }
    if (level.length > 0) levels.push(level)
    queue.splice(0, queue.length, ...next)
  }
  // Add any remaining unvisited nodes to last level
  const remaining = nodes.filter((n) => !visited.has(n.id)).map((n) => n.id)
  if (remaining.length > 0) levels.push(remaining)

  const COL_W = 130, ROW_H = 60, PAD_X = 20, PAD_Y = 20, NODE_W = 110, NODE_H = 36
  const totalW = levels.length * COL_W + PAD_X * 2
  const maxRows = Math.max(...levels.map((l) => l.length))
  const totalH = maxRows * ROW_H + PAD_Y * 2

  const pos: Record<string, { x: number; y: number }> = {}
  levels.forEach((level, col) => {
    const x = PAD_X + col * COL_W
    level.forEach((id, row) => {
      const totalRowH = level.length * ROW_H
      const y = PAD_Y + (maxRows * ROW_H - totalRowH) / 2 + row * ROW_H
      pos[id] = { x, y }
    })
  })

  return (
    <svg
      width={Math.max(totalW, 300)}
      height={Math.max(totalH, 120)}
      style={{ background: 'var(--canvas-bg)', borderRadius: 6, border: '1px solid var(--border)', overflow: 'visible' }}
    >
      {edges.map((e, i) => {
        const s = pos[e.source]
        const t = pos[e.target]
        if (!s || !t) return null
        const sx = s.x + NODE_W, sy = s.y + NODE_H / 2
        const tx = t.x, ty = t.y + NODE_H / 2
        const mx = (sx + tx) / 2
        return (
          <path key={i} d={`M${sx},${sy} C${mx},${sy} ${mx},${ty} ${tx},${ty}`}
            fill="none" stroke="var(--border)" strokeWidth={1.5} markerEnd="url(#arr)" />
        )
      })}
      <defs>
        <marker id="arr" markerWidth="6" markerHeight="6" refX="5" refY="3" orient="auto">
          <path d="M0,0 L6,3 L0,6 Z" fill="var(--muted)" />
        </marker>
      </defs>
      {nodes.map((n) => {
        const p = pos[n.id]
        if (!p) return null
        const status = resultMap[n.id]
        const col = statusColor(status)
        const label = (n.config?.node_label as string) || (n.type as string)
        return (
          <g key={n.id}>
            <rect x={p.x} y={p.y} width={NODE_W} height={NODE_H} rx={4}
              fill="var(--panel)" stroke={col} strokeWidth={status ? 2 : 1} />
            <text x={p.x + NODE_W / 2} y={p.y + 14} textAnchor="middle"
              fill={col} fontSize={9} fontFamily="monospace">
              {(n.type as string).toUpperCase()}
            </text>
            <text x={p.x + NODE_W / 2} y={p.y + 26} textAnchor="middle"
              fill="var(--fg)" fontSize={10} fontFamily="monospace">
              {label.length > 14 ? label.slice(0, 13) + '…' : label}
            </text>
            {status && (
              <text x={p.x + NODE_W / 2} y={p.y + NODE_H - 4} textAnchor="middle"
                fill={col} fontSize={8} fontFamily="monospace">
                {status}
              </text>
            )}
          </g>
        )
      })}
    </svg>
  )
}

export function ExecutionTimeline({
  nodeResults,
  startedAt,
  finishedAt,
  onClickNode,
}: {
  nodeResults: NodeResult[]
  startedAt: number
  finishedAt: number
  onClickNode?: (nodeId: string) => void
}) {
  const { locale } = useLocale()
  const zh = locale === 'zh'
  const totalMs = (finishedAt - startedAt) * 1000
  if (totalMs <= 0) return null

  const timed = nodeResults.filter((nr) => (nr.duration_ms ?? 0) > 0)
  if (timed.length === 0) return null

  // If any node has started_at_ms > 0 we can render a true Gantt waterfall.
  const hasWaterfall = timed.some((nr) => (nr.started_at_ms ?? 0) > 0)

  // Scale: use max(last node finish, totalMs) so bars never overflow
  const scale = hasWaterfall
    ? Math.max(totalMs, ...timed.map((nr) => (nr.started_at_ms ?? 0) + (nr.duration_ms ?? 0)))
    : Math.max(...timed.map((nr) => nr.duration_ms ?? 0))

  // Sort chronologically (by start) for waterfall, or by duration descending for flat view
  const sorted = hasWaterfall
    ? [...timed].sort((a, b) => (a.started_at_ms ?? 0) - (b.started_at_ms ?? 0))
    : [...timed].sort((a, b) => (b.duration_ms ?? 0) - (a.duration_ms ?? 0))

  const fmtMs = (ms: number) => ms < 1000 ? `${ms}ms` : `${(ms / 1000).toFixed(2)}s`

  return (
    <section style={{ marginBottom: 20 }}>
      <h2 style={{ marginBottom: 10 }}>
        {zh ? '执行时间线' : 'Execution Timeline'}
        <span style={{ fontSize: 12, fontWeight: 400, color: 'var(--muted)', marginLeft: 8 }}>
          {fmtMs(Math.round(totalMs))} {zh ? '总计' : 'total'}
        </span>
        {hasWaterfall && (
          <span style={{ fontSize: 11, fontWeight: 400, color: 'var(--link)', marginLeft: 8 }}>
            {zh ? '瀑布图' : 'waterfall'}
          </span>
        )}
      </h2>
      <div style={{
        background: 'var(--surface)', border: '1px solid var(--border)',
        borderRadius: 'var(--radius)', padding: '12px 14px',
        display: 'flex', flexDirection: 'column', gap: 6,
      }}>
        {sorted.map((nr) => {
          const startOffsetPct = hasWaterfall
            ? ((nr.started_at_ms ?? 0) / scale) * 100
            : 0
          const widthPct = Math.max(0.5, ((nr.duration_ms ?? 0) / scale) * 100)
          const wallPct = totalMs > 0 ? Math.round(((nr.duration_ms ?? 0) / totalMs) * 100) : 0
          const color = STATUS_COLORS[nr.status] ?? '#6b7280'
          const label = fmtMs(nr.duration_ms ?? 0)
          const barFull = widthPct > 18 // enough room to show text inside bar
          return (
            <div
              key={nr.node_id}
              style={{ display: 'flex', alignItems: 'center', gap: 8, cursor: onClickNode ? 'pointer' : 'default' }}
              onClick={() => onClickNode?.(nr.node_id)}
              title={onClickNode ? `Click to jump to ${nr.node_id}` : undefined}
            >
              <div style={{
                width: 120, flexShrink: 0,
                fontSize: 11, fontFamily: 'monospace', color: 'var(--muted)',
                overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
                textAlign: 'right',
              }} title={`${nr.node_id} (${nr.node_type})`}>
                {nr.node_id}
              </div>
              <div style={{ flex: 1, background: 'var(--panel)', borderRadius: 3, height: 18, position: 'relative' }}>
                <div style={{
                  position: 'absolute',
                  left: `${startOffsetPct}%`,
                  top: 0, bottom: 0,
                  width: `${widthPct}%`,
                  background: color,
                  opacity: 0.85,
                  borderRadius: 3,
                  transition: 'left 0.3s ease, width 0.3s ease',
                  minWidth: 2,
                }} />
                {barFull && (
                  <span style={{
                    position: 'absolute',
                    left: `calc(${startOffsetPct}% + 5px)`,
                    top: 0, bottom: 0,
                    display: 'flex', alignItems: 'center',
                    fontSize: 10, fontFamily: 'monospace',
                    color: '#fff',
                    pointerEvents: 'none',
                  }}>
                    {label}
                  </span>
                )}
              </div>
              <div style={{ width: 44, flexShrink: 0, fontSize: 10, color: 'var(--muted)', textAlign: 'right', fontFamily: 'monospace' }}>
                {barFull ? (wallPct > 0 ? `${wallPct}%` : '') : label}
              </div>
            </div>
          )
        })}
        {/* Time axis ticks */}
        {hasWaterfall && (
          <div style={{ position: 'relative', height: 14, marginLeft: 128, fontSize: 9, color: 'var(--muted)', fontFamily: 'monospace' }}>
            {[0, 25, 50, 75, 100].map((pct) => (
              <span key={pct} style={{ position: 'absolute', left: `${pct}%`, transform: 'translateX(-50%)' }}>
                {fmtMs(Math.round(scale * pct / 100))}
              </span>
            ))}
          </div>
        )}
        <div style={{ fontSize: 10, color: 'var(--muted)', marginTop: 4, display: 'flex', gap: 12 }}>
          {Object.entries(STATUS_COLORS).map(([s, c]) => (
            <span key={s} style={{ display: 'flex', alignItems: 'center', gap: 3 }}>
              <span style={{ width: 8, height: 8, borderRadius: 2, background: c, display: 'inline-block' }} />
              {s}
            </span>
          ))}
        </div>
      </div>
    </section>
  )
}

export function NoteEditor({
  tenantId, executionId, note, onSaved,
}: { tenantId: string; executionId: string; note: string | null; onSaved: (n: string | null) => void }) {
  const [editing, setEditing] = useState(false)
  const [value, setValue] = useState(note ?? '')
  const [saving, setSaving] = useState(false)
  const { locale } = useLocale()
  const zh = locale === 'zh'

  const save = async () => {
    setSaving(true)
    try {
      const next = value.trim() || null
      await api.setExecutionNote(tenantId, executionId, next)
      onSaved(next)
      setEditing(false)
    } finally { setSaving(false) }
  }

  return (
    <div style={{
      background: 'var(--panel)', border: '1px solid var(--border)', borderRadius: 8,
      padding: '10px 14px', gridColumn: '1 / -1',
    }}>
      <div style={{ fontSize: 10, fontWeight: 700, letterSpacing: 1, color: 'var(--muted)', marginBottom: 4 }}>{zh ? '备注' : 'NOTE'}</div>
      {editing ? (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
          <textarea
            autoFocus
            value={value}
            onChange={(e) => setValue(e.target.value)}
            rows={3}
            placeholder={zh ? '添加备注（如根因、解决步骤、上下文）…' : 'Add a note about this run (e.g. root cause, resolution steps, context)…'}
            style={{ fontSize: 12, background: 'var(--bg)', border: '1px solid var(--border)', borderRadius: 4, padding: '6px 8px', color: 'var(--fg)', resize: 'vertical', fontFamily: 'inherit' }}
          />
          <div style={{ display: 'flex', gap: 4 }}>
            <button className="btn btn-sm btn-primary" onClick={save} disabled={saving} style={{ fontSize: 11 }}>{zh ? '✓ 保存' : '✓ Save'}</button>
            <button className="btn btn-sm" onClick={() => { setEditing(false); setValue(note ?? '') }} style={{ fontSize: 11 }}>{zh ? '取消' : 'Cancel'}</button>
          </div>
        </div>
      ) : (
        <div style={{ display: 'flex', alignItems: 'flex-start', gap: 8 }}>
          <span style={{ fontSize: 12, color: note ? 'var(--fg)' : 'var(--muted)', fontStyle: note ? 'normal' : 'italic', flex: 1, whiteSpace: 'pre-wrap', wordBreak: 'break-word' }}>
            {note ?? (zh ? '暂无备注 — 点击 ✎ 添加' : 'No note — click ✎ to add one')}
          </span>
          <button className="btn btn-sm btn-icon" onClick={() => { setValue(note ?? ''); setEditing(true) }} style={{ fontSize: 10, opacity: 0.6, flexShrink: 0 }}>
            ✎
          </button>
        </div>
      )}
    </div>
  )
}

export function LabelEditor({
  tenantId, executionId, label, onSaved,
}: { tenantId: string; executionId: string; label: string | null; onSaved: (l: string | null) => void }) {
  const [editing, setEditing] = useState(false)
  const [value, setValue] = useState(label ?? '')
  const [saving, setSaving] = useState(false)
  const { locale } = useLocale()
  const zh = locale === 'zh'

  const save = async () => {
    setSaving(true)
    try {
      const next = value.trim() || null
      await api.patchExecutionLabel(tenantId, executionId, next)
      onSaved(next)
      setEditing(false)
    } finally { setSaving(false) }
  }

  return (
    <div style={{
      background: 'var(--panel)', border: '1px solid var(--border)', borderRadius: 8,
      padding: '10px 14px', minWidth: 120,
    }}>
      <div style={{ fontSize: 10, fontWeight: 700, letterSpacing: 1, color: 'var(--muted)', marginBottom: 4 }}>{zh ? '标签' : 'LABEL'}</div>
      {editing ? (
        <div style={{ display: 'flex', gap: 4, alignItems: 'center' }}>
          <input
            autoFocus
            value={value}
            onChange={(e) => setValue(e.target.value)}
            onKeyDown={(e) => { if (e.key === 'Enter') save(); if (e.key === 'Escape') setEditing(false) }}
            style={{ flex: 1, fontSize: 12, background: 'var(--bg)', border: '1px solid var(--border)', borderRadius: 4, padding: '2px 6px', color: 'var(--fg)' }}
            placeholder={zh ? '输入标签…' : 'Enter label…'}
          />
          <button className="btn btn-sm btn-primary" onClick={save} disabled={saving} style={{ fontSize: 11 }}>✓</button>
          <button className="btn btn-sm" onClick={() => setEditing(false)} style={{ fontSize: 11 }}><IconX aria-hidden /></button>
        </div>
      ) : (
        <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
          <span style={{ fontSize: 12, color: label ? 'var(--fg)' : 'var(--muted)', fontStyle: label ? 'normal' : 'italic' }}>
            {label ?? (zh ? '无标签' : 'no label')}
          </span>
          <button className="btn btn-sm btn-icon" onClick={() => { setValue(label ?? ''); setEditing(true) }} style={{ fontSize: 10, opacity: 0.6 }}>
            ✎
          </button>
        </div>
      )}
    </div>
  )
}
