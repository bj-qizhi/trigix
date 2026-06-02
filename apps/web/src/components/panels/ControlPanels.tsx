// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import type { ConfigProps } from './types'

export function ConditionConfig({ set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Field (from input_json) *</label>
        <input
          placeholder="status"
          value={str('field')}
          onChange={(e) => set('field', e.target.value)}
        />
      </div>
      <div className="field">
        <label>Equals <span style={{ color: 'var(--muted)' }}>(blank = check presence)</span></label>
        <input
          placeholder="active"
          value={str('equals')}
          onChange={(e) => set('equals', e.target.value || undefined as unknown as string)}
        />
      </div>
      <TemplateHint />
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{"{ result: true | false }"}</code> as output.
      </p>
    </>
  )
}

export function FanOutConfig() {
  return (
    <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
      Splits execution into parallel branches. Draw edges from this node to each branch's first node.
      Outputs{' '}
      <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>
        {'{ "ok": true, "input": {...} }'}
      </code>
      .
    </p>
  )
}

export function FanInConfig() {
  return (
    <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
      Collects results from all incoming branches. Draw edges from each branch's last node to this node.
      Outputs{' '}
      <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>
        {'{ "count": N, "results": [...] }'}
      </code>
      . Access individual results as <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{{fan_in_id.results[0]}}'}</code>.
    </p>
  )
}

export function CatchConfig({ set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Source node ID <span style={{ color: 'var(--muted)' }}>(optional)</span></label>
        <input
          placeholder="http_1"
          value={str('source')}
          onChange={(e) => set('source', e.target.value)}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>
          If set, reads the error from that specific node. Leave empty to auto-detect.
        </span>
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Connect an <strong style={{ color: 'var(--node-catch)' }}>error</strong> edge from any
        node to this Catch node. On failure, execution continues here instead of stopping.
        The caught error is available as{' '}
        <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>
          {'{{catch_id.error}}'}
        </code>{' '}
        in downstream nodes.
      </p>
    </>
  )
}

export function AssertConfig({ set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Condition * <span style={{ color: 'var(--muted)' }}>(<code>{'{{...}}'}</code> expression)</span></label>
        <textarea
          rows={2}
          placeholder="{{filter.count}}"
          value={str('condition')}
          onChange={(e) => set('condition', e.target.value)}
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>
          Truthy values: any non-empty string except "false", "null", "0".
        </span>
      </div>
      <div className="field">
        <label>Failure message <span style={{ color: 'var(--muted)' }}>(optional)</span></label>
        <input
          placeholder="Assertion failed"
          value={str('message')}
          onChange={(e) => set('message', e.target.value)}
        />
      </div>
      <TemplateHint />
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>
          {'{ "ok": true }'}
        </code>{' '}
        or fails the execution with the failure message.
      </p>
    </>
  )
}

export function DelayConfig({ set, num }: ConfigProps) {
  const seconds = num('seconds', 0)
  return (
    <>
      <div className="field">
        <label>Duration (seconds) *</label>
        <input
          type="number"
          min={0}
          max={3600}
          step={1}
          value={seconds}
          onChange={(e) => {
            const val = parseInt(e.target.value, 10)
            set('seconds', isNaN(val) ? 0 : Math.max(0, Math.min(3600, val)))
          }}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>0–3600 seconds (max 1 hour).</span>
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>
          {'{ "waited_secs": N }'}
        </code>
      </p>
    </>
  )
}

export function ForEachConfig({ config, set, str, num }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Items (array expression) <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input
          value={str('items', '{{input.items}}')}
          onChange={(e) => set('items', e.target.value)}
          placeholder="{{input.items}} or {{fetch_data.body.results}}"
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>Must resolve to a JSON array</span>
      </div>
      <div className="field">
        <label>Target Workflow ID <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input
          value={str('workflow_id', '')}
          onChange={(e) => set('workflow_id', e.target.value)}
          placeholder="Workflow ID"
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>Platform injects the published graph before execution</span>
      </div>
      <div className="field">
        <label>Input Key</label>
        <input
          value={str('input_key', 'item')}
          onChange={(e) => set('input_key', e.target.value)}
          placeholder="item"
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>Each item passed as {'{"<key>": <item>}'}</span>
      </div>
      <div className="field">
        <label>Max Concurrency</label>
        <input
          type="number"
          min={1}
          max={50}
          value={num('max_concurrency', 10)}
          onChange={(e) => set('max_concurrency', Number(e.target.value))}
        />
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ results, succeeded, failed, total }'}</code>
      </p>
    </>
  )
}

