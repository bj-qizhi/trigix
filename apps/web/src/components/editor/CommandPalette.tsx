// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useState } from 'react'
import type { FlowNode } from '../Canvas'

// Ctrl+K command palette: fuzzy-search the current graph's nodes by id / type /
// label and jump to one (Enter picks the first match, click picks a row).
// Extracted verbatim from WorkflowEditor; the search query is modal-local state
// owned here — the palette remounts fresh each time it opens.

export interface CommandPaletteProps {
  nodes: FlowNode[]
  selectedNodeId: string | null
  zh: boolean
  onPick: (nodeId: string) => void
  onClose: () => void
}

export function CommandPalette({ nodes, selectedNodeId, zh, onPick, onClose }: CommandPaletteProps) {
  const [query, setQuery] = useState('')

  const matches = (n: FlowNode) => {
    const q = query.toLowerCase()
    return !q || n.id.toLowerCase().includes(q) || (n.data.nodeType ?? '').includes(q) || ((n.data.config?.node_label as string | undefined) ?? '').toLowerCase().includes(q)
  }
  const filtered = nodes.filter(matches)

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" style={{ width: 440, padding: 0, overflow: 'hidden' }} onClick={(e) => e.stopPropagation()}>
        <div style={{ padding: '10px 14px', borderBottom: '1px solid var(--border)' }}>
          <input
            autoFocus
            placeholder={zh ? '搜索节点…（输入筛选，回车跳转）' : 'Search nodes… (type to filter, Enter to select)'}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Escape') { onClose(); return }
              if (e.key === 'Enter') {
                const q = query.toLowerCase()
                const match = nodes.find((n) =>
                  n.id.toLowerCase().includes(q) ||
                  (n.data.config?.node_label as string | undefined ?? '').toLowerCase().includes(q)
                )
                if (match) onPick(match.id)
              }
            }}
            style={{ width: '100%', border: 'none', outline: 'none', background: 'transparent', fontSize: 14 }}
          />
        </div>
        <div style={{ maxHeight: 320, overflowY: 'auto' }}>
          {filtered.map((n) => (
            <div
              key={n.id}
              onClick={() => onPick(n.id)}
              style={{
                display: 'flex', alignItems: 'center', gap: 10,
                padding: '8px 14px', cursor: 'pointer',
                background: n.id === selectedNodeId ? 'var(--panel)' : undefined,
              }}
              onMouseEnter={(e) => (e.currentTarget.style.background = 'var(--panel)')}
              onMouseLeave={(e) => (e.currentTarget.style.background = n.id === selectedNodeId ? 'var(--panel)' : '')}
            >
              <span style={{ fontSize: 11, color: 'var(--muted)', width: 70, flexShrink: 0 }}>{n.data.nodeType}</span>
              <code style={{ fontSize: 13, flex: 1 }}>{n.id}</code>
              {(n.data.config?.node_label as string | undefined) && (
                <span style={{ fontSize: 11, color: 'var(--link)' }}>{n.data.config!.node_label as string}</span>
              )}
            </div>
          ))}
          {filtered.length === 0 && (
            <div style={{ padding: '16px', color: 'var(--muted)', fontSize: 13, textAlign: 'center' }}>{zh ? '无匹配节点' : 'No nodes match'}</div>
          )}
        </div>
        <div style={{ padding: '6px 14px', borderTop: '1px solid var(--border)', fontSize: 11, color: 'var(--muted)' }}>
          {zh ? '↑↓ 导航 · Enter 跳转 · Esc 关闭' : '↑↓ navigate · Enter to jump · Esc to close'}
        </div>
      </div>
    </div>
  )
}
