import type { ExecutionRecord, NodeExecutionRecord } from '../types'

interface Props {
  execution: ExecutionRecord | null
  running: boolean
  inputJson: string
  onInputChange: (v: string) => void
  onRun: () => void
  canRun: boolean
  onApprove?: () => void
  onReject?: () => void
}

export function ExecutionPanel({ execution, running, inputJson, onInputChange, onRun, canRun, onApprove, onReject }: Props) {
  return (
    <div className="exec-panel">
      <div className="exec-panel-header">
        <span style={{ fontWeight: 600, fontSize: 13 }}>Execution</span>
        {execution && (
          <span className={`badge badge-${execution.status}`}>{execution.status}</span>
        )}
        {!execution && !running && (
          <span style={{ color: 'var(--muted)', fontSize: 12 }}>
            Run the workflow to see results
          </span>
        )}
        {running && <span style={{ color: 'var(--link)', fontSize: 12 }}>Running…</span>}

        {execution?.status === 'waiting_approval' && (
          <div style={{ display: 'flex', gap: 6, alignItems: 'center' }}>
            <span style={{ fontSize: 12, color: 'var(--muted)' }}>Waiting for approval</span>
            <button className="btn btn-success btn-sm" onClick={onApprove}>✓ Approve</button>
            <button className="btn btn-danger btn-sm" onClick={onReject}>✗ Reject</button>
          </div>
        )}

        <div className="exec-input-row">
          <label style={{ fontSize: 12, color: 'var(--muted)', margin: 0, whiteSpace: 'nowrap' }}>
            Input JSON
          </label>
          <input
            value={inputJson}
            onChange={(e) => onInputChange(e.target.value)}
            placeholder="{}"
            style={{ width: 180, fontFamily: 'monospace', fontSize: 12 }}
          />
          <button
            className="btn btn-success btn-sm"
            disabled={!canRun || running}
            onClick={onRun}
            title={!canRun ? 'Publish a version first to run' : 'Run the latest published version'}
          >
            {running ? '…' : '▶ Run'}
          </button>
        </div>
      </div>

      {execution && (
        <div className="exec-panel-body">
          {execution.node_results.length === 0 && (
            <span style={{ color: 'var(--muted)', fontSize: 12 }}>No node results.</span>
          )}
          {execution.node_results.map((nr) => (
            <NodeResultRow key={nr.node_id} nr={nr} />
          ))}
        </div>
      )}
    </div>
  )
}

function NodeResultRow({ nr }: { nr: NodeExecutionRecord }) {
  const output = (() => {
    if (nr.error) return `Error: ${nr.error}`
    if (!nr.output_json) return ''
    try {
      const parsed = JSON.parse(nr.output_json)
      return JSON.stringify(parsed, null, 0)
    } catch {
      return nr.output_json
    }
  })()

  return (
    <div className="exec-node-row">
      <span className={`dot dot-${nr.status}`} style={{ marginTop: 4 }} />
      <span className="exec-node-id">{nr.node_id}</span>
      <span
        className={`badge badge-${nr.status}`}
        style={{ fontSize: 10, padding: '1px 6px', flexShrink: 0 }}
      >
        {nr.status}
      </span>
      {output && (
        <span className="exec-node-output" style={{ flex: 1, maxHeight: 60, overflow: 'hidden' }}>
          {output}
        </span>
      )}
    </div>
  )
}
