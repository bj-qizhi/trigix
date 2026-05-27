import { useEffect, useRef, useState } from 'react'
import { useAuth } from '../AuthContext'
import * as api from '../api/client'
import type { WorkflowExport, WorkflowRecord, ScheduleSummary } from '../types'

interface Props {
  onOpen: (workflowId: string) => void
  onCredentials: () => void
  onAuditLog: () => void
  onRuns: () => void
}

function formatInterval(secs: number): string {
  if (secs >= 86400 && secs % 86400 === 0) return `every ${secs / 86400}d`
  if (secs >= 3600 && secs % 3600 === 0) return `every ${secs / 3600}h`
  if (secs >= 60 && secs % 60 === 0) return `every ${secs / 60}m`
  return `every ${secs}s`
}

export function WorkflowList({ onOpen, onCredentials, onAuditLog, onRuns }: Props) {
  const { auth, logout } = useAuth()
  const [workflows, setWorkflows] = useState<WorkflowRecord[]>([])
  const [schedules, setSchedules]  = useState<ScheduleSummary[]>([])
  const [loading, setLoading]      = useState(true)
  const [error, setError]          = useState<string | null>(null)
  const [creating, setCreating]    = useState(false)
  const [newName, setNewName]      = useState('')
  const [saving, setSaving]        = useState(false)
  const [importing, setImporting]  = useState(false)
  const [duplicating, setDuplicating] = useState<string | null>(null)
  const importRef = useRef<HTMLInputElement>(null)

  const load = () => {
    setLoading(true)
    setError(null)
    Promise.all([
      api.listWorkflows(auth!.tenantId, auth!.projectId),
      api.listSchedules(auth!.tenantId),
    ])
      .then(([wfs, scheds]) => { setWorkflows(wfs); setSchedules(scheds) })
      .catch((e: unknown) => setError(String(e)))
      .finally(() => setLoading(false))
  }

  useEffect(load, [])

  const handleCreate = async () => {
    if (!newName.trim()) return
    setSaving(true)
    try {
      const wf = await api.createWorkflow(auth!.tenantId, auth!.workspaceId, auth!.projectId, newName.trim())
      setWorkflows((prev) => [wf, ...prev])
      setCreating(false)
      setNewName('')
      onOpen(wf.id)
    } catch (e) {
      alert(String(e))
    } finally {
      setSaving(false)
    }
  }

  const handleDuplicate = async (e: React.MouseEvent, workflowId: string) => {
    e.stopPropagation()
    setDuplicating(workflowId)
    try {
      const wf = await api.duplicateWorkflow(auth!.tenantId, workflowId)
      setWorkflows((prev) => [wf, ...prev])
      onOpen(wf.id)
    } catch (e) {
      alert(String(e))
    } finally {
      setDuplicating(null)
    }
  }

  const handleImportFile = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0]
    if (!file) return
    e.target.value = ''
    setImporting(true)
    try {
      const text = await file.text()
      const data = JSON.parse(text) as WorkflowExport
      const name = data.name ?? file.name.replace(/\.json$/i, '')
      const wf = await api.importWorkflow(
        auth!.tenantId, auth!.workspaceId, auth!.projectId, name, data.graph,
      )
      setWorkflows((prev) => [wf, ...prev])
      onOpen(wf.id)
    } catch (e) {
      alert(String(e))
    } finally {
      setImporting(false)
    }
  }

  // Build a map from workflow_id → schedule for the table
  const scheduleByWorkflow = new Map(schedules.map((s) => [s.workflow_id, s]))

  return (
    <div className="app">
      <header className="topbar">
        <span className="topbar-logo">aiworkflow</span>
        <div className="topbar-actions">
          <input
            ref={importRef}
            type="file"
            accept=".json,application/json"
            style={{ display: 'none' }}
            onChange={handleImportFile}
          />
          <button
            className="btn btn-sm"
            disabled={importing}
            onClick={() => importRef.current?.click()}
            title="Import workflow from JSON file"
          >
            {importing ? 'Importing…' : '↑ Import'}
          </button>
          <button className="btn btn-sm" onClick={onRuns} title="View all execution runs">
            Runs
          </button>
          <button className="btn btn-sm" onClick={onAuditLog} title="View audit log">
            Audit Log
          </button>
          <button className="btn btn-sm" onClick={onCredentials} title="Manage stored secrets">
            🔑 Credentials
          </button>
          <button className="btn btn-primary" onClick={() => setCreating(true)}>
            + New Workflow
          </button>
          <button className="btn btn-sm" onClick={logout} title="Sign out">
            Sign out
          </button>
        </div>
      </header>

      <main className="list-page">
        <div className="list-header">
          <h1>Workflows</h1>
        </div>

        {loading && <p>Loading…</p>}
        {error && <p style={{ color: 'var(--danger-text)' }}>{error}</p>}

        {!loading && !error && workflows.length === 0 && (
          <div className="empty-state">
            <p>No workflows yet. Create one to get started.</p>
          </div>
        )}

        {!loading && workflows.length > 0 && (
          <table className="workflow-table">
            <thead>
              <tr>
                <th>Name</th>
                <th>Status</th>
                <th>Latest Version</th>
                <th>Schedule</th>
                <th></th>
              </tr>
            </thead>
            <tbody>
              {workflows.map((wf) => {
                const sched = scheduleByWorkflow.get(wf.id)
                return (
                  <tr key={wf.id} onClick={() => onOpen(wf.id)}>
                    <td className="name">{wf.name}</td>
                    <td>
                      <span className={`badge badge-${wf.status}`}>{wf.status}</span>
                    </td>
                    <td style={{ color: 'var(--muted)' }}>
                      {wf.latest_version_id ? 'v' + wf.latest_version_id.slice(-4) : '—'}
                    </td>
                    <td>
                      {sched ? (
                        <span style={{ fontSize: 12, color: 'var(--warning-text)' }}>
                          ⏱ {formatInterval(sched.interval_secs)}
                        </span>
                      ) : (
                        <span style={{ color: 'var(--muted)', fontSize: 12 }}>—</span>
                      )}
                    </td>
                    <td style={{ display: 'flex', gap: 6, alignItems: 'center' }}>
                      <button
                        className="btn btn-sm"
                        onClick={(e) => { e.stopPropagation(); onOpen(wf.id) }}
                      >
                        Open
                      </button>
                      <button
                        className="btn btn-sm"
                        disabled={duplicating === wf.id}
                        onClick={(e) => handleDuplicate(e, wf.id)}
                        title="Duplicate this workflow"
                      >
                        {duplicating === wf.id ? '…' : '⧉ Duplicate'}
                      </button>
                    </td>
                  </tr>
                )
              })}
            </tbody>
          </table>
        )}
      </main>

      {creating && (
        <div className="modal-backdrop" onClick={() => setCreating(false)}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            <h2>New Workflow</h2>
            <div className="field">
              <label>Name</label>
              <input
                autoFocus
                placeholder="e.g. Lead Enrichment"
                value={newName}
                onChange={(e) => setNewName(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && handleCreate()}
              />
            </div>
            <div className="modal-actions">
              <button className="btn" onClick={() => setCreating(false)}>Cancel</button>
              <button
                className="btn btn-primary"
                disabled={!newName.trim() || saving}
                onClick={handleCreate}
              >
                {saving ? 'Creating…' : 'Create'}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
