// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import type { ConfigProps } from './types'
import { IconX } from '../uiIcons'
import { fl } from './i18nLabels'

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
        {fl("Returns the rendered template value as output JSON.")}
      </p>
    </>
  )
}

export function ExtractConfig({ set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>{fl("Source *")}</label>
        <input
          placeholder="{{input}} or {{node_id}}"
          value={str('source')}
          onChange={(e) => set('source', e.target.value)}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>{fl("Template expression resolving to a JSON object.")}</span>
      </div>
      <div className="field">
        <label>{fl("Path *")}</label>
        <input
          placeholder="data.users.0.email"
          value={str('path')}
          onChange={(e) => set('path', e.target.value)}
          style={{ fontFamily: 'monospace' }}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>{fl("Dot-separated path into the source JSON.")}</span>
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>
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
        <label>{fl("Fields to merge")}</label>
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
            <button className="btn btn-sm btn-danger" onClick={() => update(fields.filter((_, j) => j !== i))}><IconX aria-hidden /></button>
          </div>
        ))}
        <button className="btn btn-sm" onClick={() => update([...fields, { source: '' }])}>{fl("+ Add field")}</button>
        <span style={{ fontSize: 11, color: 'var(--muted)', display: 'block', marginTop: 4 }}>
          {fl("If Key is empty, merges all top-level fields from Source. Returns merged object.")}
        </span>
      </div>
    </>
  )
}

export function DedupeConfig({ set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>{fl("Items Array *")}</label>
        <input
          placeholder="{{filter_node.items}}"
          value={str('items', '{{input}}')}
          onChange={(e) => set('items', e.target.value)}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>{fl("Template expression returning an array.")}</span>
      </div>
      <div className="field">
        <label>{fl("Dedupe Field")} <span style={{ color: 'var(--muted)' }}>{fl("(optional)")}</span></label>
        <input
          placeholder="id"
          value={str('field')}
          onChange={(e) => set('field', e.target.value)}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>{fl("Dot-path to dedupe by. Leave blank to compare whole items.")}</span>
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ "items": [...], "count": N, "removed_count": M }'}</code>
      </p>
    </>
  )
}

export function RegexConfig({ set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>{fl("Source *")}</label>
        <input
          placeholder="{{input.text}}"
          value={str('source', '{{input}}')}
          onChange={(e) => set('source', e.target.value)}
        />
      </div>
      <div className="field">
        <label>{fl("Pattern *")}</label>
        <input
          placeholder="error|warning"
          value={str('pattern')}
          onChange={(e) => set('pattern', e.target.value)}
          style={{ fontFamily: 'monospace' }}
        />
      </div>
      <div className="field">
        <label>{fl("Flags")} <span style={{ color: 'var(--muted)' }}>{fl("(optional)")}</span></label>
        <input
          placeholder="i  (case-insensitive)"
          value={str('flags')}
          onChange={(e) => set('flags', e.target.value)}
          style={{ width: 80 }}
        />
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ "matched": bool, "full_match": "...", "groups": [] }'}</code>
      </p>
    </>
  )
}

export function CsvConfig({ set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>{fl("Source *")}</label>
        <input
          placeholder="{{input.csv_data}}"
          value={str('source', '{{input}}')}
          onChange={(e) => set('source', e.target.value)}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>{fl("Template expression returning a CSV string.")}</span>
      </div>
      <div className="field">
        <label>{fl("Delimiter")}</label>
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
        <label htmlFor="csv-header" style={{ cursor: 'pointer' }}>{fl("First row is header")}</label>
      </div>
      <div className="field" style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
        <input
          type="checkbox"
          id="csv-trim"
          checked={str('trim', 'true') !== 'false'}
          onChange={(e) => set('trim', e.target.checked)}
        />
        <label htmlFor="csv-trim" style={{ cursor: 'pointer' }}>{fl("Trim cell whitespace")}</label>
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ "rows": [{...}], "count": N, "headers": [...] }'}</code>
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
        <label>{fl("Source Object *")}</label>
        <input
          placeholder="{{input}}"
          value={str('source', '{{input}}')}
          onChange={(e) => set('source', e.target.value)}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>{fl("Template expression returning a JSON object.")}</span>
      </div>
      <div className="field">
        <label>{fl("Key Mappings")}</label>
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
              style={{ background: 'none', border: 'none', color: 'var(--danger-text)', cursor: 'pointer', fontSize: 14 }}><IconX aria-hidden /></button>
          </div>
        ))}
        <button className="btn btn-secondary" style={{ marginTop: 4, fontSize: 12 }}
          onClick={() => setMappings([...mappings, { from: '', to: '' }])}>{fl("+ Add mapping")}</button>
        <span style={{ fontSize: 11, color: 'var(--muted)', marginTop: 4, display: 'block' }}>{fl("Unmapped keys are passed through unchanged.")}</span>
      </div>
    </>
  )
}

