import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useAuth } from '../AuthContext'
import * as api from '../api/client'
import type { WorkflowRecord, WorkflowVersionRecord, ExecutionRecord, ExecutionSummary, NodeExecutionRecord, NodeType } from '../types'
import { Canvas, graphFromApi, fromFlowGraph, type FlowNode, type FlowEdge } from './Canvas'
import { NodeConfigPanel } from './NodeConfigPanel'
import { ExecutionPanel } from './ExecutionPanel'

interface Props {
  workflowId: string
  onBack: () => void
}

type Toast = { id: number; message: string; kind: 'success' | 'error' }

const NODE_TYPE_LIST: { type: NodeType; label: string; color: string; icon: string }[] = [
  { type: 'trigger',   label: 'Trigger',   color: 'var(--node-trigger)',   icon: '▶' },
  { type: 'http',      label: 'HTTP',       color: 'var(--node-http)',      icon: '↗' },
  { type: 'agent',     label: 'Agent',      color: 'var(--node-agent)',     icon: '✦' },
  { type: 'condition', label: 'Condition',  color: 'var(--node-condition)', icon: '◇' },
  { type: 'approval',  label: 'Approval',   color: 'var(--node-approval)',  icon: '✋' },
  { type: 'map',       label: 'Map',        color: 'var(--node-map)',       icon: '⟳' },
  { type: 'filter',    label: 'Filter',     color: 'var(--node-filter)',    icon: '⊃' },
  { type: 'aggregate', label: 'Aggregate',  color: 'var(--node-aggregate)', icon: 'Σ' },
  { type: 'sort',      label: 'Sort',       color: 'var(--node-sort)',      icon: '⇅' },
  { type: 'transform',    label: 'Transform',   color: 'var(--node-transform)',    icon: '⇄' },
  { type: 'delay',        label: 'Delay',       color: 'var(--node-delay)',        icon: '⏱' },
  { type: 'sub_workflow', label: 'Sub-Workflow', color: 'var(--node-sub-workflow)', icon: '⤵' },
  { type: 'assert',      label: 'Assert',       color: 'var(--node-assert)',       icon: '⊘' },
  { type: 'catch',       label: 'Catch',        color: 'var(--node-catch)',        icon: '↻' },
  { type: 'fan_out',    label: 'Fan-Out',      color: 'var(--node-fan)',          icon: '⇉' },
  { type: 'fan_in',     label: 'Fan-In',       color: 'var(--node-fan)',          icon: '⇇' },
  { type: 'code',       label: 'Code',         color: 'var(--node-code)',         icon: '{ }' },
]

