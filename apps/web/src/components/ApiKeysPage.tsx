// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useEffect, useState } from 'react'
import { useAuth } from '../AuthContext'
import { useLocale } from '../useLocale'
import * as api from '../api/client'
import { useTheme } from '../useTheme'
import logoWordmark from '../assets/logo-wordmark.svg'

interface ApiKeyRecord {
  id: string
  tenant_id: string
  name: string
  prefix: string
  created_at: number
}

interface Props {
  onBack: () => void
}

function formatTs(secs: number): string {
  return new Date(secs * 1000).toLocaleString()
}

export function ApiKeysPage({ onBack }: Props) {
  const { auth } = useAuth()
  const { theme, toggle: toggleTheme } = useTheme()
  const { t, locale } = useLocale()
  const zh = locale === 'zh'
  const [keys, setKeys] = useState<ApiKeyRecord[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [newName, setNewName] = useState('')
  const [creating, setCreating] = useState(false)
  const [revealed, setRevealed] = useState<{ id: string; key: string } | null>(null)
  const [copied, setCopied] = useState(false)

  const load = () => {
    setLoading(true)
    api.listApiKeys(auth!.tenantId)
      .then(setKeys)
      .catch((e: unknown) => setError(String(e)))
      .finally(() => setLoading(false))
  }

  useEffect(load, [])

  const handleCreate = async () => {
    if (!newName.trim()) return
    setCreating(true)
    try {
      const result = await api.createApiKey(auth!.tenantId, newName.trim())
      setRevealed({ id: result.id, key: result.key })
      setNewName('')
      load()
    } catch (e: unknown) {
      setError(String(e))
    } finally {
      setCreating(false)
    }
  }

  const handleDelete = async (id: string) => {
    if (!window.confirm(zh ? '撤销此 API 密钥？此操作不可撤回。' : 'Revoke this API key? This cannot be undone.')) return
    try {
      await api.deleteApiKey(auth!.tenantId, id)
      setKeys((prev) => prev.filter((k) => k.id !== id))
      if (revealed?.id === id) setRevealed(null)
    } catch (e: unknown) {
      setError(String(e))
    }
  }

  const handleCopy = (text: string) => {
    navigator.clipboard.writeText(text).then(() => {
      setCopied(true)
      setTimeout(() => setCopied(false), 1500)
    })
  }

  return (
    <div className="app">
      <header className="topbar">
        <button className="btn btn-sm btn-icon" onClick={onBack} title={zh ? '返回' : 'Back'}>←</button>
        <span className="topbar-sep">|</span>
        <img src={logoWordmark} alt="Velara" style={{ height: '28px' }} />
        <span className="topbar-sep">|</span>
        <span className="topbar-title">{t('apikeys.title')}</span>
        <div className="topbar-actions">
          <button className="btn btn-sm" onClick={toggleTheme} title={zh ? '切换主题' : 'Toggle dark/light theme'}>
            {theme === 'dark' ? '☀' : '◑'}
          </button>
        </div>
      </header>

      <div className="list-page">
        <div className="list-header">
          <h1>{t('apikeys.title')}</h1>
        </div>

        <p style={{ marginBottom: 16, fontSize: 13, color: 'var(--muted)' }}>
          {zh
            ? <>API 密钥通过 <code style={{ background: 'var(--panel)', padding: '2px 5px', borderRadius: 4 }}>POST /v1/auth/token</code> 获取短效 JWT。完整密钥仅在创建时显示一次。</>
            : <>API keys authenticate to <code style={{ background: 'var(--panel)', padding: '2px 5px', borderRadius: 4 }}>POST /v1/auth/token</code> to receive a short-lived JWT. The full key is shown only once at creation time.</>
          }
        </p>

        {/* Create form */}
        <div style={{
          display: 'flex', gap: 8, marginBottom: 20, alignItems: 'center',
          background: 'var(--surface)', border: '1px solid var(--border)',
          borderRadius: 'var(--radius)', padding: '12px 14px',
        }}>
          <input
            className="input"
            style={{ flex: 1 }}
            placeholder={zh ? '密钥名称（如 CI/CD、Dashboard）' : 'Key name (e.g. CI/CD, Dashboard)'}
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && handleCreate()}
          />
          <button
            className="btn btn-primary"
            onClick={handleCreate}
            disabled={creating || !newName.trim()}
          >
            {creating ? (zh ? '创建中…' : 'Creating…') : (zh ? '+ 创建密钥' : '+ Create Key')}
          </button>
        </div>

        {/* One-time key reveal */}
        {revealed && (
          <div style={{
            background: 'var(--panel)', border: '1px solid var(--success-border, #16a34a)',
            borderRadius: 'var(--radius)', padding: '12px 14px', marginBottom: 16,
          }}>
            <div style={{ fontSize: 12, fontWeight: 600, color: '#16a34a', marginBottom: 8 }}>
              ✓ {zh ? 'API 密钥已创建 — 请立即复制，将不再显示' : 'API key created — copy it now, it won\'t be shown again'}
            </div>
            <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
              <code style={{
                flex: 1, fontFamily: 'monospace', fontSize: 13,
                background: 'var(--surface)', padding: '6px 10px',
                borderRadius: 4, border: '1px solid var(--border)',
                wordBreak: 'break-all',
              }}>
                {revealed.key}
              </code>
              <button
                className="btn btn-sm btn-primary"
                onClick={() => handleCopy(revealed.key)}
              >
                {copied ? (zh ? '✓ 已复制' : '✓ Copied') : (zh ? '复制' : 'Copy')}
              </button>
              <button
                className="btn btn-sm"
                onClick={() => setRevealed(null)}
              >
                {zh ? '关闭' : 'Dismiss'}
              </button>
            </div>
          </div>
        )}

        {error && (
          <div style={{ color: 'var(--danger-text)', marginBottom: 12, fontSize: 13 }}>{error}</div>
        )}

        {loading ? (
          <div style={{ color: 'var(--muted)' }}>{zh ? '加载中…' : 'Loading…'}</div>
        ) : keys.length === 0 ? (
          <div className="empty-state">{zh ? '暂无 API 密钥，请在上方创建一个。' : 'No API keys yet. Create one above to get started.'}</div>
        ) : (
          <table className="workflow-table">
            <thead>
              <tr>
                <th>{zh ? '名称' : 'Name'}</th>
                <th>{zh ? '前缀' : 'Prefix'}</th>
                <th>{zh ? '创建时间' : 'Created'}</th>
                <th style={{ width: 100 }}></th>
              </tr>
            </thead>
            <tbody>
              {keys.map((k) => (
                <tr key={k.id}>
                  <td style={{ fontWeight: 500 }}>{k.name}</td>
                  <td>
                    <code style={{ fontFamily: 'monospace', fontSize: 12, color: 'var(--muted)' }}>
                      {k.prefix}…
                    </code>
                  </td>
                  <td style={{ fontSize: 12, color: 'var(--muted)' }}>
                    {formatTs(k.created_at)}
                  </td>
                  <td>
                    <button
                      className="btn btn-sm btn-danger"
                      onClick={() => handleDelete(k.id)}
                    >
                      {t('apikeys.revoke')}
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
