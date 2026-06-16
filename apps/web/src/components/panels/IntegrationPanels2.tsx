// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import type { ConfigProps } from './types'

export function TwitchConfig({ config, set, str }: ConfigProps) {
  const method = str('method', 'GET')
  const METHODS = ['GET', 'POST', 'PUT', 'DELETE']
  return (
    <>
      <div className="field">
        <label>Client ID</label>
        <input placeholder="Twitch Client ID" value={str('client_id', '')} onChange={(e) => set('client_id', e.target.value)} />
      </div>
      <div className="field">
        <label>Access Token (OAuth)</label>
        <input type="password" placeholder="OAuth access token" value={str('access_token', '')} onChange={(e) => set('access_token', e.target.value)} />
      </div>
      <div className="field">
        <label>Method</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Endpoint</label>
        <input placeholder="/helix/streams" value={str('endpoint', '/helix/streams')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT'].includes(method) && (
        <div className="field">
          <label>Body (JSON)</label>
          <textarea rows={4}
            placeholder='{}'
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

// ── Slice 275: Figma ──────────────────────────────────────────────────────────

export function FigmaConfig({ set, str }: ConfigProps) {
  const method = str('method', 'GET')
  const METHODS = ['GET', 'POST', 'PUT', 'DELETE']
  return (
    <>
      <div className="field">
        <label>Personal Access Token</label>
        <input type="password" placeholder="figd_…" value={str('token', '')} onChange={(e) => set('token', e.target.value)} />
      </div>
      <div className="field">
        <label>Method</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Endpoint</label>
        <input placeholder="/v1/files/FILE_KEY" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Common endpoints: <code style={{ background: 'var(--panel)', padding: '1px 3px', borderRadius: 3 }}>/v1/files/KEY</code>, <code style={{ background: 'var(--panel)', padding: '1px 3px', borderRadius: 3 }}>/v1/teams/TEAM_ID/projects</code>
      </p>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

// ── Slice 276: Dropbox ────────────────────────────────────────────────────────

export function DropboxConfig({ set, str }: ConfigProps) {
  const op = str('operation', 'list_folder')
  const OPS = ['list_folder', 'get_metadata', 'delete', 'create_folder', 'search']
  const needsPath = ['list_folder', 'get_metadata', 'delete', 'create_folder'].includes(op)
  const needsQuery = op === 'search'
  return (
    <>
      <div className="field">
        <label>Access Token (OAuth2)</label>
        <input type="password" placeholder="sl.…" value={str('token', '')} onChange={(e) => set('token', e.target.value)} />
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={op} onChange={(e) => set('operation', e.target.value)}>
          {OPS.map((o) => <option key={o} value={o}>{o}</option>)}
        </select>
      </div>
      {needsPath && (
        <div className="field">
          <label>Path (empty string = root for list_folder)</label>
          <input placeholder="/Documents/report.pdf" value={str('path', '')} onChange={(e) => set('path', e.target.value)} />
        </div>
      )}
      {needsQuery && (
        <div className="field">
          <label>Search Query</label>
          <input placeholder="quarterly report" value={str('query', '')} onChange={(e) => set('query', e.target.value)} />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body, operation }'}</code>
      </p>
    </>
  )
}

// ── Slice 277: Cloudflare ─────────────────────────────────────────────────────

export function CloudflareConfig({ config, set, str }: ConfigProps) {
  const method = str('method', 'GET')
  const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE']
  return (
    <>
      <div className="field">
        <label>API Token</label>
        <input type="password" placeholder="Cloudflare API token" value={str('api_token', '')} onChange={(e) => set('api_token', e.target.value)} />
      </div>
      <div className="field">
        <label>Method</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Endpoint</label>
        <input placeholder="/zones/ZONE_ID/dns_records" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT', 'PATCH'].includes(method) && (
        <div className="field">
          <label>Body (JSON)</label>
          <textarea rows={4}
            placeholder='{"type": "A", "name": "api.example.com", "content": "1.2.3.4", "ttl": 300}'
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body, success }'}</code>
      </p>
    </>
  )
}

// ── Slice 278: Box ─────────────────────────────────────────────────────────────

export function BoxConfig({ config, set, str }: ConfigProps) {
  const method = str('method', 'GET')
  const METHODS = ['GET', 'POST', 'PUT', 'DELETE', 'OPTIONS']
  return (
    <>
      <div className="field">
        <label>Access Token</label>
        <input type="password" placeholder="Box access token" value={str('token', '')} onChange={(e) => set('token', e.target.value)} />
      </div>
      <div className="field">
        <label>Method</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Endpoint</label>
        <input placeholder="/2.0/files/FILE_ID" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT'].includes(method) && (
        <div className="field">
          <label>Body (JSON)</label>
          <textarea rows={4}
            placeholder='{"name": "example.txt"}'
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

// ── Slice 279: Okta ────────────────────────────────────────────────────────────

export function OktaConfig({ config, set, str }: ConfigProps) {
  const method = str('method', 'GET')
  const METHODS = ['GET', 'POST', 'PUT', 'DELETE']
  const tokenType = str('token_type', 'SSWS')
  return (
    <>
      <div className="field">
        <label>Okta Domain</label>
        <input placeholder="https://your-org.okta.com" value={str('domain', '')} onChange={(e) => set('domain', e.target.value)} />
      </div>
      <div className="field">
        <label>Token Type</label>
        <select value={tokenType} onChange={(e) => set('token_type', e.target.value)}>
          <option value="SSWS">SSWS (API Token)</option>
          <option value="BEARER">Bearer (OAuth)</option>
        </select>
      </div>
      <div className="field">
        <label>Token</label>
        <input type="password" placeholder="Okta API token or OAuth token" value={str('token', '')} onChange={(e) => set('token', e.target.value)} />
      </div>
      <div className="field">
        <label>Method</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Endpoint</label>
        <input placeholder="/api/v1/users" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT'].includes(method) && (
        <div className="field">
          <label>Body (JSON)</label>
          <textarea rows={4}
            placeholder='{"profile": {"firstName": "Jane", "email": "jane@example.com"}}'
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

// ── Slice 280: Zoom ────────────────────────────────────────────────────────────

export function ZoomConfig({ config, set, str }: ConfigProps) {
  const method = str('method', 'GET')
  const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE']
  return (
    <>
      <div className="field">
        <label>Access Token</label>
        <input type="password" placeholder="Zoom OAuth access token" value={str('token', '')} onChange={(e) => set('token', e.target.value)} />
      </div>
      <div className="field">
        <label>Method</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Endpoint</label>
        <input placeholder="/v2/users/me/meetings" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT', 'PATCH'].includes(method) && (
        <div className="field">
          <label>Body (JSON)</label>
          <textarea rows={4}
            placeholder='{"topic": "My Meeting", "type": 2, "duration": 60}'
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

// ── Slice 281: Spotify ─────────────────────────────────────────────────────────

export function SpotifyConfig({ config, set, str }: ConfigProps) {
  const method = str('method', 'GET')
  const METHODS = ['GET', 'POST', 'PUT', 'DELETE']
  return (
    <>
      <div className="field">
        <label>Access Token</label>
        <input type="password" placeholder="Spotify OAuth access token" value={str('token', '')} onChange={(e) => set('token', e.target.value)} />
      </div>
      <div className="field">
        <label>Method</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Endpoint</label>
        <input placeholder="/v1/me/player/currently-playing" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT'].includes(method) && (
        <div className="field">
          <label>Body (JSON)</label>
          <textarea rows={4}
            placeholder='{"uris": ["spotify:track:4iV5W9uYEdYUVa79Axb7Rh"]}'
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>. 204 No Content returns null body.
      </p>
    </>
  )
}

// ── Slice 282: Typeform ────────────────────────────────────────────────────────

export function TypeformConfig({ config, set, str }: ConfigProps) {
  const method = str('method', 'GET')
  const METHODS = ['GET', 'POST', 'PUT', 'DELETE', 'PATCH']
  return (
    <>
      <div className="field">
        <label>Personal Token</label>
        <input type="password" placeholder="Typeform personal token" value={str('token', '')} onChange={(e) => set('token', e.target.value)} />
      </div>
      <div className="field">
        <label>Method</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Endpoint</label>
        <input placeholder="/forms/FORM_ID/responses" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT', 'PATCH'].includes(method) && (
        <div className="field">
          <label>Body (JSON)</label>
          <textarea rows={4}
            placeholder='{"page_size": 25}'
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

// ── Slice 283: Webflow ─────────────────────────────────────────────────────────

export function WebflowConfig({ config, set, str }: ConfigProps) {
  const method = str('method', 'GET')
  const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE']
  return (
    <>
      <div className="field">
        <label>API Token</label>
        <input type="password" placeholder="Webflow API token or OAuth token" value={str('token', '')} onChange={(e) => set('token', e.target.value)} />
      </div>
      <div className="field">
        <label>Method</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Endpoint</label>
        <input placeholder="/sites" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT', 'PATCH'].includes(method) && (
        <div className="field">
          <label>Body (JSON)</label>
          <textarea rows={4}
            placeholder='{"fieldData": {"name": "My Page", "slug": "my-page"}}'
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

// ── Slice 284: Intercom ────────────────────────────────────────────────────────

export function IntercomConfig({ config, set, str }: ConfigProps) {
  const method = str('method', 'GET')
  const METHODS = ['GET', 'POST', 'PUT', 'DELETE']
  return (
    <>
      <div className="field">
        <label>Access Token</label>
        <input type="password" placeholder="Intercom access token" value={str('token', '')} onChange={(e) => set('token', e.target.value)} />
      </div>
      <div className="field">
        <label>Method</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Endpoint</label>
        <input placeholder="/contacts" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT'].includes(method) && (
        <div className="field">
          <label>Body (JSON)</label>
          <textarea rows={4}
            placeholder='{"email": "user@example.com", "name": "Jane Doe"}'
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

// ── Slice 285: Pipedrive ───────────────────────────────────────────────────────

export function PipedriveConfig({ config, set, str }: ConfigProps) {
  const method = str('method', 'GET')
  const METHODS = ['GET', 'POST', 'PUT', 'DELETE']
  return (
    <>
      <div className="field">
        <label>API Token</label>
        <input type="password" placeholder="Pipedrive API token" value={str('api_token', '')} onChange={(e) => set('api_token', e.target.value)} />
      </div>
      <div className="field">
        <label>Method</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Endpoint</label>
        <input placeholder="/deals" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT'].includes(method) && (
        <div className="field">
          <label>Body (JSON)</label>
          <textarea rows={4}
            placeholder='{"title": "New Deal", "value": 5000, "currency": "USD"}'
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Token is appended as <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>?api_token=…</code> query param.
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

// ── Slice 286: Trello ──────────────────────────────────────────────────────────

export function TrelloConfig({ config, set, str }: ConfigProps) {
  const method = str('method', 'GET')
  const METHODS = ['GET', 'POST', 'PUT', 'DELETE']
  return (
    <>
      <div className="field">
        <label>API Key</label>
        <input placeholder="Trello API key" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <div className="field">
        <label>Token</label>
        <input type="password" placeholder="Trello token" value={str('token', '')} onChange={(e) => set('token', e.target.value)} />
      </div>
      <div className="field">
        <label>Method</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Endpoint</label>
        <input placeholder="/boards/BOARD_ID" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT'].includes(method) && (
        <div className="field">
          <label>Body (JSON)</label>
          <textarea rows={4}
            placeholder='{"name": "New Card", "idList": "LIST_ID"}'
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Auth via <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>?key=…&token=…</code> query params.
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

// ── Slice 287: Monday ──────────────────────────────────────────────────────────

export function MondayConfig({ config, set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>API Token</label>
        <input type="password" placeholder="Monday.com API token" value={str('token', '')} onChange={(e) => set('token', e.target.value)} />
      </div>
      <div className="field">
        <label>GraphQL Query</label>
        <textarea rows={5}
          placeholder={'query { boards(limit: 10) { id name } }'}
          value={str('query', '')}
          onChange={(e) => set('query', e.target.value)}
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <div className="field">
        <label>Variables (JSON, optional)</label>
        <textarea rows={3}
          placeholder='{"boardId": 123456789}'
          value={typeof config.variables === 'string' ? config.variables : JSON.stringify(config.variables ?? {}, null, 2)}
          onChange={(e) => { try { set('variables', JSON.parse(e.target.value)) } catch { set('variables', e.target.value) } }}
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

// ── Slice 288: ClickUp ─────────────────────────────────────────────────────────

export function ClickupConfig({ config, set, str }: ConfigProps) {
  const method = str('method', 'GET')
  const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE']
  return (
    <>
      <div className="field">
        <label>Token</label>
        <input type="password" placeholder="ClickUp personal or OAuth token" value={str('token', '')} onChange={(e) => set('token', e.target.value)} />
      </div>
      <div className="field">
        <label>Method</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Endpoint</label>
        <input placeholder="/team" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT', 'PATCH'].includes(method) && (
        <div className="field">
          <label>Body (JSON)</label>
          <textarea rows={4}
            placeholder='{"name": "New Task", "description": "...", "status": "Open"}'
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

// ── Slice 289: Amplitude ───────────────────────────────────────────────────────

export function AmplitudeConfig({ config, set, str }: ConfigProps) {
  const operation = str('operation', 'track')
  const OPS = ['track', 'identify', 'export']
  return (
    <>
      <div className="field">
        <label>API Key</label>
        <input placeholder="Amplitude API key" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <div className="field">
        <label>Secret Key</label>
        <input type="password" placeholder="Amplitude secret key" value={str('secret_key', '')} onChange={(e) => set('secret_key', e.target.value)} />
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPS.map((o) => <option key={o} value={o}>{o}</option>)}
        </select>
      </div>
      {operation === 'track' && (
        <div className="field">
          <label>Events (JSON array)</label>
          <textarea rows={5}
            placeholder={'[{"event_type": "page_view", "user_id": "user1", "event_properties": {}}]'}
            value={Array.isArray(config.events) ? JSON.stringify(config.events, null, 2) : String(config.events ?? '')}
            onChange={(e) => { try { set('events', JSON.parse(e.target.value)) } catch { set('events', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      {operation === 'identify' && (
        <div className="field">
          <label>Identification (JSON array)</label>
          <textarea rows={5}
            placeholder={'[{"user_id": "user1", "user_properties": {"$set": {"plan": "pro"}}}]'}
            value={Array.isArray(config.identification) ? JSON.stringify(config.identification, null, 2) : String(config.identification ?? '')}
            onChange={(e) => { try { set('identification', JSON.parse(e.target.value)) } catch { set('identification', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      {operation === 'export' && (
        <>
          <div className="field">
            <label>Start (YYYYMMDDTHH)</label>
            <input placeholder="20241201T00" value={str('start', '')} onChange={(e) => set('start', e.target.value)} />
          </div>
          <div className="field">
            <label>End (YYYYMMDDTHH)</label>
            <input placeholder="20241231T23" value={str('end', '')} onChange={(e) => set('end', e.target.value)} />
          </div>
        </>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

// ── Slice 290: Mixpanel ────────────────────────────────────────────────────────

export function MixpanelConfig({ config, set, str }: ConfigProps) {
  const operation = str('operation', 'track')
  const OPS = ['track', 'import', 'query']
  return (
    <>
      <div className="field">
        <label>Project Token</label>
        <input placeholder="Mixpanel project token" value={str('project_token', '')} onChange={(e) => set('project_token', e.target.value)} />
      </div>
      <div className="field">
        <label>API Secret</label>
        <input type="password" placeholder="Mixpanel API secret" value={str('api_secret', '')} onChange={(e) => set('api_secret', e.target.value)} />
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPS.map((o) => <option key={o} value={o}>{o}</option>)}
        </select>
      </div>
      {(operation === 'track' || operation === 'import') && (
        <div className="field">
          <label>Events (JSON array)</label>
          <textarea rows={5}
            placeholder={'[{"event": "Sign Up", "properties": {"distinct_id": "user1", "time": 1609459200}}]'}
            value={Array.isArray(config.events) ? JSON.stringify(config.events, null, 2) : String(config.events ?? '')}
            onChange={(e) => { try { set('events', JSON.parse(e.target.value)) } catch { set('events', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      {operation === 'query' && (
        <>
          <div className="field">
            <label>Endpoint</label>
            <input placeholder="/api/query" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
          </div>
          <div className="field">
            <label>Params (JSON)</label>
            <textarea rows={4}
              placeholder='{"from_date": "2024-01-01", "to_date": "2024-01-31", "event": ["Sign Up"]}'
              value={typeof config.params === 'string' ? config.params : JSON.stringify(config.params ?? {}, null, 2)}
              onChange={(e) => { try { set('params', JSON.parse(e.target.value)) } catch { set('params', e.target.value) } }}
              style={{ fontFamily: 'monospace', fontSize: 12 }}
            />
          </div>
        </>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

// ── Slice 291: Segment ─────────────────────────────────────────────────────────

export function SegmentConfig({ config, set, str }: ConfigProps) {
  const operation = str('operation', 'track')
  const OPS = ['track', 'identify', 'page', 'group', 'alias', 'batch']
  return (
    <>
      <div className="field">
        <label>Write Key</label>
        <input type="password" placeholder="Segment write key" value={str('write_key', '')} onChange={(e) => set('write_key', e.target.value)} />
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPS.map((o) => <option key={o} value={o}>{o}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Body (JSON)</label>
        <textarea rows={6}
          placeholder={'{"userId": "user1", "event": "Order Completed", "properties": {"revenue": 99.99}}'}
          value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
          onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Auth via Basic auth (write_key:). Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

// ── Slice 292: SendGrid ────────────────────────────────────────────────────────

export function SendgridConfig({ config, set, str }: ConfigProps) {
  const method = str('method', 'POST')
  const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE']
  return (
    <>
      <div className="field">
        <label>API Key</label>
        <input type="password" placeholder="SG.xxxx API key" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <div className="field">
        <label>Method</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Endpoint</label>
        <input placeholder="/mail/send" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT', 'PATCH'].includes(method) && (
        <div className="field">
          <label>Body (JSON)</label>
          <textarea rows={8}
            placeholder={'{\n  "personalizations": [{"to": [{"email": "to@example.com"}]}],\n  "from": {"email": "from@example.com"},\n  "subject": "Hello",\n  "content": [{"type": "text/plain", "value": "Hi!"}]\n}'}
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        202/204 responses return null body. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

// ── Slice 293: Braintree ───────────────────────────────────────────────────────

export function BraintreeConfig({ config, set, str }: ConfigProps) {
  const method = str('method', 'POST')
  const METHODS = ['GET', 'POST', 'PUT', 'DELETE']
  const environment = str('environment', 'sandbox')
  return (
    <>
      <div className="field">
        <label>Environment</label>
        <select value={environment} onChange={(e) => set('environment', e.target.value)}>
          <option value="sandbox">Sandbox</option>
          <option value="production">Production</option>
        </select>
      </div>
      <div className="field">
        <label>Merchant ID</label>
        <input placeholder="Braintree merchant ID" value={str('merchant_id', '')} onChange={(e) => set('merchant_id', e.target.value)} />
      </div>
      <div className="field">
        <label>Public Key</label>
        <input placeholder="Braintree public key" value={str('public_key', '')} onChange={(e) => set('public_key', e.target.value)} />
      </div>
      <div className="field">
        <label>Private Key</label>
        <input type="password" placeholder="Braintree private key" value={str('private_key', '')} onChange={(e) => set('private_key', e.target.value)} />
      </div>
      <div className="field">
        <label>Method</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Endpoint</label>
        <input placeholder="/transactions" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT'].includes(method) && (
        <div className="field">
          <label>Body (JSON)</label>
          <textarea rows={4}
            placeholder='{"transaction": {"amount": "10.00", "payment_method_nonce": "fake-valid-nonce"}}'
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Auth via Basic auth (public_key:private_key). Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

// ── Slice 294: PayPal ──────────────────────────────────────────────────────────

export function PaypalConfig({ config, set, str }: ConfigProps) {
  const method = str('method', 'POST')
  const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE']
  const environment = str('environment', 'sandbox')
  return (
    <>
      <div className="field">
        <label>Environment</label>
        <select value={environment} onChange={(e) => set('environment', e.target.value)}>
          <option value="sandbox">Sandbox</option>
          <option value="live">Live</option>
        </select>
      </div>
      <div className="field">
        <label>Client ID</label>
        <input placeholder="PayPal client ID" value={str('client_id', '')} onChange={(e) => set('client_id', e.target.value)} />
      </div>
      <div className="field">
        <label>Client Secret</label>
        <input type="password" placeholder="PayPal client secret" value={str('client_secret', '')} onChange={(e) => set('client_secret', e.target.value)} />
      </div>
      <div className="field">
        <label>Access Token (optional)</label>
        <input type="password" placeholder="Pre-obtained access token (skip token exchange)" value={str('access_token', '')} onChange={(e) => set('access_token', e.target.value)} />
      </div>
      <div className="field">
        <label>Method</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Endpoint</label>
        <input placeholder="/v2/checkout/orders" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT', 'PATCH'].includes(method) && (
        <div className="field">
          <label>Body (JSON)</label>
          <textarea rows={5}
            placeholder={'{"intent": "CAPTURE", "purchase_units": [{"amount": {"currency_code": "USD", "value": "10.00"}}]}'}
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Auto-fetches token via client_credentials if access_token is not provided.
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

// ── Slice 295: Razorpay ────────────────────────────────────────────────────────

export function RazorpayConfig({ config, set, str }: ConfigProps) {
  const method = str('method', 'GET')
  const METHODS = ['GET', 'POST', 'PUT', 'DELETE', 'PATCH']
  return (
    <>
      <div className="field">
        <label>Key ID</label>
        <input placeholder="rzp_test_…" value={str('key_id', '')} onChange={(e) => set('key_id', e.target.value)} />
      </div>
      <div className="field">
        <label>Key Secret</label>
        <input type="password" placeholder="Razorpay key secret" value={str('key_secret', '')} onChange={(e) => set('key_secret', e.target.value)} />
      </div>
      <div className="field">
        <label>Method</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Endpoint</label>
        <input placeholder="/orders" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT', 'PATCH'].includes(method) && (
        <div className="field">
          <label>Body (JSON)</label>
          <textarea rows={4}
            placeholder='{"amount": 50000, "currency": "INR", "receipt": "order_rcptid_11"}'
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Auth via Basic auth (key_id:key_secret). Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

// ── Slice 296: Firebase ────────────────────────────────────────────────────────

export function FirebaseConfig({ config, set, str }: ConfigProps) {
  const method = str('method', 'GET')
  const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE']
  const service = str('service', 'firestore')
  return (
    <>
      <div className="field">
        <label>Project ID</label>
        <input placeholder="my-firebase-project" value={str('project_id', '')} onChange={(e) => set('project_id', e.target.value)} />
      </div>
      <div className="field">
        <label>ID Token</label>
        <input type="password" placeholder="Firebase ID token or service account token" value={str('id_token', '')} onChange={(e) => set('id_token', e.target.value)} />
      </div>
      <div className="field">
        <label>Service</label>
        <select value={service} onChange={(e) => set('service', e.target.value)}>
          <option value="firestore">Firestore</option>
          <option value="rtdb">Realtime Database</option>
          <option value="storage">Cloud Storage</option>
        </select>
      </div>
      {service === 'rtdb' && (
        <div className="field">
          <label>Database URL</label>
          <input placeholder="https://PROJECT.firebaseio.com" value={str('database_url', '')} onChange={(e) => set('database_url', e.target.value)} />
        </div>
      )}
      <div className="field">
        <label>Method</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Endpoint / Document Path</label>
        <input placeholder="/users/USER_ID" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT', 'PATCH'].includes(method) && (
        <div className="field">
          <label>Body (JSON)</label>
          <textarea rows={4}
            placeholder='{"fields": {"name": {"stringValue": "Jane"}}}'
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        RTDB embeds auth in URL. Firestore/Storage use Bearer header.
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

// ── Slice 297: Supabase ────────────────────────────────────────────────────────

export function SupabaseConfig({ config, set, str }: ConfigProps) {
  const method = str('method', 'GET')
  const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE']
  return (
    <>
      <div className="field">
        <label>Project URL</label>
        <input placeholder="https://abcdef.supabase.co" value={str('project_url', '')} onChange={(e) => set('project_url', e.target.value)} />
      </div>
      <div className="field">
        <label>API Key</label>
        <input type="password" placeholder="anon or service_role key" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <div className="field">
        <label>Method</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Endpoint</label>
        <input placeholder="/rest/v1/users" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      <div className="field">
        <label>Prefer (optional)</label>
        <input placeholder="return=representation" value={str('prefer', '')} onChange={(e) => set('prefer', e.target.value)} />
      </div>
      {['POST', 'PUT', 'PATCH'].includes(method) && (
        <div className="field">
          <label>Body (JSON)</label>
          <textarea rows={4}
            placeholder='{"name": "Jane", "email": "jane@example.com"}'
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Sends both <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>apikey</code> and <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>Authorization: Bearer</code> headers.
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

// ── Slice 298: Mailchimp ───────────────────────────────────────────────────────

export function MailchimpConfig({ config, set, str }: ConfigProps) {
  const method = str('method', 'GET')
  const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE']
  return (
    <>
      <div className="field">
        <label>API Key</label>
        <input type="password" placeholder="key-us1 (server auto-extracted)" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <div className="field">
        <label>Server Prefix (optional)</label>
        <input placeholder="us1 (auto-extracted from key if omitted)" value={str('server', '')} onChange={(e) => set('server', e.target.value)} />
      </div>
      <div className="field">
        <label>Method</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Endpoint</label>
        <input placeholder="/lists" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT', 'PATCH'].includes(method) && (
        <div className="field">
          <label>Body (JSON)</label>
          <textarea rows={5}
            placeholder={'{"email_address": "user@example.com", "status": "subscribed"}'}
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Auth via Basic auth (anystring:api_key). Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

// ── Slice 299: ActiveCampaign ──────────────────────────────────────────────────

export function ActivecampaignConfig({ config, set, str }: ConfigProps) {
  const method = str('method', 'GET')
  const METHODS = ['GET', 'POST', 'PUT', 'DELETE']
  return (
    <>
      <div className="field">
        <label>API Key</label>
        <input type="password" placeholder="ActiveCampaign API key" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <div className="field">
        <label>Base URL</label>
        <input placeholder="https://ACCOUNT.api-us1.com" value={str('base_url', '')} onChange={(e) => set('base_url', e.target.value)} />
      </div>
      <div className="field">
        <label>Method</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Endpoint</label>
        <input placeholder="/contacts" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT'].includes(method) && (
        <div className="field">
          <label>Body (JSON)</label>
          <textarea rows={5}
            placeholder={'{"contact": {"email": "user@example.com", "firstName": "Jane"}}'}
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Auth via <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>Api-Token</code> header.
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

// ── Slice 300: Klaviyo ─────────────────────────────────────────────────────────

export function KlaviyoConfig({ config, set, str }: ConfigProps) {
  const method = str('method', 'GET')
  const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE']
  return (
    <>
      <div className="field">
        <label>Private API Key</label>
        <input type="password" placeholder="pk_…" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <div className="field">
        <label>Method</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Endpoint</label>
        <input placeholder="/profiles" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT', 'PATCH'].includes(method) && (
        <div className="field">
          <label>Body (JSON)</label>
          <textarea rows={5}
            placeholder={'{"data": {"type": "profile", "attributes": {"email": "user@example.com"}}}'}
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Auth via <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>Klaviyo-API-Key {'{key}'}</code> header with API revision 2024-02-15.
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

// ── Slice 301: Resend ──────────────────────────────────────────────────────────

export function ResendConfig({ config, set, str }: ConfigProps) {
  const method = str('method', 'POST')
  const METHODS = ['GET', 'POST', 'PATCH', 'DELETE']
  return (
    <>
      <div className="field">
        <label>API Key</label>
        <input type="password" placeholder="re_…" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <div className="field">
        <label>Method</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Endpoint</label>
        <input placeholder="/emails" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PATCH'].includes(method) && (
        <div className="field">
          <label>Body (JSON)</label>
          <textarea rows={6}
            placeholder={'{\n  "from": "you@example.com",\n  "to": ["user@example.com"],\n  "subject": "Hello",\n  "html": "<p>Hi!</p>"\n}'}
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

// ── Slice 302: Contentful ──────────────────────────────────────────────────────

export function ContentfulConfig({ config, set, str }: ConfigProps) {
  const method = str('method', 'GET')
  const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE']
  const apiType = str('api_type', 'delivery')
  return (
    <>
      <div className="field">
        <label>Access Token</label>
        <input type="password" placeholder="Delivery/Preview/Management access token" value={str('access_token', '')} onChange={(e) => set('access_token', e.target.value)} />
      </div>
      <div className="field">
        <label>Space ID</label>
        <input placeholder="Contentful space ID" value={str('space_id', '')} onChange={(e) => set('space_id', e.target.value)} />
      </div>
      <div className="field">
        <label>API Type</label>
        <select value={apiType} onChange={(e) => set('api_type', e.target.value)}>
          <option value="delivery">Delivery (cdn.contentful.com)</option>
          <option value="preview">Preview (preview.contentful.com)</option>
          <option value="management">Management (api.contentful.com)</option>
        </select>
      </div>
      <div className="field">
        <label>Method</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Endpoint</label>
        <input placeholder="/entries?content_type=blogPost" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT', 'PATCH'].includes(method) && (
        <div className="field">
          <label>Body (JSON)</label>
          <textarea rows={4}
            placeholder='{"fields": {"title": {"en-US": "My Post"}}}'
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

// ── Slice 303: Algolia ─────────────────────────────────────────────────────────

export function AlgoliaConfig({ config, set, str }: ConfigProps) {
  const method = str('method', 'POST')
  const METHODS = ['GET', 'POST', 'PUT', 'DELETE']
  return (
    <>
      <div className="field">
        <label>Application ID</label>
        <input placeholder="ABC123DEF4" value={str('app_id', '')} onChange={(e) => set('app_id', e.target.value)} />
      </div>
      <div className="field">
        <label>API Key</label>
        <input type="password" placeholder="Search or Admin API key" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <div className="field">
        <label>Method</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Endpoint</label>
        <input placeholder="/1/indexes/INDEX_NAME/query" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT'].includes(method) && (
        <div className="field">
          <label>Body (JSON)</label>
          <textarea rows={4}
            placeholder='{"query": "search term", "hitsPerPage": 10}'
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Auth via <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>X-Algolia-Application-Id</code> + <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>X-Algolia-API-Key</code> headers.
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

// ── Slice 304: Postmark ────────────────────────────────────────────────────────

export function PostmarkConfig({ config, set, str }: ConfigProps) {
  const method = str('method', 'POST')
  const METHODS = ['GET', 'POST', 'PUT', 'DELETE']
  return (
    <>
      <div className="field">
        <label>Server Token</label>
        <input type="password" placeholder="Postmark server token" value={str('server_token', '')} onChange={(e) => set('server_token', e.target.value)} />
      </div>
      <div className="field">
        <label>Method</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Endpoint</label>
        <input placeholder="/email" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT'].includes(method) && (
        <div className="field">
          <label>Body (JSON)</label>
          <textarea rows={6}
            placeholder={'{\n  "From": "sender@example.com",\n  "To": "recipient@example.com",\n  "Subject": "Hello",\n  "TextBody": "Hi there!"\n}'}
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Auth via <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>X-Postmark-Server-Token</code> header.
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

// ── Slice 305: Vonage ──────────────────────────────────────────────────────────

export function VonageConfig({ config, set, str }: ConfigProps) {
  const operation = str('operation', 'sms')
  const OPS = ['sms', 'voice', 'verify']
  return (
    <>
      <div className="field">
        <label>API Key</label>
        <input placeholder="Vonage API key" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <div className="field">
        <label>API Secret</label>
        <input type="password" placeholder="Vonage API secret" value={str('api_secret', '')} onChange={(e) => set('api_secret', e.target.value)} />
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPS.map((o) => <option key={o} value={o}>{o}</option>)}
        </select>
      </div>
      {operation === 'sms' && (
        <>
          <div className="field">
            <label>To</label>
            <input placeholder="+14155551234" value={str('to', '')} onChange={(e) => set('to', e.target.value)} />
          </div>
          <div className="field">
            <label>From</label>
            <input placeholder="Vonage or your virtual number" value={str('from', '')} onChange={(e) => set('from', e.target.value)} />
          </div>
          <div className="field">
            <label>Text</label>
            <textarea rows={3} placeholder="SMS message text" value={str('text', '')} onChange={(e) => set('text', e.target.value)} />
          </div>
        </>
      )}
      {(operation === 'voice' || operation === 'verify') && (
        <>
          <div className="field">
            <label>Endpoint</label>
            <input placeholder="/v1/calls or /verify/json" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
          </div>
          <div className="field">
            <label>Body (JSON)</label>
            <textarea rows={4}
              placeholder='{"to": [{"type": "phone", "number": "14155551234"}], "from": {...}, "ncco": [...]}'
              value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
              onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
              style={{ fontFamily: 'monospace', fontSize: 12 }}
            />
          </div>
        </>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        SMS posts to rest.nexmo.com. Voice/Verify use Basic auth to api.nexmo.com.
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function TelegramConfig({ config, set, str }: ConfigProps) {
  const operation = str('operation', 'sendMessage')
  const OPERATIONS = ['sendMessage', 'sendPhoto', 'sendDocument', 'sendAudio', 'sendVideo',
                      'editMessageText', 'deleteMessage', 'getUpdates', 'getMe', 'setChatTitle']
  return (
    <>
      <div className="field">
        <label>Bot Token <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input
          type="password"
          placeholder="123456:ABC-DEF…"
          value={str('bot_token', '')}
          onChange={(e) => set('bot_token', e.target.value)}
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {['sendMessage', 'sendPhoto', 'sendDocument', 'sendAudio', 'sendVideo', 'editMessageText', 'deleteMessage'].includes(operation) && (
        <div className="field">
          <label>Chat ID <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input
            placeholder="{{input.chat_id}} or -100123456789"
            value={str('chat_id', '')}
            onChange={(e) => set('chat_id', e.target.value)}
          />
        </div>
      )}
      {['sendMessage', 'editMessageText'].includes(operation) && (
        <>
          <div className="field">
            <label>Text <span style={{ color: 'var(--danger)' }}>*</span></label>
            <textarea
              rows={3}
              placeholder="{{input.text}}"
              value={str('text', '')}
              onChange={(e) => set('text', e.target.value)}
              style={{ fontFamily: 'monospace', fontSize: 12 }}
            />
          </div>
          <div className="field">
            <label>Parse Mode</label>
            <select value={str('parse_mode', '')} onChange={(e) => set('parse_mode', e.target.value)}>
              <option value="">(none)</option>
              <option value="Markdown">Markdown</option>
              <option value="MarkdownV2">MarkdownV2</option>
              <option value="HTML">HTML</option>
            </select>
          </div>
        </>
      )}
      <div className="field">
        <label>Extra Fields (JSON)</label>
        <textarea
          rows={2}
          placeholder='{"disable_notification": true}'
          value={typeof config.extra === 'object' ? JSON.stringify(config.extra) : str('extra', '')}
          onChange={(e) => { try { set('extra', JSON.parse(e.target.value)) } catch { set('extra', e.target.value) } }}
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Calls <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>api.telegram.org/bot&#123;token&#125;/&#123;operation&#125;</code> with the provided fields.
        Returns the Telegram API response object.
      </p>
    </>
  )
}

export function ReplicateConfig({ config, set, str }: ConfigProps) {
  const operation = str('operation', 'run')
  const OPERATIONS = ['run', 'create_prediction', 'get_prediction', 'list_models']
  return (
    <>
      <div className="field">
        <label>API Token <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="r8_…" value={str('api_token', '')} onChange={(e) => set('api_token', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {['run', 'create_prediction'].includes(operation) && (
        <>
          <div className="field">
            <label>Model Version <span style={{ color: 'var(--danger)' }}>*</span></label>
            <input placeholder="abc123… (version hash)" value={str('version', '')} onChange={(e) => set('version', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>Input (JSON)</label>
            <textarea rows={3} placeholder='{"prompt": "{{input.prompt}}"}' value={typeof config.input === 'object' ? JSON.stringify(config.input, null, 2) : str('input', '')} onChange={(e) => { try { set('input', JSON.parse(e.target.value)) } catch { set('input', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
        </>
      )}
      {operation === 'get_prediction' && (
        <div className="field">
          <label>Prediction ID <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input placeholder="{{input.prediction_id}}" value={str('prediction_id', '')} onChange={(e) => set('prediction_id', e.target.value)} />
        </div>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Calls the Replicate REST API. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function MistralConfig({ config, set, str }: ConfigProps) {
  const operation = str('operation', 'chat')
  const OPERATIONS = ['chat', 'embeddings', 'list_models']
  const MODELS = ['mistral-small-latest', 'mistral-medium-latest', 'mistral-large-latest', 'mistral-embed', 'open-mistral-7b', 'open-mixtral-8x7b']
  return (
    <>
      <div className="field">
        <label>API Key <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation === 'chat' && (
        <>
          <div className="field">
            <label>Model</label>
            <select value={str('model', 'mistral-small-latest')} onChange={(e) => set('model', e.target.value)}>
              {MODELS.filter(m => m !== 'mistral-embed').map((m) => <option key={m} value={m}>{m}</option>)}
            </select>
          </div>
          <div className="field">
            <label>Prompt (or use Messages JSON)</label>
            <textarea rows={3} placeholder="{{input.prompt}}" value={str('prompt', '')} onChange={(e) => set('prompt', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>Temperature</label>
            <input type="number" min={0} max={2} step={0.1} placeholder="0.7" value={(config['temperature'] as number | undefined) ?? ''} onChange={(e) => set('temperature', e.target.value ? parseFloat(e.target.value) : undefined)} />
          </div>
          <div className="field">
            <label>Max Tokens</label>
            <input type="number" min={1} placeholder="1024" value={(config['max_tokens'] as number | undefined) ?? ''} onChange={(e) => set('max_tokens', e.target.value ? parseInt(e.target.value) : undefined)} />
          </div>
        </>
      )}
      {operation === 'embeddings' && (
        <>
          <div className="field">
            <label>Model</label>
            <select value={str('model', 'mistral-embed')} onChange={(e) => set('model', e.target.value)}>
              <option value="mistral-embed">mistral-embed</option>
            </select>
          </div>
          <div className="field">
            <label>Input (string or array)</label>
            <textarea rows={2} placeholder='"{{input.text}}"' value={str('input', '')} onChange={(e) => set('input', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
        </>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function WhatsappConfig({ set, str }: ConfigProps) {
  const messageType = str('message_type', 'text')
  const MESSAGE_TYPES = ['text', 'template', 'image', 'document', 'audio', 'video']
  return (
    <>
      <div className="field">
        <label>Access Token <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="EAA…" value={str('access_token', '')} onChange={(e) => set('access_token', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Phone Number ID <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="1234567890" value={str('phone_number_id', '')} onChange={(e) => set('phone_number_id', e.target.value)} />
      </div>
      <div className="field">
        <label>To (Recipient) <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="+1234567890 or {{input.phone}}" value={str('to', '')} onChange={(e) => set('to', e.target.value)} />
      </div>
      <div className="field">
        <label>Message Type</label>
        <select value={messageType} onChange={(e) => set('message_type', e.target.value)}>
          {MESSAGE_TYPES.map((t) => <option key={t} value={t}>{t}</option>)}
        </select>
      </div>
      {messageType === 'text' && (
        <div className="field">
          <label>Message Body</label>
          <textarea rows={3} placeholder="{{input.message}}" value={str('body', '')} onChange={(e) => set('body', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {messageType === 'template' && (
        <>
          <div className="field">
            <label>Template Name <span style={{ color: 'var(--danger)' }}>*</span></label>
            <input placeholder="hello_world" value={str('template_name', '')} onChange={(e) => set('template_name', e.target.value)} />
          </div>
          <div className="field">
            <label>Language Code</label>
            <input placeholder="en_US" value={str('language_code', 'en_US')} onChange={(e) => set('language_code', e.target.value)} />
          </div>
        </>
      )}
      {['image', 'document', 'audio', 'video'].includes(messageType) && (
        <div className="field">
          <label>Media URL</label>
          <input placeholder="https://…" value={str('media_url', '')} onChange={(e) => set('media_url', e.target.value)} />
        </div>
      )}
      <div className="field">
        <label>API Version</label>
        <input placeholder="v18.0" value={str('api_version', 'v18.0')} onChange={(e) => set('api_version', e.target.value)} />
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Sends via Meta Graph API. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function GoogledocsConfig({ config, set, str }: ConfigProps) {
  const operation = str('operation', 'get')
  const OPERATIONS = ['get', 'create', 'batch_update']
  return (
    <>
      <div className="field">
        <label>Access Token <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="ya29.…" value={str('access_token', '')} onChange={(e) => set('access_token', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {['get', 'batch_update'].includes(operation) && (
        <div className="field">
          <label>Document ID <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input placeholder="1BxiMVs0XRA…" value={str('document_id', '')} onChange={(e) => set('document_id', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {operation === 'create' && (
        <div className="field">
          <label>Document Title</label>
          <input placeholder="Untitled Document" value={str('title', '')} onChange={(e) => set('title', e.target.value)} />
        </div>
      )}
      {operation === 'batch_update' && (
        <div className="field">
          <label>Requests (JSON array)</label>
          <textarea rows={4} placeholder='[{"insertText":{"location":{"index":1},"text":"Hello"}}]' value={typeof config.requests === 'object' ? JSON.stringify(config.requests, null, 2) : str('requests', '')} onChange={(e) => { try { set('requests', JSON.parse(e.target.value)) } catch { set('requests', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Uses Google Docs REST API v1. Obtain access_token via OAuth2 (scope: documents). Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function PerplexityConfig({ config, set, str }: ConfigProps) {
  const MODELS = [
    'llama-3.1-sonar-small-128k-online',
    'llama-3.1-sonar-large-128k-online',
    'llama-3.1-sonar-huge-128k-online',
    'llama-3.1-sonar-small-128k-chat',
    'llama-3.1-sonar-large-128k-chat',
  ]
  return (
    <>
      <div className="field">
        <label>API Key <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="pplx-…" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Model</label>
        <select value={str('model', 'llama-3.1-sonar-small-128k-online')} onChange={(e) => set('model', e.target.value)}>
          {MODELS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Prompt <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={3} placeholder="{{input.question}}" value={str('prompt', '')} onChange={(e) => set('prompt', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Temperature</label>
        <input type="number" min={0} max={2} step={0.1} placeholder="0.2" value={(config['temperature'] as number | undefined) ?? ''} onChange={(e) => set('temperature', e.target.value ? parseFloat(e.target.value) : undefined)} />
      </div>
      <div className="field">
        <label>Max Tokens</label>
        <input type="number" min={1} placeholder="1024" value={(config['max_tokens'] as number | undefined) ?? ''} onChange={(e) => set('max_tokens', e.target.value ? parseInt(e.target.value) : undefined)} />
      </div>
      <div className="field" style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
        <input type="checkbox" id="pplx-citations" checked={!!config.return_citations} onChange={(e) => set('return_citations', e.target.checked)} />
        <label htmlFor="pplx-citations" style={{ margin: 0 }}>Return Citations</label>
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Online models perform real-time web search. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function CohereConfig({ config, set, str }: ConfigProps) {
  const operation = str('operation', 'chat')
  const OPERATIONS = ['chat', 'embed', 'classify', 'rerank']
  const EMBED_MODELS = ['embed-english-v3.0', 'embed-multilingual-v3.0', 'embed-english-light-v3.0']
  const RERANK_MODELS = ['rerank-english-v3.0', 'rerank-multilingual-v3.0']
  const CHAT_MODELS = ['command-r-plus', 'command-r', 'command', 'command-light']
  return (
    <>
      <div className="field">
        <label>API Key <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation === 'chat' && (
        <>
          <div className="field">
            <label>Model</label>
            <select value={str('model', 'command-r-plus')} onChange={(e) => set('model', e.target.value)}>
              {CHAT_MODELS.map((m) => <option key={m} value={m}>{m}</option>)}
            </select>
          </div>
          <div className="field">
            <label>Message <span style={{ color: 'var(--danger)' }}>*</span></label>
            <textarea rows={3} placeholder="{{input.message}}" value={str('message', '')} onChange={(e) => set('message', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>Temperature</label>
            <input type="number" min={0} max={1} step={0.1} value={(config['temperature'] as number | undefined) ?? ''} onChange={(e) => set('temperature', e.target.value ? parseFloat(e.target.value) : undefined)} />
          </div>
        </>
      )}
      {operation === 'embed' && (
        <>
          <div className="field">
            <label>Model</label>
            <select value={str('model', 'embed-english-v3.0')} onChange={(e) => set('model', e.target.value)}>
              {EMBED_MODELS.map((m) => <option key={m} value={m}>{m}</option>)}
            </select>
          </div>
          <div className="field">
            <label>Texts (JSON array) <span style={{ color: 'var(--danger)' }}>*</span></label>
            <textarea rows={2} placeholder='["{{input.text}}"]' value={typeof config.texts === 'object' ? JSON.stringify(config.texts) : str('texts', '')} onChange={(e) => { try { set('texts', JSON.parse(e.target.value)) } catch { set('texts', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>Input Type</label>
            <select value={str('input_type', 'search_document')} onChange={(e) => set('input_type', e.target.value)}>
              {['search_document','search_query','classification','clustering'].map((t) => <option key={t} value={t}>{t}</option>)}
            </select>
          </div>
        </>
      )}
      {operation === 'rerank' && (
        <>
          <div className="field">
            <label>Model</label>
            <select value={str('model', 'rerank-english-v3.0')} onChange={(e) => set('model', e.target.value)}>
              {RERANK_MODELS.map((m) => <option key={m} value={m}>{m}</option>)}
            </select>
          </div>
          <div className="field">
            <label>Query <span style={{ color: 'var(--danger)' }}>*</span></label>
            <input placeholder="{{input.query}}" value={str('query', '')} onChange={(e) => set('query', e.target.value)} />
          </div>
          <div className="field">
            <label>Documents (JSON array) <span style={{ color: 'var(--danger)' }}>*</span></label>
            <textarea rows={3} placeholder='["doc1","doc2"]' value={typeof config.documents === 'object' ? JSON.stringify(config.documents) : str('documents', '')} onChange={(e) => { try { set('documents', JSON.parse(e.target.value)) } catch { set('documents', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
        </>
      )}
      {operation === 'classify' && (
        <>
          <div className="field">
            <label>Inputs (JSON array) <span style={{ color: 'var(--danger)' }}>*</span></label>
            <textarea rows={2} placeholder='["text to classify"]' value={typeof config.inputs === 'object' ? JSON.stringify(config.inputs) : str('inputs', '')} onChange={(e) => { try { set('inputs', JSON.parse(e.target.value)) } catch { set('inputs', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>Examples (JSON array) <span style={{ color: 'var(--danger)' }}>*</span></label>
            <textarea rows={3} placeholder='[{"text":"pos example","label":"positive"}]' value={typeof config.examples === 'object' ? JSON.stringify(config.examples) : str('examples', '')} onChange={(e) => { try { set('examples', JSON.parse(e.target.value)) } catch { set('examples', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
        </>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function GoogledriveConfig({ set, str }: ConfigProps) {
  const operation = str('operation', 'list')
  const OPERATIONS = ['list', 'get', 'delete', 'create_folder']
  return (
    <>
      <div className="field">
        <label>Access Token <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="ya29.…" value={str('access_token', '')} onChange={(e) => set('access_token', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {['get', 'delete'].includes(operation) && (
        <div className="field">
          <label>File ID <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input placeholder="1BxiMVs0XRA…" value={str('file_id', '')} onChange={(e) => set('file_id', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {operation === 'list' && (
        <>
          <div className="field">
            <label>Query (Drive search)</label>
            <input placeholder="name contains 'report'" value={str('query', '')} onChange={(e) => set('query', e.target.value)} />
          </div>
          <div className="field">
            <label>Fields</label>
            <input placeholder="files(id,name,mimeType)" value={str('fields', '')} onChange={(e) => set('fields', e.target.value)} />
          </div>
        </>
      )}
      {operation === 'create_folder' && (
        <>
          <div className="field">
            <label>Folder Name</label>
            <input placeholder="New Folder" value={str('name', '')} onChange={(e) => set('name', e.target.value)} />
          </div>
          <div className="field">
            <label>Parent Folder ID</label>
            <input placeholder="(root if blank)" value={str('parent_id', '')} onChange={(e) => set('parent_id', e.target.value)} />
          </div>
        </>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Requires OAuth2 access token with Drive scope. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function WoocommerceConfig({ config, set, str }: ConfigProps) {
  const method = str('method', 'GET')
  const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE']
  return (
    <>
      <div className="field">
        <label>Site URL <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="https://mystore.com" value={str('site_url', '')} onChange={(e) => set('site_url', e.target.value)} />
      </div>
      <div className="field">
        <label>Consumer Key <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="ck_…" value={str('consumer_key', '')} onChange={(e) => set('consumer_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Consumer Secret <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="cs_…" value={str('consumer_secret', '')} onChange={(e) => set('consumer_secret', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Method</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Endpoint</label>
        <input placeholder="/wp-json/wc/v3/products" value={str('endpoint', '/wp-json/wc/v3/products')} onChange={(e) => set('endpoint', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      {['POST', 'PUT', 'PATCH'].includes(method) && (
        <div className="field">
          <label>Body (JSON)</label>
          <textarea rows={4} placeholder='{"name":"T-Shirt","regular_price":"15.00"}' value={typeof config.body === 'object' ? JSON.stringify(config.body, null, 2) : str('body', '')} onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Authenticates with Basic auth (consumer_key:consumer_secret). Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function PineconeConfig({ config, set, str }: ConfigProps) {
  const operation = str('operation', 'query')
  const OPERATIONS = ['query', 'upsert', 'delete', 'fetch']
  return (
    <>
      <div className="field">
        <label>API Key <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Index Host <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="https://my-index-abc.svc.pinecone.io" value={str('index_host', '')} onChange={(e) => set('index_host', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Namespace</label>
        <input placeholder="(optional)" value={str('namespace', '')} onChange={(e) => set('namespace', e.target.value)} />
      </div>
      {operation === 'query' && (
        <>
          <div className="field">
            <label>Vector (JSON float array) <span style={{ color: 'var(--danger)' }}>*</span></label>
            <textarea rows={2} placeholder="[0.1, 0.2, 0.3, …]" value={typeof config.vector === 'object' ? JSON.stringify(config.vector) : str('vector', '')} onChange={(e) => { try { set('vector', JSON.parse(e.target.value)) } catch { set('vector', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>Top K</label>
            <input type="number" min={1} max={10000} placeholder="10" value={(config['top_k'] as number | undefined) ?? ''} onChange={(e) => set('top_k', e.target.value ? parseInt(e.target.value) : undefined)} />
          </div>
          <div className="field" style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
            <input type="checkbox" id="pc-meta" checked={!!config.include_metadata} onChange={(e) => set('include_metadata', e.target.checked)} />
            <label htmlFor="pc-meta" style={{ margin: 0 }}>Include Metadata</label>
          </div>
        </>
      )}
      {operation === 'upsert' && (
        <div className="field">
          <label>Vectors (JSON array) <span style={{ color: 'var(--danger)' }}>*</span></label>
          <textarea rows={4} placeholder='[{"id":"v1","values":[0.1,0.2],"metadata":{"text":"hello"}}]' value={typeof config.vectors === 'object' ? JSON.stringify(config.vectors, null, 2) : str('vectors', '')} onChange={(e) => { try { set('vectors', JSON.parse(e.target.value)) } catch { set('vectors', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {['delete', 'fetch'].includes(operation) && (
        <div className="field">
          <label>IDs (JSON array) <span style={{ color: 'var(--danger)' }}>*</span></label>
          <textarea rows={2} placeholder='["id1","id2"]' value={typeof config.ids === 'object' ? JSON.stringify(config.ids) : str('ids', '')} onChange={(e) => { try { set('ids', JSON.parse(e.target.value)) } catch { set('ids', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function TogetheraiConfig({ config, set, str }: ConfigProps) {
  const operation = str('operation', 'chat')
  const OPERATIONS = ['chat', 'completions', 'embeddings']
  return (
    <>
      <div className="field">
        <label>API Key <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Model</label>
        <input placeholder="meta-llama/Llama-3.2-11B-Vision-Instruct-Turbo" value={str('model', '')} onChange={(e) => set('model', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      {['chat', 'completions'].includes(operation) && (
        <>
          <div className="field">
            <label>Prompt {operation === 'chat' ? '(or Messages JSON)' : ''}</label>
            <textarea rows={3} placeholder="{{input.prompt}}" value={str('prompt', '')} onChange={(e) => set('prompt', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>Temperature</label>
            <input type="number" min={0} max={2} step={0.1} placeholder="0.7" value={(config['temperature'] as number | undefined) ?? ''} onChange={(e) => set('temperature', e.target.value ? parseFloat(e.target.value) : undefined)} />
          </div>
          <div className="field">
            <label>Max Tokens</label>
            <input type="number" min={1} placeholder="512" value={(config['max_tokens'] as number | undefined) ?? ''} onChange={(e) => set('max_tokens', e.target.value ? parseInt(e.target.value) : undefined)} />
          </div>
        </>
      )}
      {operation === 'embeddings' && (
        <div className="field">
          <label>Input</label>
          <input placeholder="{{input.text}}" value={str('input', '')} onChange={(e) => set('input', e.target.value)} />
        </div>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Runs open-source LLMs. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function Awss3Config({ set, str }: ConfigProps) {
  const operation = str('operation', 'list')
  const OPERATIONS = ['list', 'get_object', 'put_object', 'delete_object']
  return (
    <>
      <div className="field">
        <label>Access Key ID <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="AKIA…" value={str('access_key_id', '')} onChange={(e) => set('access_key_id', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Secret Access Key <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" value={str('secret_access_key', '')} onChange={(e) => set('secret_access_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Bucket <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="my-bucket" value={str('bucket', '')} onChange={(e) => set('bucket', e.target.value)} />
      </div>
      <div className="field">
        <label>Region</label>
        <input placeholder="us-east-1" value={str('region', 'us-east-1')} onChange={(e) => set('region', e.target.value)} />
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation === 'list' && (
        <div className="field">
          <label>Prefix</label>
          <input placeholder="folder/" value={str('prefix', '')} onChange={(e) => set('prefix', e.target.value)} />
        </div>
      )}
      {['get_object', 'put_object', 'delete_object'].includes(operation) && (
        <div className="field">
          <label>Key (object path) <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input placeholder="folder/file.txt" value={str('key', '')} onChange={(e) => set('key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {operation === 'put_object' && (
        <>
          <div className="field">
            <label>Content Type</label>
            <input placeholder="text/plain" value={str('content_type', 'application/octet-stream')} onChange={(e) => set('content_type', e.target.value)} />
          </div>
          <div className="field">
            <label>Body</label>
            <textarea rows={3} placeholder="{{input.content}}" value={str('body', '')} onChange={(e) => set('body', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
        </>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Note: Uses AWS Signature V4. For production, ensure credentials have minimal required IAM permissions. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function HuggingfaceConfig({ config, set, str }: ConfigProps) {
  const operation = str('operation', 'inference')
  const OPERATIONS = ['inference', 'model_info', 'list_models']
  return (
    <>
      <div className="field">
        <label>API Token <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="hf_…" value={str('api_token', '')} onChange={(e) => set('api_token', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Model <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="facebook/bart-large-cnn" value={str('model', '')} onChange={(e) => set('model', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation === 'inference' && (
        <>
          <div className="field">
            <label>Inputs <span style={{ color: 'var(--danger)' }}>*</span></label>
            <textarea rows={3} placeholder='"{{input.text}}" or {"question":"…","context":"…"}' value={str('inputs', '')} onChange={(e) => { try { set('inputs', JSON.parse(e.target.value)) } catch { set('inputs', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>Parameters (JSON)</label>
            <textarea rows={2} placeholder='{"max_length":100}' value={typeof config.parameters === 'object' ? JSON.stringify(config.parameters) : str('parameters', '')} onChange={(e) => { try { set('parameters', JSON.parse(e.target.value)) } catch { set('parameters', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
        </>
      )}
      {operation === 'list_models' && (
        <>
          <div className="field">
            <label>Search</label>
            <input placeholder="text-classification" value={str('search', '')} onChange={(e) => set('search', e.target.value)} />
          </div>
          <div className="field">
            <label>Limit</label>
            <input type="number" min={1} max={100} placeholder="20" value={(config['limit'] as number | undefined) ?? ''} onChange={(e) => set('limit', e.target.value ? parseInt(e.target.value) : undefined)} />
          </div>
        </>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Supports any model on the Hub: summarization, classification, Q&amp;A, image, etc. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function GroqConfig({ config, set, str }: ConfigProps) {
  const operation = str('operation', 'chat')
  const OPERATIONS = ['chat', 'models']
  return (
    <>
      <div className="field">
        <label>API Key <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="gsk_…" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation === 'chat' && (
        <>
          <div className="field">
            <label>Model</label>
            <input placeholder="llama3-8b-8192" value={str('model', '')} onChange={(e) => set('model', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>Messages (JSON array)</label>
            <textarea rows={4} placeholder='[{"role":"user","content":"Hello"}]' value={typeof config.messages === 'object' ? JSON.stringify(config.messages, null, 2) : str('messages', '')} onChange={(e) => { try { set('messages', JSON.parse(e.target.value)) } catch { set('messages', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>Temperature</label>
            <input type="number" min={0} max={2} step={0.1} placeholder="1.0" value={(config['temperature'] as number | undefined) ?? ''} onChange={(e) => set('temperature', e.target.value ? parseFloat(e.target.value) : undefined)} />
          </div>
          <div className="field">
            <label>Max Tokens</label>
            <input type="number" min={1} placeholder="1024" value={(config['max_tokens'] as number | undefined) ?? ''} onChange={(e) => set('max_tokens', e.target.value ? parseInt(e.target.value) : undefined)} />
          </div>
        </>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Ultra-fast LLM inference. Models: llama3-8b-8192, llama3-70b-8192, mixtral-8x7b-32768, gemma-7b-it. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function OpenrouterConfig({ config, set, str }: ConfigProps) {
  const operation = str('operation', 'chat')
  const OPERATIONS = ['chat', 'models']
  return (
    <>
      <div className="field">
        <label>API Key <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="sk-or-…" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation === 'chat' && (
        <>
          <div className="field">
            <label>Model</label>
            <input placeholder="openai/gpt-4o" value={str('model', '')} onChange={(e) => set('model', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>Messages (JSON array)</label>
            <textarea rows={4} placeholder='[{"role":"user","content":"Hello"}]' value={typeof config.messages === 'object' ? JSON.stringify(config.messages, null, 2) : str('messages', '')} onChange={(e) => { try { set('messages', JSON.parse(e.target.value)) } catch { set('messages', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>Temperature</label>
            <input type="number" min={0} max={2} step={0.1} placeholder="1.0" value={(config['temperature'] as number | undefined) ?? ''} onChange={(e) => set('temperature', e.target.value ? parseFloat(e.target.value) : undefined)} />
          </div>
          <div className="field">
            <label>Max Tokens</label>
            <input type="number" min={1} placeholder="1024" value={(config['max_tokens'] as number | undefined) ?? ''} onChange={(e) => set('max_tokens', e.target.value ? parseInt(e.target.value) : undefined)} />
          </div>
        </>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Access 100+ models from OpenAI, Anthropic, Meta, Mistral, and more. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function QdrantConfig({ config, set, str }: ConfigProps) {
  const operation = str('operation', 'search')
  const OPERATIONS = ['search', 'upsert', 'delete', 'get_collection', 'create_collection']
  return (
    <>
      <div className="field">
        <label>Server URL <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="https://your-cluster.qdrant.io" value={str('url', '')} onChange={(e) => set('url', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>API Key</label>
        <input type="password" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Collection <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="my_collection" value={str('collection', '')} onChange={(e) => set('collection', e.target.value)} />
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation === 'search' && (
        <>
          <div className="field">
            <label>Query Vector (JSON array)</label>
            <textarea rows={2} placeholder="[0.1, 0.2, 0.3, …]" value={typeof config.vector === 'object' ? JSON.stringify(config.vector) : str('vector', '')} onChange={(e) => { try { set('vector', JSON.parse(e.target.value)) } catch { set('vector', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>Top K</label>
            <input type="number" min={1} max={100} placeholder="10" value={(config['top'] as number | undefined) ?? ''} onChange={(e) => set('top', e.target.value ? parseInt(e.target.value) : undefined)} />
          </div>
        </>
      )}
      {operation === 'upsert' && (
        <div className="field">
          <label>Points (JSON array)</label>
          <textarea rows={4} placeholder='[{"id":1,"vector":[0.1,0.2],"payload":{"text":"…"}}]' value={typeof config.points === 'object' ? JSON.stringify(config.points, null, 2) : str('points', '')} onChange={(e) => { try { set('points', JSON.parse(e.target.value)) } catch { set('points', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {operation === 'delete' && (
        <div className="field">
          <label>Point IDs (JSON array)</label>
          <textarea rows={2} placeholder='[1, 2, 3]' value={typeof config.ids === 'object' ? JSON.stringify(config.ids) : str('ids', '')} onChange={(e) => { try { set('ids', JSON.parse(e.target.value)) } catch { set('ids', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {operation === 'create_collection' && (
        <div className="field">
          <label>Vector Size</label>
          <input type="number" min={1} placeholder="1536" value={(config['vector_size'] as number | undefined) ?? ''} onChange={(e) => set('vector_size', e.target.value ? parseInt(e.target.value) : undefined)} />
        </div>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        High-performance vector similarity search. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function WeaviateConfig({ config, set, str }: ConfigProps) {
  const operation = str('operation', 'query')
  const OPERATIONS = ['query', 'create_object', 'get_object', 'delete_object']
  return (
    <>
      <div className="field">
        <label>Host <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="https://xyz.weaviate.network" value={str('host', '')} onChange={(e) => set('host', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>API Key</label>
        <input type="password" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation === 'query' && (
        <div className="field">
          <label>GraphQL Query <span style={{ color: 'var(--danger)' }}>*</span></label>
          <textarea rows={5} placeholder={'{ Get { Article(nearVector: {vector: [0.1, 0.2]}) { title } } }'} value={str('query', '')} onChange={(e) => set('query', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {(operation === 'create_object' || operation === 'get_object' || operation === 'delete_object') && (
        <div className="field">
          <label>Class <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input placeholder="Article" value={str('class', '')} onChange={(e) => set('class', e.target.value)} />
        </div>
      )}
      {operation === 'create_object' && (
        <>
          <div className="field">
            <label>Properties (JSON object)</label>
            <textarea rows={3} placeholder='{"title":"…","body":"…"}' value={typeof config.properties === 'object' ? JSON.stringify(config.properties, null, 2) : str('properties', '')} onChange={(e) => { try { set('properties', JSON.parse(e.target.value)) } catch { set('properties', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>Vector (JSON array, optional)</label>
            <textarea rows={2} placeholder="[0.1, 0.2, 0.3, …]" value={typeof config.vector === 'object' ? JSON.stringify(config.vector) : str('vector', '')} onChange={(e) => { try { set('vector', JSON.parse(e.target.value)) } catch { set('vector', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
        </>
      )}
      {(operation === 'get_object' || operation === 'delete_object' || operation === 'create_object') && (
        <div className="field">
          <label>Object ID {operation !== 'create_object' && <span style={{ color: 'var(--danger)' }}>*</span>}{operation === 'create_object' && ' (optional)'}</label>
          <input placeholder="uuid" value={str('id', '')} onChange={(e) => set('id', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Weaviate vector store (REST + GraphQL). Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function ChromaConfig({ config, set, str }: ConfigProps) {
  const operation = str('operation', 'query')
  const OPERATIONS = ['query', 'add', 'delete', 'get_collection']
  return (
    <>
      <div className="field">
        <label>Host <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="http://localhost:8000" value={str('host', '')} onChange={(e) => set('host', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>API Key</label>
        <input type="password" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation === 'get_collection' && (
        <div className="field">
          <label>Collection Name <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input placeholder="my_collection" value={str('collection', '')} onChange={(e) => set('collection', e.target.value)} />
        </div>
      )}
      {(operation === 'query' || operation === 'add' || operation === 'delete') && (
        <div className="field">
          <label>Collection ID <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input placeholder="resolve via get_collection" value={str('collection_id', '')} onChange={(e) => set('collection_id', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {operation === 'query' && (
        <>
          <div className="field">
            <label>Query Embeddings (JSON array)</label>
            <textarea rows={2} placeholder="[[0.1, 0.2, 0.3, …]]" value={typeof config.query_embeddings === 'object' ? JSON.stringify(config.query_embeddings) : str('query_embeddings', '')} onChange={(e) => { try { set('query_embeddings', JSON.parse(e.target.value)) } catch { set('query_embeddings', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>N Results</label>
            <input type="number" min={1} max={100} placeholder="10" value={(config['n_results'] as number | undefined) ?? ''} onChange={(e) => set('n_results', e.target.value ? parseInt(e.target.value) : undefined)} />
          </div>
        </>
      )}
      {operation === 'add' && (
        <>
          <div className="field">
            <label>IDs (JSON array)</label>
            <textarea rows={2} placeholder='["id1", "id2"]' value={typeof config.ids === 'object' ? JSON.stringify(config.ids) : str('ids', '')} onChange={(e) => { try { set('ids', JSON.parse(e.target.value)) } catch { set('ids', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>Embeddings (JSON array)</label>
            <textarea rows={2} placeholder="[[0.1, 0.2], [0.3, 0.4]]" value={typeof config.embeddings === 'object' ? JSON.stringify(config.embeddings) : str('embeddings', '')} onChange={(e) => { try { set('embeddings', JSON.parse(e.target.value)) } catch { set('embeddings', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>Documents (JSON array, optional)</label>
            <textarea rows={2} placeholder='["text one", "text two"]' value={typeof config.documents === 'object' ? JSON.stringify(config.documents) : str('documents', '')} onChange={(e) => { try { set('documents', JSON.parse(e.target.value)) } catch { set('documents', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
        </>
      )}
      {operation === 'delete' && (
        <div className="field">
          <label>IDs (JSON array)</label>
          <textarea rows={2} placeholder='["id1", "id2"]' value={typeof config.ids === 'object' ? JSON.stringify(config.ids) : str('ids', '')} onChange={(e) => { try { set('ids', JSON.parse(e.target.value)) } catch { set('ids', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Chroma vector store (REST data API). Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function MongodbConfig({ config, set, str }: ConfigProps) {
  const operation = str('operation', 'find')
  const OPERATIONS = ['find', 'findOne', 'insertOne', 'insertMany', 'updateOne', 'updateMany', 'deleteOne', 'deleteMany', 'aggregate']
  const jsonField = (key: string, label: string, placeholder: string, rows = 2) => (
    <div className="field">
      <label>{label}</label>
      <textarea rows={rows} placeholder={placeholder} value={typeof config[key] === 'object' ? JSON.stringify(config[key], null, 2) : str(key, '')} onChange={(e) => { try { set(key, JSON.parse(e.target.value)) } catch { set(key, e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
    </div>
  )
  return (
    <>
      <div className="field">
        <label>Data API URL <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="https://<region>.data.mongodb-api.com/app/<app-id>/endpoint/data/v1" value={str('data_api_url', '')} onChange={(e) => set('data_api_url', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>API Key <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Data Source (cluster) <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="Cluster0" value={str('data_source', '')} onChange={(e) => set('data_source', e.target.value)} />
      </div>
      <div className="field">
        <label>Database <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="mydb" value={str('database', '')} onChange={(e) => set('database', e.target.value)} />
      </div>
      <div className="field">
        <label>Collection <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="users" value={str('collection', '')} onChange={(e) => set('collection', e.target.value)} />
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {(operation === 'find' || operation === 'findOne' || operation === 'updateOne' || operation === 'updateMany' || operation === 'deleteOne' || operation === 'deleteMany') &&
        jsonField('filter', 'Filter (JSON)', '{"status":"active"}')}
      {operation === 'find' && (
        <>
          {jsonField('sort', 'Sort (JSON)', '{"createdAt":-1}')}
          <div className="field">
            <label>Limit</label>
            <input type="number" min={1} placeholder="100" value={(config['limit'] as number | undefined) ?? ''} onChange={(e) => set('limit', e.target.value ? parseInt(e.target.value) : undefined)} />
          </div>
        </>
      )}
      {operation === 'insertOne' && jsonField('document', 'Document (JSON)', '{"name":"Ada"}', 3)}
      {operation === 'insertMany' && jsonField('documents', 'Documents (JSON array)', '[{"name":"Ada"},{"name":"Lin"}]', 3)}
      {(operation === 'updateOne' || operation === 'updateMany') && jsonField('update', 'Update (JSON)', '{"$set":{"status":"done"}}', 3)}
      {operation === 'aggregate' && jsonField('pipeline', 'Pipeline (JSON array)', '[{"$match":{"x":1}},{"$group":{"_id":"$y"}}]', 4)}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        MongoDB Atlas Data API. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function ClickhouseConfig({ set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Host <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="https://abc.clickhouse.cloud:8443" value={str('host', '')} onChange={(e) => set('host', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>User</label>
        <input placeholder="default" value={str('user', '')} onChange={(e) => set('user', e.target.value)} />
      </div>
      <div className="field">
        <label>Password</label>
        <input type="password" value={str('password', '')} onChange={(e) => set('password', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Database</label>
        <input placeholder="default" value={str('database', '')} onChange={(e) => set('database', e.target.value)} />
      </div>
      <div className="field">
        <label>Query (SQL) <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={4} placeholder="SELECT * FROM events LIMIT 10" value={str('query', '')} onChange={(e) => set('query', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Format</label>
        <select value={str('format', 'JSON')} onChange={(e) => set('format', e.target.value)}>
          {['JSON', 'JSONEachRow', 'TabSeparated', 'CSV'].map((f) => <option key={f} value={f}>{f}</option>)}
        </select>
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        ClickHouse HTTP interface. A <code>FORMAT</code> clause is appended to SELECTs automatically. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function MilvusConfig({ config, set, str, num }: ConfigProps) {
  const operation = str('operation', 'search')
  const OPERATIONS = ['search', 'insert', 'query', 'delete']
  const jsonField = (key: string, label: string, placeholder: string, rows = 2) => (
    <div className="field">
      <label>{label}</label>
      <textarea rows={rows} placeholder={placeholder} value={typeof config[key] === 'object' ? JSON.stringify(config[key]) : str(key, '')} onChange={(e) => { try { set(key, JSON.parse(e.target.value)) } catch { set(key, e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
    </div>
  )
  return (
    <>
      <div className="field">
        <label>Host <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="https://xyz.zillizcloud.com" value={str('host', '')} onChange={(e) => set('host', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Token</label>
        <input type="password" placeholder="api-key or user:password" value={str('token', '')} onChange={(e) => set('token', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Collection <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="my_collection" value={str('collection', '')} onChange={(e) => set('collection', e.target.value)} />
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation === 'search' && (
        <>
          {jsonField('data', 'Query Vectors (JSON array of arrays)', '[[0.1, 0.2, 0.3, …]]')}
          <div className="field">
            <label>ANNS Field</label>
            <input placeholder="vector" value={str('anns_field', '')} onChange={(e) => set('anns_field', e.target.value)} />
          </div>
          <div className="field">
            <label>Limit</label>
            <input type="number" min={1} max={100} value={num('limit', 10)} onChange={(e) => set('limit', Number(e.target.value))} />
          </div>
        </>
      )}
      {operation === 'insert' && jsonField('data', 'Rows (JSON array of objects)', '[{"id":1,"vector":[0.1,0.2]}]', 4)}
      {(operation === 'query' || operation === 'delete') && (
        <div className="field">
          <label>Filter <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input placeholder='id in [1,2,3]' value={str('filter', '')} onChange={(e) => set('filter', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {(operation === 'search' || operation === 'query') && jsonField('output_fields', 'Output Fields (JSON array)', '["id","title"]')}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Milvus / Zilliz REST API v2. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function KafkaConfig({ config, set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>REST Proxy URL <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="http://localhost:8082" value={str('proxy_url', '')} onChange={(e) => set('proxy_url', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Topic <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="events" value={str('topic', '')} onChange={(e) => set('topic', e.target.value)} />
      </div>
      <div className="field">
        <label>Value (JSON or string) <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={3} placeholder='{"event":"signup"}' value={typeof config.value === 'object' ? JSON.stringify(config.value) : str('value', '')} onChange={(e) => { try { set('value', JSON.parse(e.target.value)) } catch { set('value', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div style={{ display: 'flex', gap: 8 }}>
        <div className="field" style={{ flex: 1 }}>
          <label>Key</label>
          <input value={str('key', '')} onChange={(e) => set('key', e.target.value)} />
        </div>
        <div className="field" style={{ flex: 1 }}>
          <label>Partition</label>
          <input type="number" min={0} placeholder="(auto)" value={(config['partition'] as number | undefined) ?? ''} onChange={(e) => set('partition', e.target.value ? parseInt(e.target.value) : undefined)} />
        </div>
      </div>
      <div className="field">
        <label>API Key / Secret <span style={{ color: 'var(--muted)' }}>(Confluent Cloud)</span></label>
        <div style={{ display: 'flex', gap: 8 }}>
          <input placeholder="api_key" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} style={{ flex: 1, fontFamily: 'monospace', fontSize: 12 }} />
          <input type="password" placeholder="api_secret" value={str('api_secret', '')} onChange={(e) => set('api_secret', e.target.value)} style={{ flex: 1, fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Kafka via the Confluent REST Proxy. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function RabbitmqConfig({ set, str, num }: ConfigProps) {
  const operation = str('operation', 'publish')
  const OPERATIONS = ['publish', 'get', 'list_queues']
  return (
    <>
      <div className="field">
        <label>Management API URL <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="http://localhost:15672" value={str('host', '')} onChange={(e) => set('host', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div style={{ display: 'flex', gap: 8 }}>
        <div className="field" style={{ flex: 1 }}>
          <label>Username <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input placeholder="guest" value={str('username', '')} onChange={(e) => set('username', e.target.value)} />
        </div>
        <div className="field" style={{ flex: 1 }}>
          <label>Password</label>
          <input type="password" value={str('password', '')} onChange={(e) => set('password', e.target.value)} />
        </div>
      </div>
      <div className="field">
        <label>Virtual Host</label>
        <input placeholder="/" value={str('vhost', '')} onChange={(e) => set('vhost', e.target.value)} />
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation === 'publish' && (
        <>
          <div className="field">
            <label>Exchange</label>
            <input placeholder="(default exchange)" value={str('exchange', '')} onChange={(e) => set('exchange', e.target.value)} />
          </div>
          <div className="field">
            <label>Routing Key <span style={{ color: 'var(--muted)' }}>(queue name for default exchange)</span></label>
            <input placeholder="my-queue" value={str('routing_key', '')} onChange={(e) => set('routing_key', e.target.value)} />
          </div>
          <div className="field">
            <label>Payload <span style={{ color: 'var(--danger)' }}>*</span></label>
            <textarea rows={3} value={str('payload', '')} onChange={(e) => set('payload', e.target.value)} />
          </div>
        </>
      )}
      {operation === 'get' && (
        <>
          <div className="field">
            <label>Queue <span style={{ color: 'var(--danger)' }}>*</span></label>
            <input placeholder="my-queue" value={str('queue', '')} onChange={(e) => set('queue', e.target.value)} />
          </div>
          <div className="field">
            <label>Count</label>
            <input type="number" min={1} max={100} value={num('count', 1)} onChange={(e) => set('count', Number(e.target.value))} />
          </div>
        </>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        RabbitMQ Management HTTP API. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function BedrockConfig({ config, set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Access Key ID <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input value={str('access_key_id', '')} onChange={(e) => set('access_key_id', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Secret Access Key <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" value={str('secret_access_key', '')} onChange={(e) => set('secret_access_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Region</label>
        <input placeholder="us-east-1" value={str('region', '')} onChange={(e) => set('region', e.target.value)} />
      </div>
      <div className="field">
        <label>Model ID <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="anthropic.claude-3-5-sonnet-20240620-v1:0" value={str('model_id', '')} onChange={(e) => set('model_id', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Body (model-native JSON) <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={6} placeholder={'{\n  "anthropic_version": "bedrock-2023-05-31",\n  "max_tokens": 1024,\n  "messages": [{"role":"user","content":"Hi"}]\n}'} value={typeof config.body === 'object' ? JSON.stringify(config.body, null, 2) : str('body', '')} onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        <small style={{ color: 'var(--muted)', fontSize: 10 }}>Schema depends on the model family (Anthropic / Titan / Llama / …).</small>
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        AWS Bedrock InvokeModel (SigV4-signed). Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function SqsConfig({ set, str, num }: ConfigProps) {
  const operation = str('operation', 'send')
  const OPERATIONS = ['send', 'receive', 'delete']
  return (
    <>
      <div className="field">
        <label>Access Key ID <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input value={str('access_key_id', '')} onChange={(e) => set('access_key_id', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Secret Access Key <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" value={str('secret_access_key', '')} onChange={(e) => set('secret_access_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Region</label>
        <input placeholder="us-east-1" value={str('region', '')} onChange={(e) => set('region', e.target.value)} />
      </div>
      <div className="field">
        <label>Queue URL <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="https://sqs.us-east-1.amazonaws.com/123/my-queue" value={str('queue_url', '')} onChange={(e) => set('queue_url', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation === 'send' && (
        <>
          <div className="field">
            <label>Message Body <span style={{ color: 'var(--danger)' }}>*</span></label>
            <textarea rows={3} value={str('message_body', '')} onChange={(e) => set('message_body', e.target.value)} />
          </div>
          <div className="field">
            <label>Message Group ID <span style={{ color: 'var(--muted)' }}>(FIFO)</span></label>
            <input value={str('message_group_id', '')} onChange={(e) => set('message_group_id', e.target.value)} />
          </div>
        </>
      )}
      {operation === 'receive' && (
        <div className="field">
          <label>Max Messages</label>
          <input type="number" min={1} max={10} value={num('max_messages', 1)} onChange={(e) => set('max_messages', Number(e.target.value))} />
        </div>
      )}
      {operation === 'delete' && (
        <div className="field">
          <label>Receipt Handle <span style={{ color: 'var(--danger)' }}>*</span></label>
          <textarea rows={2} value={str('receipt_handle', '')} onChange={(e) => set('receipt_handle', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        AWS SQS (SigV4-signed). Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function SnsConfig({ set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Access Key ID <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input value={str('access_key_id', '')} onChange={(e) => set('access_key_id', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Secret Access Key <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" value={str('secret_access_key', '')} onChange={(e) => set('secret_access_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Region</label>
        <input placeholder="us-east-1" value={str('region', '')} onChange={(e) => set('region', e.target.value)} />
      </div>
      <div className="field">
        <label>Topic ARN <span style={{ color: 'var(--muted)' }}>(or Target ARN / Phone)</span></label>
        <input placeholder="arn:aws:sns:us-east-1:123:my-topic" value={str('topic_arn', '')} onChange={(e) => set('topic_arn', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div style={{ display: 'flex', gap: 8 }}>
        <div className="field" style={{ flex: 1 }}>
          <label>Target ARN</label>
          <input value={str('target_arn', '')} onChange={(e) => set('target_arn', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
        <div className="field" style={{ flex: 1 }}>
          <label>Phone Number</label>
          <input placeholder="+15551234567" value={str('phone_number', '')} onChange={(e) => set('phone_number', e.target.value)} />
        </div>
      </div>
      <div className="field">
        <label>Subject</label>
        <input value={str('subject', '')} onChange={(e) => set('subject', e.target.value)} />
      </div>
      <div className="field">
        <label>Message <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={3} value={str('message', '')} onChange={(e) => set('message', e.target.value)} />
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        AWS SNS Publish (SigV4-signed). Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

function llmEndpointFields(str: ConfigProps['str'], set: ConfigProps['set'], defModel: string) {
  return (
    <>
      <div className="field">
        <label>API Key <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div style={{ display: 'flex', gap: 8 }}>
        <div className="field" style={{ flex: 2 }}>
          <label>Base URL</label>
          <input placeholder="(OpenAI-compatible default)" value={str('base_url', '')} onChange={(e) => set('base_url', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
        <div className="field" style={{ flex: 1 }}>
          <label>Model</label>
          <input placeholder={defModel} value={str('model', '')} onChange={(e) => set('model', e.target.value)} />
        </div>
      </div>
    </>
  )
}

function hostAuthFields(str: ConfigProps['str'], set: ConfigProps['set'], defPort: number, userLabel = 'Username') {
  return (
    <>
      <div style={{ display: 'flex', gap: 8 }}>
        <div className="field" style={{ flex: 2 }}>
          <label>Host <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input value={str('host', '')} onChange={(e) => set('host', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
        <div className="field" style={{ flex: 1 }}>
          <label>Port</label>
          <input placeholder={String(defPort)} value={str('port', '')} onChange={(e) => set('port', e.target.value ? parseInt(e.target.value) : undefined)} />
        </div>
      </div>
      <div style={{ display: 'flex', gap: 8 }}>
        <div className="field" style={{ flex: 1 }}>
          <label>{userLabel}</label>
          <input value={str('username', '')} onChange={(e) => set('username', e.target.value)} />
        </div>
        <div className="field" style={{ flex: 1 }}>
          <label>Password</label>
          <input type="password" value={str('password', '')} onChange={(e) => set('password', e.target.value)} />
        </div>
      </div>
    </>
  )
}

function fileOpFields(operation: string, str: ConfigProps['str'], set: ConfigProps['set']) {
  return (
    <>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {['list', 'download', 'upload', 'delete'].map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Path {operation !== 'list' && <span style={{ color: 'var(--danger)' }}>*</span>}</label>
        <input placeholder={operation === 'list' ? '(directory, optional)' : '/path/to/file'} value={str('path', '')} onChange={(e) => set('path', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      {operation === 'upload' && (
        <div className="field">
          <label>Content (base64) <span style={{ color: 'var(--danger)' }}>*</span></label>
          <textarea rows={3} value={str('content', '')} onChange={(e) => set('content', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
    </>
  )
}

export function WaitConfig({ set, str, num }: ConfigProps) {
  const mode = str('mode', 'duration')
  return (
    <>
      <div className="field">
        <label>Mode</label>
        <select value={mode} onChange={(e) => set('mode', e.target.value)}>
          <option value="duration">duration — pause for a time, then auto-resume</option>
          <option value="resume">resume — suspend until externally resumed</option>
        </select>
      </div>
      {mode === 'duration' && (
        <>
          <div className="field">
            <label>Seconds</label>
            <input type="number" min={0} value={num('seconds', 0)} onChange={(e) => set('seconds', Number(e.target.value))} />
          </div>
          <div className="field">
            <label>Until <span style={{ color: 'var(--muted)' }}>(RFC3339, overrides seconds)</span></label>
            <input placeholder="2026-07-01T09:00:00Z" value={str('until', '')} onChange={(e) => set('until', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
        </>
      )}
      {mode === 'resume' && (
        <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
          The run suspends here until resumed via <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>POST /v1/executions/&#123;id&#125;/approve</code> (the shared resume gate). Inline execution mode only.
        </p>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ resumed, mode, waited_secs }'}</code>
      </p>
    </>
  )
}

export function FtpConfig({ config, set, str }: ConfigProps) {
  const operation = str('operation', 'list')
  return (
    <>
      {hostAuthFields(str, set, 21)}
      <div className="field">
        <label style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
          <input type="checkbox" checked={config.secure === true} onChange={(e) => set('secure', e.target.checked)} />
          FTPS (explicit AUTH TLS)
        </label>
      </div>
      {fileOpFields(operation, str, set)}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Plain FTP or FTPS. list → <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ files, count }'}</code>; download → <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ content_base64, size }'}</code>
      </p>
    </>
  )
}

function sshKeyFields(str: ConfigProps['str'], set: ConfigProps['set']) {
  return (
    <>
      <div className="field">
        <label>Private Key <span style={{ color: 'var(--muted)' }}>(PEM — overrides password)</span></label>
        <textarea rows={3} placeholder="-----BEGIN OPENSSH PRIVATE KEY-----…" value={str('private_key', '')} onChange={(e) => set('private_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 11 }} />
      </div>
      <div className="field">
        <label>Key Passphrase</label>
        <input type="password" value={str('passphrase', '')} onChange={(e) => set('passphrase', e.target.value)} />
      </div>
    </>
  )
}

export function SftpConfig({ set, str }: ConfigProps) {
  const operation = str('operation', 'list')
  return (
    <>
      {hostAuthFields(str, set, 22)}
      {sshKeyFields(str, set)}
      {fileOpFields(operation, str, set)}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        SFTP over SSH (password or private key). Returns file listings / base64 content.
      </p>
    </>
  )
}

export function SshConfig({ set, str }: ConfigProps) {
  return (
    <>
      {hostAuthFields(str, set, 22)}
      {sshKeyFields(str, set)}
      <div className="field">
        <label>Command <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={2} placeholder="uname -a && df -h" value={str('command', '')} onChange={(e) => set('command', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Runs a command over SSH (password or private key). Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ stdout, stderr, exit_status }'}</code>
      </p>
    </>
  )
}

export function ImapConfig({ set, str, num }: ConfigProps) {
  const operation = str('operation', 'list_messages')
  return (
    <>
      {hostAuthFields(str, set, 993)}
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {['list_messages', 'list_mailboxes'].map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation === 'list_messages' && (
        <div style={{ display: 'flex', gap: 8 }}>
          <div className="field" style={{ flex: 2 }}>
            <label>Mailbox</label>
            <input placeholder="INBOX" value={str('mailbox', '')} onChange={(e) => set('mailbox', e.target.value)} />
          </div>
          <div className="field" style={{ flex: 1 }}>
            <label>Limit</label>
            <input type="number" min={1} max={100} value={num('limit', 10)} onChange={(e) => set('limit', Number(e.target.value))} />
          </div>
        </div>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        IMAP over TLS. Returns recent message envelopes <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ messages, count }'}</code>
      </p>
    </>
  )
}

export function MysqlConfig({ set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Connection URL <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="mysql://user:pass@host:3306/db" value={str('url', '')} onChange={(e) => set('url', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Query <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={4} placeholder="SELECT * FROM users LIMIT 10" value={str('query', '')} onChange={(e) => set('query', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        SELECT/WITH → <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ rows, count }'}</code>; DML → <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ rows_affected }'}</code>
      </p>
    </>
  )
}

export function SnowflakeConfig({ set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Account <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="myorg-myacct" value={str('account', '')} onChange={(e) => set('account', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Token <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="OAuth / key-pair JWT bearer" value={str('token', '')} onChange={(e) => set('token', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Token Type</label>
        <select value={str('token_type', 'OAUTH')} onChange={(e) => set('token_type', e.target.value)}>
          {['OAUTH', 'KEYPAIR_JWT'].map((t) => <option key={t} value={t}>{t}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Statement (SQL) <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={3} placeholder="SELECT CURRENT_VERSION()" value={str('statement', '')} onChange={(e) => set('statement', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div style={{ display: 'flex', gap: 8 }}>
        <div className="field" style={{ flex: 1 }}>
          <label>Warehouse</label>
          <input value={str('warehouse', '')} onChange={(e) => set('warehouse', e.target.value)} />
        </div>
        <div className="field" style={{ flex: 1 }}>
          <label>Database</label>
          <input value={str('database', '')} onChange={(e) => set('database', e.target.value)} />
        </div>
        <div className="field" style={{ flex: 1 }}>
          <label>Schema</label>
          <input value={str('schema', '')} onChange={(e) => set('schema', e.target.value)} />
        </div>
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Snowflake SQL API v2. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function BigqueryConfig({ config, set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Project <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="my-gcp-project" value={str('project', '')} onChange={(e) => set('project', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Access Token <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="OAuth2 bearer (bigquery scope)" value={str('access_token', '')} onChange={(e) => set('access_token', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Query (SQL) <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={4} placeholder="SELECT name FROM `proj.ds.table` LIMIT 10" value={str('query', '')} onChange={(e) => set('query', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div style={{ display: 'flex', gap: 8 }}>
        <div className="field" style={{ flex: 1 }}>
          <label>Max Results</label>
          <input type="number" min={1} placeholder="(default)" value={(config['max_results'] as number | undefined) ?? ''} onChange={(e) => set('max_results', e.target.value ? parseInt(e.target.value) : undefined)} />
        </div>
        <div className="field" style={{ flex: 1 }}>
          <label>Location</label>
          <input placeholder="US" value={str('location', '')} onChange={(e) => set('location', e.target.value)} />
        </div>
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        BigQuery jobs.query. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function HtmlExtractConfig({ set, str }: ConfigProps) {
  const mode = str('extract', 'text')
  return (
    <>
      <div className="field">
        <label>HTML <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={4} placeholder="{{http.body}}" value={str('html', '')} onChange={(e) => set('html', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>CSS Selector <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="div.article h2 a" value={str('selector', '')} onChange={(e) => set('selector', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Extract</label>
        <select value={mode} onChange={(e) => set('extract', e.target.value)}>
          {['text', 'html', 'attr'].map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      {mode === 'attr' && (
        <div className="field">
          <label>Attribute <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input placeholder="href" value={str('attr', '')} onChange={(e) => set('attr', e.target.value)} />
        </div>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ matches, count, first }'}</code>
      </p>
    </>
  )
}

export function RssConfig({ set, str, num }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Feed URL <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="https://example.com/feed.xml" value={str('url', '')} onChange={(e) => set('url', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Limit</label>
        <input type="number" min={1} max={200} value={num('limit', 20)} onChange={(e) => set('limit', Number(e.target.value))} />
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Parses RSS 2.0 / RSS 1.0 / Atom / JSON Feed. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ feed_title, items, count }'}</code>
      </p>
    </>
  )
}

export function EmbeddingConfig({ config, set, str }: ConfigProps) {
  return (
    <>
      {llmEndpointFields(str, set, 'text-embedding-3-small')}
      <div className="field">
        <label>Input (text or JSON array) <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={3} placeholder='"hello" 或 ["a","b"]' value={typeof config.input === 'object' ? JSON.stringify(config.input) : str('input', '')} onChange={(e) => { try { set('input', JSON.parse(e.target.value)) } catch { set('input', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ embeddings, model, usage }'}</code>
      </p>
    </>
  )
}

export function RerankerConfig({ config, set, str }: ConfigProps) {
  return (
    <>
      {llmEndpointFields(str, set, 'rerank-english-v3.0')}
      <div className="field">
        <label>Query <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input value={str('query', '')} onChange={(e) => set('query', e.target.value)} />
      </div>
      <div className="field">
        <label>Documents (JSON array) <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={3} placeholder='["doc one","doc two"]' value={typeof config.documents === 'object' ? JSON.stringify(config.documents) : str('documents', '')} onChange={(e) => { try { set('documents', JSON.parse(e.target.value)) } catch { set('documents', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Top N</label>
        <input type="number" min={1} placeholder="(all)" value={(config['top_n'] as number | undefined) ?? ''} onChange={(e) => set('top_n', e.target.value ? parseInt(e.target.value) : undefined)} />
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Cohere/Jina-style rerank. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function TextSplitterConfig({ set, str, num }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Text <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={4} value={str('text', '')} onChange={(e) => set('text', e.target.value)} />
      </div>
      <div style={{ display: 'flex', gap: 8 }}>
        <div className="field" style={{ flex: 1 }}>
          <label>Chunk Size</label>
          <input type="number" min={1} value={num('chunk_size', 1000)} onChange={(e) => set('chunk_size', Number(e.target.value))} />
        </div>
        <div className="field" style={{ flex: 1 }}>
          <label>Overlap</label>
          <input type="number" min={0} value={num('chunk_overlap', 200)} onChange={(e) => set('chunk_overlap', Number(e.target.value))} />
        </div>
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Char-boundary chunking (UTF-8 safe). Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ chunks, count }'}</code>
      </p>
    </>
  )
}

export function StructuredOutputConfig({ config, set, str }: ConfigProps) {
  return (
    <>
      {llmEndpointFields(str, set, 'gpt-4o-mini')}
      <div className="field">
        <label>Prompt Template <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={3} placeholder="Extract fields from: {{input.text}}" value={str('prompt_template', '')} onChange={(e) => set('prompt_template', e.target.value)} />
      </div>
      <div className="field">
        <label>JSON Schema (optional)</label>
        <textarea rows={3} placeholder='{"type":"object","properties":{…}}' value={typeof config.schema === 'object' ? JSON.stringify(config.schema, null, 2) : str('schema', '')} onChange={(e) => { try { set('schema', JSON.parse(e.target.value)) } catch { set('schema', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        LLM JSON output. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ data, raw, model }'}</code>
      </p>
    </>
  )
}

export function ClassifierConfig({ config, set, str }: ConfigProps) {
  return (
    <>
      {llmEndpointFields(str, set, 'gpt-4o-mini')}
      <div className="field">
        <label>Input <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={2} value={str('input', '')} onChange={(e) => set('input', e.target.value)} />
      </div>
      <div className="field">
        <label>Categories (JSON array) <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={2} placeholder='["positive","neutral","negative"]' value={typeof config.categories === 'object' ? JSON.stringify(config.categories) : str('categories', '')} onChange={(e) => { try { set('categories', JSON.parse(e.target.value)) } catch { set('categories', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ category, raw }'}</code>
      </p>
    </>
  )
}

export function ImageGenConfig({ set, str, num }: ConfigProps) {
  return (
    <>
      {llmEndpointFields(str, set, 'dall-e-3')}
      <div className="field">
        <label>Prompt <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={3} value={str('prompt', '')} onChange={(e) => set('prompt', e.target.value)} />
      </div>
      <div style={{ display: 'flex', gap: 8 }}>
        <div className="field" style={{ flex: 1 }}>
          <label>Size</label>
          <input placeholder="1024x1024" value={str('size', '')} onChange={(e) => set('size', e.target.value)} />
        </div>
        <div className="field" style={{ flex: 1 }}>
          <label>N</label>
          <input type="number" min={1} value={num('n', 1)} onChange={(e) => set('n', Number(e.target.value))} />
        </div>
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function SpeechToTextConfig({ set, str }: ConfigProps) {
  return (
    <>
      {llmEndpointFields(str, set, 'whisper-1')}
      <div className="field">
        <label>Audio (base64) <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={3} placeholder="SUQzBAAAA…" value={str('audio_base64', '')} onChange={(e) => set('audio_base64', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div style={{ display: 'flex', gap: 8 }}>
        <div className="field" style={{ flex: 1 }}>
          <label>Filename</label>
          <input placeholder="audio.mp3" value={str('filename', '')} onChange={(e) => set('filename', e.target.value)} />
        </div>
        <div className="field" style={{ flex: 1 }}>
          <label>Language</label>
          <input placeholder="(auto)" value={str('language', '')} onChange={(e) => set('language', e.target.value)} />
        </div>
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Whisper transcription. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, text }'}</code>
      </p>
    </>
  )
}

export function TtsConfig({ set, str }: ConfigProps) {
  return (
    <>
      {llmEndpointFields(str, set, 'tts-1')}
      <div className="field">
        <label>Input <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={3} value={str('input', '')} onChange={(e) => set('input', e.target.value)} />
      </div>
      <div style={{ display: 'flex', gap: 8 }}>
        <div className="field" style={{ flex: 1 }}>
          <label>Voice</label>
          <input placeholder="alloy" value={str('voice', '')} onChange={(e) => set('voice', e.target.value)} />
        </div>
        <div className="field" style={{ flex: 1 }}>
          <label>Format</label>
          <select value={str('format', 'mp3')} onChange={(e) => set('format', e.target.value)}>
            {['mp3', 'opus', 'aac', 'flac', 'wav'].map((f) => <option key={f} value={f}>{f}</option>)}
          </select>
        </div>
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ audio_base64, format }'}</code>
      </p>
    </>
  )
}

export function FeishuConfig({ config, set, str }: ConfigProps) {
  const msgType = str('msg_type', 'text')
  return (
    <>
      <div className="field">
        <label>Webhook URL <span style={{ color: 'var(--muted)' }}>(自定义机器人)</span></label>
        <input placeholder="https://open.feishu.cn/open-apis/bot/v2/hook/…" value={str('webhook_url', '')} onChange={(e) => set('webhook_url', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        <small style={{ color: 'var(--muted)', fontSize: 10 }}>填了 webhook 用机器人;否则走 App 模式(下方 tenant_access_token + receive_id)。</small>
      </div>
      <div className="field">
        <label>Message Type</label>
        <select value={msgType} onChange={(e) => set('msg_type', e.target.value)}>
          {['text', 'interactive'].map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      {msgType === 'text' && (
        <div className="field">
          <label>Text</label>
          <textarea rows={3} value={str('text', '')} onChange={(e) => set('text', e.target.value)} />
        </div>
      )}
      {msgType === 'interactive' && (
        <div className="field">
          <label>Card (JSON)</label>
          <textarea rows={4} placeholder='{"config":{},"elements":[…]}' value={typeof config.card === 'object' ? JSON.stringify(config.card, null, 2) : str('card', '')} onChange={(e) => { try { set('card', JSON.parse(e.target.value)) } catch { set('card', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      <div className="field">
        <label>Tenant Access Token <span style={{ color: 'var(--muted)' }}>(App 模式)</span></label>
        <input type="password" value={str('tenant_access_token', '')} onChange={(e) => set('tenant_access_token', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div style={{ display: 'flex', gap: 8 }}>
        <div className="field" style={{ flex: 2 }}>
          <label>Receive ID</label>
          <input value={str('receive_id', '')} onChange={(e) => set('receive_id', e.target.value)} />
        </div>
        <div className="field" style={{ flex: 1 }}>
          <label>ID Type</label>
          <select value={str('receive_id_type', 'open_id')} onChange={(e) => set('receive_id_type', e.target.value)}>
            {['open_id', 'user_id', 'union_id', 'email', 'chat_id'].map((t) => <option key={t} value={t}>{t}</option>)}
          </select>
        </div>
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        飞书 / Lark. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function DingtalkConfig({ set, str }: ConfigProps) {
  const msgType = str('msg_type', 'text')
  return (
    <>
      <div className="field">
        <label>Access Token <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input value={str('access_token', '')} onChange={(e) => set('access_token', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Secret <span style={{ color: 'var(--muted)' }}>(加签,可选)</span></label>
        <input type="password" value={str('secret', '')} onChange={(e) => set('secret', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Message Type</label>
        <select value={msgType} onChange={(e) => set('msg_type', e.target.value)}>
          {['text', 'markdown'].map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      {msgType === 'markdown' && (
        <div className="field">
          <label>Title</label>
          <input placeholder="notice" value={str('title', '')} onChange={(e) => set('title', e.target.value)} />
        </div>
      )}
      <div className="field">
        <label>Content <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={3} value={str('content', '')} onChange={(e) => set('content', e.target.value)} />
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        钉钉自定义机器人. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function WecomConfig({ set, str }: ConfigProps) {
  const msgType = str('msg_type', 'text')
  return (
    <>
      <div className="field">
        <label>Webhook Key <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input value={str('key', '')} onChange={(e) => set('key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        <small style={{ color: 'var(--muted)', fontSize: 10 }}>群机器人 webhook URL 里 key= 后面那段。</small>
      </div>
      <div className="field">
        <label>Message Type</label>
        <select value={msgType} onChange={(e) => set('msg_type', e.target.value)}>
          {['text', 'markdown'].map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Content <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={3} value={str('content', '')} onChange={(e) => set('content', e.target.value)} />
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        企业微信群机器人. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function ZipConfig({ config, set, str }: ConfigProps) {
  const operation = str('operation', 'zip')
  return (
    <>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {['zip', 'unzip'].map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation === 'zip' && (
        <div className="field">
          <label>Files (JSON array) <span style={{ color: 'var(--danger)' }}>*</span></label>
          <textarea rows={5} placeholder='[{"name":"a.txt","content":"hello"},{"name":"img.png","content":"<base64>","base64":true}]' value={typeof config.files === 'object' ? JSON.stringify(config.files, null, 2) : str('files', '')} onChange={(e) => { try { set('files', JSON.parse(e.target.value)) } catch { set('files', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          <small style={{ color: 'var(--muted)', fontSize: 10 }}>Each entry: {'{ name, content }'}; set base64:true if content is base64.</small>
        </div>
      )}
      {operation === 'unzip' && (
        <div className="field">
          <label>Zip (base64) <span style={{ color: 'var(--danger)' }}>*</span></label>
          <textarea rows={4} placeholder="UEsDBBQ…" value={str('zip_base64', '')} onChange={(e) => set('zip_base64', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {operation === 'zip'
          ? <>Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ zip_base64, file_count, size }'}</code></>
          : <>Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ files: [{name, content_base64, size}] }'}</code></>}
      </p>
    </>
  )
}

export function ImageConfig({ set, str }: ConfigProps) {
  const operation = str('operation', 'metadata')
  return (
    <>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {['metadata', 'resize', 'convert'].map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Image (base64) <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={4} placeholder="iVBORw0KGgo…" value={str('image_base64', '')} onChange={(e) => set('image_base64', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      {operation === 'resize' && (
        <div style={{ display: 'flex', gap: 8 }}>
          <div className="field" style={{ flex: 1 }}>
            <label>Width</label>
            <input type="number" min={1} placeholder="(auto)" value={str('width', '')} onChange={(e) => set('width', e.target.value ? parseInt(e.target.value) : undefined)} />
          </div>
          <div className="field" style={{ flex: 1 }}>
            <label>Height</label>
            <input type="number" min={1} placeholder="(auto)" value={str('height', '')} onChange={(e) => set('height', e.target.value ? parseInt(e.target.value) : undefined)} />
          </div>
        </div>
      )}
      {(operation === 'resize' || operation === 'convert') && (
        <div className="field">
          <label>Format {operation === 'convert' && <span style={{ color: 'var(--danger)' }}>*</span>}</label>
          <select value={str('format', operation === 'resize' ? 'png' : '')} onChange={(e) => set('format', e.target.value)}>
            {(operation === 'convert' ? [''] : []).concat(['png', 'jpeg', 'gif', 'bmp', 'webp']).map((f) => <option key={f} value={f}>{f || '— choose —'}</option>)}
          </select>
        </div>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {operation === 'metadata'
          ? <>Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ width, height, color }'}</code></>
          : <>Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ image_base64, format, width, height }'}</code></>}
      </p>
    </>
  )
}

export function PdfExtractConfig({ set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>PDF (base64) <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={5} placeholder="JVBERi0xLjc…" value={str('pdf_base64', '')} onChange={(e) => set('pdf_base64', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Extracts text from a PDF. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ text, char_count }'}</code>
      </p>
    </>
  )
}

export function OcrConfig({ set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Image (base64) <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={4} placeholder="iVBORw0KGgo…" value={str('image_base64', '')} onChange={(e) => set('image_base64', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Language</label>
        <input placeholder="eng" value={str('lang', '')} onChange={(e) => set('lang', e.target.value)} />
        <small style={{ color: 'var(--muted)', fontSize: 10 }}>tesseract lang code(s), e.g. eng, chi_sim, eng+fra. Requires the tesseract CLI on the executor host.</small>
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        OCR via tesseract. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ text, lang }'}</code>
      </p>
    </>
  )
}

export function HashConfig({ set, str }: ConfigProps) {
  const operation = str('operation', 'sha256')
  const OPERATIONS = ['sha256', 'sha384', 'sha512', 'hmac_sha256', 'hmac_sha384', 'hmac_sha512']
  const isHmac = operation.startsWith('hmac')
  return (
    <>
      <div className="field">
        <label>Algorithm</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Input</label>
        <textarea rows={3} placeholder="text to hash" value={str('input', '')} onChange={(e) => set('input', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      {isHmac && (
        <div className="field">
          <label>Key <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input type="password" value={str('key', '')} onChange={(e) => set('key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      <div className="field">
        <label>Encoding</label>
        <select value={str('encoding', 'hex')} onChange={(e) => set('encoding', e.target.value)}>
          {['hex', 'base64', 'base64url'].map((enc) => <option key={enc} value={enc}>{enc}</option>)}
        </select>
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Hash / HMAC digest. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ hash, algorithm, encoding }'}</code>
      </p>
    </>
  )
}

export function JwtConfig({ config, set, str }: ConfigProps) {
  const operation = str('operation', 'sign')
  return (
    <>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {['sign', 'verify'].map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Algorithm</label>
        <select value={str('algorithm', 'HS256')} onChange={(e) => set('algorithm', e.target.value)}>
          {['HS256', 'HS384', 'HS512'].map((a) => <option key={a} value={a}>{a}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Secret <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" value={str('secret', '')} onChange={(e) => set('secret', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      {operation === 'sign' && (
        <>
          <div className="field">
            <label>Payload (JSON object)</label>
            <textarea rows={3} placeholder='{"sub":"123","name":"Ada"}' value={typeof config.payload === 'object' ? JSON.stringify(config.payload, null, 2) : str('payload', '')} onChange={(e) => { try { set('payload', JSON.parse(e.target.value)) } catch { set('payload', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>Expires In (seconds)</label>
            <input type="number" min={1} placeholder="3600" value={(config['expires_in_secs'] as number | undefined) ?? ''} onChange={(e) => set('expires_in_secs', e.target.value ? parseInt(e.target.value) : undefined)} />
          </div>
        </>
      )}
      {operation === 'verify' && (
        <div className="field">
          <label>Token <span style={{ color: 'var(--danger)' }}>*</span></label>
          <textarea rows={3} placeholder="eyJhbGciOi…" value={str('token', '')} onChange={(e) => set('token', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {operation === 'sign'
          ? <>Signs an HMAC JWT. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ token }'}</code></>
          : <>Verifies signature + exp. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ valid, payload }'}</code></>}
      </p>
    </>
  )
}

export function GcsConfig({ set, str }: ConfigProps) {
  const operation = str('operation', 'list')
  const OPERATIONS = ['list', 'get', 'download', 'upload', 'delete']
  return (
    <>
      <div className="field">
        <label>Access Token <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="OAuth2 bearer (storage scope)" value={str('access_token', '')} onChange={(e) => set('access_token', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Bucket <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="my-bucket" value={str('bucket', '')} onChange={(e) => set('bucket', e.target.value)} />
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation === 'list' && (
        <div className="field">
          <label>Prefix</label>
          <input placeholder="folder/" value={str('prefix', '')} onChange={(e) => set('prefix', e.target.value)} />
        </div>
      )}
      {operation !== 'list' && (
        <div className="field">
          <label>Object <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input placeholder="path/to/file.txt" value={str('object', '')} onChange={(e) => set('object', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {operation === 'upload' && (
        <>
          <div className="field">
            <label>Content</label>
            <textarea rows={3} value={str('content', '')} onChange={(e) => set('content', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>Content Type</label>
            <input placeholder="text/plain" value={str('content_type', '')} onChange={(e) => set('content_type', e.target.value)} />
          </div>
        </>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Google Cloud Storage JSON API. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function AzureBlobConfig({ set, str }: ConfigProps) {
  const operation = str('operation', 'list')
  const OPERATIONS = ['list', 'get', 'put', 'delete']
  return (
    <>
      <div className="field">
        <label>Account <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="mystorageacct" value={str('account', '')} onChange={(e) => set('account', e.target.value)} />
      </div>
      <div className="field">
        <label>Container <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="my-container" value={str('container', '')} onChange={(e) => set('container', e.target.value)} />
      </div>
      <div className="field">
        <label>SAS Token <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="sv=2022-11-02&ss=b&srt=…" value={str('sas_token', '')} onChange={(e) => set('sas_token', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation !== 'list' && (
        <div className="field">
          <label>Blob <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input placeholder="path/to/blob.txt" value={str('blob', '')} onChange={(e) => set('blob', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {operation === 'put' && (
        <>
          <div className="field">
            <label>Content</label>
            <textarea rows={3} value={str('content', '')} onChange={(e) => set('content', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>Content Type</label>
            <input placeholder="application/octet-stream" value={str('content_type', '')} onChange={(e) => set('content_type', e.target.value)} />
          </div>
        </>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Azure Blob Storage REST API (SAS auth). Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function CloudinaryConfig({ set, str }: ConfigProps) {
  const operation = str('operation', 'upload')
  const OPERATIONS = ['upload', 'transform_url', 'get_resource', 'delete']
  return (
    <>
      <div className="field">
        <label>Cloud Name <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="my-cloud" value={str('cloud_name', '')} onChange={(e) => set('cloud_name', e.target.value)} />
      </div>
      <div className="field">
        <label>API Key</label>
        <input placeholder="123456789012345" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>API Secret</label>
        <input type="password" value={str('api_secret', '')} onChange={(e) => set('api_secret', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation === 'upload' && (
        <>
          <div className="field">
            <label>File URL or Base64</label>
            <input placeholder="https://… or data:image/png;base64,…" value={str('file', '')} onChange={(e) => set('file', e.target.value)} />
          </div>
          <div className="field">
            <label>Public ID</label>
            <input placeholder="my-image" value={str('public_id', '')} onChange={(e) => set('public_id', e.target.value)} />
          </div>
        </>
      )}
      {(operation === 'transform_url' || operation === 'get_resource' || operation === 'delete') && (
        <div className="field">
          <label>Public ID <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input placeholder="my-image" value={str('public_id', '')} onChange={(e) => set('public_id', e.target.value)} />
        </div>
      )}
      {operation === 'transform_url' && (
        <div className="field">
          <label>Transformation</label>
          <input placeholder="w_300,h_300,c_fill" value={str('transformation', '')} onChange={(e) => set('transformation', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Upload, transform, and manage images &amp; videos. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function GcalConfig({ set, str }: ConfigProps) {
  const operation = str('operation', 'list_events')
  const OPERATIONS = ['list_calendars', 'list_events', 'get_event', 'create_event', 'delete_event']
  return (
    <>
      <div className="field">
        <label>Access Token <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="ya29.…" value={str('access_token', '')} onChange={(e) => set('access_token', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation !== 'list_calendars' && (
        <div className="field">
          <label>Calendar ID</label>
          <input placeholder="primary" value={str('calendar_id', '')} onChange={(e) => set('calendar_id', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {(operation === 'get_event' || operation === 'delete_event') && (
        <div className="field">
          <label>Event ID <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input value={str('event_id', '')} onChange={(e) => set('event_id', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {operation === 'list_events' && (
        <div className="field">
          <label>Search Query</label>
          <input placeholder="Meeting" value={str('query', '')} onChange={(e) => set('query', e.target.value)} />
        </div>
      )}
      {operation === 'create_event' && (
        <>
          <div className="field">
            <label>Summary <span style={{ color: 'var(--danger)' }}>*</span></label>
            <input placeholder="Team Standup" value={str('summary', '')} onChange={(e) => set('summary', e.target.value)} />
          </div>
          <div className="field">
            <label>Description</label>
            <input placeholder="Daily sync" value={str('description', '')} onChange={(e) => set('description', e.target.value)} />
          </div>
          <div className="field">
            <label>Start Time <span style={{ color: 'var(--danger)' }}>*</span></label>
            <input placeholder="2025-01-15T10:00:00" value={str('start_time', '')} onChange={(e) => set('start_time', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>End Time</label>
            <input placeholder="2025-01-15T11:00:00" value={str('end_time', '')} onChange={(e) => set('end_time', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>Timezone</label>
            <input placeholder="America/New_York" value={str('timezone', '')} onChange={(e) => set('timezone', e.target.value)} />
          </div>
        </>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Uses OAuth2 access token. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function DocusignConfig({ config, set, str }: ConfigProps) {
  const operation = str('operation', 'list_envelopes')
  const OPERATIONS = ['list_envelopes', 'get_envelope', 'create_envelope', 'void_envelope']
  return (
    <>
      <div className="field">
        <label>Access Token <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" value={str('access_token', '')} onChange={(e) => set('access_token', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Account ID <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx" value={str('account_id', '')} onChange={(e) => set('account_id', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Base URL</label>
        <input placeholder="https://demo.docusign.net/restapi" value={str('base_url', '')} onChange={(e) => set('base_url', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        <small style={{ color: 'var(--muted)', fontSize: 10 }}>Use demo.docusign.net for sandbox, www.docusign.net for production</small>
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation === 'list_envelopes' && (
        <div className="field">
          <label>From Date</label>
          <input placeholder="2024-01-01" value={str('from_date', '')} onChange={(e) => set('from_date', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {(operation === 'get_envelope' || operation === 'void_envelope') && (
        <div className="field">
          <label>Envelope ID <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input value={str('envelope_id', '')} onChange={(e) => set('envelope_id', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {operation === 'void_envelope' && (
        <div className="field">
          <label>Void Reason</label>
          <input placeholder="Voided via workflow" value={str('void_reason', '')} onChange={(e) => set('void_reason', e.target.value)} />
        </div>
      )}
      {operation === 'create_envelope' && (
        <div className="field">
          <label>Envelope Body (JSON)</label>
          <textarea rows={5} placeholder='{"emailSubject":"Please sign","documents":[...],"recipients":{...},"status":"sent"}' value={typeof config.body === 'object' ? JSON.stringify(config.body, null, 2) : str('body', '')} onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        E-signature via DocuSign eSign REST API v2.1. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function XeroConfig({ config, set, str }: ConfigProps) {
  const METHOD_OPTIONS = ['GET', 'POST', 'PUT', 'DELETE']
  return (
    <>
      <div className="field">
        <label>Access Token <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" value={str('access_token', '')} onChange={(e) => set('access_token', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Tenant ID <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx" value={str('tenant_id', '')} onChange={(e) => set('tenant_id', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Endpoint</label>
        <input placeholder="/Contacts" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        <small style={{ color: 'var(--muted)', fontSize: 10 }}>Relative path: /Contacts, /Invoices, /Accounts, /Payments…</small>
      </div>
      <div className="field">
        <label>Method</label>
        <select value={str('method', 'GET')} onChange={(e) => set('method', e.target.value)}>
          {METHOD_OPTIONS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      {!['GET', 'DELETE'].includes(str('method', 'GET')) && (
        <div className="field">
          <label>Body (JSON)</label>
          <textarea rows={4} placeholder='{"Contacts":[{"Name":"Acme Corp"}]}' value={typeof config.body === 'object' ? JSON.stringify(config.body, null, 2) : str('body', '')} onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Base URL: api.xero.com/api.xro/2.0. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function CalendlyConfig({ set, str }: ConfigProps) {
  const operation = str('operation', 'get_current_user')
  const OPERATIONS = ['get_current_user', 'list_event_types', 'list_scheduled_events', 'get_scheduled_event', 'cancel_event']
  const needsUserUri = ['list_event_types', 'list_scheduled_events'].includes(operation)
  const needsEventUuid = ['get_scheduled_event', 'cancel_event'].includes(operation)
  return (
    <>
      <div className="field">
        <label>API Key (PAT) <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="eyJra…" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {needsUserUri && (
        <div className="field">
          <label>User URI <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input placeholder="https://api.calendly.com/users/xxxx" value={str('user_uri', '')} onChange={(e) => set('user_uri', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          <small style={{ color: 'var(--muted)', fontSize: 10 }}>Found in get_current_user response as resource.uri</small>
        </div>
      )}
      {operation === 'list_scheduled_events' && (
        <div className="field">
          <label>Status Filter</label>
          <select value={str('status', 'active')} onChange={(e) => set('status', e.target.value)}>
            <option value="active">active</option>
            <option value="canceled">canceled</option>
          </select>
        </div>
      )}
      {needsEventUuid && (
        <div className="field">
          <label>Event UUID <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input placeholder="xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx" value={str('event_uuid', '')} onChange={(e) => set('event_uuid', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {operation === 'cancel_event' && (
        <div className="field">
          <label>Cancellation Reason</label>
          <input placeholder="Rescheduling" value={str('reason', '')} onChange={(e) => set('reason', e.target.value)} />
        </div>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Calendly v2 API — personal access token auth. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function ApifyConfig({ config, set, str }: ConfigProps) {
  const operation = str('operation', 'run_actor')
  const OPERATIONS = ['run_actor', 'get_run', 'get_dataset_items', 'list_actors']
  return (
    <>
      <div className="field">
        <label>API Token <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="apify_api_…" value={str('api_token', '')} onChange={(e) => set('api_token', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation === 'run_actor' && (
        <>
          <div className="field">
            <label>Actor ID <span style={{ color: 'var(--danger)' }}>*</span></label>
            <input placeholder="apify/web-scraper" value={str('actor_id', '')} onChange={(e) => set('actor_id', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>Input (JSON)</label>
            <textarea rows={4} placeholder='{"startUrls":[{"url":"https://example.com"}]}' value={typeof config.input === 'object' ? JSON.stringify(config.input, null, 2) : str('input', '')} onChange={(e) => { try { set('input', JSON.parse(e.target.value)) } catch { set('input', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
        </>
      )}
      {operation === 'get_run' && (
        <div className="field">
          <label>Run ID <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input value={str('run_id', '')} onChange={(e) => set('run_id', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {operation === 'get_dataset_items' && (
        <>
          <div className="field">
            <label>Dataset ID <span style={{ color: 'var(--danger)' }}>*</span></label>
            <input value={str('dataset_id', '')} onChange={(e) => set('dataset_id', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>Limit</label>
            <input type="number" min={1} max={1000} placeholder="100" value={(config['limit'] as number | undefined) ?? ''} onChange={(e) => set('limit', e.target.value ? parseInt(e.target.value) : undefined)} />
          </div>
        </>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Run web scrapers and automation actors. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function GanalyticsConfig({ config, set, str }: ConfigProps) {
  const operation = str('operation', 'run_report')
  const OPERATIONS = ['run_report', 'run_realtime_report', 'get_metadata']
  return (
    <>
      <div className="field">
        <label>Access Token <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="ya29.…" value={str('access_token', '')} onChange={(e) => set('access_token', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Property ID <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="123456789" value={str('property_id', '')} onChange={(e) => set('property_id', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        <small style={{ color: 'var(--muted)', fontSize: 10 }}>GA4 property ID (numeric, without "properties/" prefix)</small>
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {(operation === 'run_report' || operation === 'run_realtime_report') && (
        <>
          {operation === 'run_report' && (
            <div className="field">
              <label>Date Ranges (JSON)</label>
              <textarea rows={2} placeholder='[{"startDate":"7daysAgo","endDate":"today"}]' value={typeof config.date_ranges === 'object' ? JSON.stringify(config.date_ranges) : str('date_ranges', '')} onChange={(e) => { try { set('date_ranges', JSON.parse(e.target.value)) } catch { set('date_ranges', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
            </div>
          )}
          <div className="field">
            <label>Dimensions (JSON)</label>
            <textarea rows={2} placeholder='[{"name":"date"}]' value={typeof config.dimensions === 'object' ? JSON.stringify(config.dimensions) : str('dimensions', '')} onChange={(e) => { try { set('dimensions', JSON.parse(e.target.value)) } catch { set('dimensions', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>Metrics (JSON)</label>
            <textarea rows={2} placeholder='[{"name":"sessions"}]' value={typeof config.metrics === 'object' ? JSON.stringify(config.metrics) : str('metrics', '')} onChange={(e) => { try { set('metrics', JSON.parse(e.target.value)) } catch { set('metrics', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
        </>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        GA4 Data API — OAuth2 access token. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function NeonConfig({ set, str }: ConfigProps) {
  const operation = str('operation', 'list_projects')
  const OPERATIONS = ['list_projects', 'get_project', 'create_project', 'list_branches']
  const needsProjectId = ['get_project', 'list_branches'].includes(operation)
  return (
    <>
      <div className="field">
        <label>API Key <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="neon_api_…" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {needsProjectId && (
        <div className="field">
          <label>Project ID <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input placeholder="proud-river-123456" value={str('project_id', '')} onChange={(e) => set('project_id', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {operation === 'create_project' && (
        <div className="field">
          <label>Project Name</label>
          <input placeholder="my-project" value={str('name', '')} onChange={(e) => set('name', e.target.value)} />
        </div>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Neon serverless Postgres console API. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function CopperConfig({ config, set, str }: ConfigProps) {
  const operation = str('operation', 'list')
  const OPERATIONS = ['list', 'get', 'create', 'update', 'delete']
  const RESOURCES = ['people', 'leads', 'opportunities', 'companies', 'tasks', 'activities']
  const needsId = ['get', 'update', 'delete'].includes(operation)
  return (
    <>
      <div className="field">
        <label>API Key <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>User Email <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="email" placeholder="you@company.com" value={str('user_email', '')} onChange={(e) => set('user_email', e.target.value)} />
      </div>
      <div className="field">
        <label>Resource</label>
        <select value={str('resource', 'people')} onChange={(e) => set('resource', e.target.value)}>
          {RESOURCES.map((r) => <option key={r} value={r}>{r}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {needsId && (
        <div className="field">
          <label>Record ID <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input placeholder="12345678" value={str('record_id', '')} onChange={(e) => set('record_id', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {operation === 'list' && (
        <div className="field">
          <label>Filter (JSON)</label>
          <textarea rows={3} placeholder='{"name":"Acme"}' value={typeof config.filter === 'object' ? JSON.stringify(config.filter, null, 2) : str('filter', '')} onChange={(e) => { try { set('filter', JSON.parse(e.target.value)) } catch { set('filter', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {(operation === 'create' || operation === 'update') && (
        <div className="field">
          <label>Body (JSON)</label>
          <textarea rows={4} placeholder='{"name":"Acme Corp","email":[{"email":"contact@acme.com"}]}' value={typeof config.body === 'object' ? JSON.stringify(config.body, null, 2) : str('body', '')} onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Copper CRM (Google Workspace-native). Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

// ── 国内大模型通用字段组件 ────────────────────────────────────────────────────
function CnLlmCommonFields({ str, set, num }: Pick<ConfigProps, 'str' | 'set' | 'num'>) {
  return (
    <>
      <div className="field">
        <label>System Prompt <span style={{ color: 'var(--muted)' }}>(optional)</span></label>
        <textarea rows={2} placeholder="You are a helpful assistant." value={str('system_prompt', '')} onChange={(e) => set('system_prompt', e.target.value)} />
      </div>
      <div className="field">
        <label>Prompt Template *</label>
        <textarea rows={4} placeholder="{{input.text}}" value={str('prompt_template', '')} onChange={(e) => set('prompt_template', e.target.value)} />
      </div>
      <div style={{ display: 'flex', gap: 8 }}>
        <div className="field" style={{ flex: 1 }}>
          <label>Max Tokens</label>
          <input type="number" min={64} max={32768} value={num('max_tokens', 1024)} onChange={(e) => set('max_tokens', Number(e.target.value))} />
        </div>
        <div className="field" style={{ flex: 1 }}>
          <label>Temperature</label>
          <input type="number" step={0.1} min={0} max={2} value={num('temperature', 0.7)} onChange={(e) => set('temperature', Number(e.target.value))} />
        </div>
      </div>
    </>
  )
}

export function VertexConfig({ set, str, num }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Access Token *</label>
        <input type="password" placeholder="OAuth2 bearer (cloud-platform scope)" value={str('access_token', '')} onChange={(e) => set('access_token', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Project *</label>
        <input placeholder="my-gcp-project" value={str('project', '')} onChange={(e) => set('project', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div style={{ display: 'flex', gap: 8 }}>
        <div className="field" style={{ flex: 1 }}>
          <label>Location</label>
          <input placeholder="us-central1" value={str('location', '')} onChange={(e) => set('location', e.target.value)} />
        </div>
        <div className="field" style={{ flex: 1 }}>
          <label>Model</label>
          <input placeholder="gemini-1.5-flash" value={str('model', '')} onChange={(e) => set('model', e.target.value)} />
        </div>
      </div>
      <CnLlmCommonFields str={str} set={set} num={num} />
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Vertex AI generateContent. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ content, model, usage }'}</code>
      </p>
    </>
  )
}

export function AzureOpenaiConfig({ set, str, num }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Endpoint *</label>
        <input placeholder="https://my-res.openai.azure.com" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Deployment *</label>
        <input placeholder="gpt-4o" value={str('deployment', '')} onChange={(e) => set('deployment', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>API Version</label>
        <input placeholder="2024-02-01" value={str('api_version', '')} onChange={(e) => set('api_version', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>API Key *</label>
        <input type="password" placeholder="azure key" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <CnLlmCommonFields str={str} set={set} num={num} />
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Azure OpenAI (deployment-based). Returns <code>{'{ content, model, usage }'}</code>
      </p>
    </>
  )
}

export function GrokConfig({ set, str, num }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Model</label>
        <input placeholder="grok-2-latest" value={str('model', 'grok-2-latest')} onChange={(e) => set('model', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>API Key *</label>
        <input type="password" placeholder="xai-..." value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <CnLlmCommonFields str={str} set={set} num={num} />
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        xAI Grok (OpenAI-compatible). Returns <code>{'{ content, model, usage }'}</code>
      </p>
    </>
  )
}

export function OllamaConfig({ set, str, num }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Base URL</label>
        <input placeholder="http://localhost:11434/v1/chat/completions" value={str('base_url', '')} onChange={(e) => set('base_url', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Model</label>
        <input placeholder="llama3.2" value={str('model', 'llama3.2')} onChange={(e) => set('model', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>API Key</label>
        <input type="password" placeholder="(optional, ignored by local Ollama)" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <CnLlmCommonFields str={str} set={set} num={num} />
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Self-hosted Ollama (OpenAI-compatible). Returns <code>{'{ content, model, usage }'}</code>
      </p>
    </>
  )
}

export function DeepseekConfig({ set, str, num }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Model</label>
        <select value={str('model', 'deepseek-chat')} onChange={(e) => set('model', e.target.value)}>
          <option value="deepseek-chat">deepseek-chat (V3)</option>
          <option value="deepseek-reasoner">deepseek-reasoner (R1)</option>
        </select>
      </div>
      <div className="field">
        <label>API Key *</label>
        <input type="password" placeholder="sk-..." value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <CnLlmCommonFields str={str} set={set} num={num} />
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        DeepSeek — 高性价比推理模型。返回 <code>{'{ content, model, usage }'}</code>
      </p>
    </>
  )
}

export function QwenConfig({ set, str, num }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Model</label>
        <select value={str('model', 'qwen-max')} onChange={(e) => set('model', e.target.value)}>
          <option value="qwen-max">qwen-max</option>
          <option value="qwen-plus">qwen-plus</option>
          <option value="qwen-turbo">qwen-turbo</option>
          <option value="qwen-long">qwen-long (128K)</option>
        </select>
      </div>
      <div className="field">
        <label>API Key (DashScope) *</label>
        <input type="password" placeholder="sk-..." value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <CnLlmCommonFields str={str} set={set} num={num} />
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        阿里云通义千问（DashScope）。返回 <code>{'{ content, model, usage }'}</code>
      </p>
    </>
  )
}

export function ZhipuConfig({ set, str, num }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Model</label>
        <select value={str('model', 'glm-4')} onChange={(e) => set('model', e.target.value)}>
          <option value="glm-4">glm-4</option>
          <option value="glm-4-air">glm-4-air (fast)</option>
          <option value="glm-4-flash">glm-4-flash (free)</option>
          <option value="glm-3-turbo">glm-3-turbo</option>
        </select>
      </div>
      <div className="field">
        <label>API Key *</label>
        <input type="password" placeholder="智谱 API Key" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <CnLlmCommonFields str={str} set={set} num={num} />
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        智谱 AI（GLM）。返回 <code>{'{ content, model, usage }'}</code>
      </p>
    </>
  )
}

export function MoonshotConfig({ set, str, num }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Model</label>
        <select value={str('model', 'moonshot-v1-8k')} onChange={(e) => set('model', e.target.value)}>
          <option value="moonshot-v1-8k">moonshot-v1-8k</option>
          <option value="moonshot-v1-32k">moonshot-v1-32k</option>
          <option value="moonshot-v1-128k">moonshot-v1-128k</option>
        </select>
      </div>
      <div className="field">
        <label>API Key *</label>
        <input type="password" placeholder="sk-..." value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <CnLlmCommonFields str={str} set={set} num={num} />
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        月之暗面（Kimi）。返回 <code>{'{ content, model, usage }'}</code>
      </p>
    </>
  )
}

export function DoubaoConfig({ set, str, num }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>API Key (火山引擎) *</label>
        <input type="password" placeholder="火山方舟 API Key" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <div className="field">
        <label>Endpoint ID *</label>
        <input placeholder="ep-xxxxxxxx" value={str('endpoint_id', '')} onChange={(e) => set('endpoint_id', e.target.value)} />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>豆包使用推理接入点 ID 而非模型名</span>
      </div>
      <CnLlmCommonFields str={str} set={set} num={num} />
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        字节跳动豆包（火山引擎方舟）。返回 <code>{'{ content, model, usage }'}</code>
      </p>
    </>
  )
}

export function MinimaxConfig({ set, str, num }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Model</label>
        <select value={str('model', 'abab6.5s-chat')} onChange={(e) => set('model', e.target.value)}>
          <option value="abab6.5s-chat">abab6.5s-chat</option>
          <option value="abab6.5-chat">abab6.5-chat</option>
          <option value="abab5.5s-chat">abab5.5s-chat</option>
        </select>
      </div>
      <div className="field">
        <label>API Key *</label>
        <input type="password" placeholder="MiniMax API Key" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <div className="field">
        <label>Group ID *</label>
        <input placeholder="MiniMax Group ID" value={str('group_id', '')} onChange={(e) => set('group_id', e.target.value)} />
      </div>
      <CnLlmCommonFields str={str} set={set} num={num} />
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        MiniMax。返回 <code>{'{ content, model, usage }'}</code>
      </p>
    </>
  )
}

export function ErnieConfig({ set, str, num }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Model</label>
        <select value={str('model', 'ernie-4.0-8k')} onChange={(e) => set('model', e.target.value)}>
          <option value="ernie-4.0-8k">ernie-4.0-8k</option>
          <option value="ernie-3.5-8k">ernie-3.5-8k</option>
          <option value="ernie-speed-128k">ernie-speed-128k</option>
        </select>
      </div>
      <div className="field">
        <label>API Key (Client ID) *</label>
        <input type="password" placeholder="百度云 Client ID" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <div className="field">
        <label>Secret Key (Client Secret) *</label>
        <input type="password" placeholder="百度云 Client Secret" value={str('secret_key', '')} onChange={(e) => set('secret_key', e.target.value)} />
      </div>
      <CnLlmCommonFields str={str} set={set} num={num} />
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        百度文心一言（自动 OAuth2 换取 access_token）。返回 <code>{'{ content, model, usage }'}</code>
      </p>
    </>
  )
}

export function HunyuanConfig({ set, str, num }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Model</label>
        <select value={str('model', 'hunyuan-standard')} onChange={(e) => set('model', e.target.value)}>
          <option value="hunyuan-standard">hunyuan-standard</option>
          <option value="hunyuan-turbo">hunyuan-turbo</option>
          <option value="hunyuan-lite">hunyuan-lite</option>
        </select>
      </div>
      <div className="field">
        <label>API Key *</label>
        <input type="password" placeholder="腾讯混元 API Key" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <CnLlmCommonFields str={str} set={set} num={num} />
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        腾讯混元（OpenAI 兼容接口）。返回 <code>{'{ content, model, usage }'}</code>
      </p>
    </>
  )
}
