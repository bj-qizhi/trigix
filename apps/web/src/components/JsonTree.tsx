// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useState } from 'react'

interface JsonTreeProps {
  value: unknown
  depth?: number
  defaultExpanded?: boolean
  // When set, keys / array indices / leaves become clickable and call onPick
  // with the dot-path to that value (relative to the tree root).
  onPick?: (path: string) => void
  path?: string
}

const join = (path: string, seg: string | number) => (path ? `${path}.${seg}` : String(seg))

function JsonValue({ value, depth = 0, defaultExpanded = true, onPick, path = '' }: JsonTreeProps) {
  const [open, setOpen] = useState(defaultExpanded || depth < 2)

  const pickable = (p: string, node: React.ReactNode) =>
    onPick ? (
      <span
        onMouseDown={(e) => e.preventDefault()}
        onClick={(e) => { e.stopPropagation(); onPick(p) }}
        title={`插入 {{…${p ? '.' + p : ''}}}`}
        style={{ cursor: 'pointer', borderRadius: 2 }}
        className="json-pick"
      >
        {node}
      </span>
    ) : (
      node
    )

  if (value === null) return pickable(path, <span style={{ color: 'var(--muted)' }}>null</span>)
  if (typeof value === 'boolean') return pickable(path, <span style={{ color: '#7c3aed' }}>{String(value)}</span>)
  if (typeof value === 'number') return pickable(path, <span style={{ color: '#0891b2' }}>{value}</span>)
  if (typeof value === 'string') return pickable(path, <span style={{ color: '#15803d' }}>"{value}"</span>)

  if (Array.isArray(value)) {
    if (value.length === 0) return <span style={{ color: 'var(--muted)' }}>[]</span>
    return (
      <span>
        <button
          onClick={() => setOpen((o) => !o)}
          style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--muted)', fontSize: 11, padding: '0 2px', fontFamily: 'monospace' }}
        >
          {open ? '▾' : '▸'} [{value.length}]
        </button>
        {open && (
          <div style={{ marginLeft: 14, borderLeft: '1px solid var(--border)', paddingLeft: 8 }}>
            {value.map((item, i) => (
              <div key={i} style={{ display: 'flex', gap: 4, alignItems: 'flex-start', marginBottom: 1 }}>
                {pickable(join(path, i), <span style={{ color: 'var(--muted)', fontSize: 10, minWidth: 20, textAlign: 'right', marginTop: 1 }}>{i}</span>)}
                <JsonValue value={item} depth={depth + 1} defaultExpanded={depth < 1} onPick={onPick} path={join(path, i)} />
                {i < value.length - 1 && <span style={{ color: 'var(--muted)' }}>,</span>}
              </div>
            ))}
          </div>
        )}
      </span>
    )
  }

  if (typeof value === 'object') {
    const keys = Object.keys(value as object)
    if (keys.length === 0) return <span style={{ color: 'var(--muted)' }}>{'{}'}</span>
    return (
      <span>
        <button
          onClick={() => setOpen((o) => !o)}
          style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--muted)', fontSize: 11, padding: '0 2px', fontFamily: 'monospace' }}
        >
          {open ? '▾' : '▸'} {'{'}…{'}'}
        </button>
        {open && (
          <div style={{ marginLeft: 14, borderLeft: '1px solid var(--border)', paddingLeft: 8 }}>
            {keys.map((k, i) => (
              <div key={k} style={{ display: 'flex', gap: 4, alignItems: 'flex-start', marginBottom: 1 }}>
                {pickable(join(path, k), <span style={{ color: '#b45309', fontWeight: 500, whiteSpace: 'nowrap', fontSize: 11 }}>"{k}":</span>)}
                <JsonValue value={(value as Record<string, unknown>)[k]} depth={depth + 1} defaultExpanded={depth < 1} onPick={onPick} path={join(path, k)} />
                {i < keys.length - 1 && <span style={{ color: 'var(--muted)' }}>,</span>}
              </div>
            ))}
          </div>
        )}
      </span>
    )
  }

  return <span>{String(value)}</span>
}

export function JsonTree({ raw, onPick }: { raw: string; onPick?: (path: string) => void }) {
  let parsed: { ok: true; value: unknown } | { ok: false }
  try { parsed = { ok: true, value: JSON.parse(raw) } } catch { parsed = { ok: false } }

  if (!parsed.ok) {
    return <pre style={{ margin: 0, fontSize: 11, fontFamily: 'monospace', color: 'var(--muted)', whiteSpace: 'pre-wrap', wordBreak: 'break-all' }}>{raw}</pre>
  }

  return (
    <div style={{ fontSize: 11, fontFamily: 'monospace', lineHeight: 1.6, padding: '6px 0' }}>
      <JsonValue value={parsed.value} depth={0} defaultExpanded={true} onPick={onPick} path="" />
    </div>
  )
}
