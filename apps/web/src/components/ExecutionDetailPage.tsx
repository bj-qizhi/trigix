import { useEffect, useState } from 'react'
import { useAuth } from '../AuthContext'
import * as api from '../api/client'
import type { ExecutionRecord } from '../types'

interface Props {
  executionId: string
  onBack: () => void
  onOpenWorkflow: (workflowId: string) => void
  onRetry: (newExecutionId: string) => void
}

function formatTs(secs: number): string {
  return new Date(secs * 1000).toLocaleString()
}

function formatDuration(started: number, finished?: number): string {
  if (!finished) return 'in progress…'
  const secs = finished - started
  if (secs < 60) return `${secs}s`
  return `${Math.floor(secs / 60)}m ${secs % 60}s`
}

function prettyJson(raw: string | null): string {
  if (!raw) return ''
  try { return JSON.stringify(JSON.parse(raw), null, 2) } catch { return raw }
}

export function ExecutionDetailPage({ executionId, onBack, onOpenWorkflow, onRetry }: Props) {
  const { auth } = useAuth()
  const [record, setRecord]         = useState<ExecutionRecord | null>(null)
  const [loading, setLoading]       = useState(true)
  const [error, setError]           = useState<string | null>(null)
  const [cancelling, setCancelling] = useState(false)
  const [retrying, setRetrying]     = useState(false)

  const load = (quiet = false) => {
    if (!quiet) setLoading(true)
    api.getExecution(auth!.tenantId, executionId)
      .then(setRecord)
      .catch((e: unknown) => setError(String(e)))
      .finally(() => setLoading(false))
  }

  useEffect(() => { load() }, [executionId])

  // Poll while running or waiting for approval
  useEffect(() => {
    if (!record) return
    if (record.status !== 'running' && record.status !== 'waiting_approval') return
    const timer = setInterval(() => load(true), 1500)
    return () => clearInterval(timer)
  }, [record?.id, record?.status])

  const isLive = record?.status === 'running' || record?.status === 'waiting_approval'

  const isRetryable = record?.status === 'failed' || record?.status === 'cancelled'

  const handleRetry = async () => {
    if (!record || !isRetryable) return
    setRetrying(true)
    try {
      const newExec = await api.retryExecution(auth!.tenantId, executionId)
      onRetry(newExec.id)
    } catch (e) {
      setError(String(e))
      setRetrying(false)
    }
  }

  const handleCancel = async () => {
    if (!record || !isLive) return
    setCancelling(true)
    try {
      await api.cancelExecution(auth!.tenantId, executionId)
      load(true)
    } catch (e) {
      setError(String(e))
    } finally {
      setCancelling(false)
    }
  }

  return (
    <div className="app">
      <header className="topbar">
        <button className="btn btn-sm btn-icon" onClick={onBack} title="Back to runs">←</button>
        <span className="topbar-sep">|</span>
        <span className="topbar-title" style={{ fontFamily: 'monospace', fontSize: 13 }}>
          run:{executionId.slice(-12)}
        </span>
        {record && (
          <span className={`badge badge-${record.status}`}>{record.status}</span>
        )}
        {isLive && (
          <span style={{ fontSize: 11, color: 'var(--link)', animation: 'pulse 1.5s infinite' }}>
            live
          </span>
        )}
        <div className="topbar-actions">
          {isRetryable && (
            <button
              className="btn btn-sm btn-primary"
              disabled={retrying}
              onClick={handleRetry}
              title="Retry this execution with the same input"
            >
              {retrying ? 'Retrying…' : '↺ Retry'}
            </button>
          )}
          {isLive && (
            <button
              className="btn btn-sm btn-danger"
              disabled={cancelling}
              onClick={handleCancel}
              title="Cancel this execution"
            >
              {cancelling ? 'Cancelling…' : '✕ Cancel'}
            </button>
          )}
          {record && (
            <button
              className="btn btn-sm"
              onClick={() => onOpenWorkflow(record.workflow_id)}
              title="Open workflow editor"
            >
              Open Workflow →
            </button>
          )}
        </div>
      </header>

      <main className="list-page">
        {loading && <p>Loading…</p>}
        {error && <p style={{ color: 'var(--danger-text)' }}>{error}</p>}

        {record && (
          <>
            {/* ── Summary ── */}
            <section style={{ marginBottom: 28 }}>
              <h1 style={{ marginBottom: 16 }}>Execution Summary</h1>
              <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(200px, 1fr))', gap: 12 }}>
                <StatCard label="Status">
                  <span className={`badge badge-${record.status}`}>{record.status}</span>
                </StatCard>
                <StatCard label="Started">
                  {formatTs(record.started_at)}
                </StatCard>
                <StatCard label="Finished">
                  {record.finished_at ? formatTs(record.finished_at) : '—'}
                </StatCard>
                <StatCard label="Duration">
                  {formatDuration(record.started_at, record.finished_at)}
                </StatCard>
                <StatCard label="Workflow ID">
                  <span style={{ fontFamily: 'monospace', fontSize: 12 }}>
                    {record.workflow_id.slice(-12)}
                  </span>
                </StatCard>
                <StatCard label="Version ID">
                  <span style={{ fontFamily: 'monospace', fontSize: 12 }}>
                    {record.workflow_version_id.slice(-12)}
                  </span>
                </StatCard>
              </div>
            </section>

            {/* ── Input ── */}
            <section style={{ marginBottom: 28 }}>
              <h2 style={{ marginBottom: 10 }}>Input</h2>
              <pre style={{
                background: 'var(--panel)', border: '1px solid var(--border)',
                borderRadius: 'var(--radius)', padding: '10px 12px',
                fontSize: 12, fontFamily: 'monospace', overflowX: 'auto',
                color: 'var(--muted)', lineHeight: 1.5, maxHeight: 180, overflowY: 'auto',
              }}>
                {prettyJson(record.input_json) || '{}'}
              </pre>
            </section>

            {/* ── Node Results ── */}
            <section>
              <h2 style={{ marginBottom: 10 }}>
                Node Results
                <span style={{ fontSize: 12, fontWeight: 400, color: 'var(--muted)', marginLeft: 8 }}>
                  {record.node_results.length} node{record.node_results.length !== 1 ? 's' : ''}
                </span>
              </h2>

              {record.node_results.length === 0 ? (
                <p style={{ color: 'var(--muted)' }}>No node results yet.</p>
              ) : (
                <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
                  {record.node_results.map((nr) => (
                    <div
                      key={nr.node_id}
                      style={{
                        background: 'var(--surface)', border: '1px solid var(--border)',
                        borderRadius: 'var(--radius)', overflow: 'hidden',
                      }}
                    >
                      <div style={{
                        display: 'flex', alignItems: 'center', gap: 8,
                        padding: '8px 12px', borderBottom: '1px solid var(--border)',
                        background: 'var(--panel)',
                      }}>
                        <code style={{ fontSize: 13, fontWeight: 600 }}>{nr.node_id}</code>
                        <span style={{ fontSize: 11, color: 'var(--muted)' }}>{nr.node_type}</span>
                        <span className={`badge badge-${nr.status}`} style={{ marginLeft: 'auto' }}>
                          {nr.status}
                        </span>
                      </div>
                      {nr.error ? (
                        <div style={{ padding: '8px 12px', color: 'var(--danger-text)', fontSize: 12, fontFamily: 'monospace' }}>
                          {nr.error}
                        </div>
                      ) : nr.output_json ? (
                        <pre style={{
                          margin: 0, padding: '8px 12px',
                          fontSize: 11, fontFamily: 'monospace', color: 'var(--muted)',
                          whiteSpace: 'pre-wrap', wordBreak: 'break-all',
                          maxHeight: 160, overflowY: 'auto', lineHeight: 1.5,
                        }}>
                          {prettyJson(nr.output_json)}
                        </pre>
                      ) : (
                        <div style={{ padding: '8px 12px', color: 'var(--muted)', fontSize: 12 }}>
                          No output
                        </div>
                      )}
                    </div>
                  ))}
                </div>
              )}
            </section>
          </>
        )}
      </main>
    </div>
  )
}

function StatCard({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div style={{
      background: 'var(--panel)', border: '1px solid var(--border)',
      borderRadius: 'var(--radius)', padding: '10px 14px',
    }}>
      <div style={{ fontSize: 11, color: 'var(--muted)', fontWeight: 600, marginBottom: 4, textTransform: 'uppercase', letterSpacing: '0.05em' }}>
        {label}
      </div>
      <div style={{ fontSize: 13 }}>{children}</div>
    </div>
  )
}
