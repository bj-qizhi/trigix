// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useTheme } from '../useTheme'
import { useEffect, useMemo, useState } from 'react'
import { useAuth } from '../AuthContext'
import { useLocale } from '../useLocale'
import * as api from '../api/client'
import type { AuditEvent } from '../types'
import logoWordmark from '../assets/logo-wordmark.svg'

interface Props {
  onBack: () => void
}

function formatTimestamp(secs: number): string {
  return new Date(secs * 1000).toLocaleString()
}

function downloadCsv(events: AuditEvent[]) {
  const header = 'Time,Action,Resource Type,Resource ID,Detail'
  const rows = events.map((e) => [
    new Date(e.timestamp * 1000).toISOString(),
    e.action,
    e.resource_type,
    e.resource_id,
    e.detail ?? '',
  ].map((v) => `"${String(v).replace(/"/g, '""')}"`).join(','))
  const csv = [header, ...rows].join('\n')
  const blob = new Blob([csv], { type: 'text/csv' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = `audit-log-${Date.now()}.csv`
  a.click()
  URL.revokeObjectURL(url)
}

export function AuditLogPage({ onBack }: Props) {
  const { auth } = useAuth()
  const { theme, toggle: toggleTheme } = useTheme()
  const { t, locale } = useLocale()
  const zh = locale === 'zh'
  const [events, setEvents]       = useState<AuditEvent[]>([])
  const [loading, setLoading]     = useState(true)
  const [error, setError]         = useState<string | null>(null)
  const [search, setSearch]       = useState('')
  const [actionFilter, setActionFilter] = useState('')
  const [resourceTypeFilter, setResourceTypeFilter] = useState('')
  const [dateFrom, setDateFrom] = useState('')
  const [dateTo, setDateTo] = useState('')

  const load = () => {
    setLoading(true)
    setError(null)
    api.listAuditLog(auth!.tenantId, 500)
      .then(setEvents)
      .catch((e: unknown) => setError(String(e)))
      .finally(() => setLoading(false))
  }

  useEffect(load, [])

  const allActions = useMemo(
    () => Array.from(new Set(events.map((e) => e.action))).sort(),
    [events],
  )
  const allResourceTypes = useMemo(
    () => Array.from(new Set(events.map((e) => e.resource_type).filter(Boolean))).sort(),
    [events],
  )

  const filtered = useMemo(() => {
    let out = events
    if (actionFilter) out = out.filter((e) => e.action === actionFilter)
    if (resourceTypeFilter) out = out.filter((e) => e.resource_type === resourceTypeFilter)
    if (dateFrom) {
      const fromTs = Math.floor(new Date(dateFrom).getTime() / 1000)
      out = out.filter((e) => e.timestamp >= fromTs)
    }
    if (dateTo) {
      const toTs = Math.floor(new Date(dateTo + 'T23:59:59').getTime() / 1000)
      out = out.filter((e) => e.timestamp <= toTs)
    }
    if (search) {
      const q = search.toLowerCase()
      out = out.filter(
        (e) => e.action.toLowerCase().includes(q)
          || e.resource_id.toLowerCase().includes(q)
          || (e.detail ?? '').toLowerCase().includes(q),
      )
    }
    return out
  }, [events, actionFilter, resourceTypeFilter, dateFrom, dateTo, search])

  const hasFilter = !!(actionFilter || resourceTypeFilter || dateFrom || dateTo || search)
  const clearFilters = () => { setSearch(''); setActionFilter(''); setResourceTypeFilter(''); setDateFrom(''); setDateTo('') }

  return (
    <div className="app">
      <header className="topbar">
        <img src={logoWordmark} alt="Trigix" style={{ height: '28px' }} />
        <span className="topbar-sep">|</span>
        <span className="topbar-title">{t('audit.title')}</span>
        <div className="topbar-actions">
          <button className="btn btn-sm" onClick={load}>{zh ? '刷新' : 'Refresh'}</button>
          <button
            className="btn btn-sm"
            disabled={filtered.length === 0}
            onClick={() => downloadCsv(filtered)}
            title={zh ? '下载当前视图为 CSV' : 'Download current view as CSV'}
          >
            ↓ CSV
          </button>
          <button className="btn btn-sm" onClick={toggleTheme} title={zh ? '切换主题' : 'Toggle dark/light theme'}>{theme === 'dark' ? '☀' : '◑'}</button>
          <button className="btn btn-sm" onClick={onBack}>{zh ? '← 返回' : '← Back'}</button>
        </div>
      </header>

      <main className="list-page">
        <div className="list-header">
          <h1>{t('audit.title')}</h1>
          <span style={{ color: 'var(--muted)', fontSize: 13 }}>
            {filtered.length}{hasFilter ? ` / ${events.length}` : ''} {zh ? '条事件' : 'events'}
          </span>
        </div>

        {/* Filters */}
        <div style={{ display: 'flex', gap: 8, marginBottom: 12, flexWrap: 'wrap', alignItems: 'center' }}>
          <input
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder={zh ? '搜索操作、ID、详情…' : 'Search actions, IDs, details…'}
            style={{ fontSize: 13, padding: '3px 8px', width: 220 }}
          />
          <select
            value={actionFilter}
            onChange={(e) => setActionFilter(e.target.value)}
            style={{ fontSize: 12, padding: '3px 6px' }}
          >
            <option value="">{t('audit.all.actions')}</option>
            {allActions.map((a) => (
              <option key={a} value={a}>{a}</option>
            ))}
          </select>
          {allResourceTypes.length > 0 && (
            <select
              value={resourceTypeFilter}
              onChange={(e) => setResourceTypeFilter(e.target.value)}
              style={{ fontSize: 12, padding: '3px 6px' }}
            >
              <option value="">{zh ? '全部资源类型' : 'All resource types'}</option>
              {allResourceTypes.map((rt) => (
                <option key={rt} value={rt}>{rt}</option>
              ))}
            </select>
          )}
          <label style={{ fontSize: 12, color: 'var(--muted)', display: 'flex', alignItems: 'center', gap: 4 }}>
            {zh ? '从' : 'From'}
            <input
              type="date"
              value={dateFrom}
              onChange={(e) => setDateFrom(e.target.value)}
              style={{ fontSize: 12, padding: '2px 4px', marginLeft: 4 }}
            />
          </label>
          <label style={{ fontSize: 12, color: 'var(--muted)', display: 'flex', alignItems: 'center', gap: 4 }}>
            {zh ? '至' : 'To'}
            <input
              type="date"
              value={dateTo}
              onChange={(e) => setDateTo(e.target.value)}
              style={{ fontSize: 12, padding: '2px 4px', marginLeft: 4 }}
            />
          </label>
          {hasFilter && (
            <button className="btn btn-sm" onClick={clearFilters}>
              ✕ {zh ? '清除筛选' : 'Clear filters'}
            </button>
          )}
        </div>

        {loading && <p>{zh ? '加载中…' : 'Loading…'}</p>}
        {error && <p style={{ color: 'var(--danger-text)' }}>{error}</p>}

        {!loading && !error && filtered.length === 0 && (
          <div className="empty-state">
            <p>{hasFilter ? (zh ? '无匹配事件。' : 'No events match the filter.') : (zh ? '暂无审计事件。' : 'No audit events recorded yet.')}</p>
          </div>
        )}

        {!loading && filtered.length > 0 && (
          <table className="workflow-table">
            <thead>
              <tr>
                <th>{t('audit.col.time')}</th>
                <th>{t('audit.col.action')}</th>
                <th>{t('audit.col.resource')}</th>
                <th>{t('audit.col.id')}</th>
                <th>{t('audit.col.detail')}</th>
              </tr>
            </thead>
            <tbody>
              {filtered.map((evt) => (
                <tr key={evt.id}>
                  <td style={{ fontSize: 12, color: 'var(--muted)', whiteSpace: 'nowrap' }}>
                    {formatTimestamp(evt.timestamp)}
                  </td>
                  <td>
                    <code
                      style={{ fontSize: 11, cursor: 'pointer' }}
                      onClick={() => setActionFilter(evt.action === actionFilter ? '' : evt.action)}
                      title={zh ? '点击按此操作筛选' : 'Click to filter by this action'}
                    >
                      {evt.action}
                    </code>
                  </td>
                  <td style={{ color: 'var(--muted)', fontSize: 12 }}>{evt.resource_type}</td>
                  <td style={{ color: 'var(--muted)', fontSize: 11, fontFamily: 'monospace' }}>
                    <span title={evt.resource_id}>{evt.resource_id.slice(-12)}</span>
                  </td>
                  <td style={{ color: 'var(--muted)', fontSize: 11, maxWidth: 220, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                    {evt.detail ?? ''}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </main>
    </div>
  )
}
