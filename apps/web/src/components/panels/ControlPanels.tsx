// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import type { ConfigProps } from './types'
import { IconX } from '../uiIcons'
import { fl, labelLocale } from './i18nLabels'

const CONDITION_OPS: { v: string; l: string }[] = [
  { v: 'equals', l: '= equals' },
  { v: 'not_equals', l: '≠ not equals' },
  { v: 'contains', l: 'contains' },
  { v: 'gt', l: '> greater than' },
  { v: 'lt', l: '< less than' },
  { v: 'gte', l: '≥ at least' },
  { v: 'lte', l: '≤ at most' },
  { v: 'exists', l: 'exists (has a value)' },
  { v: 'not_exists', l: 'does not exist' },
]

export function ConditionConfig({ config, set, str }: ConfigProps) {
  // Default to the legacy `equals` form when present, else `exists`.
  const op = str('operator') || (config['equals'] !== undefined ? 'equals' : 'exists')
  const needsValue = op !== 'exists' && op !== 'not_exists'
  const clear = (k: string) => set(k, undefined as unknown as string)
  return (
    <>
      <div className="field">
        <label>{fl("Source")} <span style={{ color: 'var(--muted)' }}>{fl("(optional)")}</span></label>
        <input
          placeholder="{{previous_node}}"
          value={str('source')}
          onChange={(e) => set('source', e.target.value || (undefined as unknown as string))}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>
          If set, Field is a dot-path into that node&apos;s output. Otherwise Field is read from the workflow input (or a {'{{...}}'} expression).
        </span>
      </div>
      <div className="field">
        <label>{fl("Field *")}</label>
        <input
          placeholder="status"
          value={str('field')}
          onChange={(e) => set('field', e.target.value)}
        />
      </div>
      <div className="field">
        <label>{fl("Operator")}</label>
        <select
          value={op}
          onChange={(e) => {
            const v = e.target.value
            set('operator', v)
            clear('equals')
            if (v === 'exists' || v === 'not_exists') clear('value')
          }}
        >
          {CONDITION_OPS.map((o) => <option key={o.v} value={o.v}>{o.l}</option>)}
        </select>
      </div>
      {needsValue && (
        <div className="field">
          <label>{fl("Value")}</label>
          <input
            placeholder="active"
            value={str('value') || str('equals')}
            onChange={(e) => { set('value', e.target.value); clear('equals') }}
          />
        </div>
      )}
      <TemplateHint />
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Routes the true/false branches. Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{"{ result: true | false }"}</code>.
      </p>
    </>
  )
}

