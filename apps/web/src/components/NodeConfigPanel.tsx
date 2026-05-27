import type { FlowNode } from './Canvas'
import type { NodeType, ExecutionSummary, NodeExecutionRecord } from '../types'

const NODE_LABELS: Record<NodeType, string> = {
  trigger: 'Trigger',
  http: 'HTTP',
  agent: 'Agent',
  condition: 'Condition',
  approval: 'Approval',
  map: 'Map',
  filter: 'Filter',
  aggregate: 'Aggregate',
  sort: 'Sort',
  transform: 'Transform',
  delay: 'Delay',
  sub_workflow: 'Sub-Workflow',
  assert: 'Assert',
  catch: 'Catch',
  fan_out: 'Fan-Out',
  fan_in: 'Fan-In',
  code: 'Code',
}

const NODE_COLORS: Record<NodeType, string> = {
  trigger: 'var(--node-trigger)',
  http: 'var(--node-http)',
  agent: 'var(--node-agent)',
  condition: 'var(--node-condition)',
  approval: 'var(--node-approval)',
  map: 'var(--node-map)',
  filter: 'var(--node-filter)',
  aggregate: 'var(--node-aggregate)',
  sort: 'var(--node-sort)',
  transform: 'var(--node-transform)',
  delay: 'var(--node-delay)',
  sub_workflow: 'var(--node-sub-workflow)',
  assert: 'var(--node-assert)',
  catch: 'var(--node-catch)',
  fan_out: 'var(--node-fan)',
  fan_in: 'var(--node-fan)',
  code: 'var(--node-code)',
}

interface Props {
  node: FlowNode | null
  onUpdateConfig: (nodeId: string, config: Record<string, unknown>) => void
  recentExecutions?: ExecutionSummary[]
  onSelectExecution?: (id: string) => void
  executionResult?: NodeExecutionRecord | null
  webhookUrl?: string | null
}

