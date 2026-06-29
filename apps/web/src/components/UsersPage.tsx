// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useEffect, useState } from 'react'
import { ThemeToggleIcon } from './uiIcons'
import { useAuth } from '../AuthContext'
import { useLocale } from '../useLocale'
import * as api from '../api/client'
import { SkeletonRows } from './Skeleton'
import { useTheme } from '../useTheme'
import logoWordmark from '../assets/logo-wordmark.svg'

interface Props {
  onBack: () => void
}

function formatTs(secs: number): string {
  return new Date(secs * 1000).toLocaleDateString()
}

function formatExpiry(secs: number, zh = false): string {
  const diff = secs - Math.floor(Date.now() / 1000)
  if (diff <= 0) return zh ? '已过期' : 'expired'
  const h = Math.floor(diff / 3600)
  const d = Math.floor(h / 24)
  if (zh) return d > 0 ? `剩余 ${d} 天` : `剩余 ${h} 小时`
  return d > 0 ? `${d}d left` : `${h}h left`
}

export function UsersPage({ onBack }: Props) {
  const { auth } = useAuth()
  const { theme, toggle: toggleTheme } = useTheme()
  const { t, locale } = useLocale()
  const zh = locale === 'zh'

  const [users, setUsers] = useState<api.User[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [deleting, setDeleting] = useState<string | null>(null)

  const [invites, setInvites] = useState<api.Invitation[]>([])
  const [invEmail, setInvEmail] = useState('')
  const [invRole, setInvRole] = useState('editor')
  const [invCreating, setInvCreating] = useState(false)
  const [invError, setInvError] = useState<string | null>(null)
  const [revoking, setRevoking] = useState<string | null>(null)
  const [copiedId, setCopiedId] = useState<string | null>(null)

  useEffect(() => {
    api.listAdminUsers()
      .then(setUsers)
      .catch(() => setError(zh ? '加载用户失败' : 'Failed to load users'))
      .finally(() => setLoading(false))
    api.listInvitations()
      .then(setInvites)
      .catch(() => {})
  }, [])

  async function handleDelete(user: api.User) {
    if (!confirm(zh ? `删除 ${user.email}？此操作不可撤回。` : `Delete ${user.email}? This cannot be undone.`)) return
    setDeleting(user.id)
    try {
      await api.deleteAdminUser(user.id)
      setUsers((prev) => prev.filter((u) => u.id !== user.id))
    } catch {
      setError(zh ? `删除 ${user.email} 失败` : `Failed to delete ${user.email}`)
    } finally {
      setDeleting(null)
    }
  }

  async function handleCreateInvite() {
    if (!invEmail.trim()) return
    setInvCreating(true)
    setInvError(null)
    try {
      const inv = await api.createInvitation(invEmail.trim(), invRole)
      setInvites((prev) => [inv, ...prev])
      setInvEmail('')
    } catch {
      setInvError(zh ? '创建邀请失败' : 'Failed to create invitation')
    } finally {
      setInvCreating(false)
    }
  }

  async function handleRevoke(inv: api.Invitation) {
    setRevoking(inv.id)
    try {
      await api.deleteInvitation(inv.id)
      setInvites((prev) => prev.filter((i) => i.id !== inv.id))
    } catch {
      setInvError(zh ? '撤销邀请失败' : 'Failed to revoke invitation')
    } finally {
      setRevoking(null)
    }
  }

  function inviteLink(token: string): string {
    return `${window.location.origin}?invite=${token}`
  }

  function copyLink(inv: api.Invitation) {
    navigator.clipboard.writeText(inviteLink(inv.token)).then(() => {
      setCopiedId(inv.id)
      setTimeout(() => setCopiedId(null), 1500)
    })
  }

  const currentUserId = (() => {
    try {
      if (!auth?.token) return null
      const payload = JSON.parse(atob(auth.token.split('.')[1]))
      return payload.user_id ?? null
    } catch { return null }
  })()

  const pendingInvites = invites.filter((i) => !i.used_at && i.expires_at > Math.floor(Date.now() / 1000))

  return (
    <div className="page" data-theme={theme}>
      <header className="topbar">
        <img src={logoWordmark} alt="Trigix" style={{ height: '28px' }} />
        <div className="topbar-actions">
          <button className="btn btn-sm" onClick={onBack}>{zh ? '← 返回' : '← Back'}</button>
          <button className="btn btn-sm" onClick={toggleTheme} title={zh ? '切换主题' : 'Toggle theme'}>
            {theme === 'dark' ? <ThemeToggleIcon dark /> : <ThemeToggleIcon dark={false} />}
          </button>
        </div>
      </header>

      <main style={{ maxWidth: 700, margin: '2rem auto', padding: '0 1rem' }}>
        <h2 style={{ marginBottom: '1.5rem' }}>{zh ? '用户' : 'Users'}</h2>

        {error && <p style={{ color: 'var(--error)', marginBottom: '1rem' }}>{error}</p>}

        {loading ? (
          <SkeletonRows rows={6} />
        ) : (
          <table style={{ width: '100%', borderCollapse: 'collapse', marginBottom: '0.5rem' }}>
            <thead>
              <tr style={{ borderBottom: '1px solid var(--border)', textAlign: 'left' }}>
                <th style={{ padding: '0.5rem 0.75rem', fontSize: '0.8rem', color: 'var(--fg-muted)' }}>{zh ? '邮箱' : 'Email'}</th>
                <th style={{ padding: '0.5rem 0.75rem', fontSize: '0.8rem', color: 'var(--fg-muted)' }}>{zh ? '名称' : 'Name'}</th>
                <th style={{ padding: '0.5rem 0.75rem', fontSize: '0.8rem', color: 'var(--fg-muted)' }}>{zh ? '加入时间' : 'Joined'}</th>
                <th style={{ padding: '0.5rem 0.75rem', fontSize: '0.8rem', color: 'var(--fg-muted)' }}></th>
              </tr>
            </thead>
            <tbody>
              {users.map((user) => (
                <tr key={user.id} style={{ borderBottom: '1px solid var(--border)' }}>
                  <td style={{ padding: '0.6rem 0.75rem' }}>
                    {user.email}
                    {user.id === currentUserId && (
                      <span style={{ marginLeft: '0.4rem', fontSize: '0.7rem', color: 'var(--fg-muted)' }}>({zh ? '我' : 'you'})</span>
                    )}
                  </td>
                  <td style={{ padding: '0.6rem 0.75rem', color: 'var(--fg-muted)' }}>
                    {user.name ?? <em style={{ opacity: 0.5 }}>—</em>}
                  </td>
                  <td style={{ padding: '0.6rem 0.75rem', color: 'var(--fg-muted)', fontSize: '0.85rem' }}>
                    {user.created_at ? formatTs(user.created_at) : '—'}
                  </td>
                  <td style={{ padding: '0.6rem 0.75rem', textAlign: 'right' }}>
                    {user.id !== currentUserId && (
                      <button
                        className="btn btn-sm"
                        style={{ color: 'var(--error)', borderColor: 'var(--error)' }}
                        disabled={deleting === user.id}
                        onClick={() => handleDelete(user)}
                        title={zh ? `删除 ${user.email}` : `Delete ${user.email}`}
                      >
                        {deleting === user.id ? '…' : (zh ? '删除' : 'Delete')}
                      </button>
                    )}
                  </td>
                </tr>
              ))}
              {users.length === 0 && (
                <tr>
                  <td colSpan={4} style={{ padding: '1.5rem 0.75rem', textAlign: 'center', color: 'var(--fg-muted)' }}>
                    {zh ? '未找到用户' : 'No users found'}
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        )}

        <p style={{ marginBottom: '2.5rem', fontSize: '0.8rem', color: 'var(--fg-muted)' }}>
          {zh ? `${users.length} 名用户` : `${users.length} user${users.length !== 1 ? 's' : ''}`}
        </p>

        {/* ── Invitations ── */}
        <h3 style={{ marginBottom: '1rem' }}>{zh ? '邀请用户' : 'Invite User'}</h3>
        {invError && <p style={{ color: 'var(--error)', marginBottom: '0.75rem' }}>{invError}</p>}

        <div style={{ display: 'flex', gap: '0.5rem', marginBottom: '1.5rem', flexWrap: 'wrap' }}>
          <input
            className="input"
            style={{ flex: 1, minWidth: 200 }}
            type="email"
            placeholder="user@example.com"
            value={invEmail}
            onChange={(e) => setInvEmail(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && handleCreateInvite()}
          />
          <select
            className="input"
            style={{ width: 120 }}
            value={invRole}
            onChange={(e) => setInvRole(e.target.value)}
          >
            <option value="viewer">{t('users.role.viewer')}</option>
            <option value="editor">{t('users.role.editor')}</option>
            <option value="admin">{t('users.role.admin')}</option>
          </select>
          <button className="btn btn-primary" onClick={handleCreateInvite} disabled={invCreating || !invEmail.trim()}>
            {invCreating ? (zh ? '创建中…' : 'Creating…') : (zh ? '+ 邀请' : '+ Invite')}
          </button>
        </div>

        {pendingInvites.length > 0 && (
          <>
            <h4 style={{ marginBottom: '0.75rem', fontSize: '0.85rem', color: 'var(--fg-muted)', fontWeight: 600, textTransform: 'uppercase', letterSpacing: '0.05em' }}>
              {zh ? '待接受邀请' : 'Pending Invitations'}
            </h4>
            <table style={{ width: '100%', borderCollapse: 'collapse' }}>
              <thead>
                <tr style={{ borderBottom: '1px solid var(--border)', textAlign: 'left' }}>
                  <th style={{ padding: '0.4rem 0.75rem', fontSize: '0.8rem', color: 'var(--fg-muted)' }}>{zh ? '邮箱' : 'Email'}</th>
                  <th style={{ padding: '0.4rem 0.75rem', fontSize: '0.8rem', color: 'var(--fg-muted)' }}>{zh ? '角色' : 'Role'}</th>
                  <th style={{ padding: '0.4rem 0.75rem', fontSize: '0.8rem', color: 'var(--fg-muted)' }}>{zh ? '过期' : 'Expires'}</th>
                  <th style={{ padding: '0.4rem 0.75rem', fontSize: '0.8rem', color: 'var(--fg-muted)' }}></th>
                </tr>
              </thead>
              <tbody>
                {pendingInvites.map((inv) => (
                  <tr key={inv.id} style={{ borderBottom: '1px solid var(--border)' }}>
                    <td style={{ padding: '0.5rem 0.75rem' }}>{inv.email}</td>
                    <td style={{ padding: '0.5rem 0.75rem', color: 'var(--fg-muted)', fontSize: '0.85rem' }}>{inv.role}</td>
                    <td style={{ padding: '0.5rem 0.75rem', color: 'var(--fg-muted)', fontSize: '0.85rem' }}>{formatExpiry(inv.expires_at, zh)}</td>
                    <td style={{ padding: '0.5rem 0.75rem', textAlign: 'right', display: 'flex', gap: '0.4rem', justifyContent: 'flex-end' }}>
                      <button
                        className="btn btn-sm"
                        onClick={() => copyLink(inv)}
                        title={zh ? '复制邀请链接' : 'Copy invite link'}
                      >
                        {copiedId === inv.id ? (zh ? '✓ 已复制' : '✓ Copied') : (zh ? '链接' : 'Link')}
                      </button>
                      <button
                        className="btn btn-sm"
                        style={{ color: 'var(--error)', borderColor: 'var(--error)' }}
                        disabled={revoking === inv.id}
                        onClick={() => handleRevoke(inv)}
                        title={zh ? '撤销邀请' : 'Revoke invitation'}
                      >
                        {revoking === inv.id ? '…' : (zh ? '撤销' : 'Revoke')}
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </>
        )}
      </main>
    </div>
  )
}
