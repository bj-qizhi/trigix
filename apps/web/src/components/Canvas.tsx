import { createContext, useCallback, useContext, useEffect, useMemo } from 'react'
import {
  ReactFlow,
  Background,
  Controls,
  MiniMap,
  addEdge,
  useNodesState,
  useEdgesState,
  Handle,
  Position,
  BackgroundVariant,
  type Node,
  type Edge,
  type Connection,
  type NodeProps,
} from '@xyflow/react'
import '@xyflow/react/dist/style.css'
import type { ApiNode, ApiEdge, NodeType, NodeExecutionRecord } from '../types'

const NodeStatusContext = createContext<Record<string, NodeExecutionRecord>>({})

// ── Data shapes ──────────────────────────────────────────────────────────────

export interface FlowNodeData extends Record<string, unknown> {
  label: string
  nodeType: NodeType
  config: Record<string, unknown>
}

export type FlowNode = Node<FlowNodeData>
export type FlowEdge = Edge<{ conditionLabel?: string }>

// ── Layout ────────────────────────────────────────────────────────────────────

function computePositions(
  nodes: ApiNode[],
  edges: ApiEdge[],
): Record<string, { x: number; y: number }> {
  const outgoing: Record<string, string[]> = {}
  const indegree: Record<string, number> = {}
  for (const n of nodes) { outgoing[n.id] = []; indegree[n.id] = 0 }
  for (const e of edges) {
    outgoing[e.source]?.push(e.target)
    if (indegree[e.target] !== undefined) indegree[e.target]++
  }

  const levels: Record<string, number> = {}
  const queue = nodes.filter((n) => indegree[n.id] === 0).map((n) => n.id)
  for (const id of queue) levels[id] = 0

  let i = 0
  while (i < queue.length) {
    const id = queue[i++]
    for (const t of outgoing[id] ?? []) {
      levels[t] = Math.max(levels[t] ?? 0, (levels[id] ?? 0) + 1)
      queue.push(t)
    }
  }

  const byLevel: Record<number, string[]> = {}
  for (const [id, lv] of Object.entries(levels)) {
    ;(byLevel[lv] ??= []).push(id)
  }

  const positions: Record<string, { x: number; y: number }> = {}
  const X = 280, Y = 120
  for (const [lvStr, ids] of Object.entries(byLevel)) {
    const lv = Number(lvStr)
    ids.sort().forEach((id, idx) => {
      positions[id] = { x: lv * X + 80, y: (idx - (ids.length - 1) / 2) * Y + 220 }
    })
  }
  return positions
}

// ── Format conversion ─────────────────────────────────────────────────────────

export function toFlowNodes(
  apiNodes: ApiNode[],
  positions: Record<string, { x: number; y: number }>,
): FlowNode[] {
  return apiNodes.map((n, idx) => ({
    id: n.id,
    type: n.type,
    position: positions[n.id] ?? { x: (idx % 4) * 280 + 80, y: Math.floor(idx / 4) * 120 + 80 },
    data: {
      label: n.id,
      nodeType: n.type,
      config: (n.config as Record<string, unknown>) ?? {},
    },
  }))
}

export function toFlowEdges(apiEdges: ApiEdge[]): FlowEdge[] {
  return apiEdges.map((e) => ({
    id: `e-${e.source}-${e.target}`,
    source: e.source,
    target: e.target,
    label: e.condition_label ?? undefined,
    data: { conditionLabel: e.condition_label },
    style: { stroke: '#30363d', strokeWidth: 2 },
    labelStyle: { fill: '#8b949e', fontSize: 11, fontWeight: 600 },
    labelBgStyle: { fill: '#21262d', fillOpacity: 0.9 },
    labelBgPadding: [4, 6] as [number, number],
    labelBgBorderRadius: 4,
  }))
}

