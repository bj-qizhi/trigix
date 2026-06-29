// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useState, useEffect, useRef } from 'react'
import type { FlowNode } from '../Canvas'

// Ctrl+K command palette. Runs editor commands (Save / Publish / Run / Undo …)
// AND fuzzy-searches the current graph's nodes to jump to one. Arrow keys move
// the highlight across the combined list, Enter runs/jumps the highlighted row,
// Esc closes. The query is modal-local — the palette remounts fresh each open.

export interface PaletteCommand {
  id: string
  label: string
  hint?: string
  run: () => void
}

export interface CommandPaletteProps {
  nodes: FlowNode[]
  commands: PaletteCommand[]
  selectedNodeId: string | null
  zh: boolean
  onPick: (nodeId: string) => void
  onClose: () => void
}

type Item =
  | { kind: 'cmd'; cmd: PaletteCommand }
  | { kind: 'node'; node: FlowNode }

export function CommandPalette({ nodes, commands, selectedNodeId, zh, onPick, onClose }: CommandPaletteProps) {
  const [query, setQuery] = useState('')
  const [active, setActive] = useState(0)
  const listRef = useRef<HTMLDivElement>(null)

  const q = query.toLowerCase()
  const cmds = commands.filter((c) => !q || c.label.toLowerCase().includes(q) || c.id.toLowerCase().includes(q))
  const nodeMatches = nodes.filter((n) =>
    !q || n.id.toLowerCase().includes(q) || (n.data.nodeType ?? '').includes(q) ||
    ((n.data.config?.node_label as string | undefined) ?? '').toLowerCase().includes(q),
  )

  // Flat list (commands first, then nodes) drives keyboard navigation.
  const items: Item[] = [
    ...cmds.map((cmd) => ({ kind: 'cmd', cmd } as const)),
    ...nodeMatches.map((node) => ({ kind: 'node', node } as const)),
  ]
  const activeIdx = items.length === 0 ? -1 : Math.min(active, items.length - 1)

  // Keep the highlighted row in view as the user arrows through.
  useEffect(() => {
    const el = listRef.current?.querySelector<HTMLElement>(`[data-idx="${activeIdx}"]`)
    el?.scrollIntoView({ block: 'nearest' })
  }, [activeIdx])

  const exec = (i: number) => {
    const it = items[i]
    if (!it) return
    if (it.kind === 'cmd') { it.cmd.run(); onClose() }
    else onPick(it.node.id)
  }

  const cmdCount = cmds.length

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" style={{ width: 460, padding: 0, overflow: 'hidden' }} onClick={(e) => e.stopPropagation()}>
        <div style={{ padding: '10px 14px', borderBottom: '1px solid var(--border)' }}>
          <input
            autoFocus
            placeholder={zh ? '运行命令或搜索节点…' : 'Run a command or search nodes…'}
            value={query}
            onChange={(e) => { setQuery(e.target.value); setActive(0) }}
            onKeyDown={(e) => {
              if (e.key === 'Escape') { onClose(); return }
              if (e.key === 'ArrowDown') { e.preventDefault(); setActive((a) => Math.min(a + 1, items.length - 1)) }
              else if (e.key === 'ArrowUp') { e.preventDefault(); setActive((a) => Math.max(a - 1, 0)) }
              else if (e.key === 'Enter') { e.preventDefault(); exec(activeIdx) }
            }}
            style={{ width: '100%', border: 'none', outline: 'none', background: 'transparent', fontSize: 14 }}
          />
        </div>
        <div ref={listRef} style={{ maxHeight: 360, overflowY: 'auto' }}>
          {cmds.length > 0 && (
            <div style={{ padding: '6px 14px 2px', fontSize: 10, textTransform: 'uppercase', letterSpacing: '0.04em', color: 'var(--muted)' }}>
              {zh ? '命令' : 'Commands'}
            </div>
          )}
          {cmds.map((c, i) => (
            <div
              key={c.id}
              data-idx={i}
              onClick={() => exec(i)}
              onMouseEnter={() => setActive(i)}
              style={{
                display: 'flex', alignItems: 'center', gap: 10,
                padding: '8px 14px', cursor: 'pointer',
                background: i === activeIdx ? 'var(--panel)' : undefined,
              }}
            >
              <span style={{ fontSize: 13, flex: 1 }}>{c.label}</span>
              {c.hint && <kbd style={{ fontSize: 10, color: 'var(--muted)', border: '1px solid var(--border)', borderRadius: 4, padding: '1px 5px' }}>{c.hint}</kbd>}
            </div>
          ))}
          {nodeMatches.length > 0 && (
            <div style={{ padding: '6px 14px 2px', fontSize: 10, textTransform: 'uppercase', letterSpacing: '0.04em', color: 'var(--muted)' }}>
              {zh ? '节点' : 'Nodes'}
            </div>
          )}
          {nodeMatches.map((n, j) => {
            const i = cmdCount + j
            return (
              <div
                key={n.id}
                data-idx={i}
                onClick={() => exec(i)}
                onMouseEnter={() => setActive(i)}
                style={{
                  display: 'flex', alignItems: 'center', gap: 10,
                  padding: '8px 14px', cursor: 'pointer',
                  background: i === activeIdx || n.id === selectedNodeId ? 'var(--panel)' : undefined,
                }}
              >
                <span style={{ fontSize: 11, color: 'var(--muted)', width: 70, flexShrink: 0 }}>{n.data.nodeType}</span>
                <code style={{ fontSize: 13, flex: 1 }}>{n.id}</code>
                {(n.data.config?.node_label as string | undefined) && (
                  <span style={{ fontSize: 11, color: 'var(--link)' }}>{n.data.config!.node_label as string}</span>
                )}
              </div>
            )
          })}
          {items.length === 0 && (
            <div style={{ padding: '16px', color: 'var(--muted)', fontSize: 13, textAlign: 'center' }}>{zh ? '无匹配项' : 'No matches'}</div>
          )}
        </div>
        <div style={{ padding: '6px 14px', borderTop: '1px solid var(--border)', fontSize: 11, color: 'var(--muted)' }}>
          {zh ? '↑↓ 导航 · Enter 执行 · Esc 关闭' : '↑↓ navigate · Enter to run · Esc to close'}
        </div>
      </div>
    </div>
  )
}