export function SwitchConfig({ config, set, str }: ConfigProps) {
  const cases: { match: string; label: string }[] = Array.isArray(config.cases) ? config.cases as { match: string; label: string }[] : []
  const setCases = (updated: { match: string; label: string }[]) => set('cases', updated)
  return (
    <>
      <div className="field">
        <label>Value Expression *</label>
        <input
          placeholder="{{input.status}}"
          value={str('value', '{{input}}')}
          onChange={(e) => set('value', e.target.value)}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>The value to match against cases. Supports {'{{...}}'} templates.</span>
      </div>
      <div className="field">
        <label>Cases</label>
        {cases.map((c, i) => (
          <div key={i} style={{ display: 'flex', gap: 6, marginBottom: 4, alignItems: 'center' }}>
            <input
              placeholder="match value"
              value={c.match}
              onChange={(e) => {
                const next = [...cases]
                next[i] = { ...next[i], match: e.target.value }
                setCases(next)
              }}
              style={{ flex: 1 }}
            />
            <span style={{ color: 'var(--muted)', fontSize: 12 }}>→</span>
            <input
              placeholder="edge label"
              value={c.label}
              onChange={(e) => {
                const next = [...cases]
                next[i] = { ...next[i], label: e.target.value }
                setCases(next)
              }}
              style={{ flex: 1 }}
            />
            <button
              onClick={() => setCases(cases.filter((_, j) => j !== i))}
              style={{ background: 'none', border: 'none', color: 'var(--danger-text)', cursor: 'pointer', fontSize: 14, padding: '0 2px' }}
            >✕</button>
          </div>
        ))}
        <button
          className="btn btn-secondary"
          style={{ marginTop: 4, fontSize: 12 }}
          onClick={() => setCases([...cases, { match: '', label: '' }])}
        >+ Add case</button>
        <span style={{ fontSize: 11, color: 'var(--muted)', marginTop: 4, display: 'block' }}>
          Add a case with match="*" to catch all unmatched values (default branch).
        </span>
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ "value": "...", "matched_case": "label", "matched": bool }'}</code>.<br/>
        Outgoing edges whose <strong>condition_label</strong> equals the matched case label will be followed.
      </p>
    </>
  )
}

export function LoopConfig({ set, str, num }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Items *</label>
        <input
          placeholder="{{input.items}} or {{fetch_node}}"
          value={str('items')}
          onChange={(e) => set('items', e.target.value)}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>Template expression resolving to a JSON array.</span>
      </div>
      <div className="field">
        <label>Max iterations <span style={{ color: 'var(--muted)' }}>(1–1000)</span></label>
        <input
          type="number" min={1} max={1000}
          value={num('max_iterations', 100)}
          onChange={(e) => set('max_iterations', Number(e.target.value))}
          style={{ width: 100 }}
        />
      </div>
      <div className="field">
        <label>Until path <span style={{ color: 'var(--muted)' }}>(optional — stops when falsy)</span></label>
        <input
          placeholder="active"
          value={str('until')}
          onChange={(e) => set('until', e.target.value)}
          style={{ fontFamily: 'monospace' }}
        />
      </div>
      <div className="field">
        <label>Item template <span style={{ color: 'var(--muted)' }}>(optional JSON)</span></label>
        <textarea
          rows={3}
          placeholder={'{ "id": "{{item.id}}", "name": "{{item.name}}" }'}
          value={str('template')}
          onChange={(e) => set('template', e.target.value)}
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>
          {'{ "count": N, "results": [...] }'}
        </code>
      </p>
    </>
  )
}

