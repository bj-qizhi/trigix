// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useEffect, useState } from 'react'
import * as api from '../api/client'
import type { FormTokenRecord, ExecutionRecord } from '../api/client'

export function FormPage({ token }: { token: string }) {
  const [form, setForm] = useState<FormTokenRecord | null>(null)
  const [inputJson, setInputJson] = useState('{}')
  const [submitting, setSubmitting] = useState(false)
  const [result, setResult] = useState<ExecutionRecord | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [notFound, setNotFound] = useState(false)

  useEffect(() => {
    if (!token) return
    api.getForm(token).then(setForm).catch(() => setNotFound(true))
  }, [token])

  const handleSubmit = async () => {
    if (!token) return
    try { JSON.parse(inputJson) } catch {
      setError('Invalid JSON')
      return
    }
    setSubmitting(true)
    setError(null)
    setResult(null)
    try {
      const rec = await api.submitForm(token, inputJson)
      setResult(rec)
    } catch (e) {
      setError(String(e))
    } finally {
      setSubmitting(false)
    }
  }

  if (notFound) {
    return (
      <div style={{ minHeight: '100vh', display: 'flex', alignItems: 'center', justifyContent: 'center', background: 'var(--bg)' }}>
        <div style={{ textAlign: 'center', color: 'var(--muted)', padding: 32 }}>
          <div style={{ fontSize: 40, marginBottom: 12 }}>404</div>
          <div style={{ fontSize: 16 }}>Form not found or has been removed.</div>
        </div>
      </div>
    )
  }

  if (!form) {
    return (
      <div style={{ minHeight: '100vh', display: 'flex', alignItems: 'center', justifyContent: 'center', background: 'var(--bg)' }}>
        <span style={{ color: 'var(--muted)', fontSize: 14 }}>Loading…</span>
      </div>
    )
  }

  return (
    <div style={{ minHeight: '100vh', background: 'var(--bg)', display: 'flex', flexDirection: 'column', alignItems: 'center', padding: '48px 16px' }}>
      <div style={{
        width: '100%', maxWidth: 520,
        background: 'var(--surface)',
        border: '1px solid var(--border)',
        borderRadius: 10,
        padding: '32px 28px',
        boxShadow: '0 4px 24px rgba(0,0,0,0.08)',
      }}>
        <h1 style={{ margin: '0 0 6px', fontSize: 22, fontWeight: 700 }}>{form.title}</h1>
        {form.description && (
          <p style={{ margin: '0 0 20px', color: 'var(--muted)', fontSize: 14 }}>{form.description}</p>
        )}

        {result ? (
          <div>
            <div style={{
              padding: '14px 18px',
              background: 'rgba(16,185,129,0.07)',
              border: '1px solid var(--success-text)',
              borderRadius: 8,
              marginBottom: 20,
            }}>
              <div style={{ fontSize: 13, fontWeight: 600, color: 'var(--success-text)', marginBottom: 4 }}>
                Submitted successfully
              </div>
              <div style={{ fontSize: 12, color: 'var(--muted)', fontFamily: 'monospace' }}>
                Run ID: {result.id}
              </div>
            </div>
            <button
              className="btn btn-sm"
              onClick={() => { setResult(null); setInputJson('{}') }}
              style={{ fontSize: 13 }}
            >
              Submit another
            </button>
          </div>
        ) : (
          <>
            <SchemaFormOrJson
              schema={form.input_schema as InputField[]}
              value={inputJson}
              onChange={setInputJson}
            />
            {error && (
              <div style={{ color: 'var(--danger-text)', fontSize: 12, margin: '8px 0' }}>{error}</div>
            )}
            <button
              className="btn btn-success"
              disabled={submitting}
              onClick={handleSubmit}
              style={{ marginTop: 16, width: '100%', fontSize: 14, padding: '8px 0' }}
            >
              {submitting ? 'Submitting…' : 'Submit'}
            </button>
          </>
        )}
      </div>

      <div style={{ marginTop: 24, fontSize: 11, color: 'var(--muted)' }}>
        Powered by AI Workflow
      </div>
    </div>
  )
}

interface InputField {
  key: string
  field_type: 'text' | 'number' | 'boolean' | 'json'
  required?: boolean
  description?: string
  default_value?: string
}

function SchemaFormOrJson({
  schema,
  value,
  onChange,
}: {
  schema: InputField[]
  value: string
  onChange: (v: string) => void
}) {
  const [values, setValues] = useState<Record<string, string>>(() => {
    const init: Record<string, string> = {}
    for (const f of schema) init[f.key] = f.default_value ?? ''
    return init
  })

  const hasSchema = schema.length > 0

  const set = (key: string, val: string) => {
    const next = { ...values, [key]: val }
    setValues(next)
    const obj: Record<string, unknown> = {}
    for (const f of schema) {
      const raw = next[f.key] ?? ''
      if (f.field_type === 'number') obj[f.key] = raw === '' ? null : Number(raw)
      else if (f.field_type === 'boolean') obj[f.key] = raw === 'true'
      else if (f.field_type === 'json') { try { obj[f.key] = JSON.parse(raw) } catch { obj[f.key] = raw } }
      else obj[f.key] = raw
    }
    onChange(JSON.stringify(obj))
  }

  if (!hasSchema) {
    return (
      <div>
        <label style={{ display: 'block', fontSize: 13, fontWeight: 500, marginBottom: 6 }}>
          Input JSON
        </label>
        <textarea
          value={value}
          onChange={(e) => onChange(e.target.value)}
          rows={6}
          style={{
            width: '100%', boxSizing: 'border-box',
            fontFamily: 'monospace', fontSize: 12,
            padding: '8px 10px',
            background: 'var(--bg)', border: '1px solid var(--border)',
            borderRadius: 6, color: 'var(--fg)', resize: 'vertical',
          }}
          placeholder="{}"
        />
      </div>
    )
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
      {schema.map((f) => (
        <label key={f.key} style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
          <span style={{ fontSize: 13, fontWeight: 500 }}>
            {f.key}{f.required ? <span style={{ color: 'var(--danger-text)', marginLeft: 2 }}>*</span> : ''}
          </span>
          {f.description && (
            <span style={{ fontSize: 11, color: 'var(--muted)' }}>{f.description}</span>
          )}
          {f.field_type === 'boolean' ? (
            <select
              value={values[f.key] ?? 'false'}
              onChange={(e) => set(f.key, e.target.value)}
              style={{ fontSize: 13, padding: '6px 8px', borderRadius: 6, border: '1px solid var(--border)', background: 'var(--bg)', color: 'var(--fg)' }}
            >
              <option value="true">true</option>
              <option value="false">false</option>
            </select>
          ) : f.field_type === 'json' ? (
            <textarea
              value={values[f.key] ?? ''}
              onChange={(e) => set(f.key, e.target.value)}
              rows={3}
              style={{ fontFamily: 'monospace', fontSize: 12, padding: '6px 8px', borderRadius: 6, border: '1px solid var(--border)', background: 'var(--bg)', color: 'var(--fg)', resize: 'vertical' }}
            />
          ) : (
            <input
              type={f.field_type === 'number' ? 'number' : 'text'}
              value={values[f.key] ?? ''}
              onChange={(e) => set(f.key, e.target.value)}
              placeholder={f.description || f.field_type}
              style={{ fontSize: 13, padding: '6px 8px', borderRadius: 6, border: '1px solid var(--border)', background: 'var(--bg)', color: 'var(--fg)' }}
            />
          )}
        </label>
      ))}
    </div>
  )
}
