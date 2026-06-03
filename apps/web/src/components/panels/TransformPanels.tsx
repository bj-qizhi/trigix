// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import type { ConfigProps } from './types'

export function TransformConfig({ config, set }: ConfigProps) {
  const raw = config.template
  const display = raw !== undefined && raw !== null
    ? (typeof raw === 'string' ? raw : JSON.stringify(raw, null, 2))
    : ''
  return (
    <>
      <div className="field">
        <label>
          Template *{' '}
          <span style={{ color: 'var(--muted)' }}>(JSON object or value with <code>{'{{...}}'}</code>)</span>
        </label>
        <textarea
          rows={6}
          placeholder={'{\n  "name": "{{input.name}}",\n  "score": "{{scorer.result}}"\n}'}
          value={display}
          onChange={(e) => {
            const raw = e.target.value
            if (!raw.trim()) { set('template', undefined as unknown as string); return }
            try { set('template', JSON.parse(raw)) } catch { set('template', raw) }
          }}
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <TemplateHint />
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns the rendered template value as output JSON.
      </p>
    </>
  )
}

export function ExtractConfig({ set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Source *</label>
        <input
          placeholder="{{input}} or {{node_id}}"
          value={str('source')}
          onChange={(e) => set('source', e.target.value)}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>Template expression resolving to a JSON object.</span>
      </div>
      <div className="field">
        <label>Path *</label>
        <input
          placeholder="data.users.0.email"
          value={str('path')}
          onChange={(e) => set('path', e.target.value)}
          style={{ fontFamily: 'monospace' }}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>Dot-separated path into the source JSON.</span>
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>
          {'{ "value": ..., "found": true/false }'}
        </code>
      </p>
    </>
  )
}

export function MergeConfig({ set, str }: ConfigProps) {
  const raw = str('fields') || '[]'
  const fields: Array<{ source: string; key?: string }> = (() => {
    try { return JSON.parse(raw) } catch { return [] }
  })()
  const update = (next: Array<{ source: string; key?: string }>) => {
    set('fields', JSON.stringify(next, null, 2))
  }
  return (
    <>
      <div className="field">
        <label>Fields to merge</label>
        {fields.map((f, i) => (
          <div key={i} style={{ display: 'flex', gap: 6, marginBottom: 6, alignItems: 'center' }}>
            <input
              placeholder="Source ({{node_id}})"
              value={f.source}
              onChange={(e) => { const next = [...fields]; next[i] = { ...next[i], source: e.target.value }; update(next) }}
              style={{ flex: 2, fontFamily: 'monospace', fontSize: 12 }}
            />
            <input
              placeholder="Key (optional)"
              value={f.key ?? ''}
              onChange={(e) => { const next = [...fields]; next[i] = { ...next[i], key: e.target.value || undefined }; update(next) }}
              style={{ flex: 1, fontSize: 12 }}
            />
            <button className="btn btn-sm btn-danger" onClick={() => update(fields.filter((_, j) => j !== i))}>✕</button>
          </div>
        ))}
        <button className="btn btn-sm" onClick={() => update([...fields, { source: '' }])}>+ Add field</button>
        <span style={{ fontSize: 11, color: 'var(--muted)', display: 'block', marginTop: 4 }}>
          If Key is empty, merges all top-level fields from Source. Returns merged object.
        </span>
      </div>
    </>
  )
}

export function DedupeConfig({ set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Items Array *</label>
        <input
          placeholder="{{filter_node.items}}"
          value={str('items', '{{input}}')}
          onChange={(e) => set('items', e.target.value)}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>Template expression returning an array.</span>
      </div>
      <div className="field">
        <label>Dedupe Field <span style={{ color: 'var(--muted)' }}>(optional)</span></label>
        <input
          placeholder="id"
          value={str('field')}
          onChange={(e) => set('field', e.target.value)}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>Dot-path to dedupe by. Leave blank to compare whole items.</span>
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ "items": [...], "count": N, "removed_count": M }'}</code>
      </p>
    </>
  )
}

