// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useCallback, useEffect, useRef, useState } from 'react'
import type { Dispatch, MutableRefObject, SetStateAction } from 'react'
import type { NodeType } from '../../types'
import type { FlowNode, FlowEdge } from '../Canvas'

// Owns the editable graph: nodes/edges/selection, the undo-redo history, and the
// node-CRUD mutations (add, duplicate, rename, config update, delete). Extracted
// from WorkflowEditor so the component no longer carries this branchy state
// machine inline. Behaviour is preserved verbatim — see editor-graph.spec.ts.

export interface GraphState {
  nodes: FlowNode[]
  edges: FlowEdge[]
  setNodes: Dispatch<SetStateAction<FlowNode[]>>
  setEdges: Dispatch<SetStateAction<FlowEdge[]>>
  selectedNodeId: string | null
  setSelectedNodeId: Dispatch<SetStateAction<string | null>>
  // Live mirrors for the editor's window-level keyboard handler, which must read
  // the latest values without re-subscribing on every change.
  nodesRef: MutableRefObject<FlowNode[]>
  edgesRef: MutableRefObject<FlowEdge[]>
  selectedNodeIdRef: MutableRefObject<string | null>
  recentNodeTypes: NodeType[]
  pushHistory: () => void
  undo: () => void
  redo: () => void
  addNode: (type: NodeType) => void
  addNodeAt: (type: NodeType, position: { x: number; y: number }) => void
  updateConfig: (nodeId: string, config: Record<string, unknown>) => void
  renameNodeId: (oldId: string, rawNewId: string) => { ok: boolean; error?: string }
  duplicateNode: () => void
  deleteSelected: () => void
}

export interface GraphStateOptions {
  zh: boolean
  toast: (message: string, kind?: 'success' | 'error') => void
}