export function fromFlowGraph(
  nodes: FlowNode[],
  edges: FlowEdge[],
): { nodes: ApiNode[]; edges: ApiEdge[] } {
  return {
    nodes: nodes.map((n) => ({
      id: n.id,
      type: n.data.nodeType,
      config: Object.keys(n.data.config).length > 0 ? n.data.config : undefined,
    })),
    edges: edges.map((e) => ({
      source: e.source,
      target: e.target,
      ...(e.data?.conditionLabel ? { condition_label: e.data.conditionLabel as 'true' | 'false' | 'error' } : {}),
    })),
  }
}

export function graphFromApi(
  apiNodes: ApiNode[],
  apiEdges: ApiEdge[],
): { nodes: FlowNode[]; edges: FlowEdge[] } {
  const positions = computePositions(apiNodes, apiEdges)
  return { nodes: toFlowNodes(apiNodes, positions), edges: toFlowEdges(apiEdges) }
}

// ── Custom node component ─────────────────────────────────────────────────────

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
  slack: 'Slack',
  email: 'Email',
}

const NODE_ICONS: Record<NodeType, string> = {
  trigger: '▶',
  http: '↗',
  agent: '✦',
  condition: '◇',
  approval: '✋',
  map: '⟳',
  filter: '⊃',
  aggregate: 'Σ',
  sort: '⇅',
  transform: '⇄',
  delay: '⏱',
  sub_workflow: '⤵',
  assert: '⊘',
  catch: '↻',
  fan_out: '⇉',
  fan_in: '⇇',
  code: '{ }',
  slack: '#',
  email: '@',
}

function FlowNodeComponent({ data, selected, id }: NodeProps) {
  const statuses = useContext(NodeStatusContext)
  const execResult = statuses[id]
  const d = data as FlowNodeData
  const nt = d.nodeType
  const label = nt ? (NODE_LABELS[nt] ?? nt) : 'Node'
  const icon = nt ? (NODE_ICONS[nt] ?? '●') : '●'

  const preview = (() => {
    if (!nt) return ''
    const c = d.config ?? {}
    if (nt === 'http') return (c.url as string) || 'No URL set'
    if (nt === 'agent') return (c.model as string) || 'claude-sonnet-4-6'
    if (nt === 'condition') return c.field ? `if ${String(c.field)}` : 'No field set'
    if (nt === 'approval') return 'Awaits human approval'
    if (nt === 'map') return c.items ? `map ${String(c.items)}` : 'No items set'
    if (nt === 'filter') {
      if (!c.items) return 'No items set'
      const op = (c.operator as string) || 'exists'
      return c.field ? `${String(c.field)} ${op}${c.value ? ` ${String(c.value)}` : ''}` : 'No field set'
    }
    if (nt === 'aggregate') {
      const op = (c.operation as string) || ''
      return op ? `${op}${c.field ? `(${String(c.field)})` : ''}` : 'No operation set'
    }
    if (nt === 'sort') {
      const ord = (c.order as string) || 'asc'
      return c.field ? `${String(c.field)} ${ord}` : 'No field set'
    }
    if (nt === 'transform') return c.template ? 'template configured' : 'No template set'
    if (nt === 'delay') {
      const s = c.seconds as number | undefined
      return s !== undefined ? `wait ${s}s` : 'No duration set'
    }
    if (nt === 'sub_workflow') return (c.workflow_id as string) || 'No workflow set'
    if (nt === 'assert') return c.condition ? `assert ${String(c.condition)}` : 'No condition set'
    if (nt === 'catch') return c.source ? `catch ${String(c.source)}` : 'Catches any error'
    if (nt === 'fan_out') return 'Splits into parallel branches'
    if (nt === 'fan_in') return 'Collects branch results'
    if (nt === 'code') return c.script ? String(c.script).split('\n')[0].slice(0, 40) : 'No script'
    if (nt === 'slack') return c.text ? String(c.text).slice(0, 40) : 'No message'
    if (nt === 'email') return c.to ? `to: ${String(c.to)}` : 'No recipient'
    return ''
  })()

  const execClass = execResult ? `exec-${execResult.status}` : ''

  const DOT_COLORS: Record<string, string> = {
    succeeded: 'var(--success-text)',
    failed: 'var(--danger-text)',
    running: 'var(--link)',
    waiting_approval: 'var(--approval-text)',
    skipped: 'var(--muted)',
  }

  return (
    <div className={`flow-node flow-node-${nt ?? 'unknown'} ${selected ? 'selected' : ''} ${execClass}`}>
      {execResult && (
        <span
          className="flow-node-status-dot"
          style={{ background: DOT_COLORS[execResult.status] ?? 'var(--muted)' }}
        />
      )}
      {nt !== 'trigger' && (
        <Handle type="target" position={Position.Left} style={{ background: '#30363d' }} />
      )}
      <div className="flow-node-header">
        <span>{icon}</span>
        <span>{label}</span>
        <span style={{ opacity: 0.6, fontWeight: 400, fontSize: 11, marginLeft: 'auto' }}>{id}</span>
      </div>
      <div className="flow-node-body">
        {preview || <span style={{ opacity: 0.4 }}>No config</span>}
      </div>
      <Handle type="source" position={Position.Right} style={{ background: '#30363d' }} />
    </div>
  )
}