export function RegexConfig({ set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Source *</label>
        <input
          placeholder="{{input.text}}"
          value={str('source', '{{input}}')}
          onChange={(e) => set('source', e.target.value)}
        />
      </div>
      <div className="field">
        <label>Pattern *</label>
        <input
          placeholder="error|warning"
          value={str('pattern')}
          onChange={(e) => set('pattern', e.target.value)}
          style={{ fontFamily: 'monospace' }}
        />
      </div>
      <div className="field">
        <label>Flags <span style={{ color: 'var(--muted)' }}>(optional)</span></label>
        <input
          placeholder="i  (case-insensitive)"
          value={str('flags')}
          onChange={(e) => set('flags', e.target.value)}
          style={{ width: 80 }}
        />
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ "matched": bool, "full_match": "...", "groups": [] }'}</code>
      </p>
    </>
  )
}

export function CsvConfig({ set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Source *</label>
        <input
          placeholder="{{input.csv_data}}"
          value={str('source', '{{input}}')}
          onChange={(e) => set('source', e.target.value)}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>Template expression returning a CSV string.</span>
      </div>
      <div className="field">
        <label>Delimiter</label>
        <input
          placeholder=","
          value={str('delimiter', ',')}
          onChange={(e) => set('delimiter', e.target.value)}
          style={{ width: 60 }}
        />
      </div>
      <div className="field" style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
        <input
          type="checkbox"
          id="csv-header"
          checked={str('has_header', 'true') !== 'false'}
          onChange={(e) => set('has_header', e.target.checked)}
        />
        <label htmlFor="csv-header" style={{ cursor: 'pointer' }}>First row is header</label>
      </div>
      <div className="field" style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
        <input
          type="checkbox"
          id="csv-trim"
          checked={str('trim', 'true') !== 'false'}
          onChange={(e) => set('trim', e.target.checked)}
        />
        <label htmlFor="csv-trim" style={{ cursor: 'pointer' }}>Trim cell whitespace</label>
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ "rows": [{...}], "count": N, "headers": [...] }'}</code>
      </p>
    </>
  )
}

export function RenameConfig({ config, set, str }: ConfigProps) {
  const mappings: { from: string; to: string }[] = Array.isArray(config.mappings)
    ? config.mappings as { from: string; to: string }[]
    : []
  const setMappings = (m: { from: string; to: string }[]) => set('mappings', m)
  return (
    <>
      <div className="field">
        <label>Source Object *</label>
        <input
          placeholder="{{input}}"
          value={str('source', '{{input}}')}
          onChange={(e) => set('source', e.target.value)}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>Template expression returning a JSON object.</span>
      </div>
      <div className="field">
        <label>Key Mappings</label>
        {mappings.map((m, i) => (
          <div key={i} style={{ display: 'flex', gap: 6, marginBottom: 4, alignItems: 'center' }}>
            <input placeholder="old_key" value={m.from} onChange={(e) => {
              const next = [...mappings]; next[i] = { ...next[i], from: e.target.value }; setMappings(next)
            }} style={{ flex: 1 }} />
            <span style={{ color: 'var(--muted)', fontSize: 12 }}>→</span>
            <input placeholder="new_key" value={m.to} onChange={(e) => {
              const next = [...mappings]; next[i] = { ...next[i], to: e.target.value }; setMappings(next)
            }} style={{ flex: 1 }} />
            <button onClick={() => setMappings(mappings.filter((_, j) => j !== i))}
              style={{ background: 'none', border: 'none', color: 'var(--danger-text)', cursor: 'pointer', fontSize: 14 }}>✕</button>
          </div>
        ))}
        <button className="btn btn-secondary" style={{ marginTop: 4, fontSize: 12 }}
          onClick={() => setMappings([...mappings, { from: '', to: '' }])}>+ Add mapping</button>
        <span style={{ fontSize: 11, color: 'var(--muted)', marginTop: 4, display: 'block' }}>Unmapped keys are passed through unchanged.</span>
      </div>
    </>
  )
}