export function WorkflowEditor({ workflowId, onBack }: Props) {
  const { auth } = useAuth()
  const [workflow, setWorkflow]       = useState<WorkflowRecord | null>(null)
  const [version, setVersion]         = useState<WorkflowVersionRecord | null>(null)
  const [nodes, setNodes]             = useState<FlowNode[]>([])
  const [edges, setEdges]             = useState<FlowEdge[]>([])
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null)
  const [execution, setExecution]     = useState<ExecutionRecord | null>(null)
  const [recentExecutions, setRecentExecutions] = useState<ExecutionSummary[]>([])
  const [inputJson, setInputJson]     = useState('{}')
  const [running, setRunning]         = useState(false)
  const [saving, setSaving]           = useState(false)
  const [publishing, setPublishing]   = useState(false)
  const [toasts, setToasts]           = useState<Toast[]>([])
  const [renaming, setRenaming]       = useState(false)
  const [newName, setNewName]         = useState('')
  const [webhookUrl, setWebhookUrl]   = useState<string | null>(null)
  const toastId = useRef(0)

  const toast = useCallback((message: string, kind: 'success' | 'error' = 'success') => {
    const id = ++toastId.current
    setToasts((t) => [...t, { id, message, kind }])
    setTimeout(() => setToasts((t) => t.filter((x) => x.id !== id)), 3000)
  }, [])

  const refreshHistory = useCallback(() => {
    api.listExecutions(auth!.tenantId, workflowId)
      .then(setRecentExecutions)
      .catch(() => {})
  }, [workflowId])

  // Load workflow + latest version
  useEffect(() => {
    api.getWorkflow(auth!.tenantId, workflowId).then((wf) => {
      setWorkflow(wf)
      setNewName(wf.name)
      if (wf.latest_version_id) {
        return api.getVersion(auth!.tenantId, wf.latest_version_id)
      }
      return null
    }).then((ver) => {
      if (ver) {
        setVersion(ver)
        const { nodes: fn, edges: fe } = graphFromApi(ver.graph.nodes, ver.graph.edges)
        setNodes(fn)
        setEdges(fe)
        if (ver.status === 'published') {
          api.getWebhook(auth!.tenantId, ver.id)
            .then((info) => setWebhookUrl(window.location.origin + info.url))
            .catch(() => {})
        }
      } else {
        // No version yet — start with a trigger node
        const { nodes: fn, edges: fe } = graphFromApi(
          [{ id: 'trigger', type: 'trigger' }],
          [],
        )
        setNodes(fn)
        setEdges(fe)
      }
    }).catch((e: unknown) => toast(String(e), 'error'))
    refreshHistory()
  }, [workflowId, toast, refreshHistory])

  // Poll until execution leaves running state
  useEffect(() => {
    if (!execution || execution.status !== 'running') return
    const timer = setInterval(async () => {
      try {
        const updated = await api.getExecution(auth!.tenantId, execution.id)
        setExecution(updated)
        if (updated.status !== 'running') {
          refreshHistory()
        }
      } catch {
        // ignore transient errors
      }
    }, 1000)
    return () => clearInterval(timer)
  }, [execution?.id, execution?.status, refreshHistory])

  // Add node from palette
  const addNode = useCallback((type: NodeType) => {
    const id = `${type}-${Date.now()}`
    const existing = nodes.length
    const newNode: FlowNode = {
      id,
      type,
      position: { x: (existing % 4) * 280 + 80, y: Math.floor(existing / 4) * 140 + 80 },
      data: { label: id, nodeType: type, config: {} },
    }
    setNodes((prev) => [...prev, newNode])
    setSelectedNodeId(id)
  }, [nodes.length])

  // Update node config from panel
  const handleUpdateConfig = useCallback((nodeId: string, config: Record<string, unknown>) => {
    setNodes((prev) =>
      prev.map((n) =>
        n.id === nodeId ? { ...n, data: { ...n.data, config } } : n,
      ),
    )
  }, [])

  // Save new version
  const handleSave = async () => {
    if (!workflow) return
    setSaving(true)
    try {
      const { nodes: apiNodes, edges: apiEdges } = fromFlowGraph(nodes, edges)
      const tempVersionId = `v-${Date.now()}`
      const graph = {
        workflow_version_id: tempVersionId,
        nodes: apiNodes,
        edges: apiEdges,
      }
      const ver = await api.createVersion(auth!.tenantId, workflowId, graph)
      setVersion(ver)
      toast('Version saved')
    } catch (e) {
      toast(String(e), 'error')
    } finally {
      setSaving(false)
    }
  }

  // Publish latest draft version
  const handlePublish = async () => {
    if (!version || version.status === 'published') return
    setPublishing(true)
    try {
      const ver = await api.publishVersion(auth!.tenantId, version.id)
      setVersion(ver)
      const wf = await api.getWorkflow(auth!.tenantId, workflowId)
      setWorkflow(wf)
      const info = await api.getWebhook(auth!.tenantId, ver.id)
      setWebhookUrl(window.location.origin + info.url)
      toast('Version published')
    } catch (e) {
      toast(String(e), 'error')
    } finally {
      setPublishing(false)
    }
  }

  // Approve / Reject human approval gate
  const handleApprove = async () => {
    if (!execution) return
    try {
      await api.approveExecution(execution.id)
    } catch (e) {
      toast(String(e), 'error')
    }
  }

  const handleReject = async () => {
    if (!execution) return
    try {
      await api.rejectExecution(execution.id)
    } catch (e) {
      toast(String(e), 'error')
    }
  }

  // Run execution
  const handleRun = async () => {
    if (!workflow?.latest_version_id) return
    let parsed: unknown
    try { parsed = JSON.parse(inputJson) } catch { toast('Input JSON is invalid', 'error'); return }
    void parsed
    setRunning(true)
    setExecution(null)
    try {
      const result = await api.startExecutionFromWorkflow(auth!.tenantId, workflowId, inputJson)
      setExecution(result)
      refreshHistory()
    } catch (e) {
      toast(String(e), 'error')
    } finally {
      setRunning(false)
    }
  }

  // Export current published version as JSON download
  const handleExport = async () => {
    if (!workflow?.latest_version_id) { toast('Publish a version first to export', 'error'); return }
    try {
      const exported = await api.exportWorkflow(auth!.tenantId, workflowId)
      const blob = new Blob([JSON.stringify(exported, null, 2)], { type: 'application/json' })
      const url = URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = `${exported.name.replace(/\s+/g, '-').toLowerCase()}.json`
      a.click()
      URL.revokeObjectURL(url)
    } catch (e) {
      toast(String(e), 'error')
    }
  }

  // Rename workflow
  const handleRename = async () => {
    if (!newName.trim() || newName === workflow?.name) { setRenaming(false); return }
    try {
      const wf = await api.renameWorkflow(auth!.tenantId, workflowId, newName.trim())
      setWorkflow(wf)
      toast('Renamed')
    } catch (e) {
      toast(String(e), 'error')
    } finally {
      setRenaming(false)
    }
  }

  const nodeStatuses = useMemo<Record<string, NodeExecutionRecord>>(() => {
    if (!execution) return {}
    return Object.fromEntries(execution.node_results.map((r) => [r.node_id, r]))
  }, [execution])

  const selectedNode = nodes.find((n) => n.id === selectedNodeId) ?? null
  const canRun = !!workflow?.latest_version_id && workflow.status !== 'archived'
  const canPublish = !!version && version.status === 'draft'

  return (
    <div className="app">
      {/* ── Top bar ─────────────────────────────────────────────── */}
      <header className="topbar">
        <button className="btn btn-sm btn-icon" onClick={onBack} title="Back to list">←</button>
        <span className="topbar-sep">|</span>

        {renaming ? (
          <input
            autoFocus
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            onBlur={handleRename}
            onKeyDown={(e) => { if (e.key === 'Enter') handleRename(); if (e.key === 'Escape') setRenaming(false) }}
            style={{ width: 200, fontSize: 14, fontWeight: 600 }}
          />
        ) : (
          <span
            className="topbar-title"
            style={{ cursor: 'pointer' }}
            onClick={() => setRenaming(true)}
            title="Click to rename"
          >
            {workflow?.name ?? '…'}
          </span>
        )}

        {workflow && (
          <span className={`badge badge-${workflow.status}`}>{workflow.status}</span>
        )}
        {version && (
          <span style={{ color: 'var(--muted)', fontSize: 12 }}>
            v{version.version} <span className={`badge badge-${version.status}`}>{version.status}</span>
          </span>
        )}

        <div className="topbar-actions">
          <button
            className="btn btn-sm"
            disabled={!workflow?.latest_version_id}
            onClick={handleExport}
            title="Download published version as JSON"
          >
            ↓ Export
          </button>
          <button
            className="btn btn-sm"
            disabled={saving}
            onClick={handleSave}
            title="Save current graph as a new version"
          >
            {saving ? 'Saving…' : '⬇ Save Version'}
          </button>
          <button
            className="btn btn-sm btn-primary"
            disabled={!canPublish || publishing}
            onClick={handlePublish}
            title={canPublish ? 'Publish this draft version' : 'No draft version to publish'}
          >
            {publishing ? 'Publishing…' : '✓ Publish'}
          </button>
        </div>
      </header>

      {/* ── Editor body ──────────────────────────────────────────── */}
      <div className="editor" style={{ flex: 1, overflow: 'hidden', display: 'flex', flexDirection: 'column' }}>
        <div style={{ flex: 1, display: 'flex', overflow: 'hidden' }}>
          {/* Left palette */}
          <aside className="palette">
            <div className="palette-label">Add Node</div>
            {NODE_TYPE_LIST.map(({ type, label, color, icon }) => (
              <button
                key={type}
                className="palette-node"
                onClick={() => addNode(type)}
                title={`Add a ${label} node`}
              >
                <span className="palette-dot" style={{ background: color }} />
                <span>{icon} {label}</span>
              </button>
            ))}
            <div className="palette-label" style={{ marginTop: 12 }}>Tips</div>
            <div style={{ fontSize: 11, color: 'var(--muted)', padding: '0 8px', lineHeight: 1.6 }}>
              <div>• Drag nodes to move</div>
              <div>• Drag handle → handle to connect</div>
              <div>• Select + Delete to remove</div>
              <div>• Click node to configure</div>
            </div>
          </aside>

          {/* Canvas */}
          <div className="canvas-wrap">
            {nodes.length > 0 ? (
              <Canvas
                initialNodes={nodes}
                initialEdges={edges}
                selectedNodeId={selectedNodeId}
                onSelectionChange={setSelectedNodeId}
                onNodesUpdated={setNodes}
                onEdgesUpdated={setEdges}
                nodeStatuses={nodeStatuses}
              />
            ) : (
              <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100%', color: 'var(--muted)' }}>
                Loading graph…
              </div>
            )}
          </div>

          {/* Right config panel */}
          <NodeConfigPanel
            node={selectedNode}
            onUpdateConfig={handleUpdateConfig}
            recentExecutions={recentExecutions}
            executionResult={selectedNode ? (nodeStatuses[selectedNode.id] ?? null) : null}
            webhookUrl={webhookUrl}
            onSelectExecution={async (id) => {
              try {
                const rec = await api.getExecution(auth!.tenantId, id)
                setExecution(rec)
              } catch (e) {
                toast(String(e), 'error')
              }
            }}
          />
        </div>

        {/* Bottom execution panel */}
        <ExecutionPanel
          execution={execution}
          running={running}
          inputJson={inputJson}
          onInputChange={setInputJson}
          onRun={handleRun}
          canRun={canRun}
          onApprove={handleApprove}
          onReject={handleReject}
        />
      </div>

      {/* Toasts */}
      {toasts.map((t) => (
        <div key={t.id} className={`toast toast-${t.kind}`}>
          {t.message}
        </div>
      ))}

      {/* Rename modal */}
      {renaming && (
        <div className="modal-backdrop" onClick={() => setRenaming(false)}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            <h2>Rename Workflow</h2>
            <div className="field">
              <label>Name</label>
              <input
                autoFocus
                value={newName}
                onChange={(e) => setNewName(e.target.value)}
                onKeyDown={(e) => { if (e.key === 'Enter') handleRename(); if (e.key === 'Escape') setRenaming(false) }}
              />
            </div>
            <div className="modal-actions">
              <button className="btn" onClick={() => setRenaming(false)}>Cancel</button>
              <button className="btn btn-primary" onClick={handleRename}>Save</button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
