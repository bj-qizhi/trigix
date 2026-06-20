// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import type { ConfigProps } from '../types'
import { fl } from '../i18nLabels'

export function TwitchConfig({ config, set, str }: ConfigProps) {
  const method = str('method', 'GET')
  const METHODS = ['GET', 'POST', 'PUT', 'DELETE']
  return (
    <>
      <div className="field">
        <label>{fl("Client ID")}</label>
        <input placeholder="Twitch Client ID" value={str('client_id', '')} onChange={(e) => set('client_id', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Access Token (OAuth)")}</label>
        <input type="password" placeholder="OAuth access token" value={str('access_token', '')} onChange={(e) => set('access_token', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Method")}</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>{fl("Endpoint")}</label>
        <input placeholder="/helix/streams" value={str('endpoint', '/helix/streams')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT'].includes(method) && (
        <div className="field">
          <label>{fl("Body (JSON)")}</label>
          <textarea rows={4}
            placeholder='{}'
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
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
        <label>{fl("Personal Access Token")}</label>
        <input type="password" placeholder="figd_…" value={str('token', '')} onChange={(e) => set('token', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Method")}</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>{fl("Endpoint")}</label>
        <input placeholder="/v1/files/FILE_KEY" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Common endpoints:")} <code style={{ background: 'var(--panel)', padding: '1px 3px', borderRadius: 3 }}>{fl("/v1/files/KEY")}</code>, <code style={{ background: 'var(--panel)', padding: '1px 3px', borderRadius: 3 }}>{fl("/v1/teams/TEAM_ID/projects")}</code>
      </p>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

// ── Slice 276: Dropbox ────────────────────────────────────────────────────────

export function CloudflareConfig({ config, set, str }: ConfigProps) {
  const method = str('method', 'GET')
  const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE']
  return (
    <>
      <div className="field">
        <label>{fl("API Token")}</label>
        <input type="password" placeholder="Cloudflare API token" value={str('api_token', '')} onChange={(e) => set('api_token', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Method")}</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>{fl("Endpoint")}</label>
        <input placeholder="/zones/ZONE_ID/dns_records" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT', 'PATCH'].includes(method) && (
        <div className="field">
          <label>{fl("Body (JSON)")}</label>
          <textarea rows={4}
            placeholder='{"type": "A", "name": "api.example.com", "content": "1.2.3.4", "ttl": 300}'
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body, success }'}</code>
      </p>
    </>
  )
}

// ── Slice 278: Box ─────────────────────────────────────────────────────────────

export function OktaConfig({ config, set, str }: ConfigProps) {
  const method = str('method', 'GET')
  const METHODS = ['GET', 'POST', 'PUT', 'DELETE']
  const tokenType = str('token_type', 'SSWS')
  return (
    <>
      <div className="field">
        <label>{fl("Okta Domain")}</label>
        <input placeholder="https://your-org.okta.com" value={str('domain', '')} onChange={(e) => set('domain', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Token Type")}</label>
        <select value={tokenType} onChange={(e) => set('token_type', e.target.value)}>
          <option value="SSWS">{fl("SSWS (API Token)")}</option>
          <option value="BEARER">{fl("Bearer (OAuth)")}</option>
        </select>
      </div>
      <div className="field">
        <label>{fl("Token")}</label>
        <input type="password" placeholder="Okta API token or OAuth token" value={str('token', '')} onChange={(e) => set('token', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Method")}</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>{fl("Endpoint")}</label>
        <input placeholder="/api/v1/users" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT'].includes(method) && (
        <div className="field">
          <label>{fl("Body (JSON)")}</label>
          <textarea rows={4}
            placeholder='{"profile": {"firstName": "Jane", "email": "jane@example.com"}}'
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
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
        <label>{fl("Access Token")}</label>
        <input type="password" placeholder="Zoom OAuth access token" value={str('token', '')} onChange={(e) => set('token', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Method")}</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>{fl("Endpoint")}</label>
        <input placeholder="/v2/users/me/meetings" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT', 'PATCH'].includes(method) && (
        <div className="field">
          <label>{fl("Body (JSON)")}</label>
          <textarea rows={4}
            placeholder='{"topic": "My Meeting", "type": 2, "duration": 60}'
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
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
        <label>{fl("Access Token")}</label>
        <input type="password" placeholder="Spotify OAuth access token" value={str('token', '')} onChange={(e) => set('token', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Method")}</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>{fl("Endpoint")}</label>
        <input placeholder="/v1/me/player/currently-playing" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT'].includes(method) && (
        <div className="field">
          <label>{fl("Body (JSON)")}</label>
          <textarea rows={4}
            placeholder='{"uris": ["spotify:track:4iV5W9uYEdYUVa79Axb7Rh"]}'
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>{fl(". 204 No Content returns null body.")}
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
        <label>{fl("Personal Token")}</label>
        <input type="password" placeholder="Typeform personal token" value={str('token', '')} onChange={(e) => set('token', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Method")}</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>{fl("Endpoint")}</label>
        <input placeholder="/forms/FORM_ID/responses" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT', 'PATCH'].includes(method) && (
        <div className="field">
          <label>{fl("Body (JSON)")}</label>
          <textarea rows={4}
            placeholder='{"page_size": 25}'
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
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
        <label>{fl("API Token")}</label>
        <input type="password" placeholder="Webflow API token or OAuth token" value={str('token', '')} onChange={(e) => set('token', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Method")}</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>{fl("Endpoint")}</label>
        <input placeholder="/sites" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT', 'PATCH'].includes(method) && (
        <div className="field">
          <label>{fl("Body (JSON)")}</label>
          <textarea rows={4}
            placeholder='{"fieldData": {"name": "My Page", "slug": "my-page"}}'
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
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
        <label>{fl("Access Token")}</label>
        <input type="password" placeholder="Intercom access token" value={str('token', '')} onChange={(e) => set('token', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Method")}</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>{fl("Endpoint")}</label>
        <input placeholder="/contacts" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT'].includes(method) && (
        <div className="field">
          <label>{fl("Body (JSON)")}</label>
          <textarea rows={4}
            placeholder='{"email": "user@example.com", "name": "Jane Doe"}'
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
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
        <label>{fl("API Token")}</label>
        <input type="password" placeholder="Pipedrive API token" value={str('api_token', '')} onChange={(e) => set('api_token', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Method")}</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>{fl("Endpoint")}</label>
        <input placeholder="/deals" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT'].includes(method) && (
        <div className="field">
          <label>{fl("Body (JSON)")}</label>
          <textarea rows={4}
            placeholder='{"title": "New Deal", "value": 5000, "currency": "USD"}'
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Token is appended as")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{fl("?api_token=…")}</code> {fl("query param.\n        Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
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
        <label>{fl("API Key")}</label>
        <input placeholder="Trello API key" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Token")}</label>
        <input type="password" placeholder="Trello token" value={str('token', '')} onChange={(e) => set('token', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Method")}</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>{fl("Endpoint")}</label>
        <input placeholder="/boards/BOARD_ID" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT'].includes(method) && (
        <div className="field">
          <label>{fl("Body (JSON)")}</label>
          <textarea rows={4}
            placeholder='{"name": "New Card", "idList": "LIST_ID"}'
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Auth via")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>?key=…&token=…</code> {fl("query params.\n        Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

// ── Slice 287: Monday ──────────────────────────────────────────────────────────

export function MondayConfig({ config, set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>{fl("API Token")}</label>
        <input type="password" placeholder="Monday.com API token" value={str('token', '')} onChange={(e) => set('token', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("GraphQL Query")}</label>
        <textarea rows={5}
          placeholder={'query { boards(limit: 10) { id name } }'}
          value={str('query', '')}
          onChange={(e) => set('query', e.target.value)}
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <div className="field">
        <label>{fl("Variables (JSON, optional)")}</label>
        <textarea rows={3}
          placeholder='{"boardId": 123456789}'
          value={typeof config.variables === 'string' ? config.variables : JSON.stringify(config.variables ?? {}, null, 2)}
          onChange={(e) => { try { set('variables', JSON.parse(e.target.value)) } catch { set('variables', e.target.value) } }}
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
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
        <label>{fl("Token")}</label>
        <input type="password" placeholder="ClickUp personal or OAuth token" value={str('token', '')} onChange={(e) => set('token', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Method")}</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>{fl("Endpoint")}</label>
        <input placeholder="/team" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT', 'PATCH'].includes(method) && (
        <div className="field">
          <label>{fl("Body (JSON)")}</label>
          <textarea rows={4}
            placeholder='{"name": "New Task", "description": "...", "status": "Open"}'
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
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
        <label>{fl("API Key")}</label>
        <input placeholder="Amplitude API key" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Secret Key")}</label>
        <input type="password" placeholder="Amplitude secret key" value={str('secret_key', '')} onChange={(e) => set('secret_key', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Operation")}</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPS.map((o) => <option key={o} value={o}>{o}</option>)}
        </select>
      </div>
      {operation === 'track' && (
        <div className="field">
          <label>{fl("Events (JSON array)")}</label>
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
          <label>{fl("Identification (JSON array)")}</label>
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
            <label>{fl("Start (YYYYMMDDTHH)")}</label>
            <input placeholder="20241201T00" value={str('start', '')} onChange={(e) => set('start', e.target.value)} />
          </div>
          <div className="field">
            <label>{fl("End (YYYYMMDDTHH)")}</label>
            <input placeholder="20241231T23" value={str('end', '')} onChange={(e) => set('end', e.target.value)} />
          </div>
        </>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
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
        <label>{fl("Project Token")}</label>
        <input placeholder="Mixpanel project token" value={str('project_token', '')} onChange={(e) => set('project_token', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("API Secret")}</label>
        <input type="password" placeholder="Mixpanel API secret" value={str('api_secret', '')} onChange={(e) => set('api_secret', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Operation")}</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPS.map((o) => <option key={o} value={o}>{o}</option>)}
        </select>
      </div>
      {(operation === 'track' || operation === 'import') && (
        <div className="field">
          <label>{fl("Events (JSON array)")}</label>
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
            <label>{fl("Endpoint")}</label>
            <input placeholder="/api/query" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
          </div>
          <div className="field">
            <label>{fl("Params (JSON)")}</label>
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
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
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
        <label>{fl("Write Key")}</label>
        <input type="password" placeholder="Segment write key" value={str('write_key', '')} onChange={(e) => set('write_key', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Operation")}</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPS.map((o) => <option key={o} value={o}>{o}</option>)}
        </select>
      </div>
      <div className="field">
        <label>{fl("Body (JSON)")}</label>
        <textarea rows={6}
          placeholder={'{"userId": "user1", "event": "Order Completed", "properties": {"revenue": 99.99}}'}
          value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
          onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Auth via Basic auth (write_key:). Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
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
        <label>{fl("API Key")}</label>
        <input type="password" placeholder="SG.xxxx API key" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Method")}</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>{fl("Endpoint")}</label>
        <input placeholder="/mail/send" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT', 'PATCH'].includes(method) && (
        <div className="field">
          <label>{fl("Body (JSON)")}</label>
          <textarea rows={8}
            placeholder={'{\n  "personalizations": [{"to": [{"email": "to@example.com"}]}],\n  "from": {"email": "from@example.com"},\n  "subject": "Hello",\n  "content": [{"type": "text/plain", "value": "Hi!"}]\n}'}
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("202/204 responses return null body. Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
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
        <label>{fl("Environment")}</label>
        <select value={environment} onChange={(e) => set('environment', e.target.value)}>
          <option value="sandbox">{fl("Sandbox")}</option>
          <option value="production">{fl("Production")}</option>
        </select>
      </div>
      <div className="field">
        <label>{fl("Merchant ID")}</label>
        <input placeholder="Braintree merchant ID" value={str('merchant_id', '')} onChange={(e) => set('merchant_id', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Public Key")}</label>
        <input placeholder="Braintree public key" value={str('public_key', '')} onChange={(e) => set('public_key', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Private Key")}</label>
        <input type="password" placeholder="Braintree private key" value={str('private_key', '')} onChange={(e) => set('private_key', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Method")}</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>{fl("Endpoint")}</label>
        <input placeholder="/transactions" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT'].includes(method) && (
        <div className="field">
          <label>{fl("Body (JSON)")}</label>
          <textarea rows={4}
            placeholder='{"transaction": {"amount": "10.00", "payment_method_nonce": "fake-valid-nonce"}}'
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Auth via Basic auth (public_key:private_key). Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
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
        <label>{fl("Environment")}</label>
        <select value={environment} onChange={(e) => set('environment', e.target.value)}>
          <option value="sandbox">{fl("Sandbox")}</option>
          <option value="live">{fl("Live")}</option>
        </select>
      </div>
      <div className="field">
        <label>{fl("Client ID")}</label>
        <input placeholder="PayPal client ID" value={str('client_id', '')} onChange={(e) => set('client_id', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Client Secret")}</label>
        <input type="password" placeholder="PayPal client secret" value={str('client_secret', '')} onChange={(e) => set('client_secret', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Access Token (optional)")}</label>
        <input type="password" placeholder="Pre-obtained access token (skip token exchange)" value={str('access_token', '')} onChange={(e) => set('access_token', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Method")}</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>{fl("Endpoint")}</label>
        <input placeholder="/v2/checkout/orders" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT', 'PATCH'].includes(method) && (
        <div className="field">
          <label>{fl("Body (JSON)")}</label>
          <textarea rows={5}
            placeholder={'{"intent": "CAPTURE", "purchase_units": [{"amount": {"currency_code": "USD", "value": "10.00"}}]}'}
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Auto-fetches token via client_credentials if access_token is not provided.\n        Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
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
        <label>{fl("Key ID")}</label>
        <input placeholder="rzp_test_…" value={str('key_id', '')} onChange={(e) => set('key_id', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Key Secret")}</label>
        <input type="password" placeholder="Razorpay key secret" value={str('key_secret', '')} onChange={(e) => set('key_secret', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Method")}</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>{fl("Endpoint")}</label>
        <input placeholder="/orders" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT', 'PATCH'].includes(method) && (
        <div className="field">
          <label>{fl("Body (JSON)")}</label>
          <textarea rows={4}
            placeholder='{"amount": 50000, "currency": "INR", "receipt": "order_rcptid_11"}'
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Auth via Basic auth (key_id:key_secret). Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

// ── Slice 296: Firebase ────────────────────────────────────────────────────────

export function MailchimpConfig({ config, set, str }: ConfigProps) {
  const method = str('method', 'GET')
  const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE']
  return (
    <>
      <div className="field">
        <label>{fl("API Key")}</label>
        <input type="password" placeholder="key-us1 (server auto-extracted)" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Server Prefix (optional)")}</label>
        <input placeholder="us1 (auto-extracted from key if omitted)" value={str('server', '')} onChange={(e) => set('server', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Method")}</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>{fl("Endpoint")}</label>
        <input placeholder="/lists" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT', 'PATCH'].includes(method) && (
        <div className="field">
          <label>{fl("Body (JSON)")}</label>
          <textarea rows={5}
            placeholder={'{"email_address": "user@example.com", "status": "subscribed"}'}
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Auth via Basic auth (anystring:api_key). Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
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
        <label>{fl("API Key")}</label>
        <input type="password" placeholder="ActiveCampaign API key" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Base URL")}</label>
        <input placeholder="https://ACCOUNT.api-us1.com" value={str('base_url', '')} onChange={(e) => set('base_url', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Method")}</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>{fl("Endpoint")}</label>
        <input placeholder="/contacts" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT'].includes(method) && (
        <div className="field">
          <label>{fl("Body (JSON)")}</label>
          <textarea rows={5}
            placeholder={'{"contact": {"email": "user@example.com", "firstName": "Jane"}}'}
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Auth via")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{fl("Api-Token")}</code> {fl("header.\n        Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
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
        <label>{fl("Private API Key")}</label>
        <input type="password" placeholder="pk_…" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Method")}</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>{fl("Endpoint")}</label>
        <input placeholder="/profiles" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT', 'PATCH'].includes(method) && (
        <div className="field">
          <label>{fl("Body (JSON)")}</label>
          <textarea rows={5}
            placeholder={'{"data": {"type": "profile", "attributes": {"email": "user@example.com"}}}'}
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Auth via")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{fl("Klaviyo-API-Key")} {'{key}'}</code> {fl("header with API revision 2024-02-15.\n        Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
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
        <label>{fl("API Key")}</label>
        <input type="password" placeholder="re_…" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Method")}</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>{fl("Endpoint")}</label>
        <input placeholder="/emails" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PATCH'].includes(method) && (
        <div className="field">
          <label>{fl("Body (JSON)")}</label>
          <textarea rows={6}
            placeholder={'{\n  "from": "you@example.com",\n  "to": ["user@example.com"],\n  "subject": "Hello",\n  "html": "<p>{fl("Hi!")}</p>"\n}'}
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
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
        <label>{fl("Access Token")}</label>
        <input type="password" placeholder="Delivery/Preview/Management access token" value={str('access_token', '')} onChange={(e) => set('access_token', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Space ID")}</label>
        <input placeholder="Contentful space ID" value={str('space_id', '')} onChange={(e) => set('space_id', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("API Type")}</label>
        <select value={apiType} onChange={(e) => set('api_type', e.target.value)}>
          <option value="delivery">{fl("Delivery (cdn.contentful.com)")}</option>
          <option value="preview">{fl("Preview (preview.contentful.com)")}</option>
          <option value="management">{fl("Management (api.contentful.com)")}</option>
        </select>
      </div>
      <div className="field">
        <label>{fl("Method")}</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>{fl("Endpoint")}</label>
        <input placeholder="/entries?content_type=blogPost" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT', 'PATCH'].includes(method) && (
        <div className="field">
          <label>{fl("Body (JSON)")}</label>
          <textarea rows={4}
            placeholder='{"fields": {"title": {"en-US": "My Post"}}}'
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
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
        <label>{fl("Application ID")}</label>
        <input placeholder="ABC123DEF4" value={str('app_id', '')} onChange={(e) => set('app_id', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("API Key")}</label>
        <input type="password" placeholder="Search or Admin API key" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Method")}</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>{fl("Endpoint")}</label>
        <input placeholder="/1/indexes/INDEX_NAME/query" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT'].includes(method) && (
        <div className="field">
          <label>{fl("Body (JSON)")}</label>
          <textarea rows={4}
            placeholder='{"query": "search term", "hitsPerPage": 10}'
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Auth via")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{fl("X-Algolia-Application-Id")}</code> + <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{fl("X-Algolia-API-Key")}</code> {fl("headers.\n        Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
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
        <label>{fl("Server Token")}</label>
        <input type="password" placeholder="Postmark server token" value={str('server_token', '')} onChange={(e) => set('server_token', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Method")}</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>{fl("Endpoint")}</label>
        <input placeholder="/email" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT'].includes(method) && (
        <div className="field">
          <label>{fl("Body (JSON)")}</label>
          <textarea rows={6}
            placeholder={'{\n  "From": "sender@example.com",\n  "To": "recipient@example.com",\n  "Subject": "Hello",\n  "TextBody": "Hi there!"\n}'}
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Auth via")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{fl("X-Postmark-Server-Token")}</code> {fl("header.\n        Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
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
        <label>{fl("API Key")}</label>
        <input placeholder="Vonage API key" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("API Secret")}</label>
        <input type="password" placeholder="Vonage API secret" value={str('api_secret', '')} onChange={(e) => set('api_secret', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Operation")}</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPS.map((o) => <option key={o} value={o}>{o}</option>)}
        </select>
      </div>
      {operation === 'sms' && (
        <>
          <div className="field">
            <label>{fl("To")}</label>
            <input placeholder="+14155551234" value={str('to', '')} onChange={(e) => set('to', e.target.value)} />
          </div>
          <div className="field">
            <label>{fl("From")}</label>
            <input placeholder="Vonage or your virtual number" value={str('from', '')} onChange={(e) => set('from', e.target.value)} />
          </div>
          <div className="field">
            <label>{fl("Text")}</label>
            <textarea rows={3} placeholder="SMS message text" value={str('text', '')} onChange={(e) => set('text', e.target.value)} />
          </div>
        </>
      )}
      {(operation === 'voice' || operation === 'verify') && (
        <>
          <div className="field">
            <label>{fl("Endpoint")}</label>
            <input placeholder="/v1/calls or /verify/json" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
          </div>
          <div className="field">
            <label>{fl("Body (JSON)")}</label>
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
        {fl("SMS posts to rest.nexmo.com. Voice/Verify use Basic auth to api.nexmo.com.\n        Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
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
        <label>{fl("Access Token")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="ya29.…" value={str('access_token', '')} onChange={(e) => set('access_token', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Operation")}</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {['get', 'batch_update'].includes(operation) && (
        <div className="field">
          <label>{fl("Document ID")} <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input placeholder="1BxiMVs0XRA…" value={str('document_id', '')} onChange={(e) => set('document_id', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {operation === 'create' && (
        <div className="field">
          <label>{fl("Document Title")}</label>
          <input placeholder="Untitled Document" value={str('title', '')} onChange={(e) => set('title', e.target.value)} />
        </div>
      )}
      {operation === 'batch_update' && (
        <div className="field">
          <label>{fl("Requests (JSON array)")}</label>
          <textarea rows={4} placeholder='[{"insertText":{"location":{"index":1},"text":"Hello"}}]' value={typeof config.requests === 'object' ? JSON.stringify(config.requests, null, 2) : str('requests', '')} onChange={(e) => { try { set('requests', JSON.parse(e.target.value)) } catch { set('requests', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Uses Google Docs REST API v1. Obtain access_token via OAuth2 (scope: documents). Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
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
        <label>{fl("Site URL")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="https://mystore.com" value={str('site_url', '')} onChange={(e) => set('site_url', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Consumer Key")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="ck_…" value={str('consumer_key', '')} onChange={(e) => set('consumer_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Consumer Secret")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="cs_…" value={str('consumer_secret', '')} onChange={(e) => set('consumer_secret', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Method")}</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>{fl("Endpoint")}</label>
        <input placeholder="/wp-json/wc/v3/products" value={str('endpoint', '/wp-json/wc/v3/products')} onChange={(e) => set('endpoint', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      {['POST', 'PUT', 'PATCH'].includes(method) && (
        <div className="field">
          <label>{fl("Body (JSON)")}</label>
          <textarea rows={4} placeholder='{"name":"T-Shirt","regular_price":"15.00"}' value={typeof config.body === 'object' ? JSON.stringify(config.body, null, 2) : str('body', '')} onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Authenticates with Basic auth (consumer_key:consumer_secret). Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
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
        <label>{fl("Access Token")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="ya29.…" value={str('access_token', '')} onChange={(e) => set('access_token', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Operation")}</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation !== 'list_calendars' && (
        <div className="field">
          <label>{fl("Calendar ID")}</label>
          <input placeholder="primary" value={str('calendar_id', '')} onChange={(e) => set('calendar_id', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {(operation === 'get_event' || operation === 'delete_event') && (
        <div className="field">
          <label>{fl("Event ID")} <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input value={str('event_id', '')} onChange={(e) => set('event_id', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {operation === 'list_events' && (
        <div className="field">
          <label>{fl("Search Query")}</label>
          <input placeholder="Meeting" value={str('query', '')} onChange={(e) => set('query', e.target.value)} />
        </div>
      )}
      {operation === 'create_event' && (
        <>
          <div className="field">
            <label>{fl("Summary")} <span style={{ color: 'var(--danger)' }}>*</span></label>
            <input placeholder="Team Standup" value={str('summary', '')} onChange={(e) => set('summary', e.target.value)} />
          </div>
          <div className="field">
            <label>{fl("Description")}</label>
            <input placeholder="Daily sync" value={str('description', '')} onChange={(e) => set('description', e.target.value)} />
          </div>
          <div className="field">
            <label>{fl("Start Time")} <span style={{ color: 'var(--danger)' }}>*</span></label>
            <input placeholder="2025-01-15T10:00:00" value={str('start_time', '')} onChange={(e) => set('start_time', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>{fl("End Time")}</label>
            <input placeholder="2025-01-15T11:00:00" value={str('end_time', '')} onChange={(e) => set('end_time', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>{fl("Timezone")}</label>
            <input placeholder="America/New_York" value={str('timezone', '')} onChange={(e) => set('timezone', e.target.value)} />
          </div>
        </>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Uses OAuth2 access token. Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
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
        <label>{fl("Access Token")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" value={str('access_token', '')} onChange={(e) => set('access_token', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Account ID")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx" value={str('account_id', '')} onChange={(e) => set('account_id', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Base URL")}</label>
        <input placeholder="https://demo.docusign.net/restapi" value={str('base_url', '')} onChange={(e) => set('base_url', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        <small style={{ color: 'var(--muted)', fontSize: 10 }}>{fl("Use demo.docusign.net for sandbox, www.docusign.net for production")}</small>
      </div>
      <div className="field">
        <label>{fl("Operation")}</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation === 'list_envelopes' && (
        <div className="field">
          <label>{fl("From Date")}</label>
          <input placeholder="2024-01-01" value={str('from_date', '')} onChange={(e) => set('from_date', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {(operation === 'get_envelope' || operation === 'void_envelope') && (
        <div className="field">
          <label>{fl("Envelope ID")} <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input value={str('envelope_id', '')} onChange={(e) => set('envelope_id', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {operation === 'void_envelope' && (
        <div className="field">
          <label>{fl("Void Reason")}</label>
          <input placeholder="Voided via workflow" value={str('void_reason', '')} onChange={(e) => set('void_reason', e.target.value)} />
        </div>
      )}
      {operation === 'create_envelope' && (
        <div className="field">
          <label>{fl("Envelope Body (JSON)")}</label>
          <textarea rows={5} placeholder='{"emailSubject":"Please sign","documents":[...],"recipients":{...},"status":"sent"}' value={typeof config.body === 'object' ? JSON.stringify(config.body, null, 2) : str('body', '')} onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("E-signature via DocuSign eSign REST API v2.1. Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function XeroConfig({ config, set, str }: ConfigProps) {
  const METHOD_OPTIONS = ['GET', 'POST', 'PUT', 'DELETE']
  return (
    <>
      <div className="field">
        <label>{fl("Access Token")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" value={str('access_token', '')} onChange={(e) => set('access_token', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Tenant ID")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx" value={str('tenant_id', '')} onChange={(e) => set('tenant_id', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Endpoint")}</label>
        <input placeholder="/Contacts" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        <small style={{ color: 'var(--muted)', fontSize: 10 }}>{fl("Relative path: /Contacts, /Invoices, /Accounts, /Payments\u2026")}</small>
      </div>
      <div className="field">
        <label>{fl("Method")}</label>
        <select value={str('method', 'GET')} onChange={(e) => set('method', e.target.value)}>
          {METHOD_OPTIONS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      {!['GET', 'DELETE'].includes(str('method', 'GET')) && (
        <div className="field">
          <label>{fl("Body (JSON)")}</label>
          <textarea rows={4} placeholder='{"Contacts":[{"Name":"Acme Corp"}]}' value={typeof config.body === 'object' ? JSON.stringify(config.body, null, 2) : str('body', '')} onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Base URL: api.xero.com/api.xro/2.0. Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
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
        <label>{fl("API Key (PAT)")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="eyJra…" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Operation")}</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {needsUserUri && (
        <div className="field">
          <label>{fl("User URI")} <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input placeholder="https://api.calendly.com/users/xxxx" value={str('user_uri', '')} onChange={(e) => set('user_uri', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          <small style={{ color: 'var(--muted)', fontSize: 10 }}>{fl("Found in get_current_user response as resource.uri")}</small>
        </div>
      )}
      {operation === 'list_scheduled_events' && (
        <div className="field">
          <label>{fl("Status Filter")}</label>
          <select value={str('status', 'active')} onChange={(e) => set('status', e.target.value)}>
            <option value="active">{fl("active")}</option>
            <option value="canceled">{fl("canceled")}</option>
          </select>
        </div>
      )}
      {needsEventUuid && (
        <div className="field">
          <label>{fl("Event UUID")} <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input placeholder="xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx" value={str('event_uuid', '')} onChange={(e) => set('event_uuid', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {operation === 'cancel_event' && (
        <div className="field">
          <label>{fl("Cancellation Reason")}</label>
          <input placeholder="Rescheduling" value={str('reason', '')} onChange={(e) => set('reason', e.target.value)} />
        </div>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Calendly v2 API — personal access token auth. Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
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
        <label>{fl("API Token")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="apify_api_…" value={str('api_token', '')} onChange={(e) => set('api_token', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Operation")}</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation === 'run_actor' && (
        <>
          <div className="field">
            <label>{fl("Actor ID")} <span style={{ color: 'var(--danger)' }}>*</span></label>
            <input placeholder="apify/web-scraper" value={str('actor_id', '')} onChange={(e) => set('actor_id', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>{fl("Input (JSON)")}</label>
            <textarea rows={4} placeholder='{"startUrls":[{"url":"https://example.com"}]}' value={typeof config.input === 'object' ? JSON.stringify(config.input, null, 2) : str('input', '')} onChange={(e) => { try { set('input', JSON.parse(e.target.value)) } catch { set('input', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
        </>
      )}
      {operation === 'get_run' && (
        <div className="field">
          <label>{fl("Run ID")} <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input value={str('run_id', '')} onChange={(e) => set('run_id', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {operation === 'get_dataset_items' && (
        <>
          <div className="field">
            <label>{fl("Dataset ID")} <span style={{ color: 'var(--danger)' }}>*</span></label>
            <input value={str('dataset_id', '')} onChange={(e) => set('dataset_id', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>{fl("Limit")}</label>
            <input type="number" min={1} max={1000} placeholder="100" value={(config['limit'] as number | undefined) ?? ''} onChange={(e) => set('limit', e.target.value ? parseInt(e.target.value) : undefined)} />
          </div>
        </>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Run web scrapers and automation actors. Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
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
        <label>{fl("Access Token")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="ya29.…" value={str('access_token', '')} onChange={(e) => set('access_token', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Property ID")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="123456789" value={str('property_id', '')} onChange={(e) => set('property_id', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        <small style={{ color: 'var(--muted)', fontSize: 10 }}>{fl("GA4 property ID (numeric, without \"properties/\" prefix)")}</small>
      </div>
      <div className="field">
        <label>{fl("Operation")}</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {(operation === 'run_report' || operation === 'run_realtime_report') && (
        <>
          {operation === 'run_report' && (
            <div className="field">
              <label>{fl("Date Ranges (JSON)")}</label>
              <textarea rows={2} placeholder='[{"startDate":"7daysAgo","endDate":"today"}]' value={typeof config.date_ranges === 'object' ? JSON.stringify(config.date_ranges) : str('date_ranges', '')} onChange={(e) => { try { set('date_ranges', JSON.parse(e.target.value)) } catch { set('date_ranges', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
            </div>
          )}
          <div className="field">
            <label>{fl("Dimensions (JSON)")}</label>
            <textarea rows={2} placeholder='[{"name":"date"}]' value={typeof config.dimensions === 'object' ? JSON.stringify(config.dimensions) : str('dimensions', '')} onChange={(e) => { try { set('dimensions', JSON.parse(e.target.value)) } catch { set('dimensions', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>{fl("Metrics (JSON)")}</label>
            <textarea rows={2} placeholder='[{"name":"sessions"}]' value={typeof config.metrics === 'object' ? JSON.stringify(config.metrics) : str('metrics', '')} onChange={(e) => { try { set('metrics', JSON.parse(e.target.value)) } catch { set('metrics', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
        </>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("GA4 Data API — OAuth2 access token. Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
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
        <label>{fl("API Key")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("User Email")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="email" placeholder="you@company.com" value={str('user_email', '')} onChange={(e) => set('user_email', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Resource")}</label>
        <select value={str('resource', 'people')} onChange={(e) => set('resource', e.target.value)}>
          {RESOURCES.map((r) => <option key={r} value={r}>{r}</option>)}
        </select>
      </div>
      <div className="field">
        <label>{fl("Operation")}</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {needsId && (
        <div className="field">
          <label>{fl("Record ID")} <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input placeholder="12345678" value={str('record_id', '')} onChange={(e) => set('record_id', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {operation === 'list' && (
        <div className="field">
          <label>{fl("Filter (JSON)")}</label>
          <textarea rows={3} placeholder='{"name":"Acme"}' value={typeof config.filter === 'object' ? JSON.stringify(config.filter, null, 2) : str('filter', '')} onChange={(e) => { try { set('filter', JSON.parse(e.target.value)) } catch { set('filter', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {(operation === 'create' || operation === 'update') && (
        <div className="field">
          <label>{fl("Body (JSON)")}</label>
          <textarea rows={4} placeholder='{"name":"Acme Corp","email":[{"email":"contact@acme.com"}]}' value={typeof config.body === 'object' ? JSON.stringify(config.body, null, 2) : str('body', '')} onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Copper CRM (Google Workspace-native). Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}
