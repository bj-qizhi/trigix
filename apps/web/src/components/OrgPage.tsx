// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useEffect, useState } from 'react'
import { useAuth } from '../AuthContext'
import { useLocale } from '../useLocale'
import { useTheme } from '../useTheme'
import * as api from '../api/client'
import logoWordmark from '../assets/logo-wordmark.svg'

interface Props {
  onBack: () => void
}

export function OrgPage({ onBack }: Props) {
  const { auth, login } = useAuth()
  const { t, locale, toggle: toggleLocale } = useLocale()
  const { theme, toggle: toggleTheme } = useTheme()
  const zh = locale === 'zh'
  const [orgs, setOrgs] = useState<api.OrgRecord[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  const [creating, setCreating] = useState(false)
  const [newOrgName, setNewOrgName] = useState('')
  const [saving, setSaving] = useState(false)

  const [expandedOrg, setExpandedOrg] = useState<string | null>(null)
  const [members, setMembers] = useState<Record<string, api.OrgMember[]>>({})
  const [addingMember, setAddingMember] = useState<string | null>(null)
  const [newUserId, setNewUserId] = useState('')
  const [newRole, setNewRole] = useState('editor')
  const [switchingOrg, setSwitchingOrg] = useState<string | null>(null)

  const load = async () => {
    setLoading(true)
    try {
      const data = await api.listOrgs()
      setOrgs(data)
    } catch (e: unknown) {
      const msg = String(e)
      if (msg.includes('401') || msg.toLowerCase().includes('unauthorized') || msg.toLowerCase().includes('authenticated')) {
        setError(zh ? '此页面需要邮箱账号登录。API 密钥登录不支持组织管理。' : 'This page requires email login. Organization management is not available for API key sessions.')
      } else {
        setError(zh ? '加载组织失败' : 'Failed to load organizations')
      }
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => { void load() }, [])

  const handleCreate = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!newOrgName.trim()) return
    setSaving(true)
    try {
      await api.createOrg(newOrgName.trim())
      setNewOrgName('')
      setCreating(false)
      await load()
    } catch {
      setError(zh ? '创建组织失败' : 'Failed to create organization')
    } finally {
      setSaving(false)
    }
  }

  const handleDelete = async (orgId: string) => {
    if (!confirm(zh ? '删除此组织？此操作不可撤回。' : 'Delete this organization? This cannot be undone.')) return
    try {
      await api.deleteOrg(orgId)
      await load()
    } catch {
      setError(zh ? '删除组织失败' : 'Failed to delete organization')
    }
  }

  const toggleMembers = async (orgId: string) => {
    if (expandedOrg === orgId) {
      setExpandedOrg(null)
      return
    }
    setExpandedOrg(orgId)
    if (!members[orgId]) {
      try {
        const data = await api.listOrgMembers(orgId)
        setMembers(m => ({ ...m, [orgId]: data }))
      } catch {
        // ignore
      }
    }
  }

  const handleAddMember = async (e: React.FormEvent, orgId: string) => {
    e.preventDefault()
    if (!newUserId.trim()) return
    try {
      await api.addOrgMember(orgId, newUserId.trim(), newRole)
      setNewUserId('')
      setAddingMember(null)
      const data = await api.listOrgMembers(orgId)
      setMembers(m => ({ ...m, [orgId]: data }))
    } catch {
      setError(zh ? '添加成员失败（请检查用户 ID）' : 'Failed to add member (check user ID)')
    }
  }

  const handleRemoveMember = async (orgId: string, userId: string) => {
    try {
      await api.removeOrgMember(orgId, userId)
      const data = await api.listOrgMembers(orgId)
      setMembers(m => ({ ...m, [orgId]: data }))
    } catch {
      setError(zh ? '移除成员失败' : 'Failed to remove member')
    }
  }

  const handleSwitch = async (orgId: string) => {
    setSwitchingOrg(orgId)
    try {
      const res = await api.switchOrg(orgId)
      login({
        token: res.token,
        tenantId: res.tenant_id,
        workspaceId: 'workspace-1',
        projectId: 'project-1',
        role: res.role,
      })
      onBack()
    } catch {
      setError(zh ? '切换组织失败' : 'Failed to switch organization')
    } finally {
      setSwitchingOrg(null)
    }
  }

  return (
    <div className="app" data-theme={theme}>
      <header className="topbar">
        <img src={logoWordmark} alt="Trigix" style={{ height: '28px' }} />
        <span className="topbar-sep">|</span>
        <span className="topbar-title">{t('org.title')}</span>
        <div className="topbar-actions">
          <button className="btn btn-sm btn-primary" onClick={() => setCreating(c => !c)}>
            + {zh ? '新建组织' : 'New Org'}
          </button>
          <button className="btn btn-sm" onClick={toggleTheme} title="Toggle dark/light theme">
            {theme === 'dark' ? '☀' : '◑'}
          </button>
          <button className="btn btn-sm" onClick={toggleLocale} title="切换语言 / Switch language">
            {locale === 'zh' ? 'EN' : '中'}
          </button>
          <button className="btn btn-sm" onClick={onBack}>{t('nav.back')}</button>
        </div>
      </header>

      <main className="list-page">
        {error && (
          <div style={{
            background: 'var(--danger-bg, rgba(220,38,38,0.1))',
            color: 'var(--danger-text)',
            padding: '0.75rem 1rem',
            borderRadius: '6px',
            marginBottom: '1rem',
            display: 'flex',
            justifyContent: 'space-between',
            border: '1px solid var(--danger-text)',
          }}>
            <span>{error}</span>
            <button style={{ background: 'none', border: 'none', color: 'var(--danger-text)', cursor: 'pointer' }} onClick={() => setError(null)}>✕</button>
          </div>
        )}

        {creating && (
          <form onSubmit={handleCreate} style={{
            background: 'var(--panel)',
            padding: '1rem',
            borderRadius: '6px',
            marginBottom: '1rem',
            display: 'flex',
            gap: '0.5rem',
            border: '1px solid var(--border)',
          }}>
            <input
              className="input"
              style={{ flex: 1 }}
              placeholder={zh ? '组织名称' : 'Organization name'}
              value={newOrgName}
              onChange={e => setNewOrgName(e.target.value)}
              autoFocus
              required
            />
            <button type="submit" className="btn btn-sm btn-primary" disabled={saving}>
              {saving ? (zh ? '创建中…' : 'Creating…') : t('common.create')}
            </button>
            <button type="button" className="btn btn-sm" onClick={() => setCreating(false)}>{t('common.cancel')}</button>
          </form>
        )}

        {loading ? (
          <p style={{ color: 'var(--muted)' }}>{zh ? '加载中…' : 'Loading…'}</p>
        ) : orgs.length === 0 ? (
          <div style={{ textAlign: 'center', padding: '3rem', color: 'var(--muted)' }}>
            <div style={{ fontSize: '2rem', marginBottom: '0.5rem' }}>🏢</div>
            <p>{zh ? '暂无组织，创建一个以开始团队协作。' : 'No organizations yet. Create one to collaborate with your team.'}</p>
          </div>
        ) : (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '0.75rem' }}>
            {orgs.map(org => {
              const isExpanded = expandedOrg === org.id
              const orgMembers = members[org.id] ?? []
              const isCurrentTenant = auth?.tenantId === org.id
              const isSwitching = switchingOrg === org.id

              return (
                <div key={org.id} style={{
                  background: 'var(--panel)',
                  borderRadius: '6px',
                  border: `1px solid ${isCurrentTenant ? 'var(--accent)' : 'var(--border)'}`,
                }}>
                  <div style={{ display: 'flex', alignItems: 'center', padding: '0.75rem 1rem', gap: '0.75rem' }}>
                    <span style={{ flex: 1, fontWeight: 500 }}>
                      {org.name}
                      {isCurrentTenant && (
                        <span style={{
                          marginLeft: '0.5rem',
                          fontSize: '0.7rem',
                          background: 'var(--accent)',
                          color: '#fff',
                          padding: '0.1rem 0.4rem',
                          borderRadius: '3px',
                        }}>
                          {zh ? '当前' : 'active'}
                        </span>
                      )}
                    </span>
                    <span style={{ fontSize: '0.75rem', color: 'var(--muted)' }}>
                      {new Date(org.created_at * 1000).toLocaleDateString()}
                    </span>
                    <button
                      className="btn btn-sm"
                      onClick={() => void toggleMembers(org.id)}
                      style={{ fontSize: '0.75rem' }}
                    >
                      {isExpanded ? (zh ? '▲ 成员' : '▲ Members') : (zh ? '▼ 成员' : '▼ Members')}
                    </button>
                    {!isCurrentTenant && (
                      <button
                        className="btn btn-sm btn-primary"
                        disabled={isSwitching}
                        onClick={() => void handleSwitch(org.id)}
                        style={{ fontSize: '0.75rem' }}
                      >
                        {isSwitching ? '…' : (zh ? '⇄ 切换' : '⇄ Switch')}
                      </button>
                    )}
                    <button
                      className="btn btn-sm btn-danger"
                      onClick={() => void handleDelete(org.id)}
                      style={{ fontSize: '0.75rem' }}
                    >
                      🗑
                    </button>
                  </div>

                  {isExpanded && (
                    <div style={{ borderTop: '1px solid var(--border)', padding: '0.75rem 1rem' }}>
                      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '0.5rem' }}>
                        <span style={{ fontSize: '0.8rem', color: 'var(--muted)' }}>
                          {zh ? `${orgMembers.length} 名成员` : `${orgMembers.length} member${orgMembers.length !== 1 ? 's' : ''}`}
                        </span>
                        <button
                          className="btn btn-sm"
                          style={{ fontSize: '0.75rem' }}
                          onClick={() => setAddingMember(addingMember === org.id ? null : org.id)}
                        >
                          + {zh ? '添加成员' : 'Add Member'}
                        </button>
                      </div>

                      {addingMember === org.id && (
                        <form onSubmit={e => void handleAddMember(e, org.id)} style={{ display: 'flex', gap: '0.5rem', marginBottom: '0.75rem' }}>
                          <input
                            className="input"
                            style={{ flex: 2 }}
                            placeholder={zh ? '用户 ID' : 'User ID'}
                            value={newUserId}
                            onChange={e => setNewUserId(e.target.value)}
                            required
                          />
                          <select
                            className="input"
                            style={{ flex: 1 }}
                            value={newRole}
                            onChange={e => setNewRole(e.target.value)}
                          >
                            <option value="viewer">{t('org.role.viewer')}</option>
                            <option value="editor">{t('org.role.editor')}</option>
                            <option value="admin">{t('org.role.admin')}</option>
                          </select>
                          <button type="submit" className="btn btn-sm btn-primary">{t('org.add')}</button>
                          <button type="button" className="btn btn-sm" onClick={() => setAddingMember(null)}>✕</button>
                        </form>
                      )}

                      <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: '0.8rem' }}>
                        <thead>
                          <tr style={{ color: 'var(--muted)', borderBottom: '1px solid var(--border)' }}>
                            <th style={{ textAlign: 'left', padding: '0.3rem 0', fontWeight: 400 }}>{t('org.col.user')}</th>
                            <th style={{ textAlign: 'left', padding: '0.3rem 0', fontWeight: 400 }}>{t('org.col.role')}</th>
                            <th style={{ textAlign: 'left', padding: '0.3rem 0', fontWeight: 400 }}>{t('org.col.joined')}</th>
                            <th />
                          </tr>
                        </thead>
                        <tbody>
                          {orgMembers.map(m => (
                            <tr key={m.user_id} style={{ borderBottom: '1px solid var(--border)' }}>
                              <td style={{ padding: '0.4rem 0', fontFamily: 'monospace', fontSize: '0.75rem', color: 'var(--muted)' }}>
                                {m.user_id.slice(0, 12)}…
                              </td>
                              <td style={{ padding: '0.4rem 0' }}>
                                <span style={{
                                  background: m.role === 'admin' ? 'var(--accent)' : 'var(--panel)',
                                  color: m.role === 'admin' ? '#fff' : 'var(--text)',
                                  padding: '0.1rem 0.4rem',
                                  borderRadius: '3px',
                                  fontSize: '0.7rem',
                                  border: '1px solid var(--border)',
                                }}>
                                  {m.role}
                                </span>
                              </td>
                              <td style={{ padding: '0.4rem 0', color: 'var(--muted)' }}>
                                {new Date(m.joined_at * 1000).toLocaleDateString()}
                              </td>
                              <td style={{ padding: '0.4rem 0', textAlign: 'right' }}>
                                <button
                                  className="btn btn-sm btn-danger"
                                  style={{ fontSize: '0.7rem', padding: '0.1rem 0.4rem' }}
                                  onClick={() => void handleRemoveMember(org.id, m.user_id)}
                                >
                                  ✕
                                </button>
                              </td>
                            </tr>
                          ))}
                          {orgMembers.length === 0 && (
                            <tr>
                              <td colSpan={4} style={{ padding: '0.5rem 0', color: 'var(--muted)', textAlign: 'center' }}>
                                {zh ? '暂无成员' : 'No members'}
                              </td>
                            </tr>
                          )}
                        </tbody>
                      </table>
                    </div>
                  )}
                </div>
              )
            })}
          </div>
        )}
      </main>
    </div>
  )
}