const nodeTypes = {
  trigger: FlowNodeComponent,
  http: FlowNodeComponent,
  agent: FlowNodeComponent,
  condition: FlowNodeComponent,
  approval: FlowNodeComponent,
  map: FlowNodeComponent,
  filter: FlowNodeComponent,
  aggregate: FlowNodeComponent,
  sort: FlowNodeComponent,
  transform: FlowNodeComponent,
  delay: FlowNodeComponent,
  sub_workflow: FlowNodeComponent,
  assert: FlowNodeComponent,
  catch: FlowNodeComponent,
  fan_out: FlowNodeComponent,
  fan_in: FlowNodeComponent,
  code: FlowNodeComponent,
  slack: FlowNodeComponent,
  email: FlowNodeComponent,
}

// ── Canvas component ──────────────────────────────────────────────────────────

interface Props {
  initialNodes: FlowNode[]
  initialEdges: FlowEdge[]
  selectedNodeId: string | null
  onSelectionChange: (nodeId: string | null) => void
  onNodesUpdated: (nodes: FlowNode[]) => void
  onEdgesUpdated: (edges: FlowEdge[]) => void
  nodeStatuses?: Record<string, NodeExecutionRecord>
}

export function Canvas({
  initialNodes,
  initialEdges,
  selectedNodeId,
  onSelectionChange,
  onNodesUpdated,
  onEdgesUpdated,
  nodeStatuses = {},
}: Props) {
  const [nodes, setNodes, onNodesChange] = useNodesState<FlowNode>(initialNodes)
  const [edges, setEdges, onEdgesChange] = useEdgesState(initialEdges)

  // Sync nodes from parent (palette adds, version loads).
  // Kept separate from edges so node drags don't reset edge state.
  useEffect(() => {
    setNodes(initialNodes)
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [initialNodes])

  // Sync edges only when the parent explicitly provides a new edge set (version load).
  useEffect(() => {
    setEdges(initialEdges)
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [initialEdges])

  useEffect(() => {
    onNodesUpdated(nodes)
  }, [nodes, onNodesUpdated])

  useEffect(() => {
    onEdgesUpdated(edges as FlowEdge[])
  }, [edges, onEdgesUpdated])

  // When drawing a new edge from a condition node, auto-assign true/false label.
  const onConnect = useCallback(
    (connection: Connection) => {
      const sourceNode = nodes.find((n) => n.id === connection.source)
      const targetNode = nodes.find((n) => n.id === connection.target)
      const isCondition = sourceNode?.data.nodeType === 'condition'
      const isCatchTarget = targetNode?.data.nodeType === 'catch'
      let conditionLabel: string | undefined
      if (isCatchTarget) {
        conditionLabel = 'error'
      } else if (isCondition) {
        const existingLabels = edges
          .filter((e) => e.source === connection.source)
          .map((e) => (e as FlowEdge).data?.conditionLabel)
        conditionLabel = existingLabels.includes('true') ? 'false' : 'true'
      }
      setEdges((eds) =>
        addEdge(
          {
            ...connection,
            label: conditionLabel,
            data: { conditionLabel },
            style: { stroke: '#30363d', strokeWidth: 2 },
            labelStyle: { fill: '#8b949e', fontSize: 11, fontWeight: 600 },
            labelBgStyle: { fill: '#21262d', fillOpacity: 0.9 },
            labelBgPadding: [4, 6] as [number, number],
            labelBgBorderRadius: 4,
          },
          eds,
        ),
      )
    },
    [setEdges, nodes, edges],
  )

  // Click an edge from a condition node to toggle its label.
  const onEdgeClick = useCallback(
    (_: React.MouseEvent, edge: Edge) => {
      const sourceNode = nodes.find((n) => n.id === edge.source)
      if (sourceNode?.data.nodeType !== 'condition') return
      const current = (edge as FlowEdge).data?.conditionLabel
      const next = current === 'true' ? 'false' : current === 'false' ? undefined : 'true'
      setEdges((eds) =>
        eds.map((e) =>
          e.id === edge.id
            ? {
                ...e,
                label: next,
                data: { conditionLabel: next },
              }
            : e,
        ),
      )
    },
    [nodes, setEdges],
  )

  const handleNodeClick = useCallback(
    (_: React.MouseEvent, node: Node) => onSelectionChange(node.id),
    [onSelectionChange],
  )

  const handlePaneClick = useCallback(() => onSelectionChange(null), [onSelectionChange])

  const nodesWithSelection = nodes.map((n) => ({
    ...n,
    selected: n.id === selectedNodeId,
  }))

  const displayEdges = useMemo(() => {
    const hasStatuses = Object.keys(nodeStatuses).length > 0
    if (!hasStatuses) return edges
    return edges.map((e) => {
      const result = nodeStatuses[e.source]
      if (result?.status === 'succeeded') return { ...e, style: { stroke: '#3fb950', strokeWidth: 2 } }
      if (result?.status === 'failed')    return { ...e, style: { stroke: '#f85149', strokeWidth: 2 } }
      if (result?.status === 'running')   return { ...e, style: { stroke: '#58a6ff', strokeWidth: 2 }, animated: true }
      return { ...e, style: { stroke: '#30363d', strokeWidth: 2 } }
    })
  }, [edges, nodeStatuses])

  return (
    <NodeStatusContext.Provider value={nodeStatuses}>
      <ReactFlow
        nodes={nodesWithSelection}
        edges={displayEdges}
        onNodesChange={onNodesChange}
        onEdgesChange={onEdgesChange}
        onConnect={onConnect}
        onEdgeClick={onEdgeClick}
        onNodeClick={handleNodeClick}
        onPaneClick={handlePaneClick}
        nodeTypes={nodeTypes}
        fitView
        deleteKeyCode="Delete"
        proOptions={{ hideAttribution: true }}
      >
        <Background variant={BackgroundVariant.Dots} gap={24} size={1} color="#21262d" />
        <Controls style={{ background: 'var(--panel)', border: '1px solid var(--border)' }} />
        <MiniMap
          nodeColor={(n) => {
            const nt = (n.data as FlowNodeData)?.nodeType
            const colors: Record<string, string> = {
              trigger: '#238636',
              http: '#1f6feb',
              agent: '#8957e5',
              condition: '#d29922',
              approval: '#0891b2',
              map: '#e05d44',
              filter: '#0891b2',
              aggregate: '#7c3aed',
              sort: '#d97706',
              transform: '#0d9488',
              delay: '#b45309',
              sub_workflow: '#be185d',
              assert: '#dc2626',
              catch: '#ea580c',
              fan_out: '#0891b2',
              fan_in: '#0891b2',
              code: '#7c3aed',
              slack: '#4a154b',
              email: '#0369a1',
            }
            return nt ? (colors[nt] ?? '#30363d') : '#30363d'
          }}
          style={{ background: 'var(--panel)', border: '1px solid var(--border)' }}
        />
      </ReactFlow>
    </NodeStatusContext.Provider>
  )
}
