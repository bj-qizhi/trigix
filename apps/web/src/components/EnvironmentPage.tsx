// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// Contact: managecode@gmail.com

import { useEffect, useRef, useState } from 'react'
import { useAuth } from '../AuthContext'
import { useLocale } from '../useLocale'
import * as api from '../api/client'
import type { EnvVarRecord, EnvSetSummary } from '../types'
import { useTheme } from '../useTheme'

const DEFAULT_SET = 'default'

interface Props {
  onBack: () => void
}

export function EnvironmentPage({ onBack }: Props) {
  const { auth } = useAuth()
  const { theme, toggle: toggleTheme } = useTheme()
  const { t, locale } = useLocale()
  const zh = locale === 'zh'
  const [sets, setSets] = useState<EnvSetSummary[]>([])
  const [activeSet, setActiveSet] = useState(DEFAULT_SET)
  const [vars, setVars] = useState<EnvVarRecord[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [adding, setAdding] = useState(false)
  const [newKey, setNewKey] = useState('')
  const [newValue, setNewValue] = useState('')
  const [saving, setSaving] = useState(false)
  const [editing, setEditing] = useState<string | null>(null)
  const [editValue, setEditValue] = useState('')
  const [deleting, setDeleting] = useState<string | null>(null)
  const [newSetName, setNewSetName] = useState('')
  const [creatingSet, setCreatingSet] = useState(false)
  const [varSearch, setVarSearch] = useState('')
  const [importing, setImporting] = useState(false)
  const importRef = useRef<HTMLInputElement>(null)

  const loadSets = () => {
    api.listEnvSets(auth!.tenantId)
      .then((s) => {
        const hasDefault = s.some((x) => x.name === DEFAULT_SET)
        setSets(hasDefault ? s : [{ name: DEFAULT_SET, var_count: 0 }, ...s])
      })
      .catch(() => {})
  }

  const loadVars = (set: string) => {
    setLoading(true)
    setError(null)
    api.listEnvVars(auth!.tenantId, set === DEFAULT_SET ? undefined : set)
      .then(setVars)
      .catch((e: unknown) => setError(String(e)))
      .finally(() => setLoading(false))
  }

  useEffect(() => {
    loadSets()
    loadVars(activeSet)
  }, [])

  const switchSet = (name: string) => {
    setActiveSet(name)
    setEditing(null)
    setAdding(false)
    loadVars(name)
  }

  const handleAdd = async () => {
    const k = newKey.trim()
    const v = newValue.trim()
    if (!k) return
    setSaving(true)
    try {
      const setParam = activeSet === DEFAULT_SET ? undefined : activeSet
      const rec = await api.upsertEnvVar(auth!.tenantId, k, v, setParam)
      setVars((prev) => {
        const filtered = prev.filter((r) => r.key !== rec.key)
        return [...filtered, rec].sort((a, b) => a.key.localeCompare(b.key))
      })
      setAdding(false)
      setNewKey('')
      setNewValue('')
      loadSets()
    } catch (e) {
      alert(String(e))
    } finally {
      setSaving(false)
    }
  }

  const handleSaveEdit = async (key: string) => {
    setSaving(true)
    try {
      const setParam = activeSet === DEFAULT_SET ? undefined : activeSet
      const rec = await api.upsertEnvVar(auth!.tenantId, key, editValue, setParam)
      setVars((prev) => prev.map((r) => (r.key === key ? rec : r)))
      setEditing(null)
    } catch (e) {
      alert(String(e))
    } finally {
      setSaving(false)
    }
  }

  const handleDelete = async (key: string) => {
    setDeleting(key)
    try {
      const setParam = activeSet === DEFAULT_SET ? undefined : activeSet
      await api.deleteEnvVar(auth!.tenantId, key, setParam)
      setVars((prev) => prev.filter((r) => r.key !== key))
      loadSets()
    } catch (e) {
      alert(String(e))
    } finally {
      setDeleting(null)
    }
  }

  const handleDeleteSet = async (name: string) => {
    if (!confirm(zh ? `删除变量组"${name}"及其所有变量？` : `Delete set "${name}" and all its variables?`)) return
    try {
      await api.deleteEnvSet(auth!.tenantId, name)
      setSets((prev) => prev.filter((s) => s.name !== name))
      if (activeSet === name) switchSet(DEFAULT_SET)
    } catch (e) {
      alert(String(e))
    }
  }

  const handleImportDotEnv = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0]
    if (!file) return
    e.target.value = ''
    setImporting(true)
    try {
      const text = await file.text()
      const pairs: { key: string; value: string }[] = []
      for (const rawLine of text.split('\n')) {
        const line = rawLine.trim()
        if (!line || line.startsWith('#')) continue
        const eqIdx = line.indexOf('=')
        if (eqIdx < 1) continue
        const key = line.slice(0, eqIdx).trim().toUpperCase().replace(/[^A-Z0-9_]/g, '_')
        let value = line.slice(eqIdx + 1).trim()
        if ((value.startsWith('"') && value.endsWith('"')) || (value.startsWith("'") && value.endsWith("'"))) {
          value = value.slice(1, -1)
        }
        if (key) pairs.push({ key, value })
      }
      if (pairs.length === 0) { alert(zh ? '文件中未找到有效变量。' : 'No valid variables found in file.'); return }
      const setParam = activeSet === DEFAULT_SET ? undefined : activeSet
      let ok = 0
      for (const { key, value } of pairs) {
        const rec = await api.upsertEnvVar(auth!.tenantId, key, value, setParam)
        setVars((prev) => {
          const filtered = prev.filter((r) => r.key !== rec.key)
          return [...filtered, rec].sort((a, b) => a.key.localeCompare(b.key))
        })
        ok++
      }
      loadSets()
      alert(zh ? `已导入 ${ok} 个变量。` : `Imported ${ok} variable${ok !== 1 ? 's' : ''}.`)
    } catch (err) {
      alert(String(err))
    } finally {
      setImporting(false)
    }
  }

  const handleCreateSet = () => {
    const name = newSetName.trim().toLowerCase().replace(/[^a-z0-9_-]/g, '_')
    if (!name || name === DEFAULT_SET) return
    setCreatingSet(false)
    setNewSetName('')
    setSets((prev) => {
      if (prev.some((s) => s.name === name)) return prev
      return [...prev, { name, var_count: 0 }].sort((a, b) => {
        if (a.name === DEFAULT_SET) return -1
        if (b.name === DEFAULT_SET) return 1
        return a.name.localeCompare(b.name)
      })
    })
    switchSet(name)
  }

  return (
    <div className="app">
      <header className="topbar">
        <button className="btn btn-sm btn-icon" onClick={onBack} title={zh ? '返回' : 'Back'}>←</button>
        <span className="topbar-sep">|</span>
        <span className="topbar-title">{zh ? '环境变量' : 'Environment Variables'}</span>
        <div className="topbar-actions">
          <button className="btn btn-primary btn-sm" onClick={() => { setAdding(true); setNewKey(''); setNewValue('') }}>
            + {zh ? '添加变量' : 'Add Variable'}
          </button>
          <input ref={importRef} type="file" accept=".env,text/plain" style={{ display: 'none' }} onChange={handleImportDotEnv} />
          <button
            className="btn btn-sm"
            disabled={importing}
            onClick={() => importRef.current?.click()}
            title={zh ? '从 .env 文件批量导入变量' : 'Bulk import from a .env file'}
          >
            {importing ? '…' : `↑ ${zh ? '导入 .env' : 'Import .env'}`}
          </button>
          <button className="btn btn-sm" onClick={toggleTheme} title={zh ? '切换主题' : 'Toggle dark/light theme'}>{theme === 'dark' ? '☀' : '◑'}</button>
        </div>
      </header>

      <main style={{ display: 'flex', flex: 1, overflow: 'hidden' }}>
        {/* Set sidebar */}
        <aside style={{ width: 200, borderRight: '1px solid var(--border)', padding: '16px 0', display: 'flex', flexDirection: 'column', gap: 4, overflowY: 'auto' }}>
          <div style={{ padding: '0 12px 8px', fontSize: 11, fontWeight: 600, color: 'var(--muted)', textTransform: 'uppercase', letterSpacing: '0.05em' }}>
            {zh ? '变量组' : 'Env Sets'}
          </div>
          {sets.map((s) => (
            <div
              key={s.name}
              style={{
                display: 'flex', alignItems: 'center', gap: 6, padding: '6px 12px',
                cursor: 'pointer', borderRadius: 4, margin: '0 4px',
                background: activeSet === s.name ? 'var(--panel)' : 'transparent',
                color: activeSet === s.name ? 'var(--text)' : 'var(--muted)',
              }}
              onClick={() => switchSet(s.name)}
            >
              <span style={{ flex: 1, fontWeight: activeSet === s.name ? 600 : 400, fontSize: 13, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                {s.name === DEFAULT_SET ? (zh ? '默认' : 'default') : s.name}
              </span>
              <span style={{ fontSize: 11, color: 'var(--muted)', flexShrink: 0 }}>{s.var_count}</span>
              {s.name !== DEFAULT_SET && (
                <button
                  className="btn btn-sm btn-icon"
                  style={{ padding: '0 2px', fontSize: 11, opacity: 0.5, lineHeight: 1 }}
                  onClick={(e) => { e.stopPropagation(); handleDeleteSet(s.name) }}
                  title={zh ? `删除变量组"${s.name}"` : `Delete set "${s.name}"`}
                >
                  ✕
                </button>
              )}
            </div>
          ))}
          {creatingSet ? (
            <div style={{ padding: '6px 12px', display: 'flex', gap: 4, alignItems: 'center' }}>
              <input
                autoFocus
                value={newSetName}
                onChange={(e) => setNewSetName(e.target.value.toLowerCase())}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') handleCreateSet()
                  if (e.key === 'Escape') { setCreatingSet(false); setNewSetName('') }
                }}
                placeholder={zh ? '组名称' : 'set name'}
                style={{ flex: 1, fontSize: 12, padding: '2px 4px' }}
              />
              <button className="btn btn-sm btn-primary" onClick={handleCreateSet} style={{ padding: '2px 6px' }}>✓</button>
            </div>
          ) : (
            <button
              className="btn btn-sm"
              style={{ margin: '4px 8px 0', fontSize: 12 }}
              onClick={() => setCreatingSet(true)}
            >
              + {zh ? '新建组' : 'New Set'}
            </button>
          )}
        </aside>

        {/* Vars content */}
        <div className="list-page" style={{ flex: 1, overflow: 'auto' }}>
          <div className="list-header">
            <h1>
              {activeSet === DEFAULT_SET ? (zh ? '默认变量组' : 'Default Set') : `${zh ? '变量组：' : 'Set: '}${activeSet}`}
            </h1>
          </div>

          <p style={{ color: 'var(--muted)', fontSize: 13, marginBottom: 20 }}>
            {zh ? <>在节点配置中使用 <code style={{ background: 'var(--panel)', padding: '1px 5px', borderRadius: 3 }}>{'{{env.KEY}}'}</code> 引用变量。</> : <>Use <code style={{ background: 'var(--panel)', padding: '1px 5px', borderRadius: 3 }}>{'{{env.KEY}}'}</code> in node configs.</>}
            {activeSet !== DEFAULT_SET && (
              zh
                ? <> 在运行面板中选择 <code style={{ background: 'var(--panel)', padding: '1px 5px', borderRadius: 3 }}>{activeSet}</code> 以使用这些值。</>
                : <> Select set <code style={{ background: 'var(--panel)', padding: '1px 5px', borderRadius: 3 }}>{activeSet}</code> in the run panel to use these values.</>
            )}
          </p>

          {!loading && vars.length > 0 && (
            <div style={{ marginBottom: 12 }}>
              <input
                value={varSearch}
                onChange={(e) => setVarSearch(e.target.value)}
                placeholder={zh ? '搜索变量名…' : 'Filter variables…'}
                style={{ fontSize: 13, padding: '3px 8px', width: 220 }}
              />
            </div>
          )}

          {loading && <p>{zh ? '加载中…' : 'Loading…'}</p>}
          {error && <p style={{ color: 'var(--danger-text)' }}>{error}</p>}

          {!loading && !error && vars.length === 0 && (
            <div className="empty-state">
              <p>{t('env.empty')}</p>
            </div>
          )}

          {!loading && vars.length > 0 && (() => {
            const displayVars = varSearch
              ? vars.filter((v) => v.key.toLowerCase().includes(varSearch.toLowerCase()))
              : vars
            return (
            <table className="workflow-table">
              <thead>
                <tr>
                  <th>{t('env.col.key')}</th>
                  <th>{t('env.col.value')}</th>
                  <th></th>
                </tr>
              </thead>
              <tbody>
                {displayVars.map((v) => (
                  <tr key={v.key}>
                    <td>
                      <code style={{ fontFamily: 'monospace', fontSize: 13 }}>{v.key}</code>
                    </td>
                    <td style={{ maxWidth: 400 }}>
                      {editing === v.key ? (
                        <div style={{ display: 'flex', gap: 6, alignItems: 'center' }}>
                          <input
                            autoFocus
                            value={editValue}
                            onChange={(e) => setEditValue(e.target.value)}
                            onKeyDown={(e) => {
                              if (e.key === 'Enter') handleSaveEdit(v.key)
                              if (e.key === 'Escape') setEditing(null)
                            }}
                            style={{ flex: 1 }}
                          />
                          <button className="btn btn-sm btn-primary" disabled={saving} onClick={() => handleSaveEdit(v.key)}>{zh ? '保存' : 'Save'}</button>
                          <button className="btn btn-sm" onClick={() => setEditing(null)}>{zh ? '取消' : 'Cancel'}</button>
                        </div>
                      ) : (
                        <span
                          style={{ fontFamily: 'monospace', fontSize: 12, color: 'var(--muted)', cursor: 'pointer' }}
                          onClick={() => { setEditing(v.key); setEditValue(v.value) }}
                          title={zh ? '点击编辑' : 'Click to edit'}
                        >
                          {'•'.repeat(Math.min(v.value.length, 24))}
                          {v.value.length > 24 ? '…' : ''}
                        </span>
                      )}
                    </td>
                    <td style={{ display: 'flex', gap: 6, alignItems: 'center' }}>
                      <button className="btn btn-sm" onClick={() => { setEditing(v.key); setEditValue(v.value) }}>{zh ? '编辑' : 'Edit'}</button>
                      <button
                        className="btn btn-sm btn-danger"
                        disabled={deleting === v.key}
                        onClick={() => handleDelete(v.key)}
                      >
                        {deleting === v.key ? '…' : (zh ? '删除' : 'Delete')}
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
            )
          })()}
        </div>
      </main>

      {adding && (
        <div className="modal-backdrop" onClick={() => setAdding(false)}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            <h2>{zh ? `添加变量到"${activeSet}"` : `Add Variable to "${activeSet}"`}</h2>
            <div className="field">
              <label>{t('env.col.key')}</label>
              <input
                autoFocus
                placeholder={zh ? '如 API_BASE_URL' : 'e.g. API_BASE_URL'}
                value={newKey}
                onChange={(e) => setNewKey(e.target.value.toUpperCase().replace(/[^A-Z0-9_]/g, '_'))}
                onKeyDown={(e) => e.key === 'Enter' && handleAdd()}
                style={{ fontFamily: 'monospace' }}
              />
            </div>
            <div className="field">
              <label>{t('env.col.value')}</label>
              <input
                placeholder={zh ? '如 https://api.example.com' : 'e.g. https://api.example.com'}
                value={newValue}
                onChange={(e) => setNewValue(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && handleAdd()}
              />
            </div>
            <div className="modal-actions">
              <button className="btn" onClick={() => setAdding(false)}>{zh ? '取消' : 'Cancel'}</button>
              <button className="btn btn-primary" disabled={!newKey.trim() || saving} onClick={handleAdd}>
                {saving ? (zh ? '添加中…' : 'Adding…') : (zh ? '添加' : 'Add')}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
