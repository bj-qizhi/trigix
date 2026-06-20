// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useCallback, useEffect, useState } from 'react'
import type { Dispatch, SetStateAction } from 'react'
import { useAuth } from '../../AuthContext'
import * as api from '../../api/client'
import { getStoredAuth } from '../../auth'
import type { WorkflowRecord, ExecutionRecord, ExecutionSummary, EnvSetSummary } from '../../types'

// Owns the editor's run/execution surface: the run inputs (input JSON, env set,
// label, callback URL, dry-run), the active execution + live SSE/poll updates,
// the recent-execution history, and the run / approve / reject actions.
// Extracted from WorkflowEditor; behaviour is preserved verbatim. The Ctrl+Enter
// run path is covered by editor-graph.spec.ts.

export interface WorkflowRunState {
  execution: ExecutionRecord | null
  setExecution: Dispatch<SetStateAction<ExecutionRecord | null>>
  recentExecutions: ExecutionSummary[]
  running: boolean
  inputJson: string
  setInputJson: Dispatch<SetStateAction<string>>
  envSets: EnvSetSummary[]
  envSet: string
  setEnvSet: Dispatch<SetStateAction<string>>
  runLabel: string
  setRunLabel: Dispatch<SetStateAction<string>>
  callbackUrl: string
  setCallbackUrl: Dispatch<SetStateAction<string>>
  dryRun: boolean
  setDryRun: Dispatch<SetStateAction<boolean>>
  latestExec: ExecutionSummary | null
  setLatestExec: Dispatch<SetStateAction<ExecutionSummary | null>>
  refreshHistory: () => void
  handleRun: () => Promise<void>
  handleApprove: (comment?: string) => Promise<void>
  handleReject: (comment?: string) => Promise<void>
}

export interface WorkflowRunOptions {
  workflowId: string
  workflow: WorkflowRecord | null
  zh: boolean
  toast: (message: string, kind?: 'success' | 'error') => void
  initialInput?: string
}

export function useWorkflowRun({ workflowId, workflow, zh, toast, initialInput }: WorkflowRunOptions): WorkflowRunState {
  const { auth } = useAuth()
  const [execution, setExecution] = useState<ExecutionRecord | null>(null)
  const [recentExecutions, setRecentExecutions] = useState<ExecutionSummary[]>([])
  const [inputJson, setInputJson] = useState(initialInput ?? '{}')
  const [running, setRunning] = useState(false)
  const [envSets, setEnvSets] = useState<EnvSetSummary[]>([{ name: 'default', var_count: 0 }])
  const [envSet, setEnvSet] = useState('default')
  const [runLabel, setRunLabel] = useState('')
  const [callbackUrl, setCallbackUrl] = useState('')
  const [dryRun, setDryRun] = useState(false)
  const [latestExec, setLatestExec] = useState<ExecutionSummary | null>(null)

  const refreshHistory = useCallback(() => {
    api.listExecutions(auth!.tenantId, workflowId)
      .then(setRecentExecutions)
      .catch(() => {})
  }, [workflowId])

  // Load env sets once
  useEffect(() => {
    api.listEnvSets(auth!.tenantId)
      .then((s) => {
        if (s.length > 0) {
          const hasDefault = s.some((x) => x.name === 'default')
          setEnvSets(hasDefault ? s : [{ name: 'default', var_count: 0 }, ...s])
        }
      })
      .catch(() => {})
  }, [])

  // Stream live execution updates via SSE (fall back to polling)
  useEffect(() => {
    if (!execution || execution.status !== 'running') return
    const stored = getStoredAuth()
    let source: EventSource | null = null
    let pollTimer: ReturnType<typeof setInterval> | null = null

    if (typeof EventSource !== 'undefined' && stored?.token) {
      try {
        source = new EventSource(`/v1/executions/${execution.id}/events?token=${encodeURIComponent(stored.token)}`)
        source.onmessage = (ev) => {
          try {
            const updated = JSON.parse(ev.data) as ExecutionRecord
            setExecution(updated)
            if (updated.status !== 'running') {
              refreshHistory()
              source?.close()
            }
          } catch { /* ignore parse errors */ }
        }
        source.onerror = () => {
          source?.close()
          source = null
          // Fall back to polling
          pollTimer = setInterval(async () => {
            try {
              const updated = await api.getExecution(auth!.tenantId, execution.id)
              setExecution(updated)
              if (updated.status !== 'running') { refreshHistory(); clearInterval(pollTimer!) }
            } catch { /* ignore */ }
          }, 1500)
        }
      } catch {
        source = null
      }
    }

    if (!source) {
      pollTimer = setInterval(async () => {
        try {
          const updated = await api.getExecution(auth!.tenantId, execution.id)
          setExecution(updated)
          if (updated.status !== 'running') { refreshHistory(); clearInterval(pollTimer!) }
        } catch { /* ignore */ }
      }, 1000)
    }

    return () => {
      source?.close()
      if (pollTimer) clearInterval(pollTimer)
    }
  }, [execution?.id, execution?.status, refreshHistory])

  // Approve / Reject human approval gate
  const handleApprove = async (comment?: string) => {
    if (!execution) return
    try {
      await api.approveExecution(execution.id, comment)
    } catch (e) {
      toast(String(e), 'error')
    }
  }

  const handleReject = async (comment?: string) => {
    if (!execution) return
    try {
      await api.rejectExecution(execution.id, comment)
    } catch (e) {
      toast(String(e), 'error')
    }
  }

  // Run execution
  const handleRun = async () => {
    if (!workflow?.latest_version_id) return
    let parsed: unknown
    try { parsed = JSON.parse(inputJson) } catch { toast(zh ? '输入 JSON 格式无效' : 'Input JSON is invalid', 'error'); return }
    void parsed
    setRunning(true)
    setExecution(null)
    try {
      const result = await api.startExecutionFromWorkflow(auth!.tenantId, workflowId, inputJson, envSet === 'default' ? undefined : envSet, runLabel || undefined, callbackUrl || undefined, dryRun || undefined)
      setExecution(result)
      refreshHistory()
    } catch (e) {
      toast(String(e), 'error')
    } finally {
      setRunning(false)
    }
  }

  return {
    execution, setExecution, recentExecutions, running,
    inputJson, setInputJson,
    envSets, envSet, setEnvSet,
    runLabel, setRunLabel,
    callbackUrl, setCallbackUrl,
    dryRun, setDryRun,
    latestExec, setLatestExec,
    refreshHistory, handleRun, handleApprove, handleReject,
  }
}
