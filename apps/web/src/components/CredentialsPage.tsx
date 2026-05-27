import { useEffect, useState } from 'react'
import { useAuth } from '../AuthContext'
import * as api from '../api/client'
import type { CredentialSummary } from '../types'

interface Props {
  onBack: () => void
}

export function CredentialsPage({ onBack }: Props) {
  const { auth } = useAuth()
  const [credentials, setCredentials] = useState<CredentialSummary[]>([])
  const [loading, setLoading]         = useState(true)
  const [error, setError]             = useState<string | null>(null)
  const [adding, setAdding]           = useState(false)
  const [name, setName]               = useState('')
  const [value, setValue]             = useState('')
  const [saving, setSaving]           = useState(false)

  const load = () => {
    setLoading(true)
    setError(null)
    api.listCredentials(auth!.tenantId)
      .then(setCredentials)
      .catch((e: unknown) => setError(String(e)))
      .finally(() => setLoading(false))
  }

  useEffect(load, [])

  const handleAdd = async () => {
    if (!name.trim() || !value.trim()) return
    setSaving(true)
    try {
      const cred = await api.createCredential(auth!.tenantId, name.trim(), value.trim())
      setCredentials((prev) => [...prev, cred].sort((a, b) => a.name.localeCompare(b.name)))
      setAdding(false)
      setName('')
      setValue('')
    } catch (e) {
      setError(String(e))
    } finally {
      setSaving(false)
    }
  }

  const handleDelete = async (id: string) => {
    try {
      await api.deleteCredential(auth!.tenantId, id)
      setCredentials((prev) => prev.filter((c) => c.id !== id))
    } catch (e) {
      setError(String(e))
    }
  }

  return (
    <div className="app">
      <header className="topbar">
        <button className="btn btn-sm btn-icon" onClick={onBack} title="Back">←</button>
        <span className="topbar-sep">|</span>
        <span className="topbar-logo">aiworkflow</span>
        <span className="topbar-sep">|</span>
        <span className="topbar-title">Credentials</span>
        <div className="topbar-actions">
          <button className="btn btn-sm btn-primary" onClick={() => setAdding(true)}>
            + Add Credential
          </button>
        </div>
      </header>

      <div className="list-page">
        <div className="list-header">
          <h1>Credentials</h1>
        </div>

        <p style={{ marginBottom: 16 }}>
          Store secrets here and reference them in node configs with{' '}
          <code style={{ background: 'var(--panel)', padding: '2px 6px', borderRadius: 4, fontSize: 12 }}>
            {'{{credential.name}}'}
          </code>
          . Values are never returned by the API.
        </p>

        {error && (
          <div style={{ color: 'var(--danger-text)', marginBottom: 12, fontSize: 13 }}>{error}</div>
        )}

        {loading ? (
          <div style={{ color: 'var(--muted)' }}>Loading…</div>
        ) : (
          <table className="workflow-table">
            <thead>
              <tr>
                <th>Name</th>
                <th>Reference</th>
                <th style={{ width: 80 }}></th>
              </tr>
            </thead>
            <tbody>
              {credentials.length === 0 ? (
                <tr>
                  <td colSpan={3}>
                    <div className="empty-state">No credentials yet. Add one to get started.</div>
                  </td>
                </tr>
              ) : (
                credentials.map((cred) => (
                  <tr key={cred.id}>
                    <td style={{ fontWeight: 500 }}>{cred.name}</td>
                    <td>
                      <code style={{ fontSize: 12, color: 'var(--muted)' }}>
                        {`{{credential.${cred.name}}}`}
                      </code>
                    </td>
                    <td>
                      <button
                        className="btn btn-sm btn-danger"
                        onClick={() => handleDelete(cred.id)}
                      >
                        Delete
                      </button>
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        )}
      </div>

      {adding && (
        <div className="modal-backdrop" onClick={() => setAdding(false)}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            <h2>Add Credential</h2>
            <div className="field">
              <label>Name (used in {`{{credential.name}}`})</label>
              <input
                autoFocus
                placeholder="e.g. openai-api-key"
                value={name}
                onChange={(e) => setName(e.target.value)}
                onKeyDown={(e) => { if (e.key === 'Enter') handleAdd() }}
              />
            </div>
            <div className="field">
              <label>Secret Value</label>
              <input
                type="password"
                placeholder="Paste the secret here"
                value={value}
                onChange={(e) => setValue(e.target.value)}
                onKeyDown={(e) => { if (e.key === 'Enter') handleAdd() }}
              />
            </div>
            <div className="modal-actions">
              <button className="btn" onClick={() => { setAdding(false); setName(''); setValue('') }}>
                Cancel
              </button>
              <button
                className="btn btn-primary"
                disabled={saving || !name.trim() || !value.trim()}
                onClick={handleAdd}
              >
                {saving ? 'Saving…' : 'Add'}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
