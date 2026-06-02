// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// Contact: managecode@gmail.com

import type { ReactNode } from 'react'
import type { ConfigProps } from './types'

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

/** Inline highlight of {{...}} tokens in a template string */
function TemplatePreview({ text }: { text: string }) {
  if (!text || !text.includes('{{')) return null
  const parts: ReactNode[] = []
  const re = /\{\{([^}]+)\}\}/g
  let last = 0, m: RegExpExecArray | null
  while ((m = re.exec(text)) !== null) {
    if (m.index > last) parts.push(<span key={last}>{text.slice(last, m.index)}</span>)
    parts.push(
      <code key={m.index} style={{ background: 'rgba(37,99,235,0.10)', color: 'var(--link)', padding: '0 3px', borderRadius: 3, fontSize: 10 }}>
        {'{{'}{m[1]}{'}}'}
      </code>
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

export function SlackConfig({ set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Webhook URL *</label>
        <input
          placeholder="https://hooks.slack.com/services/..."
          value={str('webhook_url')}
          onChange={(e) => set('webhook_url', e.target.value)}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>
          Use <code>{'{{credential.slack_webhook}}'}</code> to reference a stored credential.
        </span>
      </div>
      <div className="field">
        <label>Message *</label>
        <textarea
          rows={3}
          placeholder="Workflow {{input.name}} completed successfully."
          value={str('text')}
          onChange={(e) => set('text', e.target.value)}
        />
        <TemplatePreview text={str('text')} />
      </div>
      <div className="field">
        <label>Channel <span style={{ color: 'var(--muted)' }}>(optional, overrides webhook default)</span></label>
        <input
          placeholder="#alerts"
          value={str('channel')}
          onChange={(e) => set('channel', e.target.value)}
        />
      </div>
      <div className="field">
        <label>Bot name <span style={{ color: 'var(--muted)' }}>(optional)</span></label>
        <input
          placeholder="Velara"
          value={str('username')}
          onChange={(e) => set('username', e.target.value)}
        />
      </div>
      <TemplateHint />
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>
          {'{ "ok": true, "text": "..." }'}
        </code>
      </p>
    </>
  )
}

export function EmailConfig({ set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>To *</label>
        <input
          placeholder="user@example.com"
          value={str('to')}
          onChange={(e) => set('to', e.target.value)}
        />
      </div>
      <div className="field">
        <label>Subject *</label>
        <input
          placeholder="Workflow completed: {{input.name}}"
          value={str('subject')}
          onChange={(e) => set('subject', e.target.value)}
        />
        <TemplatePreview text={str('subject')} />
      </div>
      <div className="field">
        <label>Body *</label>
        <textarea
          rows={4}
          placeholder="Your workflow has completed successfully."
          value={str('body')}
          onChange={(e) => set('body', e.target.value)}
        />
        <TemplatePreview text={str('body')} />
      </div>
      <div className="field">
        <label>From <span style={{ color: 'var(--muted)' }}>(optional)</span></label>
        <input
          placeholder="noreply@velara.dev"
          value={str('from')}
          onChange={(e) => set('from', e.target.value)}
        />
      </div>
      <div className="field">
        <label>SendGrid API Key *</label>
        <input
          placeholder="{{credential.sendgrid_key}}"
          value={str('api_key')}
          onChange={(e) => set('api_key', e.target.value)}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>
          Use <code>{'{{credential.sendgrid_key}}'}</code> to reference a stored credential.
        </span>
      </div>
      <TemplateHint />
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Sends via SendGrid API. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>
          {'{ "ok": true, "to": "...", "subject": "..." }'}
        </code>
      </p>
    </>
  )
}

export function GithubConfig({ config, set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Token <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input
          value={str('token', '')}
          onChange={(e) => set('token', e.target.value)}
          placeholder="ghp_… or {{credential.github_token}}"
        />
      </div>
      <div className="field">
        <label>Endpoint <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input
          value={str('endpoint', '')}
          onChange={(e) => set('endpoint', e.target.value)}
          placeholder="/repos/owner/repo/issues"
        />
      </div>
      <div className="field-row">
        <div className="field" style={{ flex: 1 }}>
          <label>Method</label>
          <select value={str('method', 'GET')} onChange={(e) => set('method', e.target.value)}>
            <option value="GET">GET</option>
            <option value="POST">POST</option>
            <option value="PATCH">PATCH</option>
            <option value="PUT">PUT</option>
            <option value="DELETE">DELETE</option>
          </select>
        </div>
        <div className="field" style={{ flex: 2 }}>
          <label>Base URL</label>
          <input
            value={str('base_url', 'https://api.github.com')}
            onChange={(e) => set('base_url', e.target.value)}
            placeholder="https://api.github.com"
          />
        </div>
      </div>
      <div className="field">
        <label>Request Body (JSON template)</label>
        <textarea
          rows={4}
          value={str('body', '')}
          onChange={(e) => set('body', e.target.value)}
          placeholder={'{"title": "{{input.title}}", "body": "{{input.body}}"}'}
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function WebhookSendConfig({ config, set, str }: ConfigProps) {
  const headers: Record<string, string> = (config.headers as Record<string, string>) || {}

  const updateHeader = (key: string, value: string) => {
    set('headers', { ...headers, [key]: value })
  }
  const removeHeader = (key: string) => {
    const h = { ...headers }
    delete h[key]
    set('headers', h)
  }

  return (
    <>
      <div className="field">
        <label>URL <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input
          value={str('url', '')}
          onChange={(e) => set('url', e.target.value)}
          placeholder="https://hooks.example.com/webhook"
        />
      </div>
      <div className="field">
        <label>Body Template (JSON)</label>
        <textarea
          rows={4}
          value={str('body_template', '')}
          onChange={(e) => set('body_template', e.target.value)}
          placeholder={'{"event": "{{input.event}}", "data": "{{input.data}}"}'}
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>Leave blank to send current input as body</span>
      </div>
      <div className="field">
        <label>Headers</label>
        {Object.entries(headers).map(([k, v]) => (
          <div key={k} className="field-row" style={{ gap: 4, marginBottom: 4 }}>
            <input value={k} readOnly style={{ flex: 1, fontFamily: 'monospace', fontSize: 12 }} />
            <input
              value={v}
              onChange={(e) => updateHeader(k, e.target.value)}
              style={{ flex: 2, fontFamily: 'monospace', fontSize: 12 }}
            />
            <button className="btn btn-danger" style={{ padding: '2px 6px' }} onClick={() => removeHeader(k)}>✕</button>
          </div>
        ))}
        <button
          className="btn"
          style={{ marginTop: 4, fontSize: 12 }}
          onClick={() => updateHeader('X-Custom-Header', '')}
        >+ Add Header</button>
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, ok }'}</code>
      </p>
    </>
  )
}

export function JiraConfig({ config, set, str }: ConfigProps) {
  const METHOD_OPTIONS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE']
  return (
    <>
      <div className="field">
        <label>Base URL <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input
          value={str('base_url', '')}
          onChange={(e) => set('base_url', e.target.value)}
          placeholder="https://yourcompany.atlassian.net"
        />
      </div>
      <div className="field">
        <label>Email <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input
          value={str('email', '')}
          onChange={(e) => set('email', e.target.value)}
          placeholder="you@yourcompany.com or {{credential.jira_email}}"
        />
      </div>
      <div className="field">
        <label>API Token <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input
          value={str('token', '')}
          onChange={(e) => set('token', e.target.value)}
          placeholder="{{credential.jira_token}}"
        />
      </div>
      <div className="field">
        <label>Endpoint <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input
          value={str('endpoint', '')}
          onChange={(e) => set('endpoint', e.target.value)}
          placeholder="/rest/api/3/issue/PROJ-1"
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <div className="field">
        <label>Method</label>
        <select value={str('method', 'GET')} onChange={(e) => set('method', e.target.value)}>
          {METHOD_OPTIONS.map(m => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Body (JSON template)</label>
        <textarea
          rows={4}
          value={str('body', '')}
          onChange={(e) => set('body', e.target.value)}
          placeholder={'{"fields": {"summary": "{{input.title}}", "issuetype": {"name": "Task"}}}'}
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>Leave blank for GET requests</span>
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function NotionConfig({ config, set, str }: ConfigProps) {
  const METHOD_OPTIONS = ['GET', 'POST', 'PATCH', 'DELETE']
  return (
    <>
      <div className="field">
        <label>Integration Token <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input
          value={str('token', '')}
          onChange={(e) => set('token', e.target.value)}
          placeholder="{{credential.notion_token}}"
        />
      </div>
      <div className="field">
        <label>Endpoint <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input
          value={str('endpoint', '')}
          onChange={(e) => set('endpoint', e.target.value)}
          placeholder="/v1/pages or /v1/databases/:id/query"
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <div className="field">
        <label>Method</label>
        <select value={str('method', 'GET')} onChange={(e) => set('method', e.target.value)}>
          {METHOD_OPTIONS.map(m => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Body (JSON template)</label>
        <textarea
          rows={4}
          value={str('body', '')}
          onChange={(e) => set('body', e.target.value)}
          placeholder={'{"parent": {"database_id": "{{input.db_id}}"}, "properties": {"Name": {"title": [{"text": {"content": "{{input.title}"}}]}}}'}
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>Leave blank for GET requests. Uses Notion API 2022-06-28.</span>
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function LinearConfig({ config, set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>API Token <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input
          value={str('token', '')}
          onChange={(e) => set('token', e.target.value)}
          placeholder="{{credential.linear_token}}"
        />
      </div>
      <div className="field">
        <label>GraphQL Query <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea
          rows={6}
          value={str('query', '')}
          onChange={(e) => set('query', e.target.value)}
          placeholder={'query {\n  issues(filter: { assignee: { isMe: { eq: true } } }) {\n    nodes { id title state { name } }\n  }\n}'}
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <div className="field">
        <label>Variables (JSON template)</label>
        <textarea
          rows={3}
          value={str('variables', '')}
          onChange={(e) => set('variables', e.target.value)}
          placeholder={'{"teamId": "{{input.team_id}}"}'}
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, data }'}</code>
      </p>
    </>
  )
}

export function AirtableConfig({ config, set, str, num }: ConfigProps) {
  const METHOD_OPTIONS = ['GET', 'POST', 'PATCH', 'DELETE']
  return (
    <>
      <div className="field">
        <label>Personal Access Token <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input
          value={str('token', '')}
          onChange={(e) => set('token', e.target.value)}
          placeholder="{{credential.airtable_token}}"
        />
      </div>
      <div className="field">
        <label>Base ID <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input
          value={str('base_id', '')}
          onChange={(e) => set('base_id', e.target.value)}
          placeholder="appXXXXXXXXXXXXXX"
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <div className="field">
        <label>Table <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input
          value={str('table', '')}
          onChange={(e) => set('table', e.target.value)}
          placeholder="Tasks or tblXXXXXXXXXXXXXX"
        />
      </div>
      <div className="field">
        <label>Method</label>
        <select value={str('method', 'GET')} onChange={(e) => set('method', e.target.value)}>
          {METHOD_OPTIONS.map(m => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Record ID <span style={{ fontSize: 11, color: 'var(--muted)' }}>(optional)</span></label>
        <input
          value={str('record_id', '')}
          onChange={(e) => set('record_id', e.target.value)}
          placeholder="recXXXXXXXXXXXXXX or {{input.record_id}}"
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <div className="field">
        <label>Filter Formula <span style={{ fontSize: 11, color: 'var(--muted)' }}>(GET list only)</span></label>
        <input
          value={str('filter_formula', '')}
          onChange={(e) => set('filter_formula', e.target.value)}
          placeholder={"AND({Status}='Done', {Assignee}='Alice')"}
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <div className="field">
        <label>Max Records</label>
        <input
          type="number"
          min={1}
          max={100}
          value={num('max_records', 100)}
          onChange={(e) => set('max_records', Number(e.target.value))}
        />
      </div>
      <div className="field">
        <label>Body (JSON template, for POST/PATCH)</label>
        <textarea
          rows={4}
          value={str('body', '')}
          onChange={(e) => set('body', e.target.value)}
          placeholder={'{"fields": {"Name": "{{input.name}}", "Status": "In Progress"}}'}
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function DiscordConfig({ config, set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Webhook URL <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input
          value={str('webhook_url', '')}
          onChange={(e) => set('webhook_url', e.target.value)}
          placeholder="https://discord.com/api/webhooks/..."
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <div className="field">
        <label>Message Content <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea
          rows={3}
          value={str('content', '')}
          onChange={(e) => set('content', e.target.value)}
          placeholder="{{input.message}}"
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <div className="field">
        <label>Username (optional)</label>
        <input
          value={str('username', '')}
          onChange={(e) => set('username', e.target.value)}
          placeholder="MyBot"
        />
      </div>
      <div className="field">
        <label>Avatar URL (optional)</label>
        <input
          value={str('avatar_url', '')}
          onChange={(e) => set('avatar_url', e.target.value)}
          placeholder="https://..."
        />
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ ok, content }'}</code>
      </p>
    </>
  )
}

export function TeamsConfig({ config, set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Webhook URL <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input
          value={str('webhook_url', '')}
          onChange={(e) => set('webhook_url', e.target.value)}
          placeholder="https://outlook.office.com/webhook/..."
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <div className="field">
        <label>Message Text <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea
          rows={3}
          value={str('text', '')}
          onChange={(e) => set('text', e.target.value)}
          placeholder="{{input.message}}"
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <div className="field">
        <label>Title (optional)</label>
        <input
          value={str('title', '')}
          onChange={(e) => set('title', e.target.value)}
          placeholder="Notification"
        />
      </div>
      <div className="field">
        <label>Theme Color (hex, optional)</label>
        <input
          value={str('color', '')}
          onChange={(e) => set('color', e.target.value)}
          placeholder="0078D4"
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ ok, text }'}</code>
      </p>
    </>
  )
}

export function SheetsConfig({ config, set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Bearer Token <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input
          value={str('token', '')}
          onChange={(e) => set('token', e.target.value)}
          placeholder="{{credential.sheets_token}}"
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <div className="field">
        <label>Spreadsheet ID <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input
          value={str('spreadsheet_id', '')}
          onChange={(e) => set('spreadsheet_id', e.target.value)}
          placeholder="1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgVE2upms"
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <div className="field">
        <label>Range (A1 notation)</label>
        <input
          value={str('range', 'Sheet1!A1')}
          onChange={(e) => set('range', e.target.value)}
          placeholder="Sheet1!A1:Z1000"
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <div className="field">
        <label>Method</label>
        <select value={str('method', 'GET')} onChange={(e) => set('method', e.target.value)}>
          <option value="GET">GET — read values</option>
          <option value="APPEND">APPEND — append rows</option>
          <option value="UPDATE">UPDATE — write values (PUT)</option>
          <option value="CLEAR">CLEAR — clear range</option>
        </select>
      </div>
      <div className="field">
        <label>Values (for APPEND/UPDATE)</label>
        <textarea
          rows={3}
          value={str('values', '')}
          onChange={(e) => set('values', e.target.value)}
          placeholder='[["row1col1","row1col2"],["row2col1","row2col2"]]'
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>JSON 2D array of rows</span>
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body, values }'}</code>
      </p>
    </>
  )
}

export function HubspotConfig({ config, set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Private App Token <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input
          value={str('token', '')}
          onChange={(e) => set('token', e.target.value)}
          placeholder="{{credential.hubspot_token}}"
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <div className="field">
        <label>Endpoint <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input
          value={str('endpoint', '')}
          onChange={(e) => set('endpoint', e.target.value)}
          placeholder="/crm/v3/objects/contacts"
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>Relative to api.hubapi.com</span>
      </div>
      <div className="field">
        <label>Method</label>
        <select value={str('method', 'GET')} onChange={(e) => set('method', e.target.value)}>
          <option value="GET">GET — read/search</option>
          <option value="POST">POST — create</option>
          <option value="PATCH">PATCH — update</option>
          <option value="DELETE">DELETE — delete</option>
        </select>
      </div>
      <div className="field">
        <label>Body (JSON)</label>
        <textarea
          rows={3}
          value={str('body', '')}
          onChange={(e) => set('body', e.target.value)}
          placeholder='{"properties": {"email": "{{input.email}}"}}'
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function ZendeskConfig({ config, set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Subdomain <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input
          value={str('subdomain', '')}
          onChange={(e) => set('subdomain', e.target.value)}
          placeholder="mycompany"
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>Your Zendesk subdomain (before .zendesk.com)</span>
      </div>
      <div className="field">
        <label>API Token <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input
          value={str('token', '')}
          onChange={(e) => set('token', e.target.value)}
          placeholder="{{credential.zendesk_token}}"
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <div className="field">
        <label>Endpoint <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input
          value={str('endpoint', '')}
          onChange={(e) => set('endpoint', e.target.value)}
          placeholder="/tickets.json"
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>Relative to /api/v2 — e.g. /tickets.json, /users/123.json</span>
      </div>
      <div className="field">
        <label>Method</label>
        <select value={str('method', 'GET')} onChange={(e) => set('method', e.target.value)}>
          <option value="GET">GET — read/list</option>
          <option value="POST">POST — create</option>
          <option value="PUT">PUT — update</option>
          <option value="DELETE">DELETE — delete</option>
        </select>
      </div>
      <div className="field">
        <label>Body (JSON)</label>
        <textarea
          rows={3}
          value={str('body', '')}
          onChange={(e) => set('body', e.target.value)}
          placeholder='{"ticket": {"subject": "{{input.subject}}", "comment": {"body": "{{input.body}}"}}}'
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function TwilioConfig({ config, set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Account SID <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input
          value={str('account_sid', '')}
          onChange={(e) => set('account_sid', e.target.value)}
          placeholder="{{credential.twilio_sid}}"
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <div className="field">
        <label>Auth Token <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input
          value={str('auth_token', '')}
          onChange={(e) => set('auth_token', e.target.value)}
          placeholder="{{credential.twilio_token}}"
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <div className="field">
        <label>To (E.164) <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input
          value={str('to', '')}
          onChange={(e) => set('to', e.target.value)}
          placeholder="+15551234567"
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <div className="field">
        <label>From (E.164) <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input
          value={str('from', '')}
          onChange={(e) => set('from', e.target.value)}
          placeholder="+15557654321"
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <div className="field">
        <label>Message Body <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea
          rows={3}
          value={str('body', '')}
          onChange={(e) => set('body', e.target.value)}
          placeholder="{{input.message}}"
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ sid, status, to, from }'}</code>
      </p>
    </>
  )
}

export function StripeConfig({ config, set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>API Key <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input
          value={str('api_key', '')}
          onChange={(e) => set('api_key', e.target.value)}
          placeholder="{{credential.stripe_key}}"
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>sk_live_… or sk_test_…</span>
      </div>
      <div className="field">
        <label>Endpoint <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input
          value={str('endpoint', '')}
          onChange={(e) => set('endpoint', e.target.value)}
          placeholder="/customers"
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>e.g. /customers, /charges/ch_xxx, /payment_intents</span>
      </div>
      <div className="field">
        <label>Method</label>
        <select value={str('method', 'GET')} onChange={(e) => set('method', e.target.value)}>
          <option value="GET">GET — retrieve / list</option>
          <option value="POST">POST — create (form-encoded)</option>
          <option value="PATCH">PATCH — update (form-encoded)</option>
          <option value="DELETE">DELETE — delete</option>
        </select>
      </div>
      <div className="field">
        <label>Body (flat object for POST/PATCH)</label>
        <textarea
          rows={3}
          value={str('body', '')}
          onChange={(e) => set('body', e.target.value)}
          placeholder='{"email": "{{input.email}}", "name": "{{input.name}}"}'
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>For GET: used as query params. For POST/PATCH: form-encoded.</span>
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, id, object, body }'}</code>
      </p>
    </>
  )
}

export function ShopifyConfig({ config, set, str }: ConfigProps) {
  const method = str('method', 'GET')
  const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE']
  return (
    <>
      <div className="field">
        <label>Shop (subdomain)</label>
        <input placeholder="my-store" value={str('shop', '')} onChange={(e) => set('shop', e.target.value)} />
      </div>
      <div className="field">
        <label>Access Token</label>
        <input type="password" placeholder="shpat_…" value={str('token', '')} onChange={(e) => set('token', e.target.value)} />
      </div>
      <div className="field">
        <label>API Version</label>
        <input placeholder="2024-01" value={str('api_version', '2024-01')} onChange={(e) => set('api_version', e.target.value)} />
      </div>
      <div className="field">
        <label>Method</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Endpoint</label>
        <input placeholder="/products.json" value={str('endpoint', '/products.json')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT', 'PATCH'].includes(method) && (
        <div className="field">
          <label>Body (JSON)</label>
          <textarea
            rows={4}
            placeholder='{"product": {"title": "My Product"}}'
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function DatadogConfig({ config, set, str }: ConfigProps) {
  const method = str('method', 'GET')
  const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE']
  return (
    <>
      <div className="field">
        <label>API Key</label>
        <input type="password" placeholder="DD-API-KEY" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <div className="field">
        <label>Application Key (optional)</label>
        <input type="password" placeholder="DD-APPLICATION-KEY" value={str('app_key', '')} onChange={(e) => set('app_key', e.target.value)} />
      </div>
      <div className="field">
        <label>Site</label>
        <input placeholder="datadoghq.com" value={str('site', 'datadoghq.com')} onChange={(e) => set('site', e.target.value)} />
      </div>
      <div className="field">
        <label>Method</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Endpoint</label>
        <input placeholder="/api/v1/validate" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT', 'PATCH'].includes(method) && (
        <div className="field">
          <label>Body (JSON)</label>
          <textarea
            rows={4}
            placeholder='{"series": [{"metric": "my.metric", "points": [[1609459200, 1.5]]}]}'
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function SalesforceConfig({ config, set, str }: ConfigProps) {
  const method = str('method', 'GET')
  const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE']
  return (
    <>
      <div className="field">
        <label>Access Token (OAuth)</label>
        <input type="password" placeholder="00D…" value={str('token', '')} onChange={(e) => set('token', e.target.value)} />
      </div>
      <div className="field">
        <label>Instance URL</label>
        <input placeholder="https://myorg.salesforce.com" value={str('instance_url', '')} onChange={(e) => set('instance_url', e.target.value)} />
      </div>
      <div className="field">
        <label>Method</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Endpoint</label>
        <input placeholder="/services/data/v59.0/sobjects/Account" value={str('endpoint', '/services/data/v59.0/sobjects')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT', 'PATCH'].includes(method) && (
        <div className="field">
          <label>Body (JSON)</label>
          <textarea rows={4}
            placeholder='{"Name": "Acme Corp"}'
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function FreshdeskConfig({ config, set, str }: ConfigProps) {
  const method = str('method', 'GET')
  const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE']
  return (
    <>
      <div className="field">
        <label>API Key</label>
        <input type="password" placeholder="Freshdesk API key" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <div className="field">
        <label>Domain</label>
        <input placeholder="yourcompany.freshdesk.com" value={str('domain', '')} onChange={(e) => set('domain', e.target.value)} />
      </div>
      <div className="field">
        <label>Method</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Endpoint</label>
        <input placeholder="/api/v2/tickets" value={str('endpoint', '/api/v2/tickets')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT', 'PATCH'].includes(method) && (
        <div className="field">
          <label>Body (JSON)</label>
          <textarea rows={4}
            placeholder='{"subject": "Help needed", "email": "user@example.com", "priority": 1, "status": 2}'
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function MailgunConfig({ config, set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>API Key</label>
        <input type="password" placeholder="key-…" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <div className="field">
        <label>Sending Domain</label>
        <input placeholder="mg.yourdomain.com" value={str('domain', '')} onChange={(e) => set('domain', e.target.value)} />
      </div>
      <div className="field">
        <label>From</label>
        <input placeholder="sender@mg.yourdomain.com" value={str('from', '')} onChange={(e) => set('from', e.target.value)} />
      </div>
      <div className="field">
        <label>To</label>
        <input placeholder="recipient@example.com" value={str('to', '')} onChange={(e) => set('to', e.target.value)} />
      </div>
      <div className="field">
        <label>Subject</label>
        <input placeholder="Hello from Mailgun" value={str('subject', '')} onChange={(e) => set('subject', e.target.value)} />
      </div>
      <div className="field">
        <label>HTML Body</label>
        <textarea rows={3} placeholder="<h1>Hello!</h1>" value={str('html', '')} onChange={(e) => set('html', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Text Body (fallback)</label>
        <textarea rows={2} placeholder="Plain text version" value={str('text', '')} onChange={(e) => set('text', e.target.value)} />
      </div>
      <div className="field">
        <label>Region</label>
        <select value={str('region', 'us')} onChange={(e) => set('region', e.target.value)}>
          <option value="us">US (api.mailgun.net)</option>
          <option value="eu">EU (api.eu.mailgun.net)</option>
        </select>
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function AsanaConfig({ config, set, str }: ConfigProps) {
  const method = str('method', 'GET')
  const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE']
  return (
    <>
      <div className="field">
        <label>Personal Access Token</label>
        <input type="password" placeholder="1/…" value={str('token', '')} onChange={(e) => set('token', e.target.value)} />
      </div>
      <div className="field">
        <label>Method</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Endpoint</label>
        <input placeholder="/tasks" value={str('endpoint', '/tasks')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT', 'PATCH'].includes(method) && (
        <div className="field">
          <label>Body (JSON)</label>
          <textarea rows={4}
            placeholder='{"data": {"name": "My task", "projects": ["<project_gid>"]}}'
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function ServiceNowConfig({ config, set, str }: ConfigProps) {
  const method = str('method', 'GET')
  const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE']
  return (
    <>
      <div className="field">
        <label>Instance</label>
        <input placeholder="myco.service-now.com" value={str('instance', '')} onChange={(e) => set('instance', e.target.value)} />
      </div>
      <div className="field">
        <label>Username</label>
        <input placeholder="admin" value={str('username', '')} onChange={(e) => set('username', e.target.value)} />
      </div>
      <div className="field">
        <label>Password</label>
        <input type="password" placeholder="••••••••" value={str('password', '')} onChange={(e) => set('password', e.target.value)} />
      </div>
      <div className="field">
        <label>Method</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Endpoint</label>
        <input placeholder="/api/now/table/incident" value={str('endpoint', '/api/now/table/incident')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT', 'PATCH'].includes(method) && (
        <div className="field">
          <label>Body (JSON)</label>
          <textarea rows={4}
            placeholder='{"short_description": "Issue", "urgency": "2", "impact": "2"}'
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function ConfluenceConfig({ config, set, str }: ConfigProps) {
  const method = str('method', 'GET')
  const METHODS = ['GET', 'POST', 'PUT', 'DELETE']
  const authMode = str('auth_mode', 'token')
  return (
    <>
      <div className="field">
        <label>Base URL</label>
        <input placeholder="https://myco.atlassian.net/wiki" value={str('base_url', '')} onChange={(e) => set('base_url', e.target.value)} />
      </div>
      <div className="field">
        <label>Auth Mode</label>
        <select value={authMode} onChange={(e) => set('auth_mode', e.target.value)}>
          <option value="token">Bearer Token</option>
          <option value="basic">Basic (Email + API Token)</option>
        </select>
      </div>
      {authMode === 'token' ? (
        <div className="field">
          <label>Bearer Token</label>
          <input type="password" placeholder="eyJ…" value={str('token', '')} onChange={(e) => set('token', e.target.value)} />
        </div>
      ) : (
        <>
          <div className="field">
            <label>Email</label>
            <input placeholder="user@example.com" value={str('email', '')} onChange={(e) => set('email', e.target.value)} />
          </div>
          <div className="field">
            <label>API Token</label>
            <input type="password" placeholder="Atlassian API token" value={str('api_token', '')} onChange={(e) => set('api_token', e.target.value)} />
          </div>
        </>
      )}
      <div className="field">
        <label>Method</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Endpoint</label>
        <input placeholder="/rest/api/content" value={str('endpoint', '/rest/api/content')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT'].includes(method) && (
        <div className="field">
          <label>Body (JSON)</label>
          <textarea rows={5}
            placeholder='{"type": "page", "title": "New page", "space": {"key": "SPACE"}, "body": {"storage": {"value": "<p>Hello</p>", "representation": "storage"}}}'
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function BitbucketConfig({ config, set, str }: ConfigProps) {
  const method = str('method', 'GET')
  const METHODS = ['GET', 'POST', 'PUT', 'DELETE']
  return (
    <>
      <div className="field">
        <label>Username</label>
        <input placeholder="bitbucket_username" value={str('username', '')} onChange={(e) => set('username', e.target.value)} />
      </div>
      <div className="field">
        <label>App Password</label>
        <input type="password" placeholder="Bitbucket app password" value={str('app_password', '')} onChange={(e) => set('app_password', e.target.value)} />
      </div>
      <div className="field">
        <label>Method</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Endpoint</label>
        <input placeholder="/repositories/workspace/repo-slug" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT'].includes(method) && (
        <div className="field">
          <label>Body (JSON)</label>
          <textarea rows={4}
            placeholder='{"title": "My PR", "source": {"branch": {"name": "feature"}}, "destination": {"branch": {"name": "main"}}}'
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function AzureDevOpsConfig({ config, set, str }: ConfigProps) {
  const method = str('method', 'GET')
  const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE']
  return (
    <>
      <div className="field">
        <label>Personal Access Token</label>
        <input type="password" placeholder="Azure DevOps PAT" value={str('pat', '')} onChange={(e) => set('pat', e.target.value)} />
      </div>
      <div className="field">
        <label>Organization</label>
        <input placeholder="myorg" value={str('organization', '')} onChange={(e) => set('organization', e.target.value)} />
      </div>
      <div className="field">
        <label>Project (optional)</label>
        <input placeholder="MyProject" value={str('project', '')} onChange={(e) => set('project', e.target.value)} />
      </div>
      <div className="field">
        <label>Method</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Endpoint</label>
        <input placeholder="/build/builds" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      <div className="field">
        <label>API Version</label>
        <input placeholder="7.1" value={str('api_version', '7.1')} onChange={(e) => set('api_version', e.target.value)} />
      </div>
      {['POST', 'PUT', 'PATCH'].includes(method) && (
        <div className="field">
          <label>Body (JSON)</label>
          <textarea rows={4}
            placeholder='{"definition": {"id": 1}, "sourceBranch": "refs/heads/main"}'
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        URL: <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>dev.azure.com/{'{org}/{project}/_apis{endpoint}'}</code>
      </p>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function RedisConfig({ config, set, str, num }: ConfigProps) {
  const op = str('operation', 'get')
  const needsValue = ['set', 'lpush', 'rpush', 'hset'].includes(op)
  const needsField = ['hget', 'hset', 'hdel'].includes(op)
  const needsTtl = ['set', 'expire'].includes(op)
  const needsAmount = op === 'incrby'
  return (
    <>
      <div className="field">
        <label>Redis URL <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input
          value={str('url', '')}
          onChange={(e) => set('url', e.target.value)}
          placeholder="{{credential.redis_url}} or redis://localhost:6379/0"
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={op} onChange={(e) => set('operation', e.target.value)}>
          <optgroup label="String">
            <option value="get">GET — read value</option>
            <option value="set">SET — write value</option>
            <option value="del">DEL — delete key</option>
            <option value="exists">EXISTS — check existence</option>
            <option value="incr">INCR — increment</option>
            <option value="decr">DECR — decrement</option>
            <option value="incrby">INCRBY — increment by amount</option>
          </optgroup>
          <optgroup label="Expiry">
            <option value="expire">EXPIRE — set TTL</option>
            <option value="ttl">TTL — get remaining TTL</option>
          </optgroup>
          <optgroup label="Hash">
            <option value="hget">HGET — read hash field</option>
            <option value="hset">HSET — write hash field</option>
            <option value="hdel">HDEL — delete hash field</option>
            <option value="hgetall">HGETALL — read all hash fields</option>
          </optgroup>
          <optgroup label="List">
            <option value="lpush">LPUSH — prepend to list</option>
            <option value="lpop">LPOP — pop from head</option>
            <option value="rpush">RPUSH — append to list</option>
            <option value="rpop">RPOP — pop from tail</option>
            <option value="llen">LLEN — list length</option>
          </optgroup>
          <optgroup label="Other">
            <option value="keys">KEYS — list matching keys</option>
            <option value="ping">PING — health check</option>
          </optgroup>
        </select>
      </div>
      <div className="field">
        <label>Key{op !== 'ping' && <span style={{ color: 'var(--danger)' }}> *</span>}</label>
        <input
          value={str('key', '')}
          onChange={(e) => set('key', e.target.value)}
          placeholder={op === 'keys' ? 'prefix:*' : 'cache:user:{{input.user_id}}'}
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      {needsField && (
        <div className="field">
          <label>Hash Field <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input
            value={str('field', '')}
            onChange={(e) => set('field', e.target.value)}
            placeholder="field_name"
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      {needsValue && (
        <div className="field">
          <label>Value</label>
          <input
            value={str('value', '')}
            onChange={(e) => set('value', e.target.value)}
            placeholder="{{input.data}}"
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      {needsAmount && (
        <div className="field">
          <label>Amount</label>
          <input type="number" value={num('amount', 1)} onChange={(e) => set('amount', Number(e.target.value))} />
        </div>
      )}
      {needsTtl && (
        <div className="field">
          <label>TTL (seconds, 0 = no expiry)</label>
          <input type="number" min={0} value={num('ttl_secs', 0)} onChange={(e) => set('ttl_secs', Number(e.target.value))} />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ value, operation, key }'}</code>
      </p>
    </>
  )
}

export function ElasticsearchConfig({ config, set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Cluster URL <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input
          value={str('url', '')}
          onChange={(e) => set('url', e.target.value)}
          placeholder="https://my-cluster.elastic.co or http://localhost:9200"
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <div className="field">
        <label>Endpoint</label>
        <input
          value={str('endpoint', '/_search')}
          onChange={(e) => set('endpoint', e.target.value)}
          placeholder="/my-index/_search"
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <div className="field">
        <label>Method</label>
        <select value={str('method', 'GET')} onChange={(e) => set('method', e.target.value)}>
          <option value="GET">GET</option>
          <option value="POST">POST — search / create</option>
          <option value="PUT">PUT — index / update</option>
          <option value="DELETE">DELETE</option>
        </select>
      </div>
      <div className="field">
        <label>API Key (optional)</label>
        <input
          value={str('api_key', '')}
          onChange={(e) => set('api_key', e.target.value)}
          placeholder="{{credential.es_api_key}}"
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <div className="field">
        <label>Body (JSON)</label>
        <textarea
          rows={4}
          value={str('body', '')}
          onChange={(e) => set('body', e.target.value)}
          placeholder={'{"query": {"match": {"title": "{{input.search}}"}}}'  }
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body, took, hits_total }'}</code>
      </p>
    </>
  )
}

export function PagerdutyConfig({ config, set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Routing Key (Integration Key) <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input
          value={str('routing_key', '')}
          onChange={(e) => set('routing_key', e.target.value)}
          placeholder="{{credential.pagerduty_key}}"
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <div className="field">
        <label>Summary <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input
          value={str('summary', '')}
          onChange={(e) => set('summary', e.target.value)}
          placeholder="{{input.error_message}}"
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <div className="field">
        <label>Event Action</label>
        <select value={str('event_action', 'trigger')} onChange={(e) => set('event_action', e.target.value)}>
          <option value="trigger">trigger — create/update incident</option>
          <option value="acknowledge">acknowledge — ack incident</option>
          <option value="resolve">resolve — close incident</option>
        </select>
      </div>
      <div className="field">
        <label>Severity</label>
        <select value={str('severity', 'error')} onChange={(e) => set('severity', e.target.value)}>
          <option value="critical">critical</option>
          <option value="error">error</option>
          <option value="warning">warning</option>
          <option value="info">info</option>
        </select>
      </div>
      <div className="field">
        <label>Source</label>
        <input
          value={str('source', 'velara')}
          onChange={(e) => set('source', e.target.value)}
          placeholder="velara"
        />
      </div>
      <div className="field">
        <label>Dedup Key (optional)</label>
        <input
          value={str('dedup_key', '')}
          onChange={(e) => set('dedup_key', e.target.value)}
          placeholder="{{input.incident_id}}"
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>Same key = update existing incident; omit = always create new</span>
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, message, dedup_key }'}</code>
      </p>
    </>
  )
}
