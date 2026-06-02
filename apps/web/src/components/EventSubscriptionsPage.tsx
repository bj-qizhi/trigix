// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// Contact: managecode@gmail.com

import { useEffect, useState } from 'react'
import { useAuth } from '../AuthContext'
import { useLocale } from '../useLocale'
import * as api from '../api/client'
import type { EventSubscription, EventType } from '../types'
import { useTheme } from '../useTheme'

const ALL_EVENTS: EventType[] = [
  'execution.started',
  'execution.completed',
  'execution.failed',
  'execution.cancelled',
]

interface Props {
  onBack: () => void
}

function formatTs(secs: number): string {
  return new Date(secs * 1000).toLocaleString()
}

export function EventSubscriptionsPage({ onBack }: Props) {
  const { auth } = useAuth()
  const { theme, toggle: toggleTheme } = useTheme()
  const { t, locale } = useLocale()
  const zh = locale === 'zh'
  const [subs, setSubs] = useState<EventSubscription[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [url, setUrl] = useState('')
  const [desc, setDesc] = useState('')
  const [selectedEvents, setSelectedEvents] = useState<EventType[]>([])
  const [creating, setCreating] = useState(false)

  const load = () => {
    setLoading(true)
    api.listEventSubscriptions(auth!.tenantId)
      .then(setSubs)
      .catch((e: unknown) => setError(String(e)))
      .finally(() => setLoading(false))
  }

  useEffect(load, [])

  const toggleEvent = (ev: EventType) => {
    setSelectedEvents((prev) =>
      prev.includes(ev) ? prev.filter((e) => e !== ev) : [...prev, ev],
    )
  }

  const handleCreate = async () => {
    if (!url.trim()) return
    setCreating(true)
    try {
      await api.createEventSubscription(
        auth!.tenantId,
        url.trim(),
        selectedEvents,
        desc.trim() || undefined,
      )
      setUrl('')
      setDesc('')
      setSelectedEvents([])
      load()
    } catch (e: unknown) {
      setError(String(e))
    } finally {
      setCreating(false)
    }
  }

  const handleDelete = async (id: string) => {
    if (!window.confirm(zh ? '删除此订阅？' : 'Delete this subscription?')) return
    try {
      await api.deleteEventSubscription(auth!.tenantId, id)
      setSubs((prev) => prev.filter((s) => s.id !== id))
    } catch (e: unknown) {
      setError(String(e))
    }
  }

  return (
    <div className="app">
      <header className="topbar">
        <button className="btn btn-sm btn-icon" onClick={onBack} title={zh ? '返回' : 'Back'}>←</button>
        <span className="topbar-sep">|</span>
        <span className="topbar-title">{t('events.title')}</span>
        <div className="topbar-spacer" />
        <button className="btn btn-sm btn-icon" onClick={toggleTheme} title={zh ? '切换主题' : 'Toggle theme'}>
          {theme === 'dark' ? '☀️' : '🌙'}
        </button>
      </header>

      <main className="page-content" style={{ maxWidth: 720, margin: '0 auto', padding: '1.5rem 1rem' }}>
        {error && (
          <div className="alert alert-error" style={{ marginBottom: '1rem' }}>
            {error}
            <button className="btn btn-sm" style={{ marginLeft: 8 }} onClick={() => setError(null)}>✕</button>
          </div>
        )}

        <section className="card" style={{ marginBottom: '1.5rem', padding: '1rem' }}>
          <h3 style={{ marginBottom: '0.75rem' }}>{zh ? '新建订阅' : 'New Subscription'}</h3>
          <p style={{ fontSize: '0.85rem', color: 'var(--text-muted)', marginBottom: '0.75rem' }}>
            {zh
              ? '当执行生命周期事件发生时，接收 HTTP POST 通知。不勾选任何事件则订阅所有类型。'
              : 'Receive HTTP POST notifications when execution lifecycle events occur. Leave all events unchecked to subscribe to all event types.'}
          </p>
          <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
            <input
              className="input"
              placeholder={zh ? '接收端 URL（https://...）' : 'Endpoint URL (https://...)'}
              value={url}
              onChange={(e) => setUrl(e.target.value)}
            />
            <input
              className="input"
              placeholder={zh ? '描述（可选）' : 'Description (optional)'}
              value={desc}
              onChange={(e) => setDesc(e.target.value)}
            />
            <div style={{ display: 'flex', gap: '0.75rem', flexWrap: 'wrap', marginTop: '0.25rem' }}>
              {ALL_EVENTS.map((ev) => (
                <label key={ev} style={{ display: 'flex', alignItems: 'center', gap: '0.3rem', fontSize: '0.85rem', cursor: 'pointer' }}>
                  <input
                    type="checkbox"
                    checked={selectedEvents.includes(ev)}
                    onChange={() => toggleEvent(ev)}
                  />
                  {ev}
                </label>
              ))}
            </div>
            <button
              className="btn btn-primary"
              onClick={handleCreate}
              disabled={creating || !url.trim()}
              style={{ alignSelf: 'flex-start', marginTop: '0.25rem' }}
            >
              {creating ? (zh ? '创建中…' : 'Creating…') : (zh ? '创建订阅' : 'Create Subscription')}
            </button>
          </div>
        </section>

        {loading ? (
          <p style={{ color: 'var(--text-muted)' }}>{zh ? '加载中…' : 'Loading…'}</p>
        ) : subs.length === 0 ? (
          <p style={{ color: 'var(--text-muted)' }}>{zh ? '暂无订阅。' : 'No subscriptions yet.'}</p>
        ) : (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '0.75rem' }}>
            {subs.map((s) => (
              <div key={s.id} className="card" style={{ padding: '0.875rem', display: 'flex', flexDirection: 'column', gap: '0.4rem' }}>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start' }}>
                  <div>
                    <span style={{ fontFamily: 'monospace', fontSize: '0.9rem', wordBreak: 'break-all' }}>{s.url}</span>
                    {s.description && (
                      <div style={{ fontSize: '0.82rem', color: 'var(--text-muted)', marginTop: '0.2rem' }}>{s.description}</div>
                    )}
                  </div>
                  <button className="btn btn-sm btn-danger" onClick={() => handleDelete(s.id)} style={{ marginLeft: '0.75rem', flexShrink: 0 }}>
                    {t('common.delete')}
                  </button>
                </div>
                <div style={{ display: 'flex', gap: '0.4rem', flexWrap: 'wrap' }}>
                  {s.events.map((ev) => (
                    <span key={ev} className="badge badge-outline" style={{ fontSize: '0.75rem' }}>{ev}</span>
                  ))}
                </div>
                <div style={{ fontSize: '0.78rem', color: 'var(--text-muted)' }}>
                  {zh ? '创建于' : 'Created'} {formatTs(s.created_at)} · ID: {s.id}
                </div>
              </div>
            ))}
          </div>
        )}
      </main>
    </div>
  )
}