export function NodeConfigPanel({ node, onUpdateConfig, recentExecutions, onSelectExecution, executionResult, webhookUrl }: Props) {
  if (!node) {
    return (
      <div className="config-panel">
        {recentExecutions && recentExecutions.length > 0 ? (
          <>
            <div className="config-panel-header" style={{ borderBottom: '1px solid var(--border)' }}>
              Recent Executions
            </div>
            <div className="config-panel-body" style={{ overflowY: 'auto' }}>
              {recentExecutions.map((ex) => (
                <div
                  key={ex.id}
                  className="exec-node-row"
                  style={{ cursor: 'pointer', padding: '6px 8px' }}
                  onClick={() => onSelectExecution?.(ex.id)}
                  title="Click to view execution details"
                >
                  <span className={`dot dot-${ex.status}`} style={{ flexShrink: 0 }} />
                  <span style={{ fontSize: 11, color: 'var(--muted)', fontFamily: 'monospace', flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                    {ex.id.slice(0, 8)}…
                  </span>
                  <span className={`badge badge-${ex.status}`} style={{ fontSize: 10, padding: '1px 5px', flexShrink: 0 }}>
                    {ex.status}
                  </span>
                </div>
              ))}
            </div>
          </>
        ) : (
          <div className="config-empty">
            <span>Click a node to configure it</span>
          </div>
        )}
      </div>
    )
  }

  const nt = node.data.nodeType
  const config = node.data.config

  const set = (key: string, value: unknown) => {
    onUpdateConfig(node.id, { ...config, [key]: value })
  }

  const str = (key: string, fallback = '') => (config[key] as string) ?? fallback
  const num = (key: string, fallback: number) => (config[key] as number) ?? fallback

  return (
    <div className="config-panel">
      <div className="config-panel-header">
        <span
          style={{
            width: 10, height: 10, borderRadius: '50%', background: NODE_COLORS[nt] ?? '#8b949e',
            display: 'inline-block', flexShrink: 0,
          }}
        />
        {NODE_LABELS[nt] ?? nt}
        <span style={{ color: 'var(--muted)', fontWeight: 400, fontSize: 12 }}>— {node.id}</span>
      </div>

      <div className="config-panel-body">
        {executionResult && <NodeResultBox result={executionResult} />}
        {nt === 'trigger' && <TriggerConfig config={config} set={set} num={num} webhookUrl={webhookUrl} />}
        {nt === 'http' && <HttpConfig config={config} set={set} str={str} num={num} />}
        {nt === 'agent' && <AgentConfig config={config} set={set} str={str} num={num} />}
        {nt === 'condition' && <ConditionConfig config={config} set={set} str={str} num={num} />}
        {nt === 'approval' && <ApprovalConfig />}
        {nt === 'map' && <MapConfig config={config} set={set} str={str} />}
        {nt === 'filter' && <FilterConfig config={config} set={set} str={str} />}
        {nt === 'aggregate' && <AggregateConfig config={config} set={set} str={str} />}
        {nt === 'sort' && <SortConfig config={config} set={set} str={str} />}
        {nt === 'transform' && <TransformConfig config={config} set={set} str={str} />}
        {nt === 'delay' && <DelayConfig config={config} set={set} str={str} num={num} />}
        {nt === 'sub_workflow' && <SubWorkflowConfig config={config} set={set} str={str} />}
        {nt === 'assert' && <AssertConfig config={config} set={set} str={str} />}
        {nt === 'catch' && <CatchConfig config={config} set={set} str={str} />}
        {nt === 'fan_out' && <FanOutConfig />}
        {nt === 'fan_in' && <FanInConfig />}
        {nt === 'code' && <CodeConfig config={config} set={set} str={str} />}
      </div>
    </div>
  )
}

function TriggerConfig({
  config, set, num, webhookUrl,
}: { config: Record<string, unknown>; set: ConfigProps['set']; num: ConfigProps['num']; webhookUrl?: string | null }) {
  void config
  const intervalSecs = num('interval_secs', 0)
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

      {/* Schedule */}
      <div className="field">
        <label>Auto-run Interval</label>
        <select
          value={intervalSecs || ''}
          onChange={(e) =>
            set('interval_secs', e.target.value ? Number(e.target.value) : undefined as unknown as number)
          }
        >
          <option value="">None (manual / webhook only)</option>
          <option value="60">Every minute</option>
          <option value="300">Every 5 minutes</option>
          <option value="3600">Every hour</option>
          <option value="86400">Every day</option>
        </select>
        {intervalSecs > 0 && (
          <p style={{ fontSize: 11, color: 'var(--muted)', marginTop: 4 }}>
            Schedule activates when the workflow is published.
          </p>
        )}
      </div>

      {/* Webhook */}
      {webhookUrl ? (
        <div>
          <p style={{ fontSize: 12, color: 'var(--text)', marginBottom: 6, fontWeight: 500 }}>Webhook URL</p>
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
          <p style={{ fontSize: 11, color: 'var(--muted)', marginTop: 6 }}>
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

interface ConfigProps {
  config: Record<string, unknown>
  set: (key: string, value: unknown) => void
  str: (key: string, fallback?: string) => string
  num: (key: string, fallback: number) => number
}

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

function HttpConfig({ set, str, num }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>URL *</label>
        <input
          placeholder="https://api.example.com/{{input.id}}"
          value={str('url')}
          onChange={(e) => set('url', e.target.value)}
        />
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
        <label>Auth Token <span style={{ color: 'var(--muted)', fontWeight: 400 }}>(Bearer)</span></label>
        <input
          placeholder="{{credential.my-api-key}}"
          value={str('auth_token')}
          onChange={(e) => set('auth_token', e.target.value || undefined as unknown as string)}
        />
      </div>
      <div className="field">
        <label>Body (JSON)</label>
        <textarea
          placeholder={'{"id": "{{input.id}}"}'}
          value={str('body')}
          onChange={(e) => set('body', e.target.value)}
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <HeadersEditor set={set} />
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

function HeadersEditor({ set }: { set: ConfigProps['set'] }) {
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

function AgentConfig({ set, str, num }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Model</label>
        <select value={str('model', 'claude-sonnet-4-6')} onChange={(e) => set('model', e.target.value)}>
          <option value="claude-haiku-4-5-20251001">claude-haiku-4-5 (fast)</option>
          <option value="claude-sonnet-4-6">claude-sonnet-4-6 (balanced)</option>
          <option value="claude-opus-4-7">claude-opus-4-7 (powerful)</option>
        </select>
      </div>
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

function NodeResultBox({ result }: { result: NodeExecutionRecord }) {
  const prettyOutput = (() => {
    if (!result.output_json) return null
    try { return JSON.stringify(JSON.parse(result.output_json), null, 2) }
    catch { return result.output_json }
  })()

  return (
    <div className="exec-result-box">
      <div className="exec-result-box-header">
        <span className={`dot dot-${result.status}`} />
        <span>Last result</span>
        <span className={`badge badge-${result.status}`} style={{ fontSize: 10, padding: '1px 5px', marginLeft: 'auto' }}>
          {result.status}
        </span>
      </div>
      {result.error && (
        <div className="exec-result-error">{result.error}</div>
      )}
      {prettyOutput && !result.error && (
        <pre className="exec-result-output">{prettyOutput}</pre>
      )}
      {!result.error && !prettyOutput && (
        <div style={{ padding: '8px 10px', fontSize: 11, color: 'var(--muted)' }}>No output</div>
      )}
    </div>
  )
}

function ApprovalConfig() {
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

function MapConfig({ set, str }: ConfigProps) {
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

function SortConfig({ set, str }: ConfigProps) {
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

function AggregateConfig({ set, str }: ConfigProps) {
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

function FilterConfig({ set, str }: ConfigProps) {
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

function TransformConfig({ config, set }: ConfigProps) {
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

function ConditionConfig({ set, str }: ConfigProps) {
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

function DelayConfig({ set, num }: ConfigProps) {
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

function AssertConfig({ set, str }: ConfigProps) {
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

function CodeConfig({ set, str }: ConfigProps) {
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

function FanOutConfig() {
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

function FanInConfig() {
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

function CatchConfig({ set, str }: ConfigProps) {
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

function SubWorkflowConfig({ config, set, str }: ConfigProps) {
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
