// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useState } from 'react'
import type { Dispatch, SetStateAction } from 'react'
import { useAuth } from '../../AuthContext'
import * as api from '../../api/client'
import type { WorkflowRecord, WorkflowVersionRecord, ExecutionRecord, InputField } from '../../types'
import { graphFromApi, fromFlowGraph, type FlowNode, type FlowEdge } from '../Canvas'

// Owns the editor's version-persistence surface: saving a new draft version,
// publishing (and publish-and-run), the version-history modal (list, diff,
// load, rollback) and JSON export. Extracted from WorkflowEditor; behaviour is
// preserved verbatim. The save round-trip is covered by editor-graph.spec.ts.
//
// The central `version`/`setVersion` stay in the component (read across the UI
// and set by the bootstrap loader); the cross-cutting graph/run/workflow
// setters and `collectPublishWarnings` are passed in, mirroring the hook
// composition already used by useGraphState / useWorkflowRun.

export interface WorkflowPersistenceState {
  saving: boolean
  publishing: boolean
  publishingAndRunning: boolean
  versions: WorkflowVersionRecord[]
  setVersions: Dispatch<SetStateAction<WorkflowVersionRecord[]>>
  showVersions: boolean
  setShowVersions: Dispatch<SetStateAction<boolean>>
  loadingVersions: boolean
  diffVersionId: string | null
  setDiffVersionId: Dispatch<SetStateAction<string | null>>
  diffCompareId: string | null
  setDiffCompareId: Dispatch<SetStateAction<string | null>>
  showComparePicker: string | null
  setShowComparePicker: Dispatch<SetStateAction<string | null>>
  rollingBack: string | null
  saveMessage: string
  setSaveMessage: Dispatch<SetStateAction<string>>
  showSaveMessage: boolean
  setShowSaveMessage: Dispatch<SetStateAction<boolean>>
  handleSave: () => Promise<void>
  handlePublish: () => Promise<void>
  handlePublishAndRun: () => Promise<void>
  handleExport: () => Promise<void>
  handleShowVersions: () => Promise<void>
  handleLoadVersion: (versionId: string) => Promise<void>
  handleRollback: (versionId: string, versionNum: number) => Promise<void>
}

export interface WorkflowPersistenceOptions {
  workflowId: string
  zh: boolean
  toast: (message: string, kind?: 'success' | 'error') => void
  workflow: WorkflowRecord | null
  setWorkflow: Dispatch<SetStateAction<WorkflowRecord | null>>
  version: WorkflowVersionRecord | null
  setVersion: Dispatch<SetStateAction<WorkflowVersionRecord | null>>
  nodes: FlowNode[]
  edges: FlowEdge[]
  inputSchema: InputField[]
  setInputSchema: Dispatch<SetStateAction<InputField[]>>
  setNodes: Dispatch<SetStateAction<FlowNode[]>>
  setEdges: Dispatch<SetStateAction<FlowEdge[]>>
  setSelectedNodeId: Dispatch<SetStateAction<string | null>>
  setWebhookUrl: Dispatch<SetStateAction<string | null>>
  setWebhookSecret: Dispatch<SetStateAction<string | null>>
  inputJson: string
  setExecution: Dispatch<SetStateAction<ExecutionRecord | null>>
  collectPublishWarnings: () => string[]
}