export function FormatConfig({ set, str, num }: ConfigProps) {
  const op = str('operation', 'uppercase')
  return (
    <>
      <div className="field">
        <label>Source *</label>
        <input placeholder="{{input.text}}" value={str('source', '{{input}}')} onChange={(e) => set('source', e.target.value)} />
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={op} onChange={(e) => set('operation', e.target.value)}>
          <option value="uppercase">UPPERCASE</option>
          <option value="lowercase">lowercase</option>
          <option value="trim">Trim whitespace</option>
          <option value="trim_start">Trim start</option>
          <option value="trim_end">Trim end</option>
          <option value="reverse">Reverse string</option>
          <option value="length">String length (number)</option>
          <option value="word_count">Word count (number)</option>
          <option value="to_number">Parse as number</option>
          <option value="to_bool">Parse as boolean</option>
          <option value="replace">Find &amp; replace</option>
          <option value="pad_start">Pad start</option>
          <option value="truncate">Truncate</option>
        </select>
      </div>
      {op === 'replace' && (
        <div style={{ display: 'flex', gap: 8 }}>
          <div className="field" style={{ flex: 1 }}>
            <label>Find</label>
            <input value={str('from')} onChange={(e) => set('from', e.target.value)} />
          </div>
          <div className="field" style={{ flex: 1 }}>
            <label>Replace with</label>
            <input value={str('to_value')} onChange={(e) => set('to_value', e.target.value)} />
          </div>
        </div>
      )}
      {op === 'pad_start' && (
        <div style={{ display: 'flex', gap: 8 }}>
          <div className="field" style={{ flex: 1 }}>
            <label>Width</label>
            <input type="number" min={1} max={200} value={num('width', 10)} onChange={(e) => set('width', Number(e.target.value))} />
          </div>
          <div className="field" style={{ flex: 1 }}>
            <label>Pad char</label>
            <input maxLength={1} value={str('pad_char', '0')} onChange={(e) => set('pad_char', e.target.value)} style={{ width: 50 }} />
          </div>
        </div>
      )}
      {op === 'truncate' && (
        <div style={{ display: 'flex', gap: 8 }}>
          <div className="field" style={{ flex: 1 }}>
            <label>Max length</label>
            <input type="number" min={1} value={num('max_length', 100)} onChange={(e) => set('max_length', Number(e.target.value))} />
          </div>
          <div className="field" style={{ flex: 1 }}>
            <label>Suffix</label>
            <input value={str('suffix', '…')} onChange={(e) => set('suffix', e.target.value)} style={{ width: 60 }} />
          </div>
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ "result": ..., "operation": "..." }'}</code>
      </p>
    </>
  )
}

export function XmlConfig({ set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Source (XML string) <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea
          rows={4}
          value={str('source', '')}
          onChange={(e) => set('source', e.target.value)}
          placeholder="{{http_node.body}} or <root><item>1</item></root>"
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>Template expressions supported. Attributes become @attr keys.</span>
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ data: object }'}</code>
      </p>
    </>
  )
}

export function YamlConfig({ set, str }: ConfigProps) {
  const mode = str('mode', 'parse')
  return (
    <>
      <div className="field">
        <label>Mode</label>
        <select value={mode} onChange={(e) => set('mode', e.target.value)}>
          <option value="parse">Parse — YAML string → JSON object</option>
          <option value="serialize">Serialize — JSON value → YAML string</option>
        </select>
      </div>
      <div className="field">
        <label>Source <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea
          rows={4}
          value={str('source', '')}
          onChange={(e) => set('source', e.target.value)}
          placeholder={mode === 'parse' ? 'key: value\nlist:\n  - a\n  - b' : '{{transform_node.output}}'}
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>
          {mode === 'parse' ? 'YAML string to parse into JSON' : 'JSON value or template to serialize'}
        </span>
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>
          {mode === 'parse' ? '{ data: object }' : '{ yaml: string }'}
        </code>
      </p>
    </>
  )
}

export function HandlebarsConfig({ set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Template <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea
          rows={6}
          value={str('template', '')}
          onChange={(e) => set('template', e.target.value)}
          placeholder={'Hello, {{name}}!\n{{#if items}}\n{{#each items}}- {{this}}\n{{/each}}{{/if}}'}
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>Handlebars syntax: {'{{var}}'}, {'{{#if}}'}, {'{{#each}}'}, {'{{#unless}}'}</span>
      </div>
      <div className="field">
        <label>Data (JSON expression)</label>
        <input
          value={str('data', '{{input}}')}
          onChange={(e) => set('data', e.target.value)}
          placeholder="{{input}} or {{transform_node.output}}"
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>Resolved first, then used as Handlebars context object</span>
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ result: string }'}</code>
      </p>
    </>
  )
}