export function FanOutConfig() {
  return (
    <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
      {fl("Splits execution into parallel branches. Draw edges from this node to each branch's first node.\n      Outputs")}{' '}
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
      {fl("Collects results from all incoming branches. Draw edges from each branch's last node to this node.\n      Outputs")}{' '}
      <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>
        {'{ "count": N, "results": [...] }'}
      </code>
      {fl(". Access individual results as")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{{fan_in_id.results[0]}}'}</code>.
    </p>
  )
}

export function CatchConfig({ set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>{fl("Source node ID")} <span style={{ color: 'var(--muted)' }}>{fl("(optional)")}</span></label>
        <input
          placeholder="http_1"
          value={str('source')}
          onChange={(e) => set('source', e.target.value)}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>
          {fl("If set, reads the error from that specific node. Leave empty to auto-detect.")}
        </span>
      </div>
      {labelLocale() === 'zh' ? (
        <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
          {fl("从任意节点连一条")} <strong style={{ color: 'var(--node-catch)' }}>{fl("error")}</strong> {fl("边到此\n          Catch 节点。出错时执行会在这里继续而不是中断。捕获到的错误可在下游节点通过")}{' '}
          <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>
            {'{{catch_id.error}}'}
          </code>{' '}
          {fl("访问。")}
        </p>
      ) : (
        <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
          {fl("Connect an")} <strong style={{ color: 'var(--node-catch)' }}>{fl("error")}</strong> {fl("edge from any\n          node to this Catch node. On failure, execution continues here instead of stopping.\n          The caught error is available as")}{' '}
          <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>
            {'{{catch_id.error}}'}
          </code>{' '}
          {fl("in downstream nodes.")}
        </p>
      )}
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
          {fl("Truthy values: any non-empty string except \"false\", \"null\", \"0\".")}
        </span>
      </div>
      <div className="field">
        <label>{fl("Failure message")} <span style={{ color: 'var(--muted)' }}>{fl("(optional)")}</span></label>
        <input
          placeholder="Assertion failed"
          value={str('message')}
          onChange={(e) => set('message', e.target.value)}
        />
      </div>
      <TemplateHint />
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>
          {'{ "ok": true }'}
        </code>{' '}
        {fl("or fails the execution with the failure message.")}
      </p>
    </>
  )
}

export function DelayConfig({ set, num }: ConfigProps) {
  const seconds = num('seconds', 0)
  return (
    <>
      <div className="field">
        <label>{fl("Duration (seconds) *")}</label>
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
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>{fl("0–3600 seconds (max 1 hour).")}</span>
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>
          {'{ "waited_secs": N }'}
        </code>
      </p>
    </>
  )
}

export function ForEachConfig({ set, str, num }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>{fl("Items (array expression)")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input
          value={str('items', '{{input.items}}')}
          onChange={(e) => set('items', e.target.value)}
          placeholder="{{input.items}} or {{fetch_data.body.results}}"
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>{fl("Must resolve to a JSON array")}</span>
      </div>
      <div className="field">
        <label>{fl("Target Workflow ID")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input
          value={str('workflow_id', '')}
          onChange={(e) => set('workflow_id', e.target.value)}
          placeholder="Workflow ID"
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>{fl("Platform injects the published graph before execution")}</span>
      </div>
      <div className="field">
        <label>{fl("Input Key")}</label>
        <input
          value={str('input_key', 'item')}
          onChange={(e) => set('input_key', e.target.value)}
          placeholder="item"
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>Each item passed as {'{"<key>": <item>}'}</span>
      </div>
      <div className="field">
        <label>{fl("Max Concurrency")}</label>
        <input
          type="number"
          min={1}
          max={50}
          value={num('max_concurrency', 10)}
          onChange={(e) => set('max_concurrency', Number(e.target.value))}
        />
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ results, succeeded, failed, total }'}</code>
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
        <label>{fl("Value Expression *")}</label>
        <input
          placeholder="{{input.status}}"
          value={str('value', '{{input}}')}
          onChange={(e) => set('value', e.target.value)}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>The value to match against cases. Supports {'{{...}}'} templates.</span>
      </div>
      <div className="field">
        <label>{fl("Cases")}</label>
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
            ><IconX aria-hidden /></button>
          </div>
        ))}
        <button
          className="btn btn-secondary"
          style={{ marginTop: 4, fontSize: 12 }}
          onClick={() => setCases([...cases, { match: '', label: '' }])}
        >{fl("+ Add case")}</button>
        <span style={{ fontSize: 11, color: 'var(--muted)', marginTop: 4, display: 'block' }}>
          {fl('Add a case with match="*" to catch all unmatched values (default branch).')}
        </span>
      </div>
      {labelLocale() === 'zh' ? (
        <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
          {fl("返回")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ "value": "...", "matched_case": "label", "matched": bool }'}</code>。<br/>
          {fl("其")} <strong>{fl("condition_label")}</strong> {fl("等于命中 case 标签的出边会被执行。")}
        </p>
      ) : (
        <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
          {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ "value": "...", "matched_case": "label", "matched": bool }'}</code>.<br/>
          {fl("Outgoing edges whose")} <strong>{fl("condition_label")}</strong> {fl("equals the matched case label will be followed.")}
        </p>
      )}
    </>
  )
}

