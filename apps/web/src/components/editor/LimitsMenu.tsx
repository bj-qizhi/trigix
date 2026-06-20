// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useState } from 'react'
import { useAuth } from '../../AuthContext'
import * as api from '../../api/client'
import type { WorkflowRecord } from '../../types'

// Toolbar "Limits" popover: inline-edit the workflow's SLA, rate limit, max
// concurrency and AI budget. Owns all of its UI state (open flag, the four
// editing toggles and their inputs) and performs the save calls itself,
// reporting the refreshed workflow back via onUpdate. Extracted verbatim from
// WorkflowEditor's toolbar.

export interface LimitsMenuProps {
  workflow: WorkflowRecord
  workflowId: string
  zh: boolean
  toast: (message: string, kind?: 'success' | 'error') => void
  onUpdate: (wf: WorkflowRecord) => void
}

export function LimitsMenu({ workflow, workflowId, zh, toast, onUpdate }: LimitsMenuProps) {
  const { auth } = useAuth()
  const [open, setOpen] = useState(false)
  const [editingSla, setEditingSla] = useState(false)
  const [newSlaInput, setNewSlaInput] = useState('')
  const [editingRateLimit, setEditingRateLimit] = useState(false)
  const [newRateLimitInput, setNewRateLimitInput] = useState('')
  const [editingMaxConcurrent, setEditingMaxConcurrent] = useState(false)
  const [newMaxConcurrentInput, setNewMaxConcurrentInput] = useState('')
  const [editingBudget, setEditingBudget] = useState(false)
  const [newBudgetInput, setNewBudgetInput] = useState('')

  const handleSaveSla = async () => {
    const secs = newSlaInput.trim() === '' ? null : parseInt(newSlaInput.trim(), 10)
    if (newSlaInput.trim() !== '' && (isNaN(secs!) || secs! <= 0)) {
      toast(zh ? 'SLA 必须是正整数秒数' : 'SLA must be a positive integer (seconds)', 'error')
      return
    }
    try {
      const wf = await api.updateWorkflowSla(auth!.tenantId, workflowId, workflow.name, secs)
      onUpdate(wf)
      toast(secs == null ? (zh ? 'SLA 已清除' : 'SLA cleared') : (zh ? `SLA 设为 ${secs}s` : `SLA set to ${secs}s`))
    } catch (e) {
      toast(String(e), 'error')
    } finally {
      setEditingSla(false)
    }
  }

  const handleSaveRateLimit = async () => {
    const limit = newRateLimitInput.trim() === '' ? null : parseInt(newRateLimitInput.trim(), 10)
    if (newRateLimitInput.trim() !== '' && (isNaN(limit!) || limit! <= 0)) {
      toast(zh ? '速率限制必须是正整数' : 'Rate limit must be a positive integer', 'error')
      return
    }
    try {
      const wf = await api.updateWorkflowRateLimit(auth!.tenantId, workflowId, workflow.name, limit)
      onUpdate(wf)
      toast(limit == null ? (zh ? '速率限制已清除' : 'Rate limit cleared') : (zh ? `速率限制设为每小时 ${limit} 次` : `Rate limit set to ${limit}/hr`))
    } catch (e) {
      toast(String(e), 'error')
    } finally {
      setEditingRateLimit(false)
    }
  }

  const handleSaveMaxConcurrent = async () => {
    const limit = newMaxConcurrentInput.trim() === '' ? null : parseInt(newMaxConcurrentInput.trim(), 10)
    if (newMaxConcurrentInput.trim() !== '' && (isNaN(limit!) || limit! <= 0)) {
      toast(zh ? '并发限制必须是正整数' : 'Concurrent limit must be a positive integer', 'error')
      return
    }
    try {
      const wf = await api.updateWorkflowMaxConcurrentRuns(auth!.tenantId, workflowId, workflow.name, limit)
      onUpdate(wf)
      toast(limit == null ? (zh ? '并发限制已清除' : 'Concurrent limit cleared') : (zh ? `并发限制设为 ${limit}` : `Concurrent limit set to ${limit}`))
    } catch (e) {
      toast(String(e), 'error')
    } finally {
      setEditingMaxConcurrent(false)
    }
  }

  const handleSaveBudget = async () => {
    const budget = newBudgetInput.trim() === '' ? null : parseFloat(newBudgetInput.trim())
    if (newBudgetInput.trim() !== '' && (isNaN(budget!) || budget! <= 0)) {
      toast(zh ? '预算必须是正数（美元）' : 'Budget must be a positive number (USD)', 'error')
      return
    }
    try {
      const wf = await api.updateWorkflowBudget(auth!.tenantId, workflowId, workflow.name, budget)
      onUpdate(wf)
      toast(budget == null ? (zh ? 'AI 成本预算已清除' : 'AI cost budget cleared') : (zh ? `预算设为 $${budget.toFixed(2)}` : `Budget set to $${budget.toFixed(2)}`))
    } catch (e) { toast(String(e), 'error') }
    setEditingBudget(false)
  }

  return (
    <span className="tb-pop-wrap">
      <button
        className="btn btn-sm"
        onClick={() => setOpen((v) => !v)}
        title={zh ? 'SLA、速率、并发与 AI 预算' : 'SLA, rate limit, concurrency & AI budget'}
      >⚙ {zh ? '限额' : 'Limits'}{(workflow.sla_seconds != null || workflow.max_runs_per_hour != null || workflow.max_concurrent_runs != null || workflow.budget_usd != null) && <span className="tb-dot" />}</button>
      {open && (
        <div className="tb-popover" onMouseLeave={() => setOpen(false)}>
          <div className="tb-pop-title">{zh ? '限额与预算' : 'Limits & budget'}</div>
          <div className="tb-pop-row">
            <label>{zh ? 'SLA（秒）' : 'SLA (sec)'}</label>
            {editingSla ? (
              <input autoFocus type="number" min={1} value={newSlaInput} onChange={(e) => setNewSlaInput(e.target.value)} onBlur={handleSaveSla} onKeyDown={(e) => { if (e.key === 'Enter') handleSaveSla(); if (e.key === 'Escape') setEditingSla(false) }} />
            ) : (
              <button className="tb-pop-val" onClick={() => { setEditingSla(true); setNewSlaInput(workflow.sla_seconds != null ? String(workflow.sla_seconds) : '') }}>{workflow.sla_seconds != null ? `${workflow.sla_seconds}s` : (zh ? '未设置' : 'not set')}</button>
            )}
          </div>
          <div className="tb-pop-row">
            <label>{zh ? '速率（次/时）' : 'Rate (runs/hr)'}</label>
            {editingRateLimit ? (
              <input autoFocus type="number" min={1} value={newRateLimitInput} onChange={(e) => setNewRateLimitInput(e.target.value)} onBlur={handleSaveRateLimit} onKeyDown={(e) => { if (e.key === 'Enter') handleSaveRateLimit(); if (e.key === 'Escape') setEditingRateLimit(false) }} />
            ) : (
              <button className="tb-pop-val" onClick={() => { setEditingRateLimit(true); setNewRateLimitInput(workflow.max_runs_per_hour != null ? String(workflow.max_runs_per_hour) : '') }}>{workflow.max_runs_per_hour != null ? `${workflow.max_runs_per_hour}/hr` : (zh ? '未设置' : 'not set')}</button>
            )}
          </div>
          <div className="tb-pop-row">
            <label>{zh ? '并发上限' : 'Max concurrent'}</label>
            {editingMaxConcurrent ? (
              <input autoFocus type="number" min={1} value={newMaxConcurrentInput} onChange={(e) => setNewMaxConcurrentInput(e.target.value)} onBlur={handleSaveMaxConcurrent} onKeyDown={(e) => { if (e.key === 'Enter') handleSaveMaxConcurrent(); if (e.key === 'Escape') setEditingMaxConcurrent(false) }} />
            ) : (
              <button className="tb-pop-val" onClick={() => { setEditingMaxConcurrent(true); setNewMaxConcurrentInput(workflow.max_concurrent_runs != null ? String(workflow.max_concurrent_runs) : '') }}>{workflow.max_concurrent_runs != null ? String(workflow.max_concurrent_runs) : (zh ? '未设置' : 'not set')}</button>
            )}
          </div>
          <div className="tb-pop-row">
            <label>{zh ? 'AI 预算（$）' : 'AI budget ($)'}</label>
            {editingBudget ? (
              <input autoFocus type="number" min={0.01} step={0.01} value={newBudgetInput} onChange={(e) => setNewBudgetInput(e.target.value)} onBlur={handleSaveBudget} onKeyDown={(e) => { if (e.key === 'Enter') handleSaveBudget(); if (e.key === 'Escape') setEditingBudget(false) }} />
            ) : (
              <button className="tb-pop-val" onClick={() => { setEditingBudget(true); setNewBudgetInput(workflow.budget_usd != null ? String(workflow.budget_usd) : '') }}>{workflow.budget_usd != null ? `$${workflow.budget_usd.toFixed(2)}` : (zh ? '未设置' : 'not set')}</button>
            )}
          </div>
        </div>
      )}
    </span>
  )
}