export function MathConfig({ set, str, num }: ConfigProps) {
  const op = str('operation', 'add')
  const needsB = ['add', 'pow', 'mod', 'pct_change', 'log'].includes(op)
  const needsItems = ['min', 'max', 'sum', 'avg'].includes(op)
  const needsClamp = op === 'clamp'
  const needsExpr = op === 'eval'
  const needsA = !needsItems && !needsExpr
  const MATH_OPS = ['add', 'abs', 'round', 'ceil', 'floor', 'sqrt', 'pow', 'mod', 'min', 'max', 'clamp', 'log', 'pct_change', 'sum', 'avg', 'eval']

  return (
    <>
      <div className="field">
        <label>Operation</label>
        <select value={op} onChange={(e) => set('operation', e.target.value)}>
          {MATH_OPS.map((o) => <option key={o} value={o}>{o}</option>)}
        </select>
      </div>
      {needsA && (
        <div className="field">
          <label>a</label>
          <input type="number" step="any" value={num('a', 0)} onChange={(e) => set('a', Number(e.target.value))} />
        </div>
      )}
      {needsB && (
        <div className="field">
          <label>b {op === 'log' ? '(base, default e)' : op === 'pow' ? '(exponent)' : ''}</label>
          <input type="number" step="any" value={num('b', op === 'pow' ? 2 : 0)} onChange={(e) => set('b', Number(e.target.value))} />
        </div>
      )}
      {needsItems && (
        <div className="field">
          <label>{'Items (JSON array or {{node.field}})'}</label>
          <input placeholder='[1, 2, 3]' value={str('items', '')} onChange={(e) => {
            try { set('items', JSON.parse(e.target.value)) } catch { set('items', e.target.value) }
          }} />
        </div>
      )}
      {needsClamp && (
        <>
          <div className="field">
            <label>Min</label>
            <input type="number" step="any" value={num('min', 0)} onChange={(e) => set('min', Number(e.target.value))} />
          </div>
          <div className="field">
            <label>Max</label>
            <input type="number" step="any" value={num('max', 100)} onChange={(e) => set('max', Number(e.target.value))} />
          </div>
        </>
      )}
      {['round', 'avg'].includes(op) && (
        <div className="field">
          <label>Precision (decimal places)</label>
          <input type="number" min={0} max={15} value={num('precision', 2)} onChange={(e) => set('precision', Number(e.target.value))} />
        </div>
      )}
      {needsExpr && (
        <div className="field">
          <label>Expression (Rhai)</label>
          <input placeholder="2 + 2 * PI" value={str('expression', '')} onChange={(e) => set('expression', e.target.value)} />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ result, operation }'}</code>
      </p>
    </>
  )
}

export function ArrayUtilsConfig({ set, str, num }: ConfigProps) {
  const op = str('operation', 'chunk')
  const ARRAY_OPS = ['chunk', 'flatten', 'compact', 'zip', 'reverse', 'shuffle', 'sample', 'range', 'pluck', 'first_n', 'last_n']
  const needsSource = op !== 'range'
  const needsSource2 = op === 'zip'
  const needsSize = op === 'chunk'
  const needsN = ['sample', 'first_n', 'last_n'].includes(op)
  const needsRange = op === 'range'
  const needsField = op === 'pluck'

  return (
    <>
      <div className="field">
        <label>Operation</label>
        <select value={op} onChange={(e) => set('operation', e.target.value)}>
          {ARRAY_OPS.map((o) => <option key={o} value={o}>{o}</option>)}
        </select>
      </div>
      {needsSource && (
        <div className="field">
          <label>Source (JSON array or {'{{'+'node.field'+'}}'}</label>
          <input placeholder='{{trigger.items}} or [1,2,3]' value={str('source', '')} onChange={(e) => set('source', e.target.value)} />
        </div>
      )}
      {needsSource2 && (
        <div className="field">
          <label>Source 2 (second array for zip)</label>
          <input placeholder='{{node2.items}}' value={str('source2', '')} onChange={(e) => set('source2', e.target.value)} />
        </div>
      )}
      {needsSize && (
        <div className="field">
          <label>Chunk Size</label>
          <input type="number" min={1} value={num('size', 2)} onChange={(e) => set('size', Number(e.target.value))} />
        </div>
      )}
      {needsN && (
        <div className="field">
          <label>N</label>
          <input type="number" min={1} value={num('n', 1)} onChange={(e) => set('n', Number(e.target.value))} />
        </div>
      )}
      {needsRange && (
        <>
          <div className="field">
            <label>Start</label>
            <input type="number" value={num('start', 0)} onChange={(e) => set('start', Number(e.target.value))} />
          </div>
          <div className="field">
            <label>End (exclusive)</label>
            <input type="number" value={num('end', 10)} onChange={(e) => set('end', Number(e.target.value))} />
          </div>
          <div className="field">
            <label>Step</label>
            <input type="number" value={num('step', 1)} onChange={(e) => set('step', Number(e.target.value))} />
          </div>
        </>
      )}
      {needsField && (
        <div className="field">
          <label>Field (dot path to pluck)</label>
          <input placeholder="name" value={str('field', '')} onChange={(e) => set('field', e.target.value)} />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ items, count }'}</code>
      </p>
    </>
  )
}

export function ValidateConfig({ config, set, str }: ConfigProps) {
  const failOnInvalid = config.fail_on_invalid !== false
  return (
    <>
      <div className="field">
        <label>Source <span style={{ color: 'var(--muted)' }}>(JSON value to validate)</span></label>
        <input
          placeholder="{{trigger.body}}"
          value={str('source')}
          onChange={(e) => set('source', e.target.value)}
        />
      </div>
      <div className="field">
        <label>Schema <span style={{ color: 'var(--muted)' }}>(JSON object: field → type/required)</span></label>
        <textarea
          rows={6}
          placeholder={'{\n  "name": { "type": "string", "required": true },\n  "age":  { "type": "number" }\n}'}
          value={str('schema')}
          onChange={(e) => set('schema', e.target.value)}
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <div className="field" style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
        <input
          id="fail_on_invalid"
          type="checkbox"
          checked={failOnInvalid}
          onChange={(e) => set('fail_on_invalid', e.target.checked)}
          style={{ width: 14, height: 14, cursor: 'pointer' }}
        />
        <label htmlFor="fail_on_invalid" style={{ cursor: 'pointer', marginBottom: 0 }}>
          Fail node when invalid
        </label>
      </div>
      <TemplateHint />
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>
          {'{ "valid": true/false, "errors": [...] }'}
        </code>. Supported types: string, number, boolean, array, object, null.
      </p>
    </>
  )
}

export function RandomConfig({ set, str, num }: ConfigProps) {
  const randType = str('type', 'number')
  return (
    <>
      <div className="field">
        <label>Type</label>
        <select value={randType} onChange={(e) => {
          const v = e.target.value
          set('type', v === 'number_int' ? 'number' : v)
          set('integer', v === 'number_int')
        }}>
          <option value="number">Number (float)</option>
          <option value="number_int">Integer</option>
          <option value="uuid">UUID v4</option>
          <option value="boolean">Boolean</option>
          <option value="pick">Pick from list</option>
        </select>
      </div>
      {(randType === 'number' || randType === 'number_int') && (
        <div style={{ display: 'flex', gap: 8 }}>
          <div className="field" style={{ flex: 1 }}>
            <label>Min</label>
            <input type="number" value={num('min', 0)} onChange={(e) => set('min', Number(e.target.value))} />
          </div>
          <div className="field" style={{ flex: 1 }}>
            <label>Max</label>
            <input type="number" value={num('max', 100)} onChange={(e) => set('max', Number(e.target.value))} />
          </div>
        </div>
      )}
      {randType === 'pick' && (
        <div className="field">
          <label>Items (one per line)</label>
          <textarea
            rows={4}
            placeholder={'apple\nbanana\ncherry'}
            value={Array.isArray((str('_items_raw') ? JSON.parse(str('_items_raw') || '[]') : [])) ? str('_items_raw') : ''}
            onChange={(e) => {
              const lines = e.target.value.split('\n').map((l) => l.trim()).filter(Boolean)
              set('items', lines)
              set('_items_raw', e.target.value)
            }}
            style={{ minHeight: 80 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ "value": ... }'}</code>
      </p>
    </>
  )
}

export function CryptoConfig({ set, str, num }: ConfigProps) {
  const op = str('operation', 'sha256')
  const needsKey = op === 'hmac_sha256'
  const needsLength = op === 'random_hex' || op === 'random_base64'
  const needsSource = !['random_hex', 'random_base64'].includes(op)
  return (
    <>
      <div className="field">
        <label>Operation</label>
        <select value={op} onChange={(e) => set('operation', e.target.value)}>
          <option value="sha256">SHA-256 hash</option>
          <option value="sha512">SHA-512 hash</option>
          <option value="hmac_sha256">HMAC-SHA256 (requires key)</option>
          <option value="base64_encode">Base64 encode</option>
          <option value="base64_decode">Base64 decode</option>
          <option value="hex_encode">Hex encode</option>
          <option value="hex_decode">Hex decode</option>
          <option value="random_hex">Random hex bytes</option>
          <option value="random_base64">Random base64 bytes</option>
        </select>
      </div>
      {needsSource && (
        <div className="field">
          <label>Source <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input
            value={str('source', '')}
            onChange={(e) => set('source', e.target.value)}
            placeholder="{{input.data}} or literal string"
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      {needsKey && (
        <div className="field">
          <label>HMAC Key <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input
            value={str('key', '')}
            onChange={(e) => set('key', e.target.value)}
            placeholder="{{credential.hmac_key}}"
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      {needsLength && (
        <div className="field">
          <label>Byte length (max 256)</label>
          <input
            type="number"
            min={1}
            max={256}
            value={num('length', 32)}
            onChange={(e) => set('length', Number(e.target.value))}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ result, operation }'}</code>
      </p>
    </>
  )
}

export function DateConfig({ set, str, num }: ConfigProps) {
  const op = str('operation', 'now')
  const needsSource = op !== 'now'
  const needsSource2 = op === 'diff'
  const needsAmount = op === 'add' || op === 'subtract'
  return (
    <>
      <div className="field">
        <label>Operation</label>
        <select value={op} onChange={(e) => set('operation', e.target.value)}>
          <option value="now">now — current UTC time</option>
          <option value="parse">parse — date string → unix/iso</option>
          <option value="format">format — date → formatted string</option>
          <option value="add">add — add duration to date</option>
          <option value="subtract">subtract — subtract duration from date</option>
          <option value="diff">diff — difference between two dates</option>
          <option value="unix_to_iso">unix_to_iso — unix → ISO 8601</option>
          <option value="iso_to_unix">iso_to_unix — ISO → unix timestamp</option>
        </select>
      </div>
      {needsSource && (
        <div className="field">
          <label>Source (unix or ISO 8601) <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input
            value={str('source', '')}
            onChange={(e) => set('source', e.target.value)}
            placeholder="{{input.timestamp}} or 2024-01-01T00:00:00Z"
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      {needsSource2 && (
        <div className="field">
          <label>Source 2 (for diff)</label>
          <input
            value={str('source2', '')}
            onChange={(e) => set('source2', e.target.value)}
            placeholder="{{input.end_timestamp}}"
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      {needsAmount && (
        <>
          <div className="field">
            <label>Amount</label>
            <input
              type="number"
              value={num('amount', 1)}
              onChange={(e) => set('amount', Number(e.target.value))}
            />
          </div>
          <div className="field">
            <label>Unit</label>
            <select value={str('unit', 'seconds')} onChange={(e) => set('unit', e.target.value)}>
              <option value="seconds">Seconds</option>
              <option value="minutes">Minutes</option>
              <option value="hours">Hours</option>
              <option value="days">Days</option>
              <option value="weeks">Weeks</option>
            </select>
          </div>
        </>
      )}
      <div className="field">
        <label>Output format (strftime)</label>
        <input
          value={str('format', '%Y-%m-%dT%H:%M:%SZ')}
          onChange={(e) => set('format', e.target.value)}
          placeholder="%Y-%m-%dT%H:%M:%SZ"
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ unix, iso, formatted }'}</code>
        {op === 'diff' && <> or <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ seconds, minutes, hours, days }'}</code></>}
      </p>
    </>
  )
}

export function NoteConfig({ set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Note text</label>
        <textarea
          rows={5}
          placeholder="Add documentation or context for this part of the workflow…"
          value={str('text')}
          onChange={(e) => set('text', e.target.value)}
          style={{ fontFamily: 'inherit', fontSize: 13, lineHeight: 1.6 }}
        />
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Notes are purely decorative — they do not affect execution or data flow.
        The text appears as a preview on the canvas node.
      </p>
    </>
  )
}

// ── Local helpers ─────────────────────────────────────────────────────────────

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