export function MapConfig({ set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Items expression *</label>
        <input
          placeholder="{{trigger.leads}}"
          value={str('items')}
          onChange={(e) => set('items', e.target.value)}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>
          Must resolve to a JSON array.
        </span>
      </div>
      <div className="field">
        <label>
          Item template{' '}
          <span style={{ color: 'var(--muted)' }}>(optional JSON — use <code>{'{{item.field}}'}</code>)</span>
        </label>
        <textarea
          rows={4}
          placeholder={'{\n  "name": "{{item.name}}"\n}'}
          value={str('item_template') ? JSON.stringify(str('item_template') as unknown, null, 2) : ''}
          onChange={(e) => {
            const raw = e.target.value.trim()
            if (!raw) { set('item_template', undefined as unknown as string); return }
            try { set('item_template', JSON.parse(raw)) } catch { /* keep editing */ }
          }}
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <TemplateHint />
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>
          {'{ "count": N, "items": [...] }'}
        </code>
      </p>
    </>
  )
}

export function SortConfig({ set, str }: ConfigProps) {
  const order = str('order') || 'asc'
  const type = str('type') || 'string'
  return (
    <>
      <div className="field">
        <label>Items expression *</label>
        <input
          placeholder="{{trigger.rows}}"
          value={str('items')}
          onChange={(e) => set('items', e.target.value)}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>Must resolve to a JSON array.</span>
      </div>
      <div className="field">
        <label>Sort field *</label>
        <input
          placeholder="name"
          value={str('field')}
          onChange={(e) => set('field', e.target.value)}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>Dot-path on each item (e.g. <code>score</code> or <code>meta.rank</code>).</span>
      </div>
      <div style={{ display: 'flex', gap: 12 }}>
        <div className="field" style={{ flex: 1 }}>
          <label>Order</label>
          <select
            value={order}
            onChange={(e) => set('order', e.target.value)}
            style={{ background: 'var(--surface)', color: 'var(--text)', border: '1px solid var(--border)', borderRadius: 'var(--radius)', padding: '4px 8px', fontSize: 13, width: '100%' }}
          >
            <option value="asc">asc</option>
            <option value="desc">desc</option>
          </select>
        </div>
        <div className="field" style={{ flex: 1 }}>
          <label>Compare as</label>
          <select
            value={type}
            onChange={(e) => set('type', e.target.value)}
            style={{ background: 'var(--surface)', color: 'var(--text)', border: '1px solid var(--border)', borderRadius: 'var(--radius)', padding: '4px 8px', fontSize: 13, width: '100%' }}
          >
            <option value="string">string</option>
            <option value="number">number</option>
          </select>
        </div>
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>
          {'{ "count": N, "items": [...] }'}
        </code> in sorted order.
      </p>
    </>
  )
}

export function AggregateConfig({ set, str }: ConfigProps) {
  const operations = ['count', 'sum', 'avg', 'min', 'max', 'join', 'first', 'last']
  const operation = str('operation') || 'count'
  const needsField = operation !== 'count'
  const isJoin = operation === 'join'
  return (
    <>
      <div className="field">
        <label>Items expression *</label>
        <input
          placeholder="{{trigger.rows}}"
          value={str('items')}
          onChange={(e) => set('items', e.target.value)}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>Must resolve to a JSON array.</span>
      </div>
      <div className="field">
        <label>Operation *</label>
        <select
          value={operation}
          onChange={(e) => set('operation', e.target.value)}
          style={{ background: 'var(--surface)', color: 'var(--text)', border: '1px solid var(--border)', borderRadius: 'var(--radius)', padding: '4px 8px', fontSize: 13 }}
        >
          {operations.map((op) => (
            <option key={op} value={op}>{op}</option>
          ))}
        </select>
      </div>
      {needsField && (
        <div className="field">
          <label>Field {operation !== 'first' && operation !== 'last' ? '*' : ''}</label>
          <input
            placeholder="score"
            value={str('field')}
            onChange={(e) => set('field', e.target.value)}
          />
          <span style={{ fontSize: 11, color: 'var(--muted)' }}>Dot-path on each item (e.g. <code>score</code> or <code>meta.value</code>).</span>
        </div>
      )}
      {isJoin && (
        <div className="field">
          <label>Separator <span style={{ color: 'var(--muted)' }}>(default ", ")</span></label>
          <input
            placeholder=", "
            value={str('separator')}
            onChange={(e) => set('separator', e.target.value)}
          />
        </div>
      )}
      <TemplateHint />
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>
          {'{ "result": <value> }'}
        </code>
      </p>
    </>
  )
}

