// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useEffect, useState } from 'react'
import { useLocale } from '../useLocale'
import { useTheme } from '../useTheme'
import * as api from '../api/client'
import type { CustomNodeDef } from '../api/client'
import logoWordmark from '../assets/logo-wordmark.svg'

interface Props {
  onBack: () => void
}

export function CustomNodesPage({ onBack }: Props) {
  const { theme, toggle: toggleTheme } = useTheme()
  const { locale } = useLocale()
  const zh = locale === 'zh'

  const [nodes, setNodes] = useState<CustomNodeDef[]>([])
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  const [slug, setSlug] = useState('')
  const [label, setLabel] = useState('')
  const [description, setDescription] = useState('')
  const [endpoint, setEndpoint] = useState('')
  const [schema, setSchema] = useState('{\n  "type": "object",\n  "properties": {}\n}')

  const [importUrl, setImportUrl] = useState('')
  const [importMsg, setImportMsg] = useState<string | null>(null)

  const handleImport = async () => {
    if (!importUrl.trim()) return
    setBusy(true); setError(null); setImportMsg(null)
    try {
      const imported = await api.importCustomNodes(importUrl.trim())
      setImportMsg(zh ? `已导入 ${imported.length} 个节点` : `Imported ${imported.length} node(s)`)
      setImportUrl('')
      load()
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setBusy(false)
    }
  }

  const load = () => {
    api.listCustomNodes().then(setNodes).catch((e: unknown) => setError(String(e)))
  }
  useEffect(load, [])

  const handleSave = async () => {
    if (!slug.trim() || !label.trim() || !endpoint.trim()) return
    let config_schema: unknown = {}
    try {
      config_schema = schema.trim() ? JSON.parse(schema) : {}
    } catch {
      setError(zh ? '配置 Schema 不是合法 JSON' : 'Config schema is not valid JSON')
      return
    }
    setBusy(true)
    setError(null)
    try {
      await api.upsertCustomNode({ slug: slug.trim(), label: label.trim(), description: description.trim(), endpoint: endpoint.trim(), config_schema })
      setSlug(''); setLabel(''); setDescription(''); setEndpoint('')
      load()
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setBusy(false)
    }
  }

  const handleDelete = async (id: string, label: string) => {
    if (!window.confirm(zh ? `删除自定义节点 "${label}"？` : `Delete custom node "${label}"?`)) return
    try {
      await api.deleteCustomNode(id)
      load()
    } catch (e: unknown) {
      setError(String(e))
    }
  }

  const fieldStyle: React.CSSProperties = { display: 'flex', flexDirection: 'column', gap: 4 }
  const labelStyle: React.CSSProperties = { fontSize: 12, color: 'var(--muted)' }

  return (
    <div className="app">
      <header className="topbar">
        <button className="btn btn-sm btn-icon" onClick={onBack} title={zh ? '返回' : 'Back'}>←</button>
        <span className="topbar-sep">|</span>
        <img src={logoWordmark} alt="Trigix" style={{ height: '28px' }} />
        <span className="topbar-sep">|</span>
        <span className="topbar-title">{zh ? '自定义节点' : 'Custom Nodes'}</span>
        <div className="topbar-actions">
          <button className="btn btn-sm" onClick={toggleTheme} title={zh ? '切换主题' : 'Toggle theme'}>
            {theme === 'dark' ? '☀' : '◑'}
          </button>
        </div>
      </header>

      <div className="list-page">
        <div className="list-header">
          <h1>{zh ? '自定义节点（节点 SDK）' : 'Custom Nodes (Node SDK)'}</h1>
        </div>

        <p style={{ marginBottom: 16, fontSize: 13, color: 'var(--muted)' }}>
          {zh
            ? '注册用节点 SDK 编写、通过 HTTP 提供的社区/第三方节点。注册后，工作流里的「Custom」节点即可选用。从节点服务的 GET /manifest 可一键获取下列字段。'
            : 'Register community/third-party nodes written with the node SDK and served over HTTP. Once registered, pick them from the "Custom" node in workflows. The fields below come from the node service\'s GET /manifest.'}
        </p>

        {error && <p style={{ color: '#ef4444', fontSize: 13, marginBottom: 12 }}>{error}</p>}

        {/* One-click import from a node service manifest */}
        <div style={{
          display: 'flex', gap: 8, alignItems: 'flex-end', marginBottom: 16,
          background: 'var(--surface)', border: '1px solid var(--border)',
          borderRadius: 'var(--radius)', padding: '12px 14px',
        }}>
          <div style={{ ...fieldStyle, flex: 1 }}>
            <label style={labelStyle}>{zh ? '从清单一键导入（节点服务地址）' : 'Import from manifest (node service URL)'}</label>
            <input className="input" placeholder="http://your-host:9000" value={importUrl} onChange={(e) => setImportUrl(e.target.value)} />
          </div>
          <button className="btn" onClick={handleImport} disabled={busy || !importUrl.trim()}>
            {zh ? '↓ 导入全部' : '↓ Import All'}
          </button>
        </div>
        {importMsg && <p style={{ color: '#16a34a', fontSize: 13, marginBottom: 12 }}>{importMsg}</p>}

        <div style={{
          display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 10, marginBottom: 20,
          background: 'var(--surface)', border: '1px solid var(--border)',
          borderRadius: 'var(--radius)', padding: '14px 16px',
        }}>
          <div style={fieldStyle}>
            <label style={labelStyle}>Slug</label>
            <input className="input" placeholder="greet" value={slug} onChange={(e) => setSlug(e.target.value)} />
          </div>
          <div style={fieldStyle}>
            <label style={labelStyle}>{zh ? '名称' : 'Label'}</label>
            <input className="input" placeholder="Greeter" value={label} onChange={(e) => setLabel(e.target.value)} />
          </div>
          <div style={{ ...fieldStyle, gridColumn: '1 / 3' }}>
            <label style={labelStyle}>Endpoint</label>
            <input className="input" placeholder="http://your-host:9000/nodes/greet" value={endpoint} onChange={(e) => setEndpoint(e.target.value)} />
          </div>
          <div style={{ ...fieldStyle, gridColumn: '1 / 3' }}>
            <label style={labelStyle}>{zh ? '描述' : 'Description'}</label>
            <input className="input" value={description} onChange={(e) => setDescription(e.target.value)} />
          </div>
          <div style={{ ...fieldStyle, gridColumn: '1 / 3' }}>
            <label style={labelStyle}>{zh ? '配置 Schema（JSON）' : 'Config schema (JSON)'}</label>
            <textarea className="input" rows={6} style={{ fontFamily: 'monospace', fontSize: 12 }} value={schema} onChange={(e) => setSchema(e.target.value)} />
          </div>
          <div style={{ gridColumn: '1 / 3', display: 'flex', justifyContent: 'flex-end' }}>
            <button className="btn btn-primary" onClick={handleSave} disabled={busy || !slug.trim() || !label.trim() || !endpoint.trim()}>
              {busy ? (zh ? '保存中…' : 'Saving…') : (zh ? '+ 注册节点' : '+ Register Node')}
            </button>
          </div>
        </div>

        {nodes.length === 0 ? (
          <p style={{ color: 'var(--muted)' }}>{zh ? '尚无自定义节点。' : 'No custom nodes yet.'}</p>
        ) : (
          <table className="data-table" style={{ width: '100%' }}>
            <thead>
              <tr>
                <th>{zh ? '名称' : 'Label'}</th>
                <th>Slug</th>
                <th>Endpoint</th>
                <th></th>
              </tr>
            </thead>
            <tbody>
              {nodes.map((n) => (
                <tr key={n.id}>
                  <td>{n.label}</td>
                  <td><code>{n.slug}</code></td>
                  <td style={{ maxWidth: 320, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }} title={n.endpoint}>{n.endpoint}</td>
                  <td>
                    <button className="btn btn-sm btn-danger" onClick={() => handleDelete(n.id, n.label)}>
                      {zh ? '删除' : 'Delete'}
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
    </div>
  )
}
