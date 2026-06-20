// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import type { ConfigProps } from '../types'
import { fl } from '../i18nLabels'

export function WaitConfig({ set, str, num }: ConfigProps) {
  const mode = str('mode', 'duration')
  return (
    <>
      <div className="field">
        <label>{fl("Mode")}</label>
        <select value={mode} onChange={(e) => set('mode', e.target.value)}>
          <option value="duration">{fl("duration — pause for a time, then auto-resume")}</option>
          <option value="resume">{fl("resume — suspend until externally resumed")}</option>
        </select>
      </div>
      {mode === 'duration' && (
        <>
          <div className="field">
            <label>{fl("Seconds")}</label>
            <input type="number" min={0} value={num('seconds', 0)} onChange={(e) => set('seconds', Number(e.target.value))} />
          </div>
          <div className="field">
            <label>{fl("Until")} <span style={{ color: 'var(--muted)' }}>{fl("(RFC3339, overrides seconds)")}</span></label>
            <input placeholder="2026-07-01T09:00:00Z" value={str('until', '')} onChange={(e) => set('until', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
        </>
      )}
      {mode === 'resume' && (
        <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
          {fl("The run suspends here until resumed via")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>POST /v1/executions/&#123;id&#125;/approve</code> {fl("(the shared resume gate). Inline execution mode only.")}
        </p>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ resumed, mode, waited_secs }'}</code>
      </p>
    </>
  )
}

export function HtmlExtractConfig({ set, str }: ConfigProps) {
  const mode = str('extract', 'text')
  return (
    <>
      <div className="field">
        <label>{fl("HTML")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={4} placeholder="{{http.body}}" value={str('html', '')} onChange={(e) => set('html', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("CSS Selector")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="div.article h2 a" value={str('selector', '')} onChange={(e) => set('selector', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Extract")}</label>
        <select value={mode} onChange={(e) => set('extract', e.target.value)}>
          {['text', 'html', 'attr'].map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      {mode === 'attr' && (
        <div className="field">
          <label>{fl("Attribute")} <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input placeholder="href" value={str('attr', '')} onChange={(e) => set('attr', e.target.value)} />
        </div>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ matches, count, first }'}</code>
      </p>
    </>
  )
}

export function RssConfig({ set, str, num }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>{fl("Feed URL")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="https://example.com/feed.xml" value={str('url', '')} onChange={(e) => set('url', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Limit")}</label>
        <input type="number" min={1} max={200} value={num('limit', 20)} onChange={(e) => set('limit', Number(e.target.value))} />
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Parses RSS 2.0 / RSS 1.0 / Atom / JSON Feed. Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ feed_title, items, count }'}</code>
      </p>
    </>
  )
}

export function ImageConfig({ set, str }: ConfigProps) {
  const operation = str('operation', 'metadata')
  return (
    <>
      <div className="field">
        <label>{fl("Operation")}</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {['metadata', 'resize', 'convert'].map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      <div className="field">
        <label>{fl("Image (base64)")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={4} placeholder="iVBORw0KGgo…" value={str('image_base64', '')} onChange={(e) => set('image_base64', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      {operation === 'resize' && (
        <div style={{ display: 'flex', gap: 8 }}>
          <div className="field" style={{ flex: 1 }}>
            <label>{fl("Width")}</label>
            <input type="number" min={1} placeholder="(auto)" value={str('width', '')} onChange={(e) => set('width', e.target.value ? parseInt(e.target.value) : undefined)} />
          </div>
          <div className="field" style={{ flex: 1 }}>
            <label>{fl("Height")}</label>
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
          ? <>{fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ width, height, color }'}</code></>
          : <>{fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ image_base64, format, width, height }'}</code></>}
      </p>
    </>
  )
}

export function PdfExtractConfig({ set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>{fl("PDF (base64)")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={5} placeholder="JVBERi0xLjc…" value={str('pdf_base64', '')} onChange={(e) => set('pdf_base64', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Extracts text from a PDF. Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ text, char_count }'}</code>
      </p>
    </>
  )
}

export function OcrConfig({ set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>{fl("Image (base64)")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={4} placeholder="iVBORw0KGgo…" value={str('image_base64', '')} onChange={(e) => set('image_base64', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Language")}</label>
        <input placeholder="eng" value={str('lang', '')} onChange={(e) => set('lang', e.target.value)} />
        <small style={{ color: 'var(--muted)', fontSize: 10 }}>{fl("tesseract lang code(s), e.g. eng, chi_sim, eng+fra. Requires the tesseract CLI on the executor host.")}</small>
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("OCR via tesseract. Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ text, lang }'}</code>
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
        <label>{fl("Algorithm")}</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      <div className="field">
        <label>{fl("Input")}</label>
        <textarea rows={3} placeholder="text to hash" value={str('input', '')} onChange={(e) => set('input', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      {isHmac && (
        <div className="field">
          <label>{fl("Key")} <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input type="password" value={str('key', '')} onChange={(e) => set('key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      <div className="field">
        <label>{fl("Encoding")}</label>
        <select value={str('encoding', 'hex')} onChange={(e) => set('encoding', e.target.value)}>
          {['hex', 'base64', 'base64url'].map((enc) => <option key={enc} value={enc}>{enc}</option>)}
        </select>
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Hash / HMAC digest. Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ hash, algorithm, encoding }'}</code>
      </p>
    </>
  )
}

export function JwtConfig({ config, set, str }: ConfigProps) {
  const operation = str('operation', 'sign')
  return (
    <>
      <div className="field">
        <label>{fl("Operation")}</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {['sign', 'verify'].map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      <div className="field">
        <label>{fl("Algorithm")}</label>
        <select value={str('algorithm', 'HS256')} onChange={(e) => set('algorithm', e.target.value)}>
          {['HS256', 'HS384', 'HS512'].map((a) => <option key={a} value={a}>{a}</option>)}
        </select>
      </div>
      <div className="field">
        <label>{fl("Secret")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" value={str('secret', '')} onChange={(e) => set('secret', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      {operation === 'sign' && (
        <>
          <div className="field">
            <label>{fl("Payload (JSON object)")}</label>
            <textarea rows={3} placeholder='{"sub":"123","name":"Ada"}' value={typeof config.payload === 'object' ? JSON.stringify(config.payload, null, 2) : str('payload', '')} onChange={(e) => { try { set('payload', JSON.parse(e.target.value)) } catch { set('payload', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>{fl("Expires In (seconds)")}</label>
            <input type="number" min={1} placeholder="3600" value={(config['expires_in_secs'] as number | undefined) ?? ''} onChange={(e) => set('expires_in_secs', e.target.value ? parseInt(e.target.value) : undefined)} />
          </div>
        </>
      )}
      {operation === 'verify' && (
        <div className="field">
          <label>{fl("Token")} <span style={{ color: 'var(--danger)' }}>*</span></label>
          <textarea rows={3} placeholder="eyJhbGciOi…" value={str('token', '')} onChange={(e) => set('token', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {operation === 'sign'
          ? <>{fl("Signs an HMAC JWT. Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ token }'}</code></>
          : <>{fl("Verifies signature + exp. Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ valid, payload }'}</code></>}
      </p>
    </>
  )
}