export function FilterConfig({ set, str }: ConfigProps) {
  const operators = ['exists', 'not_exists', 'equals', 'not_equals', 'contains', 'gt', 'lt']
  const operator = str('operator') || 'exists'
  const needsValue = ['equals', 'not_equals', 'contains', 'gt', 'lt'].includes(operator)
  return (
    <>
      <div className="field">
        <label>Items expression *</label>
        <input
          placeholder="{{trigger.users}}"
          value={str('items')}
          onChange={(e) => set('items', e.target.value)}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>Must resolve to a JSON array.</span>
      </div>
      <div className="field">
        <label>Field *</label>
        <input
          placeholder="status"
          value={str('field')}
          onChange={(e) => set('field', e.target.value)}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>Field path on each item (e.g. <code>active</code> or <code>profile.age</code>).</span>
      </div>
      <div className="field">
        <label>Operator</label>
        <select
          value={operator}
          onChange={(e) => set('operator', e.target.value)}
          style={{ background: 'var(--surface)', color: 'var(--text)', border: '1px solid var(--border)', borderRadius: 'var(--radius)', padding: '4px 8px', fontSize: 13 }}
        >
          {operators.map((op) => (
            <option key={op} value={op}>{op}</option>
          ))}
        </select>
      </div>
      {needsValue && (
        <div className="field">
          <label>Value *</label>
          <input
            placeholder="active"
            value={str('value')}
            onChange={(e) => set('value', e.target.value)}
          />
        </div>
      )}
      <TemplateHint />
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>
          {'{ "count": N, "items": [...] }'}
        </code> with only matching items.
      </p>
    </>
  )
}

export function SplitConfig({ set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Source *</label>
        <input
          placeholder="{{input.csv_line}}"
          value={str('source', '{{input}}')}
          onChange={(e) => set('source', e.target.value)}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>Template expression returning the string to split.</span>
      </div>
      <div className="field">
        <label>Delimiter</label>
        <input
          placeholder=","
          value={str('delimiter', ',')}
          onChange={(e) => set('delimiter', e.target.value)}
        />
      </div>
      <div className="field" style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
        <input
          type="checkbox"
          id="split-trim"
          checked={str('trim', 'true') !== 'false'}
          onChange={(e) => set('trim', e.target.checked)}
        />
        <label htmlFor="split-trim" style={{ cursor: 'pointer' }}>Trim whitespace from each part</label>
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ "parts": [...], "count": N }'}</code>
      </p>
    </>
  )
}

export function JoinConfig({ set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Items Array *</label>
        <input
          placeholder="{{split_node.parts}}"
          value={str('items', '{{input}}')}
          onChange={(e) => set('items', e.target.value)}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>Template expression returning an array of strings (or objects).</span>
      </div>
      <div className="field">
        <label>Delimiter</label>
        <input
          placeholder=","
          value={str('delimiter', ',')}
          onChange={(e) => set('delimiter', e.target.value)}
        />
      </div>
      <div className="field">
        <label>Field <span style={{ color: 'var(--muted)' }}>(optional, for object arrays)</span></label>
        <input
          placeholder="name"
          value={str('field')}
          onChange={(e) => set('field', e.target.value)}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>Dot-path to extract from each object (e.g. <code>user.name</code>).</span>
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ "result": "a,b,c", "count": N }'}</code>
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
