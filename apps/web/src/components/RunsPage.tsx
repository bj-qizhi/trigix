import { useEffect, useState } from 'react'
import { useAuth } from '../AuthContext'
import * as api from '../api/client'
import type { ExecutionSummary } from '../types'

interface Props {
  onBack: () => void
  onOpenExecution: (executionId: string) => void
}

function formatTs(secs: number): string {
  return new Date(secs * 1000).toLocaleString()
}

function formatDuration(started: number): string {
  const secs = Math.floor(Date.now() / 1000) - started
  if (secs < 5) return '< 5s'
  if (secs < 60) return `${secs}s ago`
  const mins = Math.floor(secs / 60)
  if (mins < 60) return `${mins}m ago`
  return `${Math.floor(mins / 60)}h ago`
}

export function RunsPage({ onBack, onOpenExecution }: Props) {
  const { auth } = useAuth()
  const [runs, setRuns]       = useState<ExecutionSummary[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError]     = useState<string | null>(null)

  const load = () => {
    setLoading(true)
    setError(null)
    api.listExecutions(auth!.tenantId)
      .then(setRuns)
      .catch((e: unknown) => setError(String(e)))
      .finally(() => setLoading(false))
  }

  useEffect(load, [])

  return (
    <div className="app">
      <header className="topbar">
        <span className="topbar-logo">aiworkflow</span>
        <span className="topbar-sep">|</span>
        <span className="topbar-title">Run History</span>
        <div className="topbar-actions">
          <button className="btn btn-sm" onClick={load}>Refresh</button>
          <button className="btn btn-sm" onClick={onBack}>← Back</button>
        </div>
      </header>

      <main className="list-page">
        <div className="list-header">
          <h1>Execution Runs</h1>
          <span style={{ color: 'var(--muted)', fontSize: 13 }}>{runs.length} total</span>
        </div>

        {loading && <p>Loading…</p>}
        {error && <p style={{ color: 'var(--danger-text)' }}>{error}</p>}

        {!loading && !error && runs.length === 0 && (
          <div className="empty-state">
            <p>No executions yet. Run a workflow to see history here.</p>
          </div>
        )}

        {!loading && runs.length > 0 && (
          <table className="workflow-table">
            <thead>
              <tr>
                <th>Started</th>
                <th>Workflow</th>
                <th>Status</th>
                <th>Age</th>
                <th>Run ID</th>
              </tr>
            </thead>
            <tbody>
              {runs.map((run) => (
                <tr
                  key={run.id}
                  onClick={() => onOpenExecution(run.id)}
                  title="View execution details"
                >
                  <td style={{ fontSize: 12, color: 'var(--muted)', whiteSpace: 'nowrap' }}>
                    {formatTs(run.started_at)}
                  </td>
                  <td
                    className="name"
                    style={{ fontFamily: 'monospace', fontSize: 12 }}
                  >
                    {run.workflow_id.slice(-12)}
                  </td>
                  <td>
                    <span className={`badge badge-${run.status}`}>{run.status}</span>
                  </td>
                  <td style={{ color: 'var(--muted)', fontSize: 12 }}>
                    {formatDuration(run.started_at)}
                  </td>
                  <td style={{ color: 'var(--muted)', fontSize: 11, fontFamily: 'monospace' }}>
                    {run.id.slice(-8)}
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