export function useGraphState({ zh, toast }: GraphStateOptions): GraphState {
  const [nodes, setNodes] = useState<FlowNode[]>([])
  const [edges, setEdges] = useState<FlowEdge[]>([])
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null)
  const [recentNodeTypes, setRecentNodeTypes] = useState<NodeType[]>(() => {
    try { return JSON.parse(localStorage.getItem('af:recentNodes') ?? '[]') as NodeType[] } catch { return [] }
  })

  const nodesRef = useRef<FlowNode[]>([])
  const edgesRef = useRef<FlowEdge[]>([])
  const selectedNodeIdRef = useRef<string | null>(null)
  const undoStack = useRef<{ nodes: FlowNode[]; edges: FlowEdge[] }[]>([])
  const redoStack = useRef<{ nodes: FlowNode[]; edges: FlowEdge[] }[]>([])

  // Keep refs in sync with latest nodes/edges/selectedNodeId for the keyboard handler.
  useEffect(() => { nodesRef.current = nodes; edgesRef.current = edges }, [nodes, edges])
  useEffect(() => { selectedNodeIdRef.current = selectedNodeId }, [selectedNodeId])

  const pushHistory = useCallback(() => {
    undoStack.current = [...undoStack.current, { nodes: nodesRef.current, edges: edgesRef.current }].slice(-50)
    redoStack.current = []
  }, [])

  const undo = useCallback(() => {
    const snap = undoStack.current.pop()
    if (snap) {
      redoStack.current.push({ nodes: nodesRef.current, edges: edgesRef.current })
      setNodes(snap.nodes)
      setEdges(snap.edges)
    }
  }, [])

  const redo = useCallback(() => {
    const snap = redoStack.current.pop()
    if (snap) {
      undoStack.current.push({ nodes: nodesRef.current, edges: edgesRef.current })
      setNodes(snap.nodes)
      setEdges(snap.edges)
    }
  }, [])

  // Add a node at a specific canvas position (used by palette drag-and-drop).
  const addNodeAt = useCallback((type: NodeType, position: { x: number; y: number }) => {
    pushHistory()
    const id = `${type}-${Date.now()}`
    const newNode: FlowNode = { id, type, position, data: { label: id, nodeType: type, config: {} } }
    setNodes((prev) => [...prev, newNode])
    setSelectedNodeId(id)
    // Track recently used
    setRecentNodeTypes((prev) => {
      const next = [type, ...prev.filter((t) => t !== type)].slice(0, 5)
      try { localStorage.setItem('af:recentNodes', JSON.stringify(next)) } catch { /* ignore */ }
      return next
    })
  }, [pushHistory])

  // Click-to-add: drop into a tidy grid slot based on current node count.
  const addNode = useCallback((type: NodeType) => {
    const existing = nodesRef.current.length
    addNodeAt(type, { x: (existing % 4) * 280 + 80, y: Math.floor(existing / 4) * 140 + 80 })
  }, [addNodeAt])

  // Update node config from panel
  const updateConfig = useCallback((nodeId: string, config: Record<string, unknown>) => {
    setNodes((prev) =>
      prev.map((n) =>
        n.id === nodeId ? { ...n, data: { ...n.data, config } } : n,
      ),
    )
  }, [])

  // Rename a node's id. The id is referenced by edges (source/target) and by
  // template variables ({{id.field}}) inside other nodes' configs, so we rewrite
  // all of those atomically. Returns ok/error for inline panel feedback.
  const renameNodeId = useCallback((oldId: string, rawNewId: string): { ok: boolean; error?: string } => {
    const newId = rawNewId.trim()
    if (newId === oldId) return { ok: true }
    if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(newId)) {
      return { ok: false, error: zh ? 'ID 只能用字母/数字/下划线，且不以数字开头' : 'Use letters, digits, underscore; cannot start with a digit' }
    }
    if (nodesRef.current.some((n) => n.id === newId)) {
      return { ok: false, error: zh ? 'ID 已被占用' : 'ID already in use' }
    }
    pushHistory()
    // Rewrite {{oldId.field}} / {{oldId}} references in every config object.
    const esc = oldId.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
    const re = new RegExp('(\\{\\{\\s*)' + esc + '(?=[.}\\s|])', 'g')
    const rewriteCfg = (cfg: Record<string, unknown>): Record<string, unknown> => {
      try { return JSON.parse(JSON.stringify(cfg).replace(re, '$1' + newId)) } catch { return cfg }
    }
    setNodes((prev) => prev.map((n) => {
      const config = rewriteCfg(n.data.config)
      if (n.id === oldId) {
        return { ...n, id: newId, data: { ...n.data, config, label: n.data.label === oldId ? newId : n.data.label } }
      }
      return { ...n, data: { ...n.data, config } }
    }))
    setEdges((prev) => prev.map((e) => ({
      ...e,
      source: e.source === oldId ? newId : e.source,
      target: e.target === oldId ? newId : e.target,
      id: e.id && e.id.includes(oldId) ? e.id.split(oldId).join(newId) : e.id,
    })))
    setSelectedNodeId(newId)
    return { ok: true }
  }, [zh, pushHistory])

  // Duplicate the currently selected node.
  const duplicateNode = useCallback(() => {
    const node = nodesRef.current.find((n) => n.id === selectedNodeIdRef.current)
    if (!node) return
    pushHistory()
    const newId = `${node.data.nodeType ?? 'node'}-${Date.now()}`
    const newNode: FlowNode = {
      ...node,
      id: newId,
      position: { x: node.position.x + 60, y: node.position.y + 60 },
      data: { ...node.data, label: newId, config: { ...(node.data.config ?? {}) } },
    }
    setNodes((prev) => [...prev, newNode])
    setSelectedNodeId(newId)
    toast(zh ? `已复制为 ${newId}` : `Duplicated as ${newId}`)
  }, [zh, toast, pushHistory])

  // Delete the selected node and any edges touching it.
  const deleteSelected = useCallback(() => {
    const id = selectedNodeIdRef.current
    if (!id) return
    undoStack.current.push({ nodes: nodesRef.current, edges: edgesRef.current })
    redoStack.current = []
    setNodes((prev) => prev.filter((n) => n.id !== id))
    setEdges((prev) => prev.filter((ed) => ed.source !== id && ed.target !== id))
    setSelectedNodeId(null)
  }, [])

  return {
    nodes, edges, setNodes, setEdges,
    selectedNodeId, setSelectedNodeId,
    nodesRef, edgesRef, selectedNodeIdRef,
    recentNodeTypes,
    pushHistory, undo, redo,
    addNode, addNodeAt, updateConfig, renameNodeId, duplicateNode, deleteSelected,
  }
}
