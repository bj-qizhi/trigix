// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useEffect, useState } from 'react'
import { useLocale } from '../useLocale'
import { useTheme } from '../useTheme'
import * as api from '../api/client'
import type { SsoConnection } from '../api/client'
import logoWordmark from '../assets/logo-wordmark.svg'

interface Props {
  onBack: () => void
}

function formatTs(secs: number): string {
  return new Date(secs * 1000).toLocaleString()
}

export function SsoSettingsPage({ onBack }: Props) {
  const { theme, toggle: toggleTheme } = useTheme()
  const { locale } = useLocale()
  const zh = locale === 'zh'

  const [conns, setConns] = useState<SsoConnection[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [creating, setCreating] = useState(false)
  const [copied, setCopied] = useState<string | null>(null)

  const [slug, setSlug] = useState('')
  const [provider, setProvider] = useState('')
  const [issuer, setIssuer] = useState('')
  const [clientId, setClientId] = useState('')
  const [clientSecret, setClientSecret] = useState('')
  const [scopes, setScopes] = useState('openid email profile')

  const load = () => {
    setLoading(true)
    api
      .listSsoConnections()
      .then(setConns)
      .catch((e: unknown) => setError(String(e)))
      .finally(() => setLoading(false))
  }

  useEffect(load, [])

  const callbackUrl = (s: string) => `${window.location.origin}/v1/sso/${s}/callback`

  const copy = (text: string) => {
    navigator.clipboard.writeText(text).then(() => {
      setCopied(text)
      setTimeout(() => setCopied(null), 1500)
    })
  }

  const handleCreate = async () => {
    if (!slug.trim() || !provider.trim() || !issuer.trim() || !clientId.trim() || !clientSecret.trim()) return
    setCreating(true)
    setError(null)
    try {
      await api.createSsoConnection({
        slug: slug.trim(),
        provider: provider.trim(),
        issuer: issuer.trim(),
        client_id: clientId.trim(),
        client_secret: clientSecret.trim(),
        scopes: scopes.trim() || undefined,
      })
      setSlug('')
      setProvider('')
      setIssuer('')
      setClientId('')
      setClientSecret('')
      setScopes('openid email profile')
      load()
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setCreating(false)
    }
  }

  const handleDelete = async (id: string, name: string) => {
    if (!window.confirm(zh ? `删除 SSO 连接 "${name}"？` : `Delete SSO connection "${name}"?`)) return
    try {
      await api.deleteSsoConnection(id)
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
        <span className="topbar-title">{zh ? '企业 SSO' : 'Enterprise SSO'}</span>
        <div className="topbar-actions">
          <button className="btn btn-sm" onClick={toggleTheme} title={zh ? '切换主题' : 'Toggle dark/light theme'}>
            {theme === 'dark' ? '☀' : '◑'}
          </button>
        </div>
      </header>

      <div className="list-page">
        <div className="list-header">
          <h1>{zh ? '企业 SSO（OIDC）' : 'Enterprise SSO (OIDC)'}</h1>
        </div>

        <p style={{ marginBottom: 16, fontSize: 13, color: 'var(--muted)' }}>
          {zh
            ? '配置 OpenID Connect 身份提供商（Okta / Azure AD / Google Workspace）。在 IdP 注册应用时，回调地址填下表中每个连接的 Callback URL。'
            : 'Configure an OpenID Connect identity provider (Okta / Azure AD / Google Workspace). When registering the app at your IdP, use the Callback URL shown for each connection below.'}
        </p>

        {/* Create form */}
        <div style={{
          display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 10, marginBottom: 20,
          background: 'var(--surface)', border: '1px solid var(--border)',
          borderRadius: 'var(--radius)', padding: '14px 16px',
        }}>
          <div style={fieldStyle}>
            <label style={labelStyle}>{zh ? 'Slug（URL 标识，唯一）' : 'Slug (URL id, unique)'}</label>
            <input className="input" placeholder="acme-okta" value={slug} onChange={(e) => setSlug(e.target.value)} />
          </div>
          <div style={fieldStyle}>
            <label style={labelStyle}>{zh ? '提供商名称（按钮显示）' : 'Provider (button label)'}</label>
            <input className="input" placeholder="Okta" value={provider} onChange={(e) => setProvider(e.target.value)} />
          </div>
          <div style={{ ...fieldStyle, gridColumn: '1 / 3' }}>
            <label style={labelStyle}>Issuer</label>
            <input className="input" placeholder="https://your-org.okta.com" value={issuer} onChange={(e) => setIssuer(e.target.value)} />
          </div>
          <div style={fieldStyle}>
            <label style={labelStyle}>Client ID</label>
            <input className="input" value={clientId} onChange={(e) => setClientId(e.target.value)} />
          </div>
          <div style={fieldStyle}>
            <label style={labelStyle}>Client Secret</label>
            <input className="input" type="password" value={clientSecret} onChange={(e) => setClientSecret(e.target.value)} />
          </div>
          <div style={{ ...fieldStyle, gridColumn: '1 / 3' }}>
            <label style={labelStyle}>Scopes</label>
            <input className="input" value={scopes} onChange={(e) => setScopes(e.target.value)} />
          </div>
          <div style={{ gridColumn: '1 / 3', display: 'flex', justifyContent: 'flex-end' }}>
            <button
              className="btn btn-primary"
              onClick={handleCreate}
              disabled={creating || !slug.trim() || !provider.trim() || !issuer.trim() || !clientId.trim() || !clientSecret.trim()}
            >
              {creating ? (zh ? '添加中…' : 'Adding…') : (zh ? '+ 添加连接' : '+ Add Connection')}
            </button>
          </div>
        </div>

        {error && <p style={{ color: '#ef4444', fontSize: 13, marginBottom: 12 }}>{error}</p>}

        {loading ? (
          <p style={{ color: 'var(--muted)' }}>{zh ? '加载中…' : 'Loading…'}</p>
        ) : conns.length === 0 ? (
          <p style={{ color: 'var(--muted)' }}>{zh ? '尚无 SSO 连接。' : 'No SSO connections yet.'}</p>
        ) : (
          <table className="data-table" style={{ width: '100%' }}>
            <thead>
              <tr>
                <th>{zh ? '提供商' : 'Provider'}</th>
                <th>Slug</th>
                <th>Issuer</th>
                <th>Callback URL</th>
                <th>{zh ? '创建时间' : 'Created'}</th>
                <th></th>
              </tr>
            </thead>
            <tbody>
              {conns.map((c) => (
                <tr key={c.id}>
                  <td>{c.provider}</td>
                  <td><code>{c.slug}</code></td>
                  <td style={{ maxWidth: 220, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }} title={c.issuer}>{c.issuer}</td>
                  <td>
                    <button
                      className="btn btn-sm"
                      onClick={() => copy(callbackUrl(c.slug))}
                      title={callbackUrl(c.slug)}
                    >
                      {copied === callbackUrl(c.slug) ? (zh ? '✓ 已复制' : '✓ Copied') : (zh ? '⎘ 复制' : '⎘ Copy')}
                    </button>
                  </td>
                  <td style={{ color: 'var(--muted)', fontSize: 12 }}>{formatTs(c.created_at)}</td>
                  <td>
                    <button className="btn btn-sm btn-danger" onClick={() => handleDelete(c.id, c.provider)}>
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
