// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useEffect, useRef, useState } from 'react'
import * as api from '../../api/client'
import type { ConfigProps } from './types'

export function CronExpressionField({ value, onChange }: { value: string; onChange: (v: string) => void }) {
  const [preview, setPreview] = useState<string[]>([])
  const [cronError, setCronError] = useState<string | null>(null)
  const debounceRef = useRef<ReturnType<typeof setTimeout>>()

  useEffect(() => {
    clearTimeout(debounceRef.current)
    if (!value.trim()) { setPreview([]); setCronError(null); return }
    debounceRef.current = setTimeout(() => {
      api.previewCron(value.trim(), 3).then((res) => {
        if (res.error) { setCronError(res.error); setPreview([]) }
        else { setCronError(null); setPreview(res.next_times) }
      }).catch(() => {})
    }, 600)
    return () => clearTimeout(debounceRef.current)
  }, [value])

  return (
    <div className="field">
      <label>Cron Expression</label>
      <input
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder="0 9 * * 1-5"
        style={{ fontFamily: 'monospace', borderColor: cronError ? 'var(--danger-text)' : undefined }}
      />
      {cronError && (
        <p style={{ fontSize: 11, color: 'var(--danger-text)', marginTop: 4 }}>{cronError}</p>
      )}
      {preview.length > 0 && (
        <div style={{ marginTop: 6, fontSize: 11, color: 'var(--muted)' }}>
          <div style={{ fontWeight: 600, marginBottom: 2 }}>Next fires:</div>
          {preview.map((t, i) => <div key={i} style={{ fontFamily: 'monospace' }}>{t}</div>)}
        </div>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', marginTop: 4 }}>
        7-field format: <code>sec min hour day month weekday year</code>.
        Example: <code>0 0 9 * * Mon-Fri *</code> = weekdays at 9am.
      </p>
    </div>
  )
}

export function HeadersEditor({ set }: { set: ConfigProps['set'] }) {
  return (
    <div className="field">
      <label>Headers (one per line: Key: Value)</label>
      <textarea
        placeholder="Authorization: Bearer token&#10;X-Custom: value"
        style={{ fontFamily: 'monospace', fontSize: 12 }}
        onChange={(e) => {
          const headers: Record<string, string> = {}
          for (const line of e.target.value.split('\n')) {
            const i = line.indexOf(':')
            if (i > 0) {
              headers[line.slice(0, i).trim()] = line.slice(i + 1).trim()
            }
          }
          set('headers', headers)
        }}
      />
    </div>
  )
}

export function TriggerConfig({
  config, set, num, str, webhookUrl, webhookSecret,
}: { config: Record<string, unknown>; set: ConfigProps['set']; num: ConfigProps['num']; str: ConfigProps['str']; webhookUrl?: string | null; webhookSecret?: string | null }) {
  void config
  const intervalSecs = num('interval_secs', 0)
  const cronExpr = str('cron_expression', '')
  const scheduleMode = cronExpr ? 'cron' : (intervalSecs > 0 ? 'interval' : 'none')

  return (
    <div style={{ fontSize: 13, display: 'flex', flexDirection: 'column', gap: 12 }}>
      <div style={{ color: 'var(--muted)' }}>
        <p>This node starts the workflow.</p>
        <p style={{ marginTop: 4 }}>
          It passes the execution{' '}
          <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>input_json</code>{' '}
          to downstream nodes.
        </p>
      </div>

      {/* Schedule mode selector */}
      <div className="field">
        <label>Auto-run Schedule</label>
        <select
          value={scheduleMode}
          onChange={(e) => {
            const mode = e.target.value
            if (mode === 'none') {
              set('interval_secs', undefined as unknown as number)
              set('cron_expression', undefined as unknown as string)
            } else if (mode === 'interval') {
              set('cron_expression', undefined as unknown as string)
              set('interval_secs', 3600)
            } else {
              set('interval_secs', undefined as unknown as number)
              set('cron_expression', '0 9 * * 1-5')
            }
          }}
        >
          <option value="none">None (manual / webhook only)</option>
          <option value="interval">Fixed interval</option>
          <option value="cron">Cron expression</option>
        </select>
      </div>

      {scheduleMode === 'interval' && (
        <div className="field">
          <label>Interval</label>
          <select
            value={intervalSecs || ''}
            onChange={(e) =>
              set('interval_secs', e.target.value ? Number(e.target.value) : undefined as unknown as number)
            }
          >
            <option value="60">Every minute</option>
            <option value="300">Every 5 minutes</option>
            <option value="3600">Every hour</option>
            <option value="86400">Every day</option>
          </select>
          <p style={{ fontSize: 11, color: 'var(--muted)', marginTop: 4 }}>
            Schedule activates when the workflow is published.
          </p>
        </div>
      )}

      {scheduleMode === 'cron' && (
        <CronExpressionField
          value={cronExpr}
          onChange={(v) => set('cron_expression', v)}
        />
      )}

      {/* Webhook secret config */}
      <div className="field">
        <label>Webhook Secret</label>
        <input
          type="text"
          value={str('webhook_secret', '')}
          onChange={(e) => set('webhook_secret', e.target.value || undefined)}
          placeholder="Optional signing secret"
          style={{ fontFamily: 'monospace' }}
        />
        <p style={{ fontSize: 11, color: 'var(--muted)', marginTop: 4 }}>
          If set, callers must send <code>X-Webhook-Signature: sha256=&lt;hmac&gt;</code>.
          Publish to activate.
        </p>
      </div>

      {/* Failure alert webhook */}
      <div className="field">
        <label>Failure Alert URL <span style={{ color: 'var(--muted)', fontWeight: 400 }}>(optional)</span></label>
        <input
          type="url"
          value={str('on_failure_url', '')}
          onChange={(e) => set('on_failure_url', e.target.value || undefined)}
          placeholder="https://your-service/alert"
        />
        <p style={{ fontSize: 11, color: 'var(--muted)', marginTop: 4 }}>
          POST with <code>{'{ execution_id, workflow_id, status, started_at, finished_at }'}</code> when execution fails.
        </p>
      </div>

      {/* Webhook URL display */}
      {webhookUrl ? (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
          <p style={{ fontSize: 12, color: 'var(--text)', fontWeight: 500, margin: 0 }}>Webhook URL</p>
          <div style={{ display: 'flex', gap: 6, alignItems: 'flex-start' }}>
            <code style={{ fontSize: 11, background: 'var(--panel)', padding: '5px 7px', borderRadius: 4, flex: 1, wordBreak: 'break-all', lineHeight: 1.5 }}>
              {webhookUrl}
            </code>
            <button
              className="btn btn-sm"
              style={{ flexShrink: 0 }}
              onClick={() => void navigator.clipboard.writeText(webhookUrl)}
              title="Copy webhook URL"
            >
              Copy
            </button>
          </div>
          {webhookSecret && (
            <>
              <p style={{ fontSize: 12, color: 'var(--text)', fontWeight: 500, margin: 0 }}>Webhook Secret</p>
              <div style={{ display: 'flex', gap: 6, alignItems: 'center' }}>
                <code style={{ fontSize: 11, background: 'var(--panel)', padding: '4px 7px', borderRadius: 4, flex: 1, wordBreak: 'break-all' }}>
                  {webhookSecret}
                </code>
                <button
                  className="btn btn-sm"
                  style={{ flexShrink: 0 }}
                  onClick={() => void navigator.clipboard.writeText(webhookSecret)}
                  title="Copy secret"
                >
                  Copy
                </button>
              </div>
            </>
          )}
          <p style={{ fontSize: 11, color: 'var(--muted)' }}>
            POST JSON to this URL to trigger an execution.
          </p>
        </div>
      ) : (
        <p style={{ fontSize: 12, color: 'var(--muted)' }}>
          Publish a version to get a webhook URL.
        </p>
      )}
    </div>
  )
}

export function HttpConfig({ config, set, str, num }: ConfigProps) {
  const authType = str('auth_type', 'none')
  return (
    <>
      <div className="field">
        <label>URL *</label>
        <input
          placeholder="https://api.example.com/{{input.id}}"
          value={str('url')}
          onChange={(e) => set('url', e.target.value)}
        />
        <TemplatePreview text={str('url')} />
      </div>
      <TemplateHint />
      <div className="field">
        <label>Method</label>
        <select value={str('method', 'GET')} onChange={(e) => set('method', e.target.value)}>
          <option>GET</option>
          <option>POST</option>
          <option>PUT</option>
          <option>PATCH</option>
          <option>DELETE</option>
        </select>
      </div>
      <div className="field">
        <label>Auth Type</label>
        <select value={authType} onChange={(e) => set('auth_type', e.target.value)}>
          <option value="none">None</option>
          <option value="bearer">Bearer Token</option>
          <option value="oauth2">OAuth2 Client Credentials</option>
        </select>
      </div>
      {authType === 'bearer' && (
        <div className="field">
          <label>Bearer Token</label>
          <input
            placeholder="{{credential.my-api-key}}"
            value={str('auth_token')}
            onChange={(e) => set('auth_token', e.target.value)}
          />
        </div>
      )}
      {authType === 'oauth2' && (
        <>
          <div className="field">
            <label>Token URL *</label>
            <input
              placeholder="https://auth.example.com/oauth/token"
              value={str('oauth2_token_url')}
              onChange={(e) => set('oauth2_token_url', e.target.value)}
            />
          </div>
          <div className="field">
            <label>Client ID</label>
            <input
              placeholder="{{credential.oauth_client_id}}"
              value={str('oauth2_client_id')}
              onChange={(e) => set('oauth2_client_id', e.target.value)}
            />
          </div>
          <div className="field">
            <label>Client Secret</label>
            <input
              placeholder="{{credential.oauth_client_secret}}"
              value={str('oauth2_client_secret')}
              onChange={(e) => set('oauth2_client_secret', e.target.value)}
            />
          </div>
          <div className="field">
            <label>Scope <span style={{ color: 'var(--muted)', fontWeight: 400 }}>(optional)</span></label>
            <input
              placeholder="read write"
              value={str('oauth2_scope')}
              onChange={(e) => set('oauth2_scope', e.target.value)}
            />
          </div>
        </>
      )}
      <div className="field">
        <label>Body (JSON)</label>
        <textarea
          placeholder={'{"id": "{{input.id}}"}'}
          value={str('body')}
          onChange={(e) => set('body', e.target.value)}
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
        <TemplatePreview text={str('body')} />
      </div>
      <HeadersEditor set={set} />
      <div className="field" style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
        <input
          id="fail_on_error"
          type="checkbox"
          checked={config.fail_on_error !== false}
          onChange={(e) => set('fail_on_error', e.target.checked)}
          style={{ width: 14, height: 14, cursor: 'pointer' }}
        />
        <label htmlFor="fail_on_error" style={{ cursor: 'pointer', marginBottom: 0 }}>
          Fail node on non-2xx status
        </label>
      </div>
      <div style={{ display: 'flex', gap: 8 }}>
        <div className="field" style={{ flex: 1 }}>
          <label>Retries</label>
          <input
            type="number" min={0} max={5}
            value={num('max_retries', 0)}
            onChange={(e) => set('max_retries', Number(e.target.value))}
          />
        </div>
        <div className="field" style={{ flex: 1 }}>
          <label>Timeout (s)</label>
          <input
            type="number" min={0} max={300} placeholder="none"
            value={num('timeout_secs', 0) || ''}
            onChange={(e) => set('timeout_secs', Number(e.target.value) || undefined as unknown as number)}
          />
        </div>
      </div>
    </>
  )
}

export function AgentConfig({ config, set, str, num }: ConfigProps) {
  const tools = (config['tools'] as string[] | undefined) ?? []
  const toggleTool = (tool: string, on: boolean) => {
    const next = on ? [...tools, tool] : tools.filter((t) => t !== tool)
    set('tools', next.length ? next : undefined)
  }
  return (
    <>
      <div className="field">
        <label>Provider</label>
        <select
          value={str('provider', 'anthropic')}
          onChange={(e) => {
            const p = e.target.value
            set('provider', p)
            if (p === 'openai' && str('model', '').startsWith('claude')) set('model', '')
            if (p === 'anthropic') {
              set('model', 'claude-sonnet-4-6')
              set('base_url', undefined)
            }
          }}
        >
          <option value="anthropic">Anthropic (Claude)</option>
          <option value="openai">OpenAI-compatible (Qwen / DeepSeek / Zhipu / vLLM …)</option>
        </select>
      </div>
      {str('provider', 'anthropic') === 'anthropic' ? (
        <div className="field">
          <label>Model</label>
          <select value={str('model', 'claude-sonnet-4-6')} onChange={(e) => set('model', e.target.value)}>
            <option value="claude-haiku-4-5-20251001">claude-haiku-4-5 (fast)</option>
            <option value="claude-sonnet-4-6">claude-sonnet-4-6 (balanced)</option>
            <option value="claude-opus-4-7">claude-opus-4-7 (powerful)</option>
          </select>
        </div>
      ) : (
        <>
          <div className="field">
            <label>Model</label>
            <input
              value={str('model')}
              placeholder="qwen-plus / deepseek-chat / glm-4 / moonshot-v1-8k"
              onChange={(e) => set('model', e.target.value)}
            />
          </div>
          <div className="field">
            <label>Base URL</label>
            <input
              value={str('base_url')}
              placeholder="https://dashscope.aliyuncs.com/compatible-mode/v1"
              onChange={(e) => set('base_url', e.target.value)}
            />
            <span style={{ fontSize: 11, color: 'var(--muted)' }}>
              API key via the OPENAI_API_KEY env var on the runtime (or set api_key in raw config).
            </span>
          </div>
        </>
      )}
      <div className="field">
        <label>System Prompt</label>
        <textarea
          placeholder="You are a helpful assistant."
          value={str('system_prompt')}
          onChange={(e) => set('system_prompt', e.target.value)}
          style={{ minHeight: 80 }}
        />
      </div>
      <div className="field">
        <label>Prompt Template</label>
        <textarea
          placeholder={'Analyze lead {{input.lead_id}}: {{input}}'}
          value={str('prompt_template')}
          onChange={(e) => set('prompt_template', e.target.value)}
          style={{ minHeight: 60 }}
        />
      </div>
      <TemplateHint />
      <div className="field">
        <label>Max Tokens</label>
        <input
          type="number"
          min={64}
          max={8192}
          value={num('max_tokens', 1024)}
          onChange={(e) => set('max_tokens', Number(e.target.value))}
        />
      </div>
      <div className="field">
        <label>Tools (tool-use loop)</label>
        <div style={{ display: 'flex', gap: 14, flexWrap: 'wrap' }}>
          {(['calculator', 'rag_search', 'http_request'] as const).map((tool) => (
            <label key={tool} style={{ display: 'flex', alignItems: 'center', gap: 4, fontSize: 13 }}>
              <input type="checkbox" checked={tools.includes(tool)} onChange={(e) => toggleTool(tool, e.target.checked)} />
              {tool}
            </label>
          ))}
        </div>
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>
          When tools are enabled the agent can call them in a loop until it answers.
        </span>
      </div>
      {tools.includes('rag_search') && (
        <div className="field">
          <label>Knowledge Base (for rag_search)</label>
          <input value={str('kb')} onChange={(e) => set('kb', e.target.value)} />
        </div>
      )}
      {tools.includes('http_request') && (
        <div className="field">
          <label>HTTP allowlist (optional, comma-separated hosts)</label>
          <input
            placeholder="api.internal, data.example.com"
            value={((config['http_allow_hosts'] as string[] | undefined) ?? []).join(', ')}
            onChange={(e) => {
              const hosts = e.target.value.split(',').map((h) => h.trim()).filter(Boolean)
              set('http_allow_hosts', hosts.length ? hosts : undefined)
            }}
          />
          <span style={{ fontSize: 11, color: 'var(--muted)' }}>
            Empty = allow any public host (private/loopback/metadata IPs are always blocked).
          </span>
        </div>
      )}
      {tools.length > 0 && (
        <div className="field">
          <label>Max Agent Steps</label>
          <input type="number" min={1} max={20} value={num('max_iterations', 6)} onChange={(e) => set('max_iterations', Number(e.target.value))} />
        </div>
      )}
      <div style={{ display: 'flex', gap: 8 }}>
        <div className="field" style={{ flex: 1 }}>
          <label>Retries</label>
          <input
            type="number" min={0} max={5}
            value={num('max_retries', 0)}
            onChange={(e) => set('max_retries', Number(e.target.value))}
          />
        </div>
        <div className="field" style={{ flex: 1 }}>
          <label>Timeout (s)</label>
          <input
            type="number" min={0} max={300} placeholder="none"
            value={num('timeout_secs', 0) || ''}
            onChange={(e) => set('timeout_secs', Number(e.target.value) || undefined as unknown as number)}
          />
        </div>
      </div>
    </>
  )
}

export function ApprovalConfig() {
  return (
    <div style={{ color: 'var(--muted)', fontSize: 13 }}>
      <p>This node pauses execution until a human approves or rejects it.</p>
      <p style={{ marginTop: 8 }}>
        While waiting, the execution status changes to{' '}
        <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>
          waiting_approval
        </code>
        .
      </p>
      <p style={{ marginTop: 8 }}>
        Use the <strong>Approve</strong> or <strong>Reject</strong> buttons in the Execution panel
        to resume.
      </p>
    </div>
  )
}

export function CodeConfig({ set, str }: ConfigProps) {
  const example = `// Available: input, nodes["node_id"]
let count = input["count"];
#{ doubled: count * 2, ok: true }`
  return (
    <>
      <div className="field">
        <label>Script * <span style={{ color: 'var(--muted)' }}>(Rhai — JavaScript-like)</span></label>
        <textarea
          rows={10}
          placeholder={example}
          value={str('script')}
          onChange={(e) => set('script', e.target.value)}
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Variables: <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>input</code> (workflow input map),{' '}
        <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>nodes["id"]</code> (prior node output).
        Return a map <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>#{'{ key: value }'}</code> or any value.
        <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{{...}}'}</code> expressions are resolved before execution.
      </p>
    </>
  )
}

export function SubWorkflowConfig({ config, set, str }: ConfigProps) {
  const raw = config.input_template
  const display = raw !== undefined && raw !== null
    ? (typeof raw === 'string' ? raw : JSON.stringify(raw, null, 2))
    : ''
  return (
    <>
      <div className="field">
        <label>Workflow ID *</label>
        <input
          placeholder="wf-abc123"
          value={str('workflow_id')}
          onChange={(e) => set('workflow_id', e.target.value)}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>
          ID of the workflow to call. Its published version will be resolved at execution start.
        </span>
      </div>
      <div className="field">
        <label>
          Input template{' '}
          <span style={{ color: 'var(--muted)' }}>(optional JSON with <code>{'{{...}}'}</code>)</span>
        </label>
        <textarea
          rows={4}
          placeholder={'{\n  "user": "{{input.user}}"\n}'}
          value={display}
          onChange={(e) => {
            const raw = e.target.value
            if (!raw.trim()) { set('input_template', undefined as unknown as string); return }
            try { set('input_template', JSON.parse(raw)) } catch { set('input_template', raw) }
          }}
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>
          If omitted, the current execution input is passed through.
        </span>
      </div>
      <TemplateHint />
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>
          {'{ "status": "succeeded", "output": {...} }'}
        </code>
      </p>
    </>
  )
}

// ── Shared helpers used by CorePanels ─────────────────────────────────────────

function TemplateHint() {
  return (
    <p style={{ fontSize: 11, color: 'var(--muted)', marginTop: -6, lineHeight: 1.6 }}>
      Templates:{' '}
      <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{{input.field}}'}</code>
      {' · '}
      <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{{node_id.field}}'}</code>
      {' · '}
      <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{{credential.name}}'}</code>
    </p>
  )
}

function TemplatePreview({ text }: { text: string }) {
  if (!text || !text.includes('{{')) return null
  const parts: React.ReactNode[] = []
  const re = /\{\{([^}]+)\}\}/g
  let last = 0, m: RegExpExecArray | null
  while ((m = re.exec(text)) !== null) {
    if (m.index > last) parts.push(<span key={last}>{text.slice(last, m.index)}</span>)
    parts.push(
      <code key={m.index} style={{ background: 'rgba(37,99,235,0.10)', color: 'var(--link)', padding: '0 3px', borderRadius: 3, fontSize: 10 }}>
        {'{{'}{m[1]}{'}}'}</code>
    )
    last = m.index + m[0].length
  }
  if (last < text.length) parts.push(<span key={last}>{text.slice(last)}</span>)
  return (
    <div style={{
      marginTop: 4, padding: '5px 8px', fontSize: 11, lineHeight: 1.6,
      background: 'var(--canvas-bg)', border: '1px solid var(--border)',
      borderRadius: 4, color: 'var(--muted)', wordBreak: 'break-word',
    }}>
      {parts}
    </div>
  )
}

export function CustomConfig({ config, set, str }: ConfigProps) {
  const [nodes, setNodes] = useState<api.CustomNodeDef[]>([])
  useEffect(() => { api.listCustomNodes().then(setNodes).catch(() => {}) }, [])
  const selected = nodes.find((n) => n.slug === config['custom_node'])
  const onPick = (slug: string) => {
    const n = nodes.find((x) => x.slug === slug)
    set('custom_node', slug || undefined)
    set('endpoint', n?.endpoint || undefined)
  }
  const props = (selected?.config_schema?.properties ?? {}) as Record<string, { type?: string; title?: string }>
  return (
    <>
      <div className="field">
        <label>Custom Node</label>
        <select value={str('custom_node')} onChange={(e) => onPick(e.target.value)}>
          <option value="">— select a registered node —</option>
          {nodes.map((n) => <option key={n.slug} value={n.slug}>{n.label}</option>)}
        </select>
        {nodes.length === 0 && (
          <span style={{ fontSize: 11, color: 'var(--muted)' }}>
            No custom nodes registered. Add one in the nav menu → Custom Nodes.
          </span>
        )}
      </div>
      {selected?.description && (
        <p style={{ fontSize: 12, color: 'var(--muted)', margin: '0 0 6px' }}>{selected.description}</p>
      )}
      {Object.entries(props).map(([key, p]) => (
        <div className="field" key={key}>
          <label>{p.title || key}</label>
          <input value={str(key)} onChange={(e) => set(key, e.target.value)} />
        </div>
      ))}
    </>
  )
}