export function LoopConfig({ set, str, num }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>{fl("Items *")}</label>
        <input
          placeholder="{{input.items}} or {{fetch_node}}"
          value={str('items')}
          onChange={(e) => set('items', e.target.value)}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>{fl("Template expression resolving to a JSON array.")}</span>
      </div>
      <div className="field">
        <label>{fl("Max iterations")} <span style={{ color: 'var(--muted)' }}>(1–1000)</span></label>
        <input
          type="number" min={1} max={1000}
          value={num('max_iterations', 100)}
          onChange={(e) => set('max_iterations', Number(e.target.value))}
          style={{ width: 100 }}
        />
      </div>
      <div className="field">
        <label>{fl("Until path")} <span style={{ color: 'var(--muted)' }}>{fl("(optional — stops when falsy)")}</span></label>
        <input
          placeholder="active"
          value={str('until')}
          onChange={(e) => set('until', e.target.value)}
          style={{ fontFamily: 'monospace' }}
        />
      </div>
      <div className="field">
        <label>{fl("Item template")} <span style={{ color: 'var(--muted)' }}>{fl("(optional JSON)")}</span></label>
        <textarea
          rows={3}
          placeholder={'{ "id": "{{item.id}}", "name": "{{item.name}}" }'}
          value={str('template')}
          onChange={(e) => set('template', e.target.value)}
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>
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
        <label>{fl("Items expression *")}</label>
        <input
          placeholder="{{trigger.leads}}"
          value={str('items')}
          onChange={(e) => set('items', e.target.value)}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>
          {fl("Must resolve to a JSON array.")}
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
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>
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
        <label>{fl("Items expression *")}</label>
        <input
          placeholder="{{trigger.rows}}"
          value={str('items')}
          onChange={(e) => set('items', e.target.value)}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>{fl("Must resolve to a JSON array.")}</span>
      </div>
      <div className="field">
        <label>{fl("Sort field *")}</label>
        <input
          placeholder="name"
          value={str('field')}
          onChange={(e) => set('field', e.target.value)}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>Dot-path on each item (e.g. <code>score</code> or <code>meta.rank</code>).</span>
      </div>
      <div style={{ display: 'flex', gap: 12 }}>
        <div className="field" style={{ flex: 1 }}>
          <label>{fl("Order")}</label>
          <select
            value={order}
            onChange={(e) => set('order', e.target.value)}
            style={{ background: 'var(--surface)', color: 'var(--text)', border: '1px solid var(--border)', borderRadius: 'var(--radius)', padding: '4px 8px', fontSize: 13, width: '100%' }}
          >
            <option value="asc">{fl("asc")}</option>
            <option value="desc">{fl("desc")}</option>
          </select>
        </div>
        <div className="field" style={{ flex: 1 }}>
          <label>{fl("Compare as")}</label>
          <select
            value={type}
            onChange={(e) => set('type', e.target.value)}
            style={{ background: 'var(--surface)', color: 'var(--text)', border: '1px solid var(--border)', borderRadius: 'var(--radius)', padding: '4px 8px', fontSize: 13, width: '100%' }}
          >
            <option value="string">{fl("string")}</option>
            <option value="number">{fl("number")}</option>
          </select>
        </div>
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>
          {'{ "count": N, "items": [...] }'}
        </code> {fl("in sorted order.")}
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
        <label>{fl("Items expression *")}</label>
        <input
          placeholder="{{trigger.rows}}"
          value={str('items')}
          onChange={(e) => set('items', e.target.value)}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>{fl("Must resolve to a JSON array.")}</span>
      </div>
      <div className="field">
        <label>{fl("Operation *")}</label>
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
          <label>{fl("Separator")} <span style={{ color: 'var(--muted)' }}>{fl("(default \", \")")}</span></label>
          <input
            placeholder=", "
            value={str('separator')}
            onChange={(e) => set('separator', e.target.value)}
          />
        </div>
      )}
      <TemplateHint />
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>
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
        <label>{fl("Items expression *")}</label>
        <input
          placeholder="{{trigger.users}}"
          value={str('items')}
          onChange={(e) => set('items', e.target.value)}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>{fl("Must resolve to a JSON array.")}</span>
      </div>
      <div className="field">
        <label>{fl("Field *")}</label>
        <input
          placeholder="status"
          value={str('field')}
          onChange={(e) => set('field', e.target.value)}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>Field path on each item (e.g. <code>active</code> or <code>profile.age</code>).</span>
      </div>
      <div className="field">
        <label>{fl("Operator")}</label>
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
          <label>{fl("Value *")}</label>
          <input
            placeholder="active"
            value={str('value')}
            onChange={(e) => set('value', e.target.value)}
          />
        </div>
      )}
      <TemplateHint />
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>
          {'{ "count": N, "items": [...] }'}
        </code> {fl("with only matching items.")}
      </p>
    </>
  )
}

export function SplitConfig({ set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>{fl("Source *")}</label>
        <input
          placeholder="{{input.csv_line}}"
          value={str('source', '{{input}}')}
          onChange={(e) => set('source', e.target.value)}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>{fl("Template expression returning the string to split.")}</span>
      </div>
      <div className="field">
        <label>{fl("Delimiter")}</label>
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
        <label htmlFor="split-trim" style={{ cursor: 'pointer' }}>{fl("Trim whitespace from each part")}</label>
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ "parts": [...], "count": N }'}</code>
      </p>
    </>
  )
}

export function JoinConfig({ set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>{fl("Items Array *")}</label>
        <input
          placeholder="{{split_node.parts}}"
          value={str('items', '{{input}}')}
          onChange={(e) => set('items', e.target.value)}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>{fl("Template expression returning an array of strings (or objects).")}</span>
      </div>
      <div className="field">
        <label>{fl("Delimiter")}</label>
        <input
          placeholder=","
          value={str('delimiter', ',')}
          onChange={(e) => set('delimiter', e.target.value)}
        />
      </div>
      <div className="field">
        <label>{fl("Field")} <span style={{ color: 'var(--muted)' }}>{fl("(optional, for object arrays)")}</span></label>
        <input
          placeholder="name"
          value={str('field')}
          onChange={(e) => set('field', e.target.value)}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>Dot-path to extract from each object (e.g. <code>user.name</code>).</span>
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ "result": "a,b,c", "count": N }'}</code>
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
