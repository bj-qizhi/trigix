// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

// Friendly renderer for a workflow's output_json. When the workflow declares an
// output_schema, fields are shown labeled; values render by content type —
// images/video preview, URLs as links, arrays-of-objects as tables, everything
// else as text or a JSON tree. A toggle drops back to raw JSON for developers.

import { useState } from 'react'
import type { OutputField } from '../../types'
import { JsonTree } from '../JsonTree'
import { useLocale } from '../../useLocale'

const IMG_EXT = /\.(png|jpe?g|gif|webp|svg|bmp)(\?|$)/i
const VID_EXT = /\.(mp4|webm|mov|m4v)(\?|$)/i
const B64 = /^[A-Za-z0-9+/=\s]+$/

// Recognise a base64 image (data URL, or a bare payload with a known magic
// prefix) and return a renderable data URL.
function asImageDataUrl(s: string): string | null {
  if (/^data:image\/[a-z0-9.+-]+;base64,/i.test(s)) return s
  if (s.length > 128 && B64.test(s)) {
    if (s.startsWith('iVBORw0KGgo')) return `data:image/png;base64,${s}`
    if (s.startsWith('/9j/')) return `data:image/jpeg;base64,${s}`
    if (s.startsWith('R0lGOD')) return `data:image/gif;base64,${s}`
    if (s.startsWith('UklGR')) return `data:image/webp;base64,${s}`
  }
  return null
}

const imgStyle = { maxWidth: '100%', maxHeight: 260, borderRadius: 6, border: '1px solid var(--border)' } as const
const cell = { border: '1px solid var(--border)', padding: '4px 8px', textAlign: 'left', verticalAlign: 'top' } as const

function fmtCell(v: unknown): string {
  if (v === null || v === undefined) return ''
  if (typeof v === 'object') return JSON.stringify(v)
  return String(v)
}

function ValueView({ value }: { value: unknown }) {
  const { locale } = useLocale()
  const zh = locale === 'zh'

  if (value === null || value === undefined) return <span style={{ color: 'var(--muted)' }}>—</span>

  if (typeof value === 'string') {
    const img = asImageDataUrl(value)
    if (img) return <img src={img} alt="" style={imgStyle} />
    if (/^https?:\/\//i.test(value)) {
      if (IMG_EXT.test(value)) return <img src={value} alt="" style={imgStyle} />
      if (VID_EXT.test(value)) return <video src={value} controls style={{ maxWidth: '100%', maxHeight: 260 }} />
      return <a href={value} target="_blank" rel="noreferrer" style={{ wordBreak: 'break-all' }}>{value}</a>
    }
    if (value.length > 512 && B64.test(value)) {
      return <span style={{ color: 'var(--muted)', fontSize: 12 }}>{zh ? `二进制数据（${value.length} 字符，base64）` : `binary data (${value.length} chars, base64)`}</span>
    }
    return <span style={{ whiteSpace: 'pre-wrap', wordBreak: 'break-word' }}>{value}</span>
  }

  if (typeof value === 'number' || typeof value === 'boolean') {
    return <span style={{ fontFamily: 'monospace' }}>{String(value)}</span>
  }

  // Array of plain objects → table.
  if (Array.isArray(value) && value.length > 0 && value.every((v) => v && typeof v === 'object' && !Array.isArray(v))) {
    const cols = Array.from(new Set(value.flatMap((v) => Object.keys(v as object)))).slice(0, 12)
    return (
      <div style={{ overflowX: 'auto' }}>
        <table style={{ borderCollapse: 'collapse', fontSize: 12, width: '100%' }}>
          <thead><tr>{cols.map((c) => <th key={c} style={{ ...cell, background: 'var(--bg)' }}>{c}</th>)}</tr></thead>
          <tbody>
            {value.slice(0, 50).map((row, i) => (
              <tr key={i}>{cols.map((c) => <td key={c} style={cell}>{fmtCell((row as Record<string, unknown>)[c])}</td>)}</tr>
            ))}
          </tbody>
        </table>
        {value.length > 50 && <p style={{ fontSize: 11, color: 'var(--muted)', marginTop: 4 }}>… {value.length} {zh ? '行' : 'rows'}</p>}
      </div>
    )
  }

  return <JsonTree raw={JSON.stringify(value)} />
}

export function ResultView({ outputJson, outputSchema }: { outputJson: string; outputSchema?: OutputField[] }) {
  const { locale } = useLocale()
  const zh = locale === 'zh'
  const [raw, setRaw] = useState(false)

  let parsed: unknown
  try {
    parsed = JSON.parse(outputJson)
  } catch {
    return <pre style={{ fontSize: 12, whiteSpace: 'pre-wrap' }}>{outputJson}</pre>
  }

  const toggle = (
    <button className="btn btn-sm" onClick={() => setRaw((r) => !r)} style={{ marginBottom: 10 }}>
      {raw ? (zh ? '友好视图' : 'Friendly view') : (zh ? '原始 JSON' : 'Raw JSON')}
    </button>
  )

  if (raw) return <>{toggle}<JsonTree raw={outputJson} /></>

  if (outputSchema && outputSchema.length > 0 && parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
    const obj = parsed as Record<string, unknown>
    return (
      <>
        {toggle}
        <div style={{ display: 'grid', gap: 14 }}>
          {outputSchema.map((f) => (
            <div key={f.key}>
              <div style={{ fontSize: 12, fontWeight: 600 }}>
                {f.key}
                {f.description && <span style={{ color: 'var(--muted)', fontWeight: 400, marginLeft: 6 }}>— {f.description}</span>}
              </div>
              <div style={{ marginTop: 3 }}><ValueView value={obj[f.key]} /></div>
            </div>
          ))}
        </div>
      </>
    )
  }

  return <>{toggle}<ValueView value={parsed} /></>
}
