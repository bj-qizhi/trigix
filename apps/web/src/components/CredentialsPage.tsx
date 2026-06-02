// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useEffect, useState, useCallback } from 'react'
import { useAuth } from '../AuthContext'
import { useLocale } from '../useLocale'
import * as api from '../api/client'
import type { CredentialSummary } from '../types'
import { useTheme } from '../useTheme'

interface Props {
  onBack: () => void
}

function formatAge(ts: number): string {
  const d = Math.floor((Date.now() / 1000 - ts) / 86400)
  if (d === 0) return 'today'
  if (d === 1) return 'yesterday'
  return `${d}d ago`
}

function expiryInfo(expiresAt: number | undefined): { label: string; color: string } | null {
  if (!expiresAt) return null
  const now = Date.now() / 1000
  const daysLeft = Math.ceil((expiresAt - now) / 86400)
  if (daysLeft <= 0) return { label: 'Expired', color: 'var(--danger-text)' }
  if (daysLeft <= 7)  return { label: `${daysLeft}d`, color: '#f59e0b' }
  if (daysLeft <= 30) return { label: `${daysLeft}d`, color: '#eab308' }
  return { label: `${daysLeft}d`, color: 'var(--muted)' }
}

export function CredentialsPage({ onBack }: Props) {
  const { auth } = useAuth()
  const { theme, toggle: toggleTheme } = useTheme()
  const { t, locale } = useLocale()
  const zh = locale === 'zh'
  const [credentials, setCredentials] = useState<CredentialSummary[]>([])
  const [usageMap, setUsageMap]       = useState<Record<string, api.CredentialUsageEntry[]>>({})
  const [loading, setLoading]         = useState(true)
  const [error, setError]             = useState<string | null>(null)
  const [adding, setAdding]           = useState(false)
  const [name, setName]               = useState('')
  const [value, setValue]             = useState('')
  const [description, setDescription] = useState('')
  const [expiresAt, setExpiresAt]     = useState('')
  const [saving, setSaving]           = useState(false)
  const [search, setSearch]           = useState('')
  const [copiedRef, setCopiedRef]     = useState<string | null>(null)
  const [editingId, setEditingId]     = useState<string | null>(null)
  const [editValue, setEditValue]     = useState('')
  const [editDesc, setEditDesc]       = useState('')
  const [editExpiry, setEditExpiry]   = useState('')
  const [editSaving, setEditSaving]   = useState(false)

  const load = useCallback(() => {
    setLoading(true)
    setError(null)
    Promise.all([
      api.listCredentials(auth!.tenantId),
      api.getCredentialUsage(auth!.tenantId),
    ])
      .then(([creds, usage]) => {
        setCredentials(creds)
        setUsageMap(usage.usages)
      })
      .catch((e: unknown) => setError(String(e)))
      .finally(() => setLoading(false))
  }, [auth])

  useEffect(load, [load])

  const handleAdd = async () => {
    if (!name.trim() || !value.trim()) return
    setSaving(true)
    try {
      const cred = await api.createCredential(auth!.tenantId, name.trim(), value.trim())
      // If description or expiry set, patch immediately
      const hasExtra = description.trim() || expiresAt
      if (hasExtra) {
        const expTs = expiresAt ? Math.floor(new Date(expiresAt).getTime() / 1000) : null
        await api.updateCredential(auth!.tenantId, cred.id, {
          description: description.trim() || null,
          expires_at: expTs,
        })
        load()
      } else {
        setCredentials((prev) => [...prev, cred].sort((a, b) => a.name.localeCompare(b.name)))
      }
      setAdding(false)
      setName(''); setValue(''); setDescription(''); setExpiresAt('')
    } catch (e) {
      setError(String(e))
    } finally {
      setSaving(false)
    }
  }

  const handleDelete = async (id: string) => {
    if (!window.confirm(zh ? '删除此凭证？' : 'Delete this credential?')) return
    try {
      await api.deleteCredential(auth!.tenantId, id)
      setCredentials((prev) => prev.filter((c) => c.id !== id))
    } catch (e) {
      setError(String(e))
    }
  }

  const openEdit = (cred: CredentialSummary) => {
    setEditingId(cred.id)
    setEditValue('')
    setEditDesc(cred.description ?? '')
    if (cred.expires_at) {
      const d = new Date(cred.expires_at * 1000)
      setEditExpiry(d.toISOString().slice(0, 10))
    } else {
      setEditExpiry('')
    }
  }

  const handleSaveEdit = async () => {
    if (!editingId) return
    setEditSaving(true)
    try {
      const expTs = editExpiry ? Math.floor(new Date(editExpiry).getTime() / 1000) : null
      await api.updateCredential(auth!.tenantId, editingId, {
        value: editValue.trim() || undefined,
        description: editDesc.trim() || null,
        expires_at: expTs,
      })
      setEditingId(null)
      load()
    } catch (e) {
      setError(String(e))
    } finally {
      setEditSaving(false)
    }
  }

  const now = Date.now() / 1000
  const expiringSoon = credentials.filter((c) => c.expires_at && c.expires_at - now < 30 * 86400)

  return (
    <div className="app">
      <header className="topbar">
        <button className="btn btn-sm btn-icon" onClick={onBack} title={zh ? '返回' : 'Back'}>←</button>
        <span className="topbar-sep">|</span>
        <span className="topbar-logo">aiworkflow</span>
        <span className="topbar-sep">|</span>
        <span className="topbar-title">{t('credentials.title')}</span>
        <div className="topbar-actions">
          <button className="btn btn-sm btn-primary" onClick={() => setAdding(true)}>
            + {zh ? '添加凭证' : 'Add Credential'}
          </button>
          <button className="btn btn-sm" onClick={toggleTheme} title={zh ? '切换主题' : 'Toggle dark/light theme'}>{theme === 'dark' ? '☀' : '◑'}</button>
        </div>
      </header>

      <div className="list-page">
        <div className="list-header">
          <h1>{t('credentials.title')}</h1>
        </div>

        {expiringSoon.length > 0 && (
          <div style={{ background: '#7c2d12', color: '#fed7aa', padding: '8px 12px', borderRadius: 6, marginBottom: 12, fontSize: 13 }}>
            ⚠ {expiringSoon.length} credential{expiringSoon.length > 1 ? 's' : ''} expiring within 30 days:{' '}
            {expiringSoon.map((c) => {
              const info = expiryInfo(c.expires_at)
              return <strong key={c.id} style={{ marginRight: 8 }}>{c.name} ({info?.label})</strong>
            })}
          </div>
        )}

        <p style={{ marginBottom: 16 }}>
          {zh
            ? <>在节点配置中使用 <code style={{ background: 'var(--panel)', padding: '2px 6px', borderRadius: 4, fontSize: 12 }}>{'{{credential.name}}'}</code> 引用这里存储的密钥。API 不返回实际值。</>
            : <>Store secrets here and reference them in node configs with{' '}
              <code style={{ background: 'var(--panel)', padding: '2px 6px', borderRadius: 4, fontSize: 12 }}>{'{{credential.name}}'}</code>
              . Values are never returned by the API.</>
          }
        </p>

        {error && (
          <div style={{ color: 'var(--danger-text)', marginBottom: 12, fontSize: 13 }}>{error}</div>
        )}

        {!loading && credentials.length > 0 && (
          <div style={{ marginBottom: 10 }}>
            <input
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder={zh ? '搜索凭证名称…' : 'Filter credentials…'}
              style={{ fontSize: 13, padding: '3px 8px', width: 220 }}
            />
          </div>
        )}

        {loading ? (
          <div style={{ color: 'var(--muted)' }}>{zh ? '加载中…' : 'Loading…'}</div>
        ) : (
          <table className="workflow-table">
            <thead>
              <tr>
                <th>{zh ? '名称' : 'Name'}</th>
                <th>{zh ? '引用变量' : 'Reference'}</th>
                <th>{zh ? '到期时间' : 'Expires'}</th>
                <th>{zh ? '修改时间' : 'Updated'}</th>
                <th>{zh ? '使用的工作流' : 'Used by'}</th>
                <th style={{ width: 140 }}></th>
              </tr>
            </thead>
            <tbody>
              {credentials.length === 0 ? (
                <tr>
                  <td colSpan={6}>
                    <div className="empty-state">{zh ? '暂无凭证，请添加一个开始使用。' : 'No credentials yet. Add one to get started.'}</div>
                  </td>
                </tr>
              ) : (() => {
                const visible = search ? credentials.filter((c) => c.name.toLowerCase().includes(search.toLowerCase())) : credentials
                if (visible.length === 0) return (
                  <tr><td colSpan={6} style={{ color: 'var(--muted)', textAlign: 'center', padding: 16, fontSize: 13 }}>{zh ? '无匹配凭证' : 'No matching credentials'}</td></tr>
                )
                return visible.map((cred) => {
                  const ref = `{{credential.${cred.name}}}`
                  const expInfo = expiryInfo(cred.expires_at)
                  return (
                    <tr key={cred.id}>
                      <td>
                        <div style={{ fontWeight: 500 }}>{cred.name}</div>
                        {cred.description && <div style={{ fontSize: 11, color: 'var(--muted)', marginTop: 2 }}>{cred.description}</div>}
                      </td>
                      <td>
                        <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                          <code style={{ fontSize: 12, color: 'var(--muted)' }}>{ref}</code>
                          <button
                            className="btn btn-sm btn-icon"
                            style={{ fontSize: 10, padding: '1px 5px' }}
                            title={zh ? '复制引用变量' : 'Copy reference'}
                            onClick={() => {
                              navigator.clipboard.writeText(ref).then(() => {
                                setCopiedRef(cred.id)
                                setTimeout(() => setCopiedRef(null), 1500)
                              }).catch(() => {})
                            }}
                          >
                            {copiedRef === cred.id ? '✓' : '⎘'}
                          </button>
                        </div>
                      </td>
                      <td>
                        {expInfo ? (
                          <span style={{ fontSize: 12, color: expInfo.color, fontWeight: 500 }}>
                            {expInfo.label === 'Expired' ? '🔴 Expired' : `⏱ ${expInfo.label}`}
                          </span>
                        ) : (
                          <span style={{ fontSize: 12, color: 'var(--muted)' }}>—</span>
                        )}
                      </td>
                      <td style={{ fontSize: 12, color: 'var(--muted)' }}>
                        {cred.updated_at ? formatAge(cred.updated_at) : '—'}
                      </td>
                      <td>
                        {(() => {
                          const wfs = usageMap[cred.name] ?? []
                          if (wfs.length === 0) return <span style={{ fontSize: 12, color: 'var(--muted)' }}>—</span>
                          return (
                            <div style={{ display: 'flex', flexWrap: 'wrap', gap: 4 }}>
                              {wfs.map((u) => (
                                <span
                                  key={u.workflow_id}
                                  title={`${u.workflow_name} (v${u.version})`}
                                  style={{ fontSize: 11, background: 'var(--surface)', border: '1px solid var(--border)', borderRadius: 4, padding: '1px 6px', color: 'var(--link)', whiteSpace: 'nowrap' }}
                                >
                                  {u.workflow_name.length > 16 ? u.workflow_name.slice(0, 14) + '…' : u.workflow_name}
                                </span>
                              ))}
                            </div>
                          )
                        })()}
                      </td>
                      <td>
                        <div style={{ display: 'flex', gap: 4 }}>
                          <button
                            className="btn btn-sm"
                            onClick={() => openEdit(cred)}
                            title={zh ? '编辑/轮换' : 'Edit / Rotate'}
                          >
                            ✎
                          </button>
                          <button
                            className="btn btn-sm btn-danger"
                            onClick={() => handleDelete(cred.id)}
                          >
                            {t('common.delete')}
                          </button>
                        </div>
                      </td>
                    </tr>
                  )
                })
              })()}
            </tbody>
          </table>
        )}
      </div>

      {adding && (
        <div className="modal-backdrop" onClick={() => setAdding(false)}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            <h2>{zh ? '添加凭证' : 'Add Credential'}</h2>
            <div className="field">
              <label>{zh ? `名称（用于 ${'{{credential.name}}'}）` : `Name (used in ${'{{credential.name}}'})`}</label>
              <input
                autoFocus
                placeholder={zh ? '如 openai-api-key' : 'e.g. openai-api-key'}
                value={name}
                onChange={(e) => setName(e.target.value)}
                onKeyDown={(e) => { if (e.key === 'Enter') handleAdd() }}
              />
            </div>
            <div className="field">
              <label>{zh ? '密钥值' : 'Secret Value'}</label>
              <input
                type="password"
                placeholder={zh ? '粘贴密钥到此处' : 'Paste the secret here'}
                value={value}
                onChange={(e) => setValue(e.target.value)}
                onKeyDown={(e) => { if (e.key === 'Enter') handleAdd() }}
              />
            </div>
            <div className="field">
              <label>{zh ? '描述（可选）' : 'Description (optional)'}</label>
              <input
                placeholder={zh ? '如 OpenAI production key' : 'e.g. OpenAI production key'}
                value={description}
                onChange={(e) => setDescription(e.target.value)}
              />
            </div>
            <div className="field">
              <label>{zh ? '到期日期（可选）' : 'Expiry Date (optional)'}</label>
              <input
                type="date"
                value={expiresAt}
                onChange={(e) => setExpiresAt(e.target.value)}
              />
            </div>
            <div className="modal-actions">
              <button className="btn" onClick={() => { setAdding(false); setName(''); setValue(''); setDescription(''); setExpiresAt('') }}>
                {t('common.cancel')}
              </button>
              <button
                className="btn btn-primary"
                disabled={saving || !name.trim() || !value.trim()}
                onClick={handleAdd}
              >
                {saving ? (zh ? '保存中…' : 'Saving…') : (zh ? '添加' : 'Add')}
              </button>
            </div>
          </div>
        </div>
      )}

      {editingId && (
        <div className="modal-backdrop" onClick={() => setEditingId(null)}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            <h2>{zh ? '编辑凭证' : 'Edit Credential'}</h2>
            <div className="field">
              <label>{zh ? '新密钥值（留空则保持不变）' : 'New Secret Value (leave blank to keep current)'}</label>
              <input
                type="password"
                placeholder={zh ? '粘贴新密钥…' : 'Paste new secret…'}
                value={editValue}
                onChange={(e) => setEditValue(e.target.value)}
              />
            </div>
            <div className="field">
              <label>{zh ? '描述' : 'Description'}</label>
              <input
                placeholder={zh ? '描述此凭证的用途' : 'Describe what this credential is for'}
                value={editDesc}
                onChange={(e) => setEditDesc(e.target.value)}
              />
            </div>
            <div className="field">
              <label>{zh ? '到期日期（留空则清除）' : 'Expiry Date (leave blank to clear)'}</label>
              <input
                type="date"
                value={editExpiry}
                onChange={(e) => setEditExpiry(e.target.value)}
              />
            </div>
            <div className="modal-actions">
              <button className="btn" onClick={() => setEditingId(null)}>{t('common.cancel')}</button>
              <button
                className="btn btn-primary"
                disabled={editSaving}
                onClick={handleSaveEdit}
              >
                {editSaving ? (zh ? '保存中…' : 'Saving…') : (zh ? '保存' : 'Save')}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
