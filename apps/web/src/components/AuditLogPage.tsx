import { useEffect, useState } from 'react'
import { useAuth } from '../AuthContext'
import * as api from '../api/client'
import type { AuditEvent } from '../types'

interface Props {
  onBack: () => void
}

function formatTimestamp(secs: number): string {
  return new Date(secs * 1000).toLocaleString()
}

export function AuditLogPage({ onBack }: Props) {
  const { auth } = useAuth()
  const [events, setEvents]   = useState<AuditEvent[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError]     = useState<string | null>(null)

  const load = () => {
    setLoading(true)
    setError(null)
    api.listAuditLog(auth!.tenantId, 100)
      .then(setEvents)
      .catch((e: unknown) => setError(String(e)))
      .finally(() => setLoading(false))
  }

  useEffect(load, [])

  return (
    <div className="app">
      <header className="topbar">
        <span className="topbar-logo">aiworkflow</span>
        <div className="topbar-actions">
          <button className="btn btn-sm" onClick={onBack}>← Back</button>
        </div>
      </header>

      <main className="list-page">
        <div className="list-header">
          <h1>Audit Log</h1>
          <button className="btn btn-sm" onClick={load}>Refresh</button>
        </div>

        {loading && <p>Loading…</p>}
        {error && <p style={{ color: 'var(--danger-text)' }}>{error}</p>}

        {!loading && !error && events.length === 0 && (
          <div className="empty-state">
            <p>No audit events recorded yet.</p>
          </div>
        )}

        {!loading && events.length > 0 && (
          <table className="workflow-table">
            <thead>
              <tr>
                <th>Time</th>
                <th>Action</th>
                <th>Resource</th>
                <th>ID</th>
              </tr>
            </thead>
            <tbody>
              {events.map((evt) => (
                <tr key={evt.id}>
                  <td style={{ fontSize: 12, color: 'var(--muted)', whiteSpace: 'nowrap' }}>
                    {formatTimestamp(evt.timestamp)}
                  </td>
                  <td>
                    <code style={{ fontSize: 12 }}>{evt.action}</code>
                  </td>
                  <td style={{ color: 'var(--muted)', fontSize: 12 }}>{evt.resource_type}</td>
                  <td style={{ color: 'var(--muted)', fontSize: 11, fontFamily: 'monospace' }}>
                    {evt.resource_id.slice(-8)}
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