export function useWorkflowPersistence(opts: WorkflowPersistenceOptions): WorkflowPersistenceState {
  const {
    workflowId, zh, toast,
    workflow, setWorkflow, version, setVersion,
    nodes, edges, inputSchema, setInputSchema,
    setNodes, setEdges, setSelectedNodeId,
    setWebhookUrl, setWebhookSecret,
    inputJson, setExecution,
    collectPublishWarnings,
  } = opts
  const { auth } = useAuth()

  const [saving, setSaving] = useState(false)
  const [publishing, setPublishing] = useState(false)
  const [publishingAndRunning, setPublishingAndRunning] = useState(false)
  const [versions, setVersions] = useState<WorkflowVersionRecord[]>([])
  const [showVersions, setShowVersions] = useState(false)
  const [loadingVersions, setLoadingVersions] = useState(false)
  const [diffVersionId, setDiffVersionId] = useState<string | null>(null)
  const [diffCompareId, setDiffCompareId] = useState<string | null>(null)
  const [showComparePicker, setShowComparePicker] = useState<string | null>(null)
  const [rollingBack, setRollingBack] = useState<string | null>(null)
  const [saveMessage, setSaveMessage] = useState('')
  const [showSaveMessage, setShowSaveMessage] = useState(false)

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
        input_schema: inputSchema,
      }
      const ver = await api.createVersion(auth!.tenantId, workflowId, graph, saveMessage.trim() || undefined)
      setVersion(ver)
      setSaveMessage('')
      setShowSaveMessage(false)
      toast(zh ? '版本已保存' : 'Version saved')
    } catch (e) {
      toast(String(e), 'error')
    } finally {
      setSaving(false)
    }
  }

  // Publish latest draft version
  const handlePublish = async () => {
    if (!version || version.status === 'published') return
    const warnings = collectPublishWarnings()
    if (warnings.length > 0) {
      const msg = `Publishing with ${warnings.length} warning${warnings.length > 1 ? 's' : ''}:\n\n${warnings.map((w) => `• ${w}`).join('\n')}\n\nPublish anyway?`
      if (!window.confirm(msg)) return
    }
    setPublishing(true)
    try {
      const ver = await api.publishVersion(auth!.tenantId, version.id)
      setVersion(ver)
      const wf = await api.getWorkflow(auth!.tenantId, workflowId)
      setWorkflow(wf)
      const info = await api.getWebhook(auth!.tenantId, ver.id)
      setWebhookUrl(window.location.origin + info.url)
      setWebhookSecret(info.secret ?? null)
      toast(zh ? '版本已发布' : 'Version published')
    } catch (e) {
      toast(String(e), 'error')
    } finally {
      setPublishing(false)
    }
  }

  const handlePublishAndRun = async () => {
    if (!version || version.status === 'published') return
    const warnings = collectPublishWarnings()
    if (warnings.length > 0) {
      const msg = `Publishing with ${warnings.length} warning${warnings.length > 1 ? 's' : ''}:\n\n${warnings.map((w) => `• ${w}`).join('\n')}\n\nPublish and run anyway?`
      if (!window.confirm(msg)) return
    }
    setPublishingAndRunning(true)
    try {
      const ver = await api.publishVersion(auth!.tenantId, version.id)
      setVersion(ver)
      const wf = await api.getWorkflow(auth!.tenantId, workflowId)
      setWorkflow(wf)
      const info = await api.getWebhook(auth!.tenantId, ver.id)
      setWebhookUrl(window.location.origin + info.url)
      setWebhookSecret(info.secret ?? null)
      const rec = await api.startExecutionFromVersion(auth!.tenantId, ver.id, inputJson || '{}')
      setExecution(rec)
      toast(zh ? '已发布并开始运行' : 'Published and started run')
    } catch (e) {
      toast(String(e), 'error')
    } finally {
      setPublishingAndRunning(false)
    }
  }

  // Export current published version as JSON download
  const handleExport = async () => {
    if (!workflow?.latest_version_id) { toast(zh ? '请先发布一个版本后再导出' : 'Publish a version first to export', 'error'); return }
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

  // Open version history modal
  const handleShowVersions = async () => {
    setShowVersions(true)
    setLoadingVersions(true)
    try {
      const vers = await api.listVersions(auth!.tenantId, workflowId)
      setVersions(vers.sort((a, b) => b.version - a.version))
    } catch (e) {
      toast(String(e), 'error')
      setShowVersions(false)
    } finally {
      setLoadingVersions(false)
    }
  }

  // Load a specific version's graph into the canvas
  const handleLoadVersion = async (versionId: string) => {
    try {
      const ver = await api.getVersion(auth!.tenantId, versionId)
      setVersion(ver)
      const { nodes: fn, edges: fe } = graphFromApi(ver.graph.nodes, ver.graph.edges)
      setNodes(fn)
      setEdges(fe)
      setInputSchema(ver.graph.input_schema ?? [])
      setSelectedNodeId(null)
      setShowVersions(false)
      toast(zh ? `已加载 v${ver.version}` : `Loaded v${ver.version}`)
    } catch (e) {
      toast(String(e), 'error')
    }
  }

  // Rollback to a historical version (creates new draft version)
  const handleRollback = async (versionId: string, versionNum: number) => {
    if (!window.confirm(zh ? `回滚到 v${versionNum}？这将基于 v${versionNum} 创建一个新草稿版本。` : `Rollback to v${versionNum}? This creates a new draft version based on v${versionNum}.`)) return
    setRollingBack(versionId)
    try {
      const newVer = await api.rollbackVersion(auth!.tenantId, workflowId, versionId)
      setVersions((prev) => [newVer, ...prev])
      setVersion(newVer)
      const { nodes: fn, edges: fe } = graphFromApi(newVer.graph.nodes, newVer.graph.edges)
      setNodes(fn)
      setEdges(fe)
      setInputSchema(newVer.graph.input_schema ?? [])
      setSelectedNodeId(null)
      setShowVersions(false)
      toast(zh ? `已回滚到 v${versionNum} — 新草稿 v${newVer.version} 已创建` : `Rolled back to v${versionNum} — new draft v${newVer.version} created`)
    } catch (e) {
      toast(String(e), 'error')
    } finally {
      setRollingBack(null)
    }
  }

  return {
    saving, publishing, publishingAndRunning,
    versions, setVersions, showVersions, setShowVersions, loadingVersions,
    diffVersionId, setDiffVersionId, diffCompareId, setDiffCompareId,
    showComparePicker, setShowComparePicker, rollingBack,
    saveMessage, setSaveMessage, showSaveMessage, setShowSaveMessage,
    handleSave, handlePublish, handlePublishAndRun, handleExport,
    handleShowVersions, handleLoadVersion, handleRollback,
  }
}
