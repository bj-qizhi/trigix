import { useState } from 'react'
import { useAuth } from '../AuthContext'

const API_BASE = import.meta.env.VITE_API_BASE ?? 'http://localhost:38080'

export function LoginPage() {
  const { login } = useAuth()
  const [apiKey, setApiKey] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    setError(null)
    setLoading(true)
    try {
      const res = await fetch(`${API_BASE}/v1/auth/token`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ api_key: apiKey }),
      })
      if (!res.ok) {
        const body = await res.text().catch(() => '')
        throw new Error(body || `${res.status} ${res.statusText}`)
      }
      const data = await res.json() as {
        token: string
        tenant_id: string
        workspace_id: string
        project_id: string
      }
      login({
        token: data.token,
        tenantId: data.tenant_id,
        workspaceId: data.workspace_id,
        projectId: data.project_id,
      })
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Login failed')
    } finally {
      setLoading(false)
    }
  }

  return (
    <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100vh', background: '#0f172a' }}>
      <form
        onSubmit={handleSubmit}
        style={{ background: '#1e293b', padding: '2rem', borderRadius: '8px', minWidth: '320px', display: 'flex', flexDirection: 'column', gap: '1rem' }}
      >
        <h1 style={{ color: '#f1f5f9', margin: 0, fontSize: '1.25rem' }}>AgentFlow</h1>
        <p style={{ color: '#94a3b8', margin: 0, fontSize: '0.875rem' }}>Enter your API key to continue</p>
        <input
          type="password"
          placeholder="API key"
          value={apiKey}
          onChange={(e) => setApiKey(e.target.value)}
          required
          style={{ padding: '0.5rem 0.75rem', borderRadius: '4px', border: '1px solid #334155', background: '#0f172a', color: '#f1f5f9', fontSize: '0.875rem' }}
        />
        {error && <p style={{ color: '#ef4444', margin: 0, fontSize: '0.8rem' }}>{error}</p>}
        <button
          type="submit"
          disabled={loading}
          className="btn btn-primary"
        >
          {loading ? 'Signing in…' : 'Sign in'}
        </button>
      </form>
    </div>
  )
}