export function FormatConfig({ set, str, num }: ConfigProps) {
  const op = str('operation', 'uppercase')
  return (
    <>
      <div className="field">
        <label>{fl("Source *")}</label>
        <input placeholder="{{input.text}}" value={str('source', '{{input}}')} onChange={(e) => set('source', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Operation")}</label>
        <select value={op} onChange={(e) => set('operation', e.target.value)}>
          <option value="uppercase">{fl("UPPERCASE")}</option>
          <option value="lowercase">{fl("lowercase")}</option>
          <option value="trim">{fl("Trim whitespace")}</option>
          <option value="trim_start">{fl("Trim start")}</option>
          <option value="trim_end">{fl("Trim end")}</option>
          <option value="reverse">{fl("Reverse string")}</option>
          <option value="length">{fl("String length (number)")}</option>
          <option value="word_count">{fl("Word count (number)")}</option>
          <option value="to_number">{fl("Parse as number")}</option>
          <option value="to_bool">{fl("Parse as boolean")}</option>
          <option value="replace">{fl("Find & replace")}</option>
          <option value="pad_start">{fl("Pad start")}</option>
          <option value="truncate">{fl("Truncate")}</option>
        </select>
      </div>
      {op === 'replace' && (
        <div style={{ display: 'flex', gap: 8 }}>
          <div className="field" style={{ flex: 1 }}>
            <label>{fl("Find")}</label>
            <input value={str('from')} onChange={(e) => set('from', e.target.value)} />
          </div>
          <div className="field" style={{ flex: 1 }}>
            <label>{fl("Replace with")}</label>
            <input value={str('to_value')} onChange={(e) => set('to_value', e.target.value)} />
          </div>
        </div>
      )}
      {op === 'pad_start' && (
        <div style={{ display: 'flex', gap: 8 }}>
          <div className="field" style={{ flex: 1 }}>
            <label>{fl("Width")}</label>
            <input type="number" min={1} max={200} value={num('width', 10)} onChange={(e) => set('width', Number(e.target.value))} />
          </div>
          <div className="field" style={{ flex: 1 }}>
            <label>{fl("Pad char")}</label>
            <input maxLength={1} value={str('pad_char', '0')} onChange={(e) => set('pad_char', e.target.value)} style={{ width: 50 }} />
          </div>
        </div>
      )}
      {op === 'truncate' && (
        <div style={{ display: 'flex', gap: 8 }}>
          <div className="field" style={{ flex: 1 }}>
            <label>{fl("Max length")}</label>
            <input type="number" min={1} value={num('max_length', 100)} onChange={(e) => set('max_length', Number(e.target.value))} />
          </div>
          <div className="field" style={{ flex: 1 }}>
            <label>{fl("Suffix")}</label>
            <input value={str('suffix', '…')} onChange={(e) => set('suffix', e.target.value)} style={{ width: 60 }} />
          </div>
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ "result": ..., "operation": "..." }'}</code>
      </p>
    </>
  )
}

export function XmlConfig({ set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>{fl("Source (XML string)")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea
          rows={4}
          value={str('source', '')}
          onChange={(e) => set('source', e.target.value)}
          placeholder="{{http_node.body}} or <root><item>1</item></root>"
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>{fl("Template expressions supported. Attributes become @attr keys.")}</span>
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ data: object }'}</code>
      </p>
    </>
  )
}

export function YamlConfig({ set, str }: ConfigProps) {
  const mode = str('mode', 'parse')
  return (
    <>
      <div className="field">
        <label>{fl("Mode")}</label>
        <select value={mode} onChange={(e) => set('mode', e.target.value)}>
          <option value="parse">{fl("Parse — YAML string → JSON object")}</option>
          <option value="serialize">{fl("Serialize — JSON value → YAML string")}</option>
        </select>
      </div>
      <div className="field">
        <label>{fl("Source")} <span style={{ color: 'var(--danger)' }}>*</span></label>
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
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>
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
        <label>{fl("Template")} <span style={{ color: 'var(--danger)' }}>*</span></label>
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
        <label>{fl("Data (JSON expression)")}</label>
        <input
          value={str('data', '{{input}}')}
          onChange={(e) => set('data', e.target.value)}
          placeholder="{{input}} or {{transform_node.output}}"
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>{fl("Resolved first, then used as Handlebars context object")}</span>
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ result: string }'}</code>
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
        <label>{fl("Operation")}</label>
        <select value={op} onChange={(e) => set('operation', e.target.value)}>
          {MATH_OPS.map((o) => <option key={o} value={o}>{o}</option>)}
        </select>
      </div>
      {needsA && (
        <div className="field">
          <label>{fl("a")}</label>
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
            <label>{fl("Min")}</label>
            <input type="number" step="any" value={num('min', 0)} onChange={(e) => set('min', Number(e.target.value))} />
          </div>
          <div className="field">
            <label>{fl("Max")}</label>
            <input type="number" step="any" value={num('max', 100)} onChange={(e) => set('max', Number(e.target.value))} />
          </div>
        </>
      )}
      {['round', 'avg'].includes(op) && (
        <div className="field">
          <label>{fl("Precision (decimal places)")}</label>
          <input type="number" min={0} max={15} value={num('precision', 2)} onChange={(e) => set('precision', Number(e.target.value))} />
        </div>
      )}
      {needsExpr && (
        <div className="field">
          <label>{fl("Expression (Rhai)")}</label>
          <input placeholder="2 + 2 * PI" value={str('expression', '')} onChange={(e) => set('expression', e.target.value)} />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ result, operation }'}</code>
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
        <label>{fl("Operation")}</label>
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
          <label>{fl("Source 2 (second array for zip)")}</label>
          <input placeholder='{{node2.items}}' value={str('source2', '')} onChange={(e) => set('source2', e.target.value)} />
        </div>
      )}
      {needsSize && (
        <div className="field">
          <label>{fl("Chunk Size")}</label>
          <input type="number" min={1} value={num('size', 2)} onChange={(e) => set('size', Number(e.target.value))} />
        </div>
      )}
      {needsN && (
        <div className="field">
          <label>{fl("N")}</label>
          <input type="number" min={1} value={num('n', 1)} onChange={(e) => set('n', Number(e.target.value))} />
        </div>
      )}
      {needsRange && (
        <>
          <div className="field">
            <label>{fl("Start")}</label>
            <input type="number" value={num('start', 0)} onChange={(e) => set('start', Number(e.target.value))} />
          </div>
          <div className="field">
            <label>{fl("End (exclusive)")}</label>
            <input type="number" value={num('end', 10)} onChange={(e) => set('end', Number(e.target.value))} />
          </div>
          <div className="field">
            <label>{fl("Step")}</label>
            <input type="number" value={num('step', 1)} onChange={(e) => set('step', Number(e.target.value))} />
          </div>
        </>
      )}
      {needsField && (
        <div className="field">
          <label>{fl("Field (dot path to pluck)")}</label>
          <input placeholder="name" value={str('field', '')} onChange={(e) => set('field', e.target.value)} />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ items, count }'}</code>
      </p>
    </>
  )
}

export function ValidateConfig({ config, set, str }: ConfigProps) {
  const failOnInvalid = config.fail_on_invalid !== false
  return (
    <>
      <div className="field">
        <label>{fl("Source")} <span style={{ color: 'var(--muted)' }}>{fl("(JSON value to validate)")}</span></label>
        <input
          placeholder="{{trigger.body}}"
          value={str('source')}
          onChange={(e) => set('source', e.target.value)}
        />
      </div>
      <div className="field">
        <label>{fl("Schema")} <span style={{ color: 'var(--muted)' }}>{fl("(JSON object: field → type/required)")}</span></label>
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
        <label htmlFor="fail_on_invalid" style={{ cursor: 'pointer', marginBottom: 0 }}>{fl("Fail node when invalid")}</label>
      </div>
      <TemplateHint />
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>
          {'{ "valid": true/false, "errors": [...] }'}
        </code>{fl(". Supported types: string, number, boolean, array, object, null.")}
      </p>
    </>
  )
}

export function RandomConfig({ set, str, num }: ConfigProps) {
  const randType = str('type', 'number')
  return (
    <>
      <div className="field">
        <label>{fl("Type")}</label>
        <select value={randType} onChange={(e) => {
          const v = e.target.value
          set('type', v === 'number_int' ? 'number' : v)
          set('integer', v === 'number_int')
        }}>
          <option value="number">{fl("Number (float)")}</option>
          <option value="number_int">{fl("Integer")}</option>
          <option value="uuid">{fl("UUID v4")}</option>
          <option value="boolean">{fl("Boolean")}</option>
          <option value="pick">{fl("Pick from list")}</option>
        </select>
      </div>
      {(randType === 'number' || randType === 'number_int') && (
        <div style={{ display: 'flex', gap: 8 }}>
          <div className="field" style={{ flex: 1 }}>
            <label>{fl("Min")}</label>
            <input type="number" value={num('min', 0)} onChange={(e) => set('min', Number(e.target.value))} />
          </div>
          <div className="field" style={{ flex: 1 }}>
            <label>{fl("Max")}</label>
            <input type="number" value={num('max', 100)} onChange={(e) => set('max', Number(e.target.value))} />
          </div>
        </div>
      )}
      {randType === 'pick' && (
        <div className="field">
          <label>{fl("Items (one per line)")}</label>
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
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ "value": ... }'}</code>
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
        <label>{fl("Operation")}</label>
        <select value={op} onChange={(e) => set('operation', e.target.value)}>
          <option value="sha256">{fl("SHA-256 hash")}</option>
          <option value="sha512">{fl("SHA-512 hash")}</option>
          <option value="hmac_sha256">{fl("HMAC-SHA256 (requires key)")}</option>
          <option value="base64_encode">{fl("Base64 encode")}</option>
          <option value="base64_decode">{fl("Base64 decode")}</option>
          <option value="hex_encode">{fl("Hex encode")}</option>
          <option value="hex_decode">{fl("Hex decode")}</option>
          <option value="random_hex">{fl("Random hex bytes")}</option>
          <option value="random_base64">{fl("Random base64 bytes")}</option>
        </select>
      </div>
      {needsSource && (
        <div className="field">
          <label>{fl("Source")} <span style={{ color: 'var(--danger)' }}>*</span></label>
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
          <label>{fl("HMAC Key")} <span style={{ color: 'var(--danger)' }}>*</span></label>
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
          <label>{fl("Byte length (max 256)")}</label>
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
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ result, operation }'}</code>
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
        <label>{fl("Operation")}</label>
        <select value={op} onChange={(e) => set('operation', e.target.value)}>
          <option value="now">{fl("now — current UTC time")}</option>
          <option value="parse">{fl("parse — date string → unix/iso")}</option>
          <option value="format">{fl("format — date → formatted string")}</option>
          <option value="add">{fl("add — add duration to date")}</option>
          <option value="subtract">{fl("subtract — subtract duration from date")}</option>
          <option value="diff">{fl("diff — difference between two dates")}</option>
          <option value="unix_to_iso">{fl("unix_to_iso — unix → ISO 8601")}</option>
          <option value="iso_to_unix">{fl("iso_to_unix — ISO → unix timestamp")}</option>
        </select>
      </div>
      {needsSource && (
        <div className="field">
          <label>{fl("Source (unix or ISO 8601)")} <span style={{ color: 'var(--danger)' }}>*</span></label>
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
          <label>{fl("Source 2 (for diff)")}</label>
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
            <label>{fl("Amount")}</label>
            <input
              type="number"
              value={num('amount', 1)}
              onChange={(e) => set('amount', Number(e.target.value))}
            />
          </div>
          <div className="field">
            <label>{fl("Unit")}</label>
            <select value={str('unit', 'seconds')} onChange={(e) => set('unit', e.target.value)}>
              <option value="seconds">{fl("Seconds")}</option>
              <option value="minutes">{fl("Minutes")}</option>
              <option value="hours">{fl("Hours")}</option>
              <option value="days">{fl("Days")}</option>
              <option value="weeks">{fl("Weeks")}</option>
            </select>
          </div>
        </>
      )}
      <div className="field">
        <label>{fl("Output format (strftime)")}</label>
        <input
          value={str('format', '%Y-%m-%dT%H:%M:%SZ')}
          onChange={(e) => set('format', e.target.value)}
          placeholder="%Y-%m-%dT%H:%M:%SZ"
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ unix, iso, formatted }'}</code>
        {op === 'diff' && <> {fl("or")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ seconds, minutes, hours, days }'}</code></>}
      </p>
    </>
  )
}

export function NoteConfig({ set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>{fl("Note text")}</label>
        <textarea
          rows={5}
          placeholder="Add documentation or context for this part of the workflow…"
          value={str('text')}
          onChange={(e) => set('text', e.target.value)}
          style={{ fontFamily: 'inherit', fontSize: 13, lineHeight: 1.6 }}
        />
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Notes are purely decorative — they do not affect execution or data flow.\n        The text appears as a preview on the canvas node.")}
      </p>
    </>
  )
}

// ── Local helpers ─────────────────────────────────────────────────────────────

function TemplateHint() {
  return (
    <p style={{ fontSize: 11, color: 'var(--muted)', marginTop: -6, lineHeight: 1.6 }}>
      {fl("Templates:")}{' '}
      <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{{input.field}}'}</code>
      {' · '}
      <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{{node_id.field}}'}</code>
      {' · '}
      <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{{credential.name}}'}</code>
    </p>
  )
}
