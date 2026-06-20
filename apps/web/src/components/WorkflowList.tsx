// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useEffect, useRef, useState } from 'react'
import { IconBell, IconFolder, IconGlobe, IconLock, ThemeToggleIcon } from './uiIcons'
import type { IconType } from 'react-icons'
import {
  PiListChecks, PiSealCheck, PiGraph, PiChartLine, PiGift, PiClock, PiPulse,
  PiClipboardText, PiSlidersHorizontal, PiSquaresFour, PiWebhooksLogo, PiKey,
  PiShieldCheck, PiBooks, PiPuzzlePiece, PiBroadcast, PiBuildings, PiLockKey,
} from 'react-icons/pi'
import { useAuth } from '../AuthContext'
import logoWordmark from '../assets/logo-wordmark.svg'
import * as api from '../api/client'
import type { WorkflowExport, WorkflowRecord, ScheduleSummary, ExecutionSummary, WorkflowGraph } from '../types'
import { TemplatesModal, type Template } from './TemplatesModal'
import { CreateWorkflowModal, SystemInfoModal, ShortcutsModal } from './workflowlist/WorkflowListModals'
import { GenerateWorkflowModal } from './GenerateWorkflowModal'
import { useTheme } from '../useTheme'
import { useLocale } from '../useLocale'

interface Props {
  onOpen: (workflowId: string) => void
  onOpenExecution?: (id: string) => void
  onCredentials: () => void
  onAuditLog: () => void
  onRuns: (workflowFilter?: string) => void
  onAnalytics: () => void
  onEnvironment: () => void
  onWorkspaces: () => void
  onWebhooks: () => void
  onApiKeys: () => void
  onSso: () => void
  onKnowledge: () => void
  onCustomNodes: () => void
  onEventSubscriptions: () => void
  onOrgs: () => void
  onAccount: () => void
  onAffiliate: () => void
  onPayouts: () => void
  onUsers: () => void
  onSchedules: () => void
  onMonitoring: () => void
  onApprovals: () => void
  onWorkflowDeps: () => void
}

function formatAge(startedAtSecs: number, zh = false): string {
  const diff = Math.floor(Date.now() / 1000) - startedAtSecs
  if (zh) {
    if (diff < 60) return `${diff}秒前`
    if (diff < 3600) return `${Math.floor(diff / 60)}分钟前`
    if (diff < 86400) return `${Math.floor(diff / 3600)}小时前`
    return `${Math.floor(diff / 86400)}天前`
  }
  if (diff < 60) return `${diff}s ago`
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`
  return `${Math.floor(diff / 86400)}d ago`
}

interface WorkflowStatsProps {
  stats?: { total: number; succeeded: number; failed: number; running: number; lastAt: number }
  onViewRuns?: () => void
}

function Sparkline({ data }: { data: number[] }) {
  const max = Math.max(...data, 1)
  const w = 42, h = 16, barW = 4, gap = 2
  return (
    <svg width={w} height={h} style={{ verticalAlign: 'middle', opacity: 0.7 }}>
      <title>{`Last 7 days: ${data.join(', ')}`}</title>
      {data.map((v, i) => {
        const barH = Math.max(2, Math.round((v / max) * h))
        return (
          <rect
            key={i}
            x={i * (barW + gap)}
            y={h - barH}
            width={barW}
            height={barH}
            fill={v === 0 ? 'var(--border)' : 'var(--link)'}
            rx={1}
          />
        )
      })}
    </svg>
  )
}

function WorkflowStats({ stats, onViewRuns }: WorkflowStatsProps) {
  const { locale } = useLocale()
  const zh = locale === 'zh'
  if (!stats || stats.total === 0) {
    return <span style={{ color: 'var(--muted)', fontSize: 12 }}>—</span>
  }
  const successRate = Math.round((stats.succeeded / stats.total) * 100)
  const rateColor = successRate >= 90 ? 'var(--success-text)' : successRate >= 70 ? 'var(--warning-text)' : 'var(--danger-text)'
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 12 }}>
      <span
        style={{ color: 'var(--muted)', cursor: onViewRuns ? 'pointer' : 'default', textDecoration: onViewRuns ? 'underline' : 'none' }}
        title={onViewRuns ? (zh ? '查看此工作流的运行记录' : 'View runs for this workflow') : undefined}
        onClick={onViewRuns}
      >{stats.total}</span>
      <span style={{ color: rateColor, fontWeight: 600 }}>{successRate}%</span>
      {stats.running > 0 && (
        <span className="dot dot-running" style={{ width: 6, height: 6 }} title={zh ? `${stats.running} 个运行中` : `${stats.running} running`} />
      )}
      {stats.failed > 0 && (
        <span style={{ color: 'var(--danger-text)', fontSize: 11 }} title={zh ? `${stats.failed} 个失败` : `${stats.failed} failed`}>
          {stats.failed}✕
        </span>
      )}
      {stats.lastAt > 0 && (
        <span style={{ color: 'var(--muted)', fontSize: 11 }} title={zh ? '上次运行' : 'Last run'}>
          {formatAge(stats.lastAt, zh)}
        </span>
      )}
    </div>
  )
}

function formatInterval(secs: number, zh = false): string {
  if (zh) {
    if (secs >= 86400 && secs % 86400 === 0) return `每 ${secs / 86400} 天`
    if (secs >= 3600 && secs % 3600 === 0) return `每 ${secs / 3600} 小时`
    if (secs >= 60 && secs % 60 === 0) return `每 ${secs / 60} 分钟`
    return `每 ${secs} 秒`
  }
  if (secs >= 86400 && secs % 86400 === 0) return `every ${secs / 86400}d`
  if (secs >= 3600 && secs % 3600 === 0) return `every ${secs / 3600}h`
  if (secs >= 60 && secs % 60 === 0) return `every ${secs / 60}m`
  return `every ${secs}s`
}

const RECENT_KEY = 'af:recent-workflows'
const MAX_RECENT = 5

function getRecentIds(): string[] {
  try { return JSON.parse(localStorage.getItem(RECENT_KEY) ?? '[]') } catch { return [] }
}
function addRecentId(id: string) {
  const ids = [id, ...getRecentIds().filter((x) => x !== id)].slice(0, MAX_RECENT)
  try { localStorage.setItem(RECENT_KEY, JSON.stringify(ids)) } catch { /* ignore */ }
}

export function WorkflowList({ onOpen, onOpenExecution, onCredentials, onAuditLog, onRuns, onAnalytics, onEnvironment, onWorkspaces, onWebhooks, onApiKeys, onSso, onKnowledge, onCustomNodes, onEventSubscriptions, onOrgs, onAccount, onAffiliate, onPayouts, onUsers, onSchedules, onMonitoring, onApprovals, onWorkflowDeps }: Props) {
  const { auth, logout } = useAuth()
  const { theme, toggle: toggleTheme } = useTheme()
  const { locale, toggle: toggleLocale, t } = useLocale()
  const zh = locale === 'zh'
  const [workflows, setWorkflows] = useState<WorkflowRecord[]>([])
  const [schedules, setSchedules]  = useState<ScheduleSummary[]>([])
  const [execSummaries, setExecSummaries] = useState<ExecutionSummary[]>([])
  const [loading, setLoading]      = useState(true)
  const [error, setError]          = useState<string | null>(null)
  const [creating, setCreating]    = useState(false)
  const [importing, setImporting]  = useState(false)
  const [duplicating, setDuplicating] = useState<string | null>(null)
  const [quickRunning, setQuickRunning] = useState<string | null>(null)
  const [quickRunResult, setQuickRunResult] = useState<{ id: string; wfId: string; status: string } | null>(null)
  const [showTemplates, setShowTemplates] = useState(false)
  const [showGenerate, setShowGenerate] = useState(false)
  const [search, setSearch] = useState('')
  const [tagFilter, setTagFilter] = useState('')
  const [statusFilter, setStatusFilter] = useState<'all' | 'published' | 'draft' | 'archived'>('all')
  const [folderFilter, setFolderFilter] = useState('')
  const [runTodayOnly, setRunTodayOnly] = useState(false)
  const [sortBy, setSortBy] = useState<'name' | 'status' | 'runs' | 'recent' | 'created' | 'modified'>('name')
  const [viewMode, setViewMode] = useState<'table' | 'card' | 'kanban' | 'activity'>('table')
  const [hiddenCols, setHiddenCols] = useState<Set<string>>(() => {
    try { return new Set(JSON.parse(localStorage.getItem('af:wl:hidden-cols') ?? '[]') as string[]) } catch { return new Set() }
  })
  const [showColSettings, setShowColSettings] = useState(false)
  const colSettingsRef = useRef<HTMLDivElement>(null)
  const [editingTags, setEditingTags] = useState<WorkflowRecord | null>(null)
  const [tagInput, setTagInput] = useState('')
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set())
  const [bulkArchiving, setBulkArchiving] = useState(false)
  const [restoring, setRestoring] = useState<string | null>(null)
  const [editingDescId, setEditingDescId] = useState<string | null>(null)
  const [descInput, setDescInput] = useState('')
  const [movingFolder, setMovingFolder] = useState<WorkflowRecord | null>(null)
  const [folderInput, setFolderInput] = useState('')
  const [focusedIdx, setFocusedIdx] = useState<number>(-1)
  const [recentIds, setRecentIds] = useState<string[]>(getRecentIds)
  const [showSystemInfo, setShowSystemInfo] = useState(false)
  const [systemInfo, setSystemInfo] = useState<api.SystemInfo | null>(null)
  const [execStats, setExecStats] = useState<api.ExecutionStats | null>(null)
  const [showGlobalSearch, setShowGlobalSearch] = useState(false)
  const [globalQuery, setGlobalQuery] = useState('')
  const [globalResults, setGlobalResults] = useState<api.SearchResult | null>(null)
  const [globalSearching, setGlobalSearching] = useState(false)
  const [billingStatus, setBillingStatus] = useState<api.BillingStatus | null>(null)
  const [serverNotifs, setServerNotifs] = useState<api.AppNotification[]>([])
  const [serverUnread, setServerUnread] = useState(0)
  const [expiringCreds, setExpiringCreds] = useState<api.CredentialSummary[]>([])
  const [showNavMenu, setShowNavMenu] = useState(false)
  const [navMenuRect, setNavMenuRect] = useState<{ top: number; right: number }>({ top: 0, right: 0 })
  const [showUserMenu, setShowUserMenu] = useState(false)
  const [showNotifications, setShowNotifications] = useState(false)
  const [showActivity, setShowActivity] = useState(false)
  const [showShortcuts, setShowShortcuts] = useState(false)
  const [showQuickRun, setShowQuickRun] = useState(false)
  const [qrSearch, setQrSearch] = useState('')
  const [qrInput, setQrInput] = useState('{}')
  const [qrDry, setQrDry] = useState(false)
  const [qrLabel, setQrLabel] = useState('')
  const [qrStarting, setQrStarting] = useState(false)
  const [qrResult, setQrResult] = useState<{ id: string; status: string } | null>(null)
  const qrSearchRef = useRef<HTMLInputElement>(null)
  const navMenuRef = useRef<HTMLDivElement>(null)
  const userMenuRef = useRef<HTMLDivElement>(null)
  const notifRef = useRef<HTMLDivElement>(null)

  const openWorkflow = (id: string) => {
    addRecentId(id)
    setRecentIds(getRecentIds())
    onOpen(id)
  }
  const importRef = useRef<HTMLInputElement>(null)
  const searchRef = useRef<HTMLInputElement>(null)
  const filteredWorkflowsRef = useRef<WorkflowRecord[]>([])
  const focusedIdxRef = useRef<number>(-1)

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement).tagName
      const inInput = tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT'
      if (e.key === '/' && !inInput) {
        e.preventDefault()
        searchRef.current?.focus()
        return
      }
      if ((e.key === 'f' || e.key === 'F') && (e.ctrlKey || e.metaKey) && e.shiftKey) {
        e.preventDefault()
        setShowGlobalSearch(true)
        return
      }
      if ((e.key === 'r' || e.key === 'R') && (e.ctrlKey || e.metaKey) && !e.shiftKey) {
        e.preventDefault()
        setShowQuickRun(true)
        setQrResult(null)
        setTimeout(() => qrSearchRef.current?.focus(), 50)
        return
      }
      if (e.key === '?' || e.key === 'h') { e.preventDefault(); setShowShortcuts(true); return }
      if (inInput) return
      if (e.key === 'n') { e.preventDefault(); setCreating(true); return }
      if (e.key === 'j' || e.key === 'ArrowDown') {
        e.preventDefault()
        setFocusedIdx((prev) => Math.min(prev + 1, filteredWorkflowsRef.current.length - 1))
      } else if (e.key === 'k' || e.key === 'ArrowUp') {
        e.preventDefault()
        setFocusedIdx((prev) => Math.max(prev - 1, 0))
      } else if (e.key === 'Enter') {
        const wf = filteredWorkflowsRef.current[focusedIdxRef.current]
        if (wf) { e.preventDefault(); openWorkflow(wf.id) }
      }
    }
    document.addEventListener('keydown', handler)
    return () => document.removeEventListener('keydown', handler)
  }, [onOpen])

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (navMenuRef.current && !navMenuRef.current.contains(e.target as Node)) setShowNavMenu(false)
      if (userMenuRef.current && !userMenuRef.current.contains(e.target as Node)) setShowUserMenu(false)
      if (notifRef.current && !notifRef.current.contains(e.target as Node)) setShowNotifications(false)
      if (colSettingsRef.current && !colSettingsRef.current.contains(e.target as Node)) setShowColSettings(false)
    }
    document.addEventListener('mousedown', handler)
    return () => document.removeEventListener('mousedown', handler)
  }, [])

  useEffect(() => {
    if (!showGlobalSearch || globalQuery.trim().length < 2) { setGlobalResults(null); return }
    setGlobalSearching(true)
    const timer = setTimeout(() => {
      api.search(auth!.tenantId, globalQuery.trim())
        .then(setGlobalResults)
        .catch(() => {})
        .finally(() => setGlobalSearching(false))
    }, 300)
    return () => clearTimeout(timer)
  }, [globalQuery, showGlobalSearch])

  const load = () => {
    setLoading(true)
    setError(null)
    const loaded = Promise.all([
      api.listWorkflows(auth!.tenantId, auth!.projectId),
      api.listSchedules(auth!.tenantId),
      api.listExecutions(auth!.tenantId),
      api.getExecutionStats(auth!.tenantId),
    ])
      .then(([wfs, scheds, execs, stats]) => {
        setWorkflows(wfs)
        setSchedules(scheds)
        setExecSummaries(execs)
        setExecStats(stats)
      })
      .catch((e: unknown) => setError(String(e)))
      .finally(() => setLoading(false))
    api.getBillingStatus().then(setBillingStatus).catch(() => {})
    api.listNotifications(auth!.tenantId, 20).then((r) => { setServerNotifs(r.notifications); setServerUnread(r.unread_count) }).catch(() => {})
    api.listExpiringCredentials(auth!.tenantId, 7).then(setExpiringCreds).catch(() => {})
    return loaded
  }

  useEffect(() => { void load() }, [])

  const handleCreate = async (name: string, description?: string) => {
    const wf = await api.createWorkflow(auth!.tenantId, auth!.workspaceId, auth!.projectId, name, description)
    setWorkflows((prev) => [wf, ...prev])
    openWorkflow(wf.id)
  }

  const handleQuickRun = async (e: React.MouseEvent, wf: WorkflowRecord) => {
    e.stopPropagation()
    if (!wf.latest_version_id) return
    setQuickRunning(wf.id)
    try {
      const exec = await api.startExecutionFromWorkflow(auth!.tenantId, wf.id, '{}')
      setQuickRunResult({ id: exec.id, wfId: wf.id, status: exec.status })
      setExecSummaries((prev) => [
        { id: exec.id, tenant_id: exec.tenant_id, workflow_id: exec.workflow_id, workflow_version_id: exec.workflow_version_id, status: exec.status, started_at: exec.started_at },
        ...prev,
      ])
    } catch (e) {
      alert(String(e))
    } finally {
      setQuickRunning(null)
    }
  }

  const handleDuplicate = async (e: React.MouseEvent, workflowId: string) => {
    e.stopPropagation()
    setDuplicating(workflowId)
    try {
      const wf = await api.duplicateWorkflow(auth!.tenantId, workflowId)
      setWorkflows((prev) => [wf, ...prev])
      openWorkflow(wf.id)
    } catch (e) {
      alert(String(e))
    } finally {
      setDuplicating(null)
    }
  }

  const handlePin = async (e: React.MouseEvent, wf: WorkflowRecord) => {
    e.stopPropagation()
    try {
      const updated = wf.pinned
        ? await api.unpinWorkflow(auth!.tenantId, wf.id)
        : await api.pinWorkflow(auth!.tenantId, wf.id)
      setWorkflows((prev) => {
        const next = prev.map((w) => w.id === updated.id ? updated : w)
        return [...next.filter((w) => w.pinned), ...next.filter((w) => !w.pinned)]
      })
    } catch (e) {
      alert(String(e))
    }
  }

  const handleToggleVisibility = async (e: React.MouseEvent, wf: WorkflowRecord) => {
    e.stopPropagation()
    const next = wf.visibility === 'private' ? 'tenant' : 'private'
    try {
      const updated = await api.setWorkflowVisibility(auth!.tenantId, wf.id, next)
      setWorkflows((prev) => prev.map((w) => w.id === updated.id ? updated : w))
    } catch (err) {
      alert(String(err))
    }
  }

  const handleImportFile = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0]
    if (!file) return
    e.target.value = ''
    setImporting(true)
    try {
      const text = await file.text()
      const data = JSON.parse(text) as WorkflowExport
      const name = data.name ?? file.name.replace(/\.json$/i, '')
      const wf = await api.importWorkflow(
        auth!.tenantId, auth!.workspaceId, auth!.projectId, name, data.graph,
      )
      setWorkflows((prev) => [wf, ...prev])
      openWorkflow(wf.id)
    } catch (e) {
      alert(String(e))
    } finally {
      setImporting(false)
    }
  }

  const handleImportClipboard = async () => {
    try {
      const text = await navigator.clipboard.readText()
      const data = JSON.parse(text) as WorkflowExport
      if (!data.graph || !data.graph.nodes) throw new Error('Invalid workflow JSON — missing graph.nodes')
      const name = data.name ?? 'Pasted Workflow'
      setImporting(true)
      const wf = await api.importWorkflow(
        auth!.tenantId, auth!.workspaceId, auth!.projectId, name, data.graph,
      )
      setWorkflows((prev) => [wf, ...prev])
      openWorkflow(wf.id)
    } catch (e) {
      alert(zh ? `粘贴板导入失败：${String(e)}` : `Clipboard import failed: ${String(e)}`)
    } finally {
      setImporting(false)
    }
  }

  const handleImportTemplate = async (template: Template) => {
    setShowTemplates(false)
    try {
      const wf = await api.importWorkflow(
        auth!.tenantId, auth!.workspaceId, auth!.projectId, template.name, template.graph,
      )
      setWorkflows((prev) => [wf, ...prev])
      openWorkflow(wf.id)
    } catch (e) {
      alert(String(e))
    }
  }

  const handleGenerateImport = async (graph: WorkflowGraph, name: string, description: string) => {
    try {
      const wf = await api.importWorkflow(
        auth!.tenantId, auth!.workspaceId ?? '', auth!.projectId ?? '', name, graph,
        { description },
      )
      setWorkflows((prev) => [wf, ...prev])
      openWorkflow(wf.id)
    } catch (e) {
      alert(String(e))
    }
  }

  // Build a map from workflow_id → schedule for the table
  const scheduleByWorkflow = new Map(schedules.map((s) => [s.workflow_id, s]))
  const allTags = Array.from(new Set(workflows.flatMap((wf) => wf.tags ?? []))).sort()
  const allFolders = Array.from(new Set(workflows.map((wf) => wf.folder).filter(Boolean) as string[])).sort()

  // Compute per-workflow stats from execution summaries
  const statsByWorkflow = (() => {
    const map = new Map<string, { total: number; succeeded: number; failed: number; running: number; lastAt: number }>()
    for (const ex of execSummaries) {
      const cur = map.get(ex.workflow_id) ?? { total: 0, succeeded: 0, failed: 0, running: 0, lastAt: 0 }
      cur.total++
      if (ex.status === 'succeeded') cur.succeeded++
      else if (ex.status === 'failed') cur.failed++
      else if (ex.status === 'running' || ex.status === 'waiting_approval') cur.running++
      if (ex.started_at > cur.lastAt) cur.lastAt = ex.started_at
      map.set(ex.workflow_id, cur)
    }
    return map
  })()

  const todayStart = Math.floor(new Date().setHours(0, 0, 0, 0) / 1000)

  // 7-day daily run count per workflow for sparklines
  const sparklineByWorkflow = (() => {
    const dayMs = 86400
    const now = Math.floor(Date.now() / 1000)
    const map = new Map<string, number[]>()
    for (const ex of execSummaries) {
      const daysAgo = Math.floor((now - ex.started_at) / dayMs)
      if (daysAgo < 0 || daysAgo >= 7) continue
      const arr = map.get(ex.workflow_id) ?? Array(7).fill(0) as number[]
      arr[6 - daysAgo]++
      map.set(ex.workflow_id, arr)
    }
    return map
  })()

  const filteredWorkflows = (() => {
    const base = workflows.filter((wf) => {
      const q = search.trim().toLowerCase()
      const matchesSearch = !q || wf.name.toLowerCase().includes(q) || (wf.description?.toLowerCase().includes(q)) || (wf.tags ?? []).some((t) => t.toLowerCase().includes(q))
      const matchesTag = !tagFilter || (wf.tags ?? []).includes(tagFilter)
      const matchesStatus = statusFilter === 'all' || wf.status === statusFilter
      const matchesRunToday = !runTodayOnly || (statsByWorkflow.get(wf.id)?.lastAt ?? 0) >= todayStart
      const matchesFolder = !folderFilter || wf.folder === folderFilter
      return matchesSearch && matchesTag && matchesStatus && matchesRunToday && matchesFolder
    })
    // Pinned always float to top; within each group apply sort
    const pinned = base.filter((w) => w.pinned)
    const unpinned = base.filter((w) => !w.pinned)
    const cmp = (a: WorkflowRecord, b: WorkflowRecord): number => {
      if (sortBy === 'name') return a.name.localeCompare(b.name)
      if (sortBy === 'status') return a.status.localeCompare(b.status)
      if (sortBy === 'runs') return (statsByWorkflow.get(b.id)?.total ?? 0) - (statsByWorkflow.get(a.id)?.total ?? 0)
      if (sortBy === 'recent') return (statsByWorkflow.get(b.id)?.lastAt ?? 0) - (statsByWorkflow.get(a.id)?.lastAt ?? 0)
      if (sortBy === 'created') return (b.created_at ?? 0) - (a.created_at ?? 0)
      if (sortBy === 'modified') return (b.updated_at ?? 0) - (a.updated_at ?? 0)
      return 0
    }
    return [...pinned.sort(cmp), ...unpinned.sort(cmp)]
  })()
  filteredWorkflowsRef.current = filteredWorkflows
  focusedIdxRef.current = focusedIdx

  const handleSaveTags = async () => {
    if (!editingTags) return
    const tags = tagInput.split(',').map((t) => t.trim().toLowerCase().replace(/[^a-z0-9_-]/g, '')).filter(Boolean)
    try {
      const updated = await api.updateWorkflowTags(auth!.tenantId, editingTags.id, editingTags.name, tags)
      setWorkflows((prev) => prev.map((wf) => wf.id === updated.id ? updated : wf))
      setEditingTags(null)
    } catch (e) {
      alert(String(e))
    }
  }

  const handleMoveFolder = async () => {
    if (!movingFolder) return
    const folder = folderInput.trim() || null
    try {
      const updated = await api.moveWorkflowToFolder(auth!.tenantId, movingFolder.id, folder)
      setWorkflows((prev) => prev.map((w) => w.id === updated.id ? updated : w))
      setMovingFolder(null)
    } catch (e) {
      alert(String(e))
    }
  }

  const handleSaveDescription = async (wf: WorkflowRecord) => {
    const newDesc = descInput.trim() || undefined
    setEditingDescId(null)
    if (newDesc === (wf.description ?? undefined)) return
    try {
      const updated = await api.updateWorkflowDescription(auth!.tenantId, wf.id, wf.name, newDesc)
      setWorkflows((prev) => prev.map((w) => w.id === updated.id ? updated : w))
    } catch (e) {
      alert(String(e))
    }
  }

  const handleRestore = async (e: React.MouseEvent, workflowId: string) => {
    e.stopPropagation()
    setRestoring(workflowId)
    try {
      const updated = await api.restoreWorkflow(auth!.tenantId, workflowId)
      setWorkflows((prev) => prev.map((w) => w.id === updated.id ? updated : w))
    } catch (err) {
      alert(String(err))
    } finally {
      setRestoring(null)
    }
  }

  const toggleSelect = (id: string) => {
    setSelectedIds((prev) => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }

  const handleSelectAll = () => {
    if (selectedIds.size === filteredWorkflows.length) {
      setSelectedIds(new Set())
    } else {
      setSelectedIds(new Set(filteredWorkflows.map((w) => w.id)))
    }
  }

  const handleBulkArchive = async () => {
    const toArchive = filteredWorkflows.filter((w) => selectedIds.has(w.id) && w.status !== 'archived')
    if (!toArchive.length || !window.confirm(zh ? `归档 ${toArchive.length} 个工作流？` : `Archive ${toArchive.length} workflow(s)?`)) return
    setBulkArchiving(true)
    try {
      const updated = await Promise.all(toArchive.map((w) => api.archiveWorkflow(auth!.tenantId, w.id)))
      setWorkflows((prev) => prev.map((w) => updated.find((u) => u.id === w.id) ?? w))
      setSelectedIds(new Set())
    } catch (e) {
      alert(String(e))
    } finally {
      setBulkArchiving(false)
    }
  }

  const [bulkExporting, setBulkExporting] = useState(false)
  const [bundleImporting, setBundleImporting] = useState(false)
  const [hoverPreviewId, setHoverPreviewId] = useState<string | null>(null)
  const hoverTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const bundleImportRef = useRef<HTMLInputElement>(null)

  const handleImportBundle = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0]
    if (!file) return
    e.target.value = ''
    setBundleImporting(true)
    try {
      const text = await file.text()
      const bundle = JSON.parse(text) as { version?: number; workflows?: Array<{ name: string; graph: { nodes: unknown[]; edges: unknown[] }; description?: string; readme?: string; tags?: string[] }> }
      const items = bundle.workflows ?? []
      if (!Array.isArray(items) || items.length === 0) throw new Error('No workflows found in bundle')
      const results: Array<{ name: string; ok: boolean; error?: string }> = []
      for (const item of items) {
        if (!item.graph?.nodes) { results.push({ name: item.name ?? '?', ok: false, error: 'missing graph' }); continue }
        try {
          const wf = await api.importWorkflow(
            auth!.tenantId, auth!.workspaceId, auth!.projectId, item.name, item.graph as WorkflowGraph,
            { description: item.description, readme: item.readme, tags: item.tags },
          )
          setWorkflows((prev) => [wf, ...prev])
          results.push({ name: item.name, ok: true })
        } catch (err) {
          results.push({ name: item.name, ok: false, error: String(err) })
        }
      }
      const ok = results.filter((r) => r.ok).length
      const fail = results.filter((r) => !r.ok).length
      const msg = fail > 0
        ? (zh ? `导入完成：${ok} 成功，${fail} 失败` : `Import done: ${ok} succeeded, ${fail} failed`)
        : (zh ? `成功导入 ${ok} 个工作流` : `Successfully imported ${ok} workflow${ok !== 1 ? 's' : ''}`)
      alert(msg)
    } catch (e) {
      alert(String(e))
    } finally {
      setBundleImporting(false)
    }
  }

  const handleBulkExport = async () => {
    const selected = workflows.filter((w) => selectedIds.has(w.id))
    setBulkExporting(true)
    try {
      const bundle = await Promise.all(
        selected.map(async (w) => {
          try {
            const ex = await api.exportWorkflow(auth!.tenantId, w.id)
            return { ...ex, tags: w.tags, description: w.description, folder: w.folder }
          } catch {
            return { name: w.name, tags: w.tags, graph: null, exported_at: Math.floor(Date.now() / 1000) }
          }
        })
      )
      const data = JSON.stringify({ version: 1, exported_at: new Date().toISOString(), workflows: bundle }, null, 2)
      const blob = new Blob([data], { type: 'application/json' })
      const url = URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = `workflows-bundle-${new Date().toISOString().slice(0, 10)}.json`
      a.click()
      URL.revokeObjectURL(url)
      setSelectedIds(new Set())
    } catch (e) {
      alert(String(e))
    } finally {
      setBulkExporting(false)
    }
  }

  return (
    <div className="app">
      <header className="topbar">
        <img src={logoWordmark} alt="Trigix" style={{ height: '28px' }} />

        {/* ── Primary actions (center) ── */}
        <div className="topbar-actions" style={{ flex: 1 }}>
          <input
            ref={importRef}
            type="file"
            accept=".json,application/json"
            style={{ display: 'none' }}
            onChange={handleImportFile}
          />
          <input
            ref={bundleImportRef}
            type="file"
            accept=".json,application/json"
            style={{ display: 'none' }}
            onChange={handleImportBundle}
          />
          <button className="btn btn-sm" onClick={() => setShowTemplates(true)} title="Browse workflow templates">
            {t('wl.templates')}
          </button>
          <button
            className="btn btn-sm"
            onClick={() => setShowGenerate(true)}
            title="Generate a workflow using AI"
            style={{ background: 'var(--node-claude)', color: '#fff', border: 'none' }}
          >
            {t('wl.generate')}
          </button>
          <button
            className="btn btn-sm"
            disabled={importing}
            onClick={() => importRef.current?.click()}
            title="Import workflow from JSON file"
          >
            <svg width="12" height="12" viewBox="0 0 12 12" fill="none" style={{ marginRight: 4, verticalAlign: 'middle' }}>
              <path d="M6 1v7M3 5l3 3 3-3M1 10h10" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/>
            </svg>
            {importing ? (locale === 'zh' ? '导入中…' : 'Importing…') : t('wl.import')}
          </button>
          <button
            className="btn btn-sm"
            disabled={importing}
            onClick={handleImportClipboard}
            title="Import workflow from clipboard JSON"
          >
            {t('wl.import.clipboard')}
          </button>
          <button
            className="btn btn-sm"
            onClick={() => { setShowGlobalSearch(true); setGlobalQuery('') }}
            title="Global search (Ctrl+Shift+F)"
          >
            <svg width="13" height="13" viewBox="0 0 13 13" fill="none" style={{ marginRight: 4, verticalAlign: 'middle' }}>
              <circle cx="5.5" cy="5.5" r="4" stroke="currentColor" strokeWidth="1.5"/>
              <path d="M9 9l2.5 2.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round"/>
            </svg>
            {t('wl.btn.search')}
          </button>
          <button className="btn btn-primary" onClick={() => setCreating(true)}>
            + {t('wl.create')}
          </button>
          <button
            className="btn btn-sm btn-success"
            onClick={() => { setShowQuickRun(true); setQrResult(null); setTimeout(() => qrSearchRef.current?.focus(), 50) }}
            title={zh ? '快速运行工作流 (Ctrl+R)' : 'Quick run workflow (Ctrl+R)'}
            style={{ fontSize: 11 }}
          >
            ▶ {zh ? '快速运行…' : 'Quick Run…'}
          </button>
          <button
            className={`btn btn-sm${showActivity ? ' btn-primary' : ''}`}
            onClick={() => setShowActivity((v) => !v)}
            title={locale === 'zh' ? '最近运行活动' : 'Recent activity'}
            style={showActivity ? { background: 'var(--accent)', color: '#fff', border: 'none' } : {}}
          >
            ⚡
          </button>
        </div>

        {/* ── Right zone: nav menu + theme/locale + user menu ── */}
        <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>

          {/* Nav dropdown */}
          <div ref={navMenuRef} style={{ position: 'relative' }}>
            <button
              className="btn btn-sm"
              onClick={(e) => {
                const rect = (e.currentTarget as HTMLElement).getBoundingClientRect()
                setNavMenuRect({ top: rect.bottom + 6, right: window.innerWidth - rect.right })
                setShowNavMenu((v) => !v)
              }}
              title="Navigation"
              style={{ fontWeight: 600, letterSpacing: '0.02em' }}
            >
              ☰
              {execSummaries.filter((r) => r.status === 'waiting_approval').length > 0 && (
                <span style={{
                  display: 'inline-block',
                  marginLeft: 4,
                  background: 'var(--approval-text)',
                  color: 'var(--bg)',
                  borderRadius: '50%',
                  width: 16,
                  height: 16,
                  fontSize: 10,
                  fontWeight: 700,
                  lineHeight: '16px',
                  textAlign: 'center',
                  verticalAlign: 'middle',
                }}>
                  {execSummaries.filter((r) => r.status === 'waiting_approval').length}
                </span>
              )}
            </button>
            {showNavMenu && (
              <div style={{
                position: 'fixed',
                top: navMenuRect.top,
                right: navMenuRect.right,
                background: 'var(--surface)',
                border: '1px solid var(--border)',
                borderRadius: 8,
                boxShadow: '0 8px 24px rgba(0,0,0,0.15)',
                minWidth: 200,
                zIndex: 9999,
                maxHeight: 'calc(100vh - 80px)',
                overflowY: 'auto',
              }}>
                {([
                  { icon: PiListChecks, label: t('wl.btn.runs'), action: () => { onRuns(); setShowNavMenu(false) } },
                  { icon: PiSealCheck, label: locale === 'zh' ? '审批队列' : 'Approvals', action: () => { onApprovals(); setShowNavMenu(false) }, badge: execSummaries.filter((r) => r.status === 'waiting_approval').length || undefined },
                  { icon: PiGraph, label: locale === 'zh' ? '依赖图' : 'Dep Graph', action: () => { onWorkflowDeps(); setShowNavMenu(false) } },
                  { icon: PiChartLine, label: t('wl.btn.analytics'), action: () => { onAnalytics(); setShowNavMenu(false) } },
                  { icon: PiGift, label: locale === 'zh' ? '推荐返佣' : 'Affiliate', action: () => { onAffiliate(); setShowNavMenu(false) } },
                  { icon: PiClock, label: locale === 'zh' ? '计划任务' : 'Schedules', action: () => { onSchedules(); setShowNavMenu(false) } },
                  { icon: PiPulse, label: locale === 'zh' ? '监控中心' : 'Monitoring', action: () => { onMonitoring(); setShowNavMenu(false) } },
                  { icon: PiClipboardText, label: t('wl.btn.audit'), action: () => { onAuditLog(); setShowNavMenu(false) } },
                  { icon: PiSlidersHorizontal, label: t('wl.btn.environment'), action: () => { onEnvironment(); setShowNavMenu(false) } },
                  { icon: PiSquaresFour, label: t('wl.btn.workspaces'), action: () => { onWorkspaces(); setShowNavMenu(false) } },
                  { icon: PiWebhooksLogo, label: t('wl.btn.webhooks'), action: () => { onWebhooks(); setShowNavMenu(false) } },
                  { icon: PiKey, label: t('wl.btn.apikeys'), action: () => { onApiKeys(); setShowNavMenu(false) } },
                  { icon: PiShieldCheck, label: locale === 'zh' ? '企业 SSO' : 'Enterprise SSO', action: () => { onSso(); setShowNavMenu(false) } },
                  { icon: PiBooks, label: locale === 'zh' ? '知识库' : 'Knowledge Bases', action: () => { onKnowledge(); setShowNavMenu(false) } },
                  { icon: PiPuzzlePiece, label: locale === 'zh' ? '自定义节点' : 'Custom Nodes', action: () => { onCustomNodes(); setShowNavMenu(false) } },
                  { icon: PiBroadcast, label: t('wl.btn.events'), action: () => { onEventSubscriptions(); setShowNavMenu(false) } },
                  { icon: PiBuildings, label: t('wl.btn.orgs'), action: () => { onOrgs(); setShowNavMenu(false) } },
                  { icon: PiLockKey, label: t('wl.btn.credentials'), action: () => { onCredentials(); setShowNavMenu(false) } },
                ] as { icon: IconType; label: string; action: () => void; badge?: number }[]).map((item) => (
                  <button
                    key={item.label}
                    onClick={item.action}
                    style={{
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'space-between',
                      width: '100%',
                      padding: '8px 16px',
                      background: 'none',
                      border: 'none',
                      cursor: 'pointer',
                      fontSize: '0.875rem',
                      color: 'var(--text)',
                      textAlign: 'left',
                    }}
                    onMouseEnter={(e) => (e.currentTarget.style.background = 'var(--hover)')}
                    onMouseLeave={(e) => (e.currentTarget.style.background = 'none')}
                  >
                    <span style={{ display: 'inline-flex', alignItems: 'center', gap: 10 }}>
                      <item.icon size={16} style={{ opacity: 0.75, flexShrink: 0 }} />
                      {item.label}
                    </span>
                    {!!item.badge && (
                      <span style={{
                        background: 'var(--approval-text)',
                        color: 'var(--bg)',
                        borderRadius: 10,
                        padding: '1px 6px',
                        fontSize: 11,
                        fontWeight: 700,
                      }}>
                        {item.badge}
                      </span>
                    )}
                  </button>
                ))}
                <div style={{ borderTop: '1px solid var(--border)', margin: '4px 0' }} />
                <a
                  href="/docs"
                  target="_blank"
                  rel="noreferrer"
                  onClick={() => setShowNavMenu(false)}
                  style={{
                    display: 'block',
                    padding: '8px 16px',
                    fontSize: '0.875rem',
                    color: 'var(--text)',
                    textDecoration: 'none',
                  }}
                  onMouseEnter={(e) => (e.currentTarget.style.background = 'var(--hover)')}
                  onMouseLeave={(e) => (e.currentTarget.style.background = 'none')}
                >
                  API Docs ↗
                </a>
                <button
                  onClick={() => {
                    setShowSystemInfo(true)
                    setShowNavMenu(false)
                    if (!systemInfo) api.getSystemInfo().then(setSystemInfo).catch(() => {})
                  }}
                  style={{
                    display: 'block',
                    width: '100%',
                    padding: '8px 16px',
                    background: 'none',
                    border: 'none',
                    cursor: 'pointer',
                    fontSize: '0.875rem',
                    color: 'var(--text)',
                    textAlign: 'left',
                  }}
                  onMouseEnter={(e) => (e.currentTarget.style.background = 'var(--hover)')}
                  onMouseLeave={(e) => (e.currentTarget.style.background = 'none')}
                >
                  {locale === 'zh' ? 'ℹ 系统信息' : 'ℹ System Info'}
                </button>
              </div>
            )}
          </div>

          {/* Notification bell */}
          {(() => {
            const now = Math.floor(Date.now() / 1000)
            const recentFailed = execSummaries.filter((e) => e.status === 'failed' && now - e.started_at < 86400)
            const approvalPending = execSummaries.filter((e) => e.status === 'waiting_approval')
            const quotaWarn = billingStatus && billingStatus.usage_pct >= 80
            const total = approvalPending.length + (quotaWarn ? 1 : 0) + serverUnread
            return (
              <div ref={notifRef} style={{ position: 'relative' }}>
                <button
                  className="btn btn-sm"
                  onClick={() => setShowNotifications((v) => !v)}
                  title={locale === 'zh' ? '通知' : 'Notifications'}
                  style={{ position: 'relative' }}
                >
                  <IconBell size={15} />
                  {total > 0 && (
                    <span style={{
                      position: 'absolute', top: -4, right: -4,
                      background: 'var(--danger-text, #dc2626)', color: '#fff',
                      borderRadius: '50%', width: 16, height: 16, fontSize: 9,
                      fontWeight: 700, display: 'flex', alignItems: 'center', justifyContent: 'center',
                    }}>
                      {Math.min(total, 99)}
                    </span>
                  )}
                </button>
                {showNotifications && (
                  <div style={{
                    position: 'absolute', top: 'calc(100% + 6px)', right: 0, minWidth: 320, maxWidth: 380,
                    background: 'var(--surface)', border: '1px solid var(--border)', borderRadius: 8,
                    boxShadow: '0 8px 24px rgba(0,0,0,0.15)', zIndex: 1000, overflow: 'hidden',
                  }}>
                    <div style={{ padding: '10px 14px', borderBottom: '1px solid var(--border)', fontWeight: 600, fontSize: 13 }}>
                      {locale === 'zh' ? `通知 ${total > 0 ? `(${total})` : ''}` : `Notifications ${total > 0 ? `(${total})` : ''}`}
                    </div>
                    {total === 0 && (
                      <div style={{ padding: '1.5rem', textAlign: 'center', color: 'var(--muted)', fontSize: 13 }}>
                        {locale === 'zh' ? '没有新通知 ✓' : 'All clear ✓'}
                      </div>
                    )}
                    {approvalPending.length > 0 && (
                      <button
                        onClick={() => { onRuns(); setShowNotifications(false) }}
                        style={{ display: 'flex', width: '100%', padding: '10px 14px', gap: 10, alignItems: 'flex-start', background: 'var(--warning-bg, #fef9c3)', border: 'none', cursor: 'pointer', textAlign: 'left' }}
                        onMouseEnter={(e) => (e.currentTarget.style.filter = 'brightness(0.97)')}
                        onMouseLeave={(e) => (e.currentTarget.style.filter = 'none')}
                      >
                        <span style={{ fontSize: 16 }}>✋</span>
                        <div>
                          <div style={{ fontWeight: 600, fontSize: 13, color: 'var(--warning-text, #92400e)' }}>
                            {locale === 'zh' ? `${approvalPending.length} 条执行待审批` : `${approvalPending.length} execution${approvalPending.length !== 1 ? 's' : ''} waiting for approval`}
                          </div>
                          <div style={{ fontSize: 11, color: 'var(--muted)', marginTop: 2 }}>
                            {locale === 'zh' ? '点击前往审批' : 'Click to review'}
                          </div>
                        </div>
                      </button>
                    )}
                    {quotaWarn && billingStatus && (
                      <button
                        onClick={() => { setShowNotifications(false) }}
                        style={{ display: 'flex', width: '100%', padding: '10px 14px', gap: 10, alignItems: 'flex-start', background: billingStatus.usage_pct >= 100 ? 'var(--danger-bg, #fee2e2)' : 'var(--warning-bg, #fef9c3)', border: 'none', cursor: 'pointer', textAlign: 'left' }}
                      >
                        <span style={{ fontSize: 16 }}>⚠</span>
                        <div>
                          <div style={{ fontWeight: 600, fontSize: 13, color: billingStatus.usage_pct >= 100 ? 'var(--danger-text, #dc2626)' : 'var(--warning-text, #92400e)' }}>
                            {billingStatus.usage_pct >= 100
                              ? (locale === 'zh' ? '执行配额已耗尽' : 'Execution quota exhausted')
                              : (locale === 'zh' ? `配额已使用 ${billingStatus.usage_pct.toFixed(0)}%` : `Quota ${billingStatus.usage_pct.toFixed(0)}% used`)}
                          </div>
                          <div style={{ fontSize: 11, color: 'var(--muted)', marginTop: 2 }}>
                            {billingStatus.usage.executions_used.toLocaleString()} / {billingStatus.quota.max_executions_per_month.toLocaleString()}
                          </div>
                        </div>
                      </button>
                    )}
                    {recentFailed.slice(0, 5).map((e) => {
                      const wfName = workflows.find((w) => w.id === e.workflow_id)?.name ?? e.workflow_id.slice(0, 8)
                      const ageMin = Math.floor((Math.floor(Date.now() / 1000) - e.started_at) / 60)
                      return (
                        <button
                          key={e.id}
                          onClick={() => { onRuns(); setShowNotifications(false) }}
                          style={{ display: 'flex', width: '100%', padding: '8px 14px', gap: 10, alignItems: 'flex-start', background: 'none', border: 'none', cursor: 'pointer', textAlign: 'left', borderTop: '1px solid var(--border)' }}
                          onMouseEnter={(e2) => (e2.currentTarget.style.background = 'var(--hover)')}
                          onMouseLeave={(e2) => (e2.currentTarget.style.background = 'none')}
                        >
                          <span style={{ fontSize: 14, marginTop: 1 }}>✕</span>
                          <div style={{ flex: 1, minWidth: 0 }}>
                            <div style={{ fontWeight: 500, fontSize: 13, color: 'var(--danger-text, #dc2626)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                              {locale === 'zh' ? `失败：${wfName}` : `Failed: ${wfName}`}
                            </div>
                            <div style={{ fontSize: 11, color: 'var(--muted)', marginTop: 1 }}>
                              {ageMin < 60 ? (locale === 'zh' ? `${ageMin} 分钟前` : `${ageMin}m ago`) : (locale === 'zh' ? `${Math.floor(ageMin / 60)} 小时前` : `${Math.floor(ageMin / 60)}h ago`)}
                            </div>
                          </div>
                        </button>
                      )
                    })}
                    {recentFailed.length > 5 && (
                      <button
                        onClick={() => { onRuns(); setShowNotifications(false) }}
                        style={{ display: 'block', width: '100%', padding: '8px 14px', background: 'none', border: 'none', borderTop: '1px solid var(--border)', cursor: 'pointer', fontSize: 12, color: 'var(--link)', textAlign: 'center' }}
                      >
                        {locale === 'zh' ? `查看全部 ${recentFailed.length} 条失败记录 →` : `View all ${recentFailed.length} failures →`}
                      </button>
                    )}
                    {serverNotifs.length > 0 && (
                      <>
                        <div style={{ padding: '6px 14px 2px', fontSize: 10, fontWeight: 700, color: 'var(--muted)', textTransform: 'uppercase', letterSpacing: '0.08em', borderTop: '1px solid var(--border)', display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                          <span>{locale === 'zh' ? '系统通知' : 'System Notifications'}</span>
                          {serverUnread > 0 && (
                            <button
                              style={{ fontSize: 10, color: 'var(--link)', background: 'none', border: 'none', cursor: 'pointer', padding: 0 }}
                              onClick={() => {
                                api.markAllNotificationsRead().catch(() => {})
                                setServerNotifs((prev) => prev.map((n) => ({ ...n, read: true })))
                                setServerUnread(0)
                              }}
                            >
                              {locale === 'zh' ? '全部标记已读' : 'Mark all read'}
                            </button>
                          )}
                        </div>
                        {serverNotifs.map((n) => {
                          const levelColor = n.level === 'error' ? 'var(--danger-text, #dc2626)' : n.level === 'warning' ? 'var(--warning-text, #92400e)' : 'var(--text)'
                          const levelIcon = n.level === 'error' ? '✕' : n.level === 'warning' ? '⚠' : 'ℹ'
                          const ageMin = Math.floor((Date.now() / 1000 - n.created_at) / 60)
                          return (
                            <div
                              key={n.id}
                              style={{ display: 'flex', padding: '8px 14px', gap: 8, alignItems: 'flex-start', borderTop: '1px solid var(--border)', background: n.read ? 'none' : 'rgba(99,102,241,0.04)' }}
                            >
                              <span style={{ fontSize: 12, color: levelColor, marginTop: 2, flexShrink: 0 }}>{levelIcon}</span>
                              <div style={{ flex: 1, minWidth: 0 }}>
                                <div style={{ fontWeight: n.read ? 400 : 600, fontSize: 12, color: levelColor }}>{n.title}</div>
                                {n.body && <div style={{ fontSize: 11, color: 'var(--muted)', marginTop: 1, wordBreak: 'break-word' }}>{n.body}</div>}
                                <div style={{ fontSize: 10, color: 'var(--muted)', marginTop: 2 }}>
                                  {ageMin < 60 ? (locale === 'zh' ? `${ageMin} 分钟前` : `${ageMin}m ago`) : (locale === 'zh' ? `${Math.floor(ageMin / 60)} 小时前` : `${Math.floor(ageMin / 60)}h ago`)}
                                </div>
                              </div>
                              <button
                                style={{ fontSize: 10, color: 'var(--muted)', background: 'none', border: 'none', cursor: 'pointer', flexShrink: 0, padding: 0, marginTop: 2 }}
                                title={locale === 'zh' ? '删除' : 'Dismiss'}
                                onClick={() => {
                                  if (!n.read) { api.markNotificationRead(n.id).catch(() => {}); setServerUnread((c) => Math.max(0, c - 1)) }
                                  api.deleteNotification(n.id).catch(() => {})
                                  setServerNotifs((prev) => prev.filter((x) => x.id !== n.id))
                                }}
                              >
                                ✕
                              </button>
                            </div>
                          )
                        })}
                      </>
                    )}
                  </div>
                )}
              </div>
            )
          })()}

          <button className="btn btn-sm" onClick={toggleTheme} title="Toggle dark/light theme">
            {theme === 'dark' ? <ThemeToggleIcon dark /> : <ThemeToggleIcon dark={false} />}
          </button>
          <button className="btn btn-sm" onClick={toggleLocale} title="切换语言 / Switch language">
            {locale === 'zh' ? 'EN' : '中'}
          </button>

          {/* User avatar dropdown */}
          <div ref={userMenuRef} style={{ position: 'relative' }}>
            <button
              onClick={() => setShowUserMenu((v) => !v)}
              title={`Signed in as ${auth?.role ?? 'user'}`}
              style={{
                width: 32,
                height: 32,
                borderRadius: '50%',
                background: 'var(--accent)',
                color: '#fff',
                border: 'none',
                cursor: 'pointer',
                fontWeight: 700,
                fontSize: '0.8rem',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                flexShrink: 0,
              }}
            >
              {(auth?.role ?? 'U')[0].toUpperCase()}
            </button>
            {showUserMenu && (
              <div style={{
                position: 'absolute',
                top: 'calc(100% + 6px)',
                right: 0,
                background: 'var(--surface)',
                border: '1px solid var(--border)',
                borderRadius: 8,
                boxShadow: '0 8px 24px rgba(0,0,0,0.15)',
                minWidth: 180,
                zIndex: 1000,
                overflow: 'hidden',
              }}>
                {auth?.role && (
                  <div style={{ padding: '10px 16px 8px', borderBottom: '1px solid var(--border)' }}>
                    <span style={{
                      padding: '2px 8px',
                      borderRadius: 4,
                      fontSize: '0.7rem',
                      fontWeight: 700,
                      textTransform: 'uppercase',
                      letterSpacing: '0.05em',
                      background: auth.role === 'admin' ? '#312e81' : auth.role === 'viewer' ? '#1e3a5f' : '#14532d',
                      color: auth.role === 'admin' ? '#c7d2fe' : auth.role === 'viewer' ? '#93c5fd' : '#86efac',
                    }}>
                      {auth.role}
                    </span>
                  </div>
                )}
                <button
                  onClick={() => { onAccount(); setShowUserMenu(false) }}
                  style={{
                    display: 'block',
                    width: '100%',
                    padding: '8px 16px',
                    background: 'none',
                    border: 'none',
                    cursor: 'pointer',
                    fontSize: '0.875rem',
                    color: 'var(--text)',
                    textAlign: 'left',
                  }}
                  onMouseEnter={(e) => (e.currentTarget.style.background = 'var(--hover)')}
                  onMouseLeave={(e) => (e.currentTarget.style.background = 'none')}
                >
                  {t('wl.btn.account')}
                </button>
                {auth?.role === 'admin' && (
                  <button
                    onClick={() => { onUsers(); setShowUserMenu(false) }}
                    style={{
                      display: 'block',
                      width: '100%',
                      padding: '8px 16px',
                      background: 'none',
                      border: 'none',
                      cursor: 'pointer',
                      fontSize: '0.875rem',
                      color: 'var(--text)',
                      textAlign: 'left',
                    }}
                    onMouseEnter={(e) => (e.currentTarget.style.background = 'var(--hover)')}
                    onMouseLeave={(e) => (e.currentTarget.style.background = 'none')}
                  >
                    {t('wl.btn.users')}
                  </button>
                )}
                {auth?.role === 'admin' && (
                  <button
                    onClick={() => { onPayouts(); setShowUserMenu(false) }}
                    style={{
                      display: 'block',
                      width: '100%',
                      padding: '8px 16px',
                      background: 'none',
                      border: 'none',
                      cursor: 'pointer',
                      fontSize: '0.875rem',
                      color: 'var(--text)',
                      textAlign: 'left',
                    }}
                    onMouseEnter={(e) => (e.currentTarget.style.background = 'var(--hover)')}
                    onMouseLeave={(e) => (e.currentTarget.style.background = 'none')}
                  >
                    {locale === 'zh' ? '提现审批' : 'Payout approvals'}
                  </button>
                )}
                <div style={{ borderTop: '1px solid var(--border)', margin: '4px 0' }} />
                <button
                  onClick={() => { logout(); setShowUserMenu(false) }}
                  style={{
                    display: 'block',
                    width: '100%',
                    padding: '8px 16px',
                    background: 'none',
                    border: 'none',
                    cursor: 'pointer',
                    fontSize: '0.875rem',
                    color: 'var(--danger-text, #dc2626)',
                    textAlign: 'left',
                  }}
                  onMouseEnter={(e) => (e.currentTarget.style.background = 'var(--hover)')}
                  onMouseLeave={(e) => (e.currentTarget.style.background = 'none')}
                >
                  {t('nav.signout')}
                </button>
              </div>
            )}
          </div>
        </div>
      </header>

      {/* ── Billing quota warning banner ── */}
      {billingStatus && billingStatus.usage_pct >= 80 && (
        <div style={{
          background: billingStatus.usage_pct >= 100 ? 'var(--danger-bg, #fee2e2)' : 'var(--warning-bg, #fef9c3)',
          color: billingStatus.usage_pct >= 100 ? 'var(--danger-text, #991b1b)' : 'var(--warning-text, #713f12)',
          padding: '8px 20px',
          fontSize: '0.85rem',
          display: 'flex',
          alignItems: 'center',
          gap: '8px',
        }}>
          <span style={{ fontWeight: 600 }}>
            {billingStatus.usage_pct >= 100
              ? (locale === 'zh' ? '⚠ 本月执行配额已耗尽' : '⚠ Monthly execution quota exhausted')
              : (locale === 'zh' ? '⚠ 执行配额使用超过 80%' : '⚠ Execution quota over 80% used')}
          </span>
          <span>
            {locale === 'zh'
              ? `已使用 ${billingStatus.usage.executions_used.toLocaleString()} / ${billingStatus.quota.max_executions_per_month.toLocaleString()} 次（${billingStatus.usage_pct.toFixed(1)}%），套餐：${billingStatus.quota.tier}`
              : `${billingStatus.usage.executions_used.toLocaleString()} / ${billingStatus.quota.max_executions_per_month.toLocaleString()} used (${billingStatus.usage_pct.toFixed(1)}%), tier: ${billingStatus.quota.tier}`}
          </span>
          {billingStatus.usage_pct >= 100 && (
            <span style={{ marginLeft: 'auto' }}>
              {locale === 'zh' ? '请升级套餐以继续运行工作流。' : 'Upgrade your plan to continue running workflows.'}
            </span>
          )}
        </div>
      )}

      {/* ── Expiring credentials banner ── */}
      {expiringCreds.length > 0 && (
        <div style={{
          background: 'var(--warning-bg, #fef9c3)',
          color: 'var(--warning-text, #713f12)',
          padding: '8px 20px',
          fontSize: '0.85rem',
          display: 'flex',
          alignItems: 'center',
          gap: 8,
        }}>
          <span style={{ fontWeight: 600 }}>
            {zh
              ? `${expiringCreds.length} 个凭据将在 7 天内过期：`
              : `${expiringCreds.length} credential${expiringCreds.length !== 1 ? 's' : ''} expiring within 7 days:`}
          </span>
          <span style={{ display: 'flex', gap: 6, flexWrap: 'wrap' }}>
            {expiringCreds.map((c) => (
              <span key={c.id} style={{ padding: '1px 8px', background: 'rgba(0,0,0,0.08)', borderRadius: 12, fontSize: 12 }}>
                {c.name}
              </span>
            ))}
          </span>
          <button
            className="btn btn-sm"
            style={{ marginLeft: 'auto', fontSize: 11 }}
            onClick={onCredentials}
          >
            {zh ? '管理凭据 →' : 'Manage credentials →'}
          </button>
        </div>
      )}

      <main className="list-page">
        {/* ── Dashboard summary bar ── */}
        {!loading && workflows.length > 0 && (() => {
          const published = workflows.filter((w) => w.status === 'published').length
          // Prefer backend aggregate stats (full history) over client-side count (limited to loaded page)
          const totalRuns = execStats?.total ?? execSummaries.length
          const running   = execStats ? (execStats.running + execStats.waiting_approval) : execSummaries.filter((e) => e.status === 'running' || e.status === 'waiting_approval').length
          const succeeded = execStats?.succeeded ?? execSummaries.filter((e) => e.status === 'succeeded').length
          const successRate = totalRuns > 0 ? Math.round(succeeded / totalRuns * 100) : 0
          const todayStart = Math.floor(new Date().setHours(0, 0, 0, 0) / 1000)
          const todayRuns = execSummaries.filter((e) => e.started_at >= todayStart).length
          return (
            <div style={{
              display: 'flex', gap: 10, marginBottom: 18, flexWrap: 'wrap',
            }}>
              {[
                { label: t('wl.stat.workflows'), value: `${workflows.length}`, sub: `${published} ${t('wl.filter.published').toLowerCase()}` },
                { label: t('wl.stat.runs'), value: String(totalRuns), sub: `${todayRuns} ${locale === 'zh' ? '今日' : 'today'}` },
                { label: t('wl.stat.running'), value: String(running), sub: running > 0 ? (locale === 'zh' ? '活跃中' : 'active now') : (locale === 'zh' ? '空闲' : 'idle'), highlight: running > 0 ? 'var(--link)' : undefined },
                { label: t('wl.stat.success_rate'), value: totalRuns > 0 ? `${successRate}%` : '—', sub: `${succeeded} of ${totalRuns}`, highlight: successRate >= 90 ? 'var(--success)' : successRate >= 70 ? '#d97706' : totalRuns > 0 ? 'var(--danger-text)' : undefined },
              ].map(({ label, value, sub, highlight }) => (
                <div key={label} style={{
                  flex: '1 1 120px', minWidth: 100,
                  background: 'var(--panel)', border: '1px solid var(--border)',
                  borderRadius: 'var(--radius)', padding: '10px 14px',
                }}>
                  <div style={{ fontSize: 11, color: 'var(--muted)', fontWeight: 600, textTransform: 'uppercase', letterSpacing: '0.05em', marginBottom: 2 }}>{label}</div>
                  <div style={{ fontSize: 20, fontWeight: 700, color: highlight ?? 'var(--fg)' }}>{value}</div>
                  <div style={{ fontSize: 11, color: 'var(--muted)', marginTop: 1 }}>{sub}</div>
                </div>
              ))}
            </div>
          )
        })()}

        <div className="list-header">
          <h1>{t('wl.stat.workflows')}</h1>
          <div style={{ display: 'flex', alignItems: 'center', gap: 8, flexWrap: 'wrap' }}>
            <div style={{ position: 'relative' }}>
              <input
                ref={searchRef}
                placeholder={t('wl.search.placeholder')}
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                style={{ width: 200, paddingRight: search ? 28 : 10 }}
              />
              {search && (
                <button
                  onClick={() => setSearch('')}
                  style={{ position: 'absolute', right: 6, top: '50%', transform: 'translateY(-50%)', background: 'none', border: 'none', color: 'var(--muted)', cursor: 'pointer', fontSize: 14, padding: 0, lineHeight: 1 }}
                >
                  ✕
                </button>
              )}
            </div>
            {(['all', 'published', 'draft', 'archived'] as const).map((s) => (
              <button
                key={s}
                onClick={() => setStatusFilter(s)}
                style={{
                  padding: '2px 8px', borderRadius: 12, border: '1px solid',
                  borderColor: statusFilter === s ? 'var(--link)' : 'var(--border)',
                  background: statusFilter === s ? 'var(--link)' : 'transparent',
                  color: statusFilter === s ? '#fff' : 'var(--muted)',
                  fontSize: 12, cursor: 'pointer', textTransform: 'capitalize',
                }}
              >
                {s === 'all' ? (zh ? '全部' : 'All') : s === 'published' ? (zh ? '已发布' : 'published') : s === 'draft' ? (zh ? '草稿' : 'draft') : (zh ? '已归档' : 'archived')}
              </button>
            ))}
            <button
              onClick={() => setRunTodayOnly((v) => !v)}
              style={{
                padding: '2px 8px', borderRadius: 12, border: '1px solid',
                borderColor: runTodayOnly ? 'var(--link)' : 'var(--border)',
                background: runTodayOnly ? 'var(--link)' : 'transparent',
                color: runTodayOnly ? '#fff' : 'var(--muted)',
                fontSize: 12, cursor: 'pointer',
              }}
              title="Show only workflows that ran today"
            >
              {t('wl.filter.today')}
            </button>
            {allTags.map((tag) => (
              <button
                key={tag}
                onClick={() => setTagFilter(tagFilter === tag ? '' : tag)}
                style={{
                  padding: '2px 8px', borderRadius: 12, border: '1px solid',
                  borderColor: tagFilter === tag ? 'var(--link)' : 'var(--border)',
                  background: tagFilter === tag ? 'var(--link)' : 'transparent',
                  color: tagFilter === tag ? '#fff' : 'var(--muted)',
                  fontSize: 12, cursor: 'pointer',
                }}
              >
                #{tag}
              </button>
            ))}
            {allFolders.map((folder) => (
              <button
                key={folder}
                onClick={() => setFolderFilter(folderFilter === folder ? '' : folder)}
                style={{
                  padding: '2px 8px', borderRadius: 12, border: '1px solid',
                  borderColor: folderFilter === folder ? 'var(--link)' : 'var(--border)',
                  background: folderFilter === folder ? 'var(--link)' : 'transparent',
                  color: folderFilter === folder ? '#fff' : 'var(--muted)',
                  fontSize: 12, cursor: 'pointer',
                }}
                title={`Filter by folder: ${folder}`}
              >
                {folder}
              </button>
            ))}
            <select
              value={sortBy}
              onChange={(e) => setSortBy(e.target.value as typeof sortBy)}
              style={{ fontSize: 12, padding: '2px 6px' }}
              title={zh ? '排序方式' : 'Sort order'}
            >
              <option value="name">{zh ? '名称 A→Z' : 'Name A→Z'}</option>
              <option value="status">{zh ? '状态' : 'Status'}</option>
              <option value="runs">{zh ? '运行次数最多' : 'Most Runs'}</option>
              <option value="recent">{zh ? '最近运行' : 'Recently Run'}</option>
              <option value="modified">{zh ? '最近修改' : 'Recently Modified'}</option>
              <option value="created">{zh ? '最新创建' : 'Newest First'}</option>
            </select>
            <button
              className={`btn${viewMode === 'table' ? ' btn-primary' : ''}`}
              style={{ fontSize: 12, padding: '2px 8px' }}
              title="Table view"
              onClick={() => setViewMode('table')}
            >☰</button>
            <button
              className={`btn${viewMode === 'card' ? ' btn-primary' : ''}`}
              style={{ fontSize: 12, padding: '2px 8px' }}
              title="Card view"
              onClick={() => setViewMode('card')}
            >⊞</button>
            <button
              className={`btn${viewMode === 'kanban' ? ' btn-primary' : ''}`}
              style={{ fontSize: 12, padding: '2px 8px' }}
              title={zh ? '看板视图（按文件夹）' : 'Kanban view (by folder)'}
              onClick={() => setViewMode('kanban')}
            >☷</button>
            <button
              className={`btn${viewMode === 'activity' ? ' btn-primary' : ''}`}
              style={{ fontSize: 12, padding: '2px 8px' }}
              title={zh ? '活动时间线视图' : 'Activity timeline view'}
              onClick={() => setViewMode('activity' as typeof viewMode)}
            >⏱</button>
            {viewMode === 'table' && (
              <div ref={colSettingsRef} style={{ position: 'relative' }}>
                <button
                  className="btn btn-sm"
                  style={{ fontSize: 11 }}
                  onClick={() => setShowColSettings((v) => !v)}
                  title={zh ? '列显示设置' : 'Column visibility'}
                >⚙ {zh ? '列' : 'Cols'}</button>
                {showColSettings && (
                  <div style={{ position: 'absolute', right: 0, top: '110%', zIndex: 100, background: 'var(--surface)', border: '1px solid var(--border)', borderRadius: 8, padding: '8px 12px', minWidth: 160, boxShadow: '0 4px 12px rgba(0,0,0,0.3)' }}>
                    {(['status', 'tags', 'version', 'modified', 'schedule', 'runs'] as const).map((col) => (
                      <label key={col} style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '4px 0', cursor: 'pointer', fontSize: 13 }}>
                        <input
                          type="checkbox"
                          checked={!hiddenCols.has(col)}
                          onChange={() => {
                            setHiddenCols((prev) => {
                              const next = new Set(prev)
                              if (next.has(col)) next.delete(col); else next.add(col)
                              try { localStorage.setItem('af:wl:hidden-cols', JSON.stringify([...next])) } catch { /* ignore */ }
                              return next
                            })
                          }}
                        />
                        {col === 'status' ? (zh ? '状态' : 'Status') : col === 'tags' ? (zh ? '标签' : 'Tags') : col === 'version' ? (zh ? '版本' : 'Version') : col === 'modified' ? (zh ? '修改时间' : 'Modified') : col === 'schedule' ? (zh ? '调度' : 'Schedule') : (zh ? '执行次数' : 'Runs')}
                      </label>
                    ))}
                  </div>
                )}
              </div>
            )}
            {(search || tagFilter || statusFilter !== 'all' || runTodayOnly) && (
              <span style={{ fontSize: 12, color: 'var(--muted)' }}>
                {filteredWorkflows.length} of {workflows.length}
              </span>
            )}
          </div>
        </div>

        {recentIds.length > 0 && !search && !tagFilter && statusFilter === 'all' && (
          <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 8, flexWrap: 'wrap' }}>
            <span style={{ fontSize: 11, color: 'var(--muted)', marginRight: 2 }}>{zh ? '最近：' : 'Recent:'}</span>
            {recentIds
              .map((id) => workflows.find((w) => w.id === id))
              .filter(Boolean)
              .map((wf) => wf && (
                <button
                  key={wf.id}
                  onClick={() => openWorkflow(wf.id)}
                  style={{
                    padding: '1px 8px', borderRadius: 10, border: '1px solid var(--border)',
                    background: 'var(--panel)', color: 'var(--fg)', fontSize: 11,
                    cursor: 'pointer', maxWidth: 140, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
                  }}
                  title={wf.name}
                >
                  {wf.name}
                </button>
              ))}
          </div>
        )}

        {selectedIds.size > 0 && (
          <div style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '6px 14px', background: 'var(--panel)', border: '1px solid var(--border)', borderRadius: 6, marginBottom: 10 }}>
            <span style={{ fontSize: 12, fontWeight: 600, color: 'var(--link)' }}>{zh ? `已选 ${selectedIds.size} 个` : `${selectedIds.size} selected`}</span>
            <button
              className="btn btn-sm btn-danger"
              disabled={bulkArchiving}
              onClick={handleBulkArchive}
            >
              {bulkArchiving ? '…' : (zh ? `归档 (${[...selectedIds].filter(id => workflows.find(w => w.id === id)?.status !== 'archived').length})` : `Archive (${[...selectedIds].filter(id => workflows.find(w => w.id === id)?.status !== 'archived').length})`)}
            </button>
            <button className="btn btn-sm" disabled={bulkExporting} onClick={handleBulkExport} title={zh ? '导出所选工作流（含图结构）' : 'Export selected workflows with full graph'}>
              {bulkExporting ? '…' : (zh ? '↓ 导出' : '↓ Export Bundle')}
            </button>
            <button className="btn btn-sm" onClick={() => setSelectedIds(new Set())}>{zh ? '清除' : 'Clear'}</button>
          </div>
        )}

        {/* Bundle import bar — always visible near top of list when not in bulk-select mode */}
        {selectedIds.size === 0 && (
          <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 6 }}>
            <button
              className="btn btn-sm"
              disabled={bundleImporting}
              onClick={() => bundleImportRef.current?.click()}
              title={zh ? '从 JSON 包文件导入多个工作流' : 'Import multiple workflows from a bundle JSON file'}
              style={{ fontSize: 11 }}
            >
              {bundleImporting ? (zh ? '导入中…' : 'Importing…') : (zh ? '↑ 导入包' : '↑ Import Bundle')}
            </button>
          </div>
        )}

        {loading && <p>{zh ? '加载中…' : 'Loading…'}</p>}
        {error && <p style={{ color: 'var(--danger-text)' }}>{error}</p>}

        {!loading && !error && workflows.length === 0 && (
          <div className="empty-state" style={{ padding: '48px 32px', textAlign: 'center' }}>
            <div style={{ fontSize: 40, marginBottom: 16 }}>⚡</div>
            <h2 style={{ margin: '0 0 8px', fontSize: 18 }}>{t('wl.empty.title')}</h2>
            <p style={{ color: 'var(--muted)', marginBottom: 24, fontSize: 14 }}>
              {t('wl.empty.desc')}
            </p>
            <div style={{ display: 'flex', gap: 12, justifyContent: 'center', flexWrap: 'wrap' }}>
              <button className="btn btn-primary" onClick={() => setCreating(true)}>
                + {t('wl.create')}
              </button>
              <button className="btn" onClick={() => setShowTemplates(true)}>
                {t('wl.templates')}
              </button>
            </div>
          </div>
        )}

        {!loading && !error && workflows.length > 0 && filteredWorkflows.length === 0 && (
          <div className="empty-state">
            <p>{zh ? `无匹配的工作流${search ? `："${search}"` : ''}${tagFilter ? `，标签 #${tagFilter}` : ''}${statusFilter !== 'all' ? `，状态"${statusFilter}"` : ''}。` : `No workflows match${search ? ` "${search}"` : ''}${tagFilter ? ` with tag #${tagFilter}` : ''}${statusFilter !== 'all' ? ` with status "${statusFilter}"` : ''}.`}</p>
          </div>
        )}

        {!loading && filteredWorkflows.length > 0 && viewMode === 'card' && (
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(280px, 1fr))', gap: 14, padding: '8px 0' }}>
            {filteredWorkflows.map((wf) => {
              const stats = statsByWorkflow.get(wf.id)
              const isRunning = (stats?.running ?? 0) > 0
              const spark = sparklineByWorkflow.get(wf.id) ?? []
              const successPct = stats && stats.total > 0 ? Math.round((stats.succeeded / stats.total) * 100) : null
              const sched = scheduleByWorkflow.get(wf.id)
              const borderColor = wf.status === 'published' ? 'var(--success-text, #16a34a)' : wf.status === 'archived' ? 'var(--muted)' : 'var(--border)'
              return (
                <div
                  key={wf.id}
                  style={{
                    background: 'var(--panel)',
                    border: '1px solid var(--border)',
                    borderLeft: `3px solid ${borderColor}`,
                    borderRadius: 8,
                    padding: '14px 14px 10px',
                    cursor: 'pointer',
                    position: 'relative',
                    display: 'flex',
                    flexDirection: 'column',
                    gap: 6,
                    transition: 'box-shadow 0.15s, border-color 0.15s',
                  }}
                  onMouseEnter={(e) => { e.currentTarget.style.boxShadow = '0 2px 12px rgba(0,0,0,0.1)'; e.currentTarget.style.borderColor = 'var(--link)' }}
                  onMouseLeave={(e) => { e.currentTarget.style.boxShadow = 'none'; e.currentTarget.style.borderColor = 'var(--border)' }}
                >
                  {/* Top meta row */}
                  <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                    <span className={`status-badge status-${wf.status}`} style={{ fontSize: 10 }}>{wf.status}</span>
                    {isRunning && <span className="running-dot" />}
                    {wf.pinned && <span title="Pinned" style={{ fontSize: 12 }}>★</span>}
                    {wf.locked && <span title="Locked" style={{ fontSize: 11 }}><IconLock size={12} /></span>}
                    {sched && !sched.paused && <span title={`Scheduled: ${sched.cron_expression ?? `every ${sched.interval_secs}s`}`} style={{ fontSize: 11 }}>⏱</span>}
                    {wf.folder && <span style={{ fontSize: 10, color: 'var(--muted)', marginLeft: 'auto' }}>{wf.folder}</span>}
                  </div>

                  {/* Name */}
                  <div
                    style={{ fontWeight: 700, fontSize: 14, wordBreak: 'break-word', lineHeight: 1.3, cursor: 'pointer', color: 'var(--text)' }}
                    onClick={() => openWorkflow(wf.id)}
                  >
                    {wf.name}
                  </div>

                  {/* Description */}
                  {wf.description && (
                    <div style={{ fontSize: 12, color: 'var(--muted)', overflow: 'hidden', textOverflow: 'ellipsis', display: '-webkit-box', WebkitLineClamp: 2, WebkitBoxOrient: 'vertical' }}>
                      {wf.description}
                    </div>
                  )}

                  {/* Tags */}
                  {wf.tags && wf.tags.length > 0 && (
                    <div style={{ display: 'flex', flexWrap: 'wrap', gap: 3 }}>
                      {wf.tags.slice(0, 4).map((tag) => (
                        <span
                          key={tag}
                          onClick={(e) => { e.stopPropagation(); setTagFilter(tag) }}
                          style={{ fontSize: 10, background: 'var(--bg)', border: '1px solid var(--border)', borderRadius: 3, padding: '1px 5px', color: 'var(--muted)', cursor: 'pointer' }}
                        >
                          #{tag}
                        </span>
                      ))}
                      {wf.tags.length > 4 && <span style={{ fontSize: 10, color: 'var(--muted)' }}>+{wf.tags.length - 4}</span>}
                    </div>
                  )}

                  {/* Stats row */}
                  <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginTop: 2 }}>
                    <div style={{ fontSize: 11, color: 'var(--muted)', display: 'flex', gap: 6, alignItems: 'center' }}>
                      {stats ? (
                        <>
                          <span>{stats.total} {zh ? '次' : 'runs'}</span>
                          {successPct !== null && (
                            <span style={{ fontWeight: 600, color: successPct >= 90 ? 'var(--success-text)' : successPct >= 70 ? '#d97706' : 'var(--danger-text)' }}>
                              {successPct}%
                            </span>
                          )}
                        </>
                      ) : (
                        <span>{zh ? '暂无运行' : 'No runs'}</span>
                      )}
                    </div>
                    {spark.length > 0 && <Sparkline data={spark} />}
                  </div>

                  {/* Quick actions */}
                  <div
                    style={{ display: 'flex', gap: 4, paddingTop: 6, borderTop: '1px solid var(--border)', marginTop: 2 }}
                    onClick={(e) => e.stopPropagation()}
                  >
                    <button
                      className="btn btn-sm"
                      style={{ flex: 1, fontSize: 11, padding: '3px 0' }}
                      onClick={() => openWorkflow(wf.id)}
                      title={zh ? '打开编辑器' : 'Open editor'}
                    >
                      {zh ? '编辑' : 'Edit'}
                    </button>
                    {wf.status === 'published' && (
                      <button
                        className="btn btn-sm"
                        style={{ flex: 1, fontSize: 11, padding: '3px 0', color: 'var(--success-text)', borderColor: 'var(--success-text)' }}
                        disabled={quickRunning === wf.id}
                        onClick={(e) => handleQuickRun(e, wf)}
                        title={zh ? '快速运行' : 'Quick run'}
                      >
                        {quickRunning === wf.id ? '…' : '▶ Run'}
                      </button>
                    )}
                    <button
                      className="btn btn-sm"
                      style={{ flex: 1, fontSize: 11, padding: '3px 0' }}
                      onClick={() => onRuns(wf.name)}
                      title={zh ? '查看运行记录' : 'View runs'}
                    >
                      {zh ? '记录' : 'Runs'}
                    </button>
                  </div>
                </div>
              )
            })}
          </div>
        )}

        {!loading && filteredWorkflows.length > 0 && viewMode === 'kanban' && (() => {
          // Group by folder (or 'Uncategorized')
          const folderGroups = new Map<string, typeof filteredWorkflows>()
          const defaultCol = zh ? '未分类' : 'Uncategorized'
          for (const wf of filteredWorkflows) {
            const col = wf.folder || defaultCol
            if (!folderGroups.has(col)) folderGroups.set(col, [])
            folderGroups.get(col)!.push(wf)
          }
          const columns = [...folderGroups.entries()]
          return (
            <div style={{ display: 'flex', gap: 16, overflowX: 'auto', paddingBottom: 12, alignItems: 'flex-start' }}>
              {columns.map(([folderName, wfs]) => (
                <div key={folderName} style={{
                  minWidth: 240, maxWidth: 280, flexShrink: 0,
                  background: 'var(--panel)', borderRadius: 8, border: '1px solid var(--border)', overflow: 'hidden',
                }}>
                  <div style={{
                    padding: '10px 14px', fontWeight: 600, fontSize: 13,
                    borderBottom: '1px solid var(--border)', background: 'var(--surface)',
                    display: 'flex', alignItems: 'center', justifyContent: 'space-between',
                  }}>
                    <span>{folderName}</span>
                    <span style={{ fontSize: 11, fontWeight: 400, color: 'var(--muted)', background: 'var(--panel)', borderRadius: 10, padding: '1px 7px' }}>{wfs.length}</span>
                  </div>
                  <div style={{ display: 'flex', flexDirection: 'column', gap: 0 }}>
                    {wfs.map((wf) => {
                      const statusBadge = wf.status === 'published' ? { label: zh ? '已发布' : 'Published', color: '#3fb950' }
                        : wf.status === 'archived' ? { label: zh ? '已归档' : 'Archived', color: '#8b949e' }
                        : { label: zh ? '草稿' : 'Draft', color: 'var(--muted)' }
                      const hasSchedule = schedules.some((s) => s.workflow_id === wf.id)
                      return (
                        <div
                          key={wf.id}
                          onClick={() => openWorkflow(wf.id)}
                          style={{
                            padding: '10px 14px', cursor: 'pointer', borderBottom: '1px solid var(--border)',
                            transition: 'background 0.12s',
                          }}
                          onMouseEnter={(e) => (e.currentTarget.style.background = 'var(--hover)')}
                          onMouseLeave={(e) => (e.currentTarget.style.background = '')}
                        >
                          <div style={{ display: 'flex', alignItems: 'flex-start', justifyContent: 'space-between', gap: 4 }}>
                            <span style={{ fontWeight: 500, fontSize: 13, lineHeight: '1.3' }}>{wf.name}</span>
                            <span style={{ fontSize: 10, color: statusBadge.color, background: `${statusBadge.color}22`, borderRadius: 4, padding: '1px 5px', flexShrink: 0, marginTop: 1 }}>
                              {statusBadge.label}
                            </span>
                          </div>
                          {wf.description && (
                            <div style={{ fontSize: 11, color: 'var(--muted)', marginTop: 3, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                              {wf.description}
                            </div>
                          )}
                          <div style={{ display: 'flex', gap: 6, marginTop: 6, flexWrap: 'wrap' }}>
                            {(wf.tags ?? []).map((tag) => (
                              <span key={tag} style={{ fontSize: 10, background: 'var(--border)', borderRadius: 4, padding: '1px 5px', color: 'var(--muted)' }}>{tag}</span>
                            ))}
                            {hasSchedule && <span style={{ fontSize: 10, color: '#58a6ff' }}>⏱</span>}
                            {wf.locked && <span style={{ fontSize: 10, color: '#f85149' }}><IconLock size={12} /></span>}
                          </div>
                        </div>
                      )
                    })}
                  </div>
                </div>
              ))}
            </div>
          )
        })()}

        {/* ── Activity timeline view ── */}
        {!loading && viewMode === 'activity' && (() => {
          const filteredWfIds = new Set(filteredWorkflows.map((w) => w.id))
          const activityItems = execSummaries
            .filter((e) => filteredWfIds.has(e.workflow_id))
            .sort((a, b) => b.started_at - a.started_at)
            .slice(0, 100)

          if (activityItems.length === 0) {
            return (
              <div style={{ padding: '32px', textAlign: 'center', color: 'var(--muted)', fontSize: 13 }}>
                {zh ? '暂无执行记录' : 'No execution activity yet for these workflows.'}
              </div>
            )
          }

          // Group by date
          const groups = new Map<string, typeof activityItems>()
          for (const item of activityItems) {
            const d = new Date(item.started_at * 1000)
            const key = d.toLocaleDateString()
            if (!groups.has(key)) groups.set(key, [])
            groups.get(key)!.push(item)
          }

          const statusColor: Record<string, string> = {
            succeeded: 'var(--success-text)', failed: 'var(--danger-text)',
            running: 'var(--link)', cancelled: 'var(--muted)', waiting_approval: '#d29922',
          }

          return (
            <div style={{ padding: '12px 0', maxWidth: 700 }}>
              {[...groups.entries()].map(([date, items]) => (
                <div key={date} style={{ marginBottom: 20 }}>
                  <div style={{ fontSize: 11, fontWeight: 700, color: 'var(--muted)', textTransform: 'uppercase', letterSpacing: '0.08em', marginBottom: 8, paddingLeft: 4 }}>
                    {date}
                  </div>
                  <div style={{ display: 'flex', flexDirection: 'column', gap: 0 }}>
                    {items.map((exec, i) => {
                      const wf = filteredWorkflows.find((w) => w.id === exec.workflow_id)
                      const dur = exec.finished_at != null ? exec.finished_at - exec.started_at : null
                      const t = new Date(exec.started_at * 1000)
                      const timeStr = `${String(t.getHours()).padStart(2, '0')}:${String(t.getMinutes()).padStart(2, '0')}`
                      return (
                        <div
                          key={exec.id}
                          style={{
                            display: 'flex', alignItems: 'flex-start', gap: 12, padding: '8px 8px',
                            borderBottom: i < items.length - 1 ? '1px solid var(--border)' : undefined,
                            cursor: onOpenExecution ? 'pointer' : undefined,
                          }}
                          onClick={() => onOpenExecution?.(exec.id)}
                        >
                          {/* Timeline dot */}
                          <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', paddingTop: 4 }}>
                            <div style={{ width: 10, height: 10, borderRadius: '50%', background: statusColor[exec.status] ?? 'var(--muted)', flexShrink: 0 }} />
                            {i < items.length - 1 && <div style={{ width: 1, height: 28, background: 'var(--border)', marginTop: 4 }} />}
                          </div>
                          <div style={{ flex: 1, minWidth: 0 }}>
                            <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                              <span style={{ fontWeight: 600, fontSize: 13, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', maxWidth: 220 }}>
                                {wf?.name ?? exec.workflow_id.slice(0, 8)}
                              </span>
                              <span className={`badge badge-${exec.status}`} style={{ fontSize: 10 }}>{exec.status}</span>
                              {exec.trigger_type && <span className="badge" style={{ fontSize: 10 }}>{exec.trigger_type}</span>}
                              {exec.label && <span style={{ fontSize: 11, color: 'var(--link)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', maxWidth: 100 }}>{exec.label}</span>}
                            </div>
                            <div style={{ fontSize: 11, color: 'var(--muted)', marginTop: 2 }}>
                              {timeStr}
                              {dur != null && <span style={{ marginLeft: 8 }}>{dur < 60 ? `${dur}s` : `${Math.floor(dur / 60)}m ${dur % 60}s`}</span>}
                              <span style={{ marginLeft: 8, fontFamily: 'monospace', fontSize: 10, opacity: 0.6 }}>{exec.id.slice(0, 8)}…</span>
                            </div>
                          </div>
                        </div>
                      )
                    })}
                  </div>
                </div>
              ))}
            </div>
          )
        })()}

        {!loading && filteredWorkflows.length > 0 && viewMode === 'table' && (
          <table className="workflow-table">
            <thead>
              <tr>
                <th style={{ width: 32, padding: '0 8px' }}>
                  <input
                    type="checkbox"
                    checked={selectedIds.size > 0 && selectedIds.size === filteredWorkflows.length}
                    ref={(el) => { if (el) el.indeterminate = selectedIds.size > 0 && selectedIds.size < filteredWorkflows.length }}
                    onChange={handleSelectAll}
                    title="Select all"
                  />
                </th>
                <th>{t('wl.col.name')}</th>
                {!hiddenCols.has('status') && <th>{t('wl.col.status')}</th>}
                {!hiddenCols.has('tags') && <th>{t('wl.col.tags')}</th>}
                {!hiddenCols.has('version') && <th>{locale === 'zh' ? '最新版本' : 'Latest Version'}</th>}
                {!hiddenCols.has('modified') && <th>{t('wl.col.modified')}</th>}
                {!hiddenCols.has('schedule') && <th>{t('wl.col.schedule')}</th>}
                {!hiddenCols.has('runs') && <th>{t('wl.col.runs')}</th>}
                <th></th>
              </tr>
            </thead>
            <tbody>
              {filteredWorkflows.map((wf, idx) => {
                const sched = scheduleByWorkflow.get(wf.id)
                const isSelected = selectedIds.has(wf.id)
                const isFocused = idx === focusedIdx
                return (
                  <tr
                    key={wf.id}
                    onClick={() => { setFocusedIdx(idx); openWorkflow(wf.id) }}
                    onMouseEnter={() => {
                      setFocusedIdx(idx)
                      if (hoverTimerRef.current) clearTimeout(hoverTimerRef.current)
                      hoverTimerRef.current = setTimeout(() => setHoverPreviewId(wf.id), 600)
                    }}
                    onMouseLeave={() => {
                      if (hoverTimerRef.current) clearTimeout(hoverTimerRef.current)
                      setHoverPreviewId(null)
                    }}
                    style={{ background: isSelected ? 'color-mix(in srgb, var(--link) 8%, transparent)' : isFocused ? 'var(--panel)' : undefined, outline: isFocused ? '1px solid var(--link)' : undefined, position: 'relative' }}
                  >
                    <td style={{ width: 32, padding: '0 8px' }} onClick={(e) => e.stopPropagation()}>
                      <input
                        type="checkbox"
                        checked={isSelected}
                        onChange={() => toggleSelect(wf.id)}
                      />
                    </td>
                    <td className="name" style={{ position: 'relative' }}>
                      <div title={wf.created_at ? `Created: ${new Date(wf.created_at * 1000).toLocaleDateString()}` : undefined}>{wf.name}</div>
                      {hoverPreviewId === wf.id && (() => {
                        const stats = statsByWorkflow.get(wf.id)
                        const sched = scheduleByWorkflow.get(wf.id)
                        return (
                          <div
                            onClick={(e) => e.stopPropagation()}
                            style={{
                              position: 'absolute', top: '100%', left: 0, zIndex: 1000,
                              background: 'var(--surface)', border: '1px solid var(--border)',
                              borderRadius: 10, boxShadow: '0 8px 24px rgba(0,0,0,0.18)',
                              padding: '14px 16px', minWidth: 280, maxWidth: 340,
                              fontSize: 12, pointerEvents: 'none',
                            }}
                          >
                            <div style={{ fontWeight: 700, fontSize: 14, marginBottom: 6 }}>{wf.name}</div>
                            {wf.description && <div style={{ color: 'var(--text-secondary)', marginBottom: 8, fontSize: 12 }}>{wf.description}</div>}
                            <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap', marginBottom: 8 }}>
                              <span className={`badge badge-${wf.status}`}>{wf.status}</span>
                              {wf.locked && <span className="badge">{zh ? '已锁定' : 'Locked'}</span>}
                              {wf.pinned && <span className="badge">★ {zh ? '已置顶' : 'Pinned'}</span>}
                              {wf.folder && <span className="badge">{wf.folder}</span>}
                            </div>
                            {stats && stats.total > 0 && (
                              <div style={{ display: 'flex', gap: 12, marginBottom: 8, fontSize: 12 }}>
                                <span><strong>{stats.total}</strong> {zh ? '次运行' : 'runs'}</span>
                                <span style={{ color: 'var(--success-text)' }}><strong>{stats.succeeded}</strong> {zh ? '成功' : 'ok'}</span>
                                {stats.failed > 0 && <span style={{ color: 'var(--danger-text)' }}><strong>{stats.failed}</strong> {zh ? '失败' : 'failed'}</span>}
                                {stats.lastAt > 0 && <span style={{ color: 'var(--muted)' }}>{formatAge(stats.lastAt, zh)}</span>}
                              </div>
                            )}
                            {sched && (
                              <div style={{ color: 'var(--muted)', fontSize: 11 }}>
                                ⏱ {sched.cron_expression ?? `${zh ? '每' : 'every'} ${sched.interval_secs}s`}
                                {sched.paused && <span style={{ marginLeft: 6, color: 'var(--warning-text)' }}>({zh ? '已暂停' : 'paused'})</span>}
                              </div>
                            )}
                            {(wf.tags ?? []).length > 0 && (
                              <div style={{ display: 'flex', gap: 4, flexWrap: 'wrap', marginTop: 6 }}>
                                {(wf.tags ?? []).map((tag) => (
                                  <span key={tag} className="badge">{tag}</span>
                                ))}
                              </div>
                            )}
                            {wf.updated_at && (
                              <div style={{ color: 'var(--muted)', fontSize: 10, marginTop: 6 }}>
                                {zh ? '更新于' : 'Updated'} {formatAge(wf.updated_at, zh)}
                              </div>
                            )}
                          </div>
                        )
                      })()}
                      {editingDescId === wf.id ? (
                        <input
                          autoFocus
                          value={descInput}
                          onChange={(e) => setDescInput(e.target.value)}
                          onBlur={() => handleSaveDescription(wf)}
                          onKeyDown={(e) => { if (e.key === 'Enter') handleSaveDescription(wf); if (e.key === 'Escape') setEditingDescId(null) }}
                          onClick={(e) => e.stopPropagation()}
                          placeholder={zh ? '添加描述…' : 'Add description…'}
                          style={{ fontSize: 11, width: 260, marginTop: 2 }}
                        />
                      ) : (
                        <div
                          style={{ fontSize: 11, color: 'var(--muted)', marginTop: 1, maxWidth: 280, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', cursor: 'text' }}
                          title={zh ? '双击编辑描述' : 'Double-click to edit description'}
                          onDoubleClick={(e) => { e.stopPropagation(); setEditingDescId(wf.id); setDescInput(wf.description ?? '') }}
                        >
                          {wf.description ?? <span style={{ opacity: 0.4, fontStyle: 'italic' }}>{zh ? '添加描述…' : 'add description…'}</span>}
                        </div>
                      )}
                      {wf.folder && (
                        <div
                          style={{ fontSize: 10, color: 'var(--muted)', marginTop: 1, display: 'flex', alignItems: 'center', gap: 3, cursor: 'pointer' }}
                          onClick={(e) => { e.stopPropagation(); setFolderFilter(folderFilter === wf.folder ? '' : (wf.folder ?? '')) }}
                          title={`Folder: ${wf.folder} — click to filter`}
                        >
                          <span style={{ textDecoration: 'underline dotted' }}>{wf.folder}</span>
                        </div>
                      )}
                    </td>
                    {!hiddenCols.has('status') && (
                      <td>
                        <span className={`badge badge-${wf.status}`}>{wf.status}</span>
                      </td>
                    )}
                    {!hiddenCols.has('tags') && (
                      <td onClick={(e) => e.stopPropagation()}>
                        <div style={{ display: 'flex', gap: 4, flexWrap: 'wrap', alignItems: 'center' }}>
                          {(wf.tags ?? []).map((tag) => (
                            <span
                              key={tag}
                              onClick={() => setTagFilter(tagFilter === tag ? '' : tag)}
                              style={{
                                padding: '1px 7px', borderRadius: 12, fontSize: 11,
                                background: 'var(--panel)', border: '1px solid var(--border)',
                                color: 'var(--muted)', cursor: 'pointer',
                                fontWeight: tagFilter === tag ? 600 : 400,
                              }}
                            >
                              #{tag}
                            </span>
                          ))}
                          <button
                            className="btn btn-sm btn-icon"
                            style={{ fontSize: 11, padding: '0 4px', opacity: 0.5 }}
                            onClick={(e) => { e.stopPropagation(); setEditingTags(wf); setTagInput((wf.tags ?? []).join(', ')) }}
                            title="Edit tags"
                          >
                            ✎
                          </button>
                        </div>
                      </td>
                    )}
                    {!hiddenCols.has('version') && (
                      <td style={{ color: 'var(--muted)' }}>
                        {wf.latest_version_id ? 'v' + wf.latest_version_id.slice(-4) : '—'}
                      </td>
                    )}
                    {!hiddenCols.has('modified') && (
                      <td style={{ color: 'var(--muted)', fontSize: 12 }}>
                        {wf.updated_at ? formatAge(wf.updated_at, zh) : '—'}
                      </td>
                    )}
                    {!hiddenCols.has('schedule') && (
                      <td>
                        {sched ? (
                          <span style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 12 }}>
                            <span style={{ color: sched.paused ? 'var(--muted)' : 'var(--warning-text)' }}>
                              {sched.paused ? '⏸' : '⏱'} {sched.cron_expression
                                ? <code style={{ fontSize: 11 }}>{sched.cron_expression}</code>
                                : formatInterval(sched.interval_secs, zh)}
                              {sched.paused && <span style={{ marginLeft: 4, color: 'var(--muted)' }}>{zh ? '（已暂停）' : '(paused)'}</span>}
                            </span>
                            <button
                              className="btn btn-sm"
                              style={{ padding: '1px 6px', fontSize: 11 }}
                              onClick={async (e) => {
                                e.stopPropagation()
                                try {
                                  if (sched.paused) await api.resumeSchedule(sched.workflow_version_id)
                                  else await api.pauseSchedule(sched.workflow_version_id)
                                  const updated = await api.listSchedules(auth!.tenantId)
                                  setSchedules(updated)
                                } catch { /* ignore */ }
                              }}
                              title={sched.paused ? 'Resume scheduled runs' : 'Pause scheduled runs'}
                            >
                              {sched.paused ? '▶' : '⏸'}
                            </button>
                          </span>
                        ) : (
                          <span style={{ color: 'var(--muted)', fontSize: 12 }}>—</span>
                        )}
                      </td>
                    )}
                    {!hiddenCols.has('runs') && (
                      <td>
                        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                          <WorkflowStats stats={statsByWorkflow.get(wf.id)} onViewRuns={statsByWorkflow.get(wf.id)?.total ? () => onRuns(wf.name) : undefined} />
                          {sparklineByWorkflow.has(wf.id) && (
                            <Sparkline data={sparklineByWorkflow.get(wf.id)!} />
                          )}
                        </div>
                      </td>
                    )}
                    <td style={{ display: 'flex', gap: 6, alignItems: 'center' }}>
                      <button
                        className="btn btn-sm"
                        onClick={(e) => { e.stopPropagation(); openWorkflow(wf.id) }}
                      >
                        {t('wl.run.open')}
                      </button>
                      {wf.latest_version_id && wf.status === 'published' && (
                        <button
                          className="btn btn-sm btn-success"
                          disabled={quickRunning === wf.id}
                          onClick={(e) => handleQuickRun(e, wf)}
                          title="Run now with empty input"
                        >
                          {quickRunning === wf.id ? '…' : t('wl.run.quick')}
                        </button>
                      )}
                      <button
                        className="btn btn-sm btn-icon"
                        onClick={(e) => handlePin(e, wf)}
                        title={wf.pinned ? (zh ? '取消置顶' : 'Unpin') : (zh ? '置顶' : 'Pin to top')}
                        style={{ fontSize: 14, opacity: wf.pinned ? 1 : 0.4 }}
                      >
                        {wf.pinned ? '★' : '☆'}
                      </button>
                      <button
                        className="btn btn-sm btn-icon"
                        onClick={(e) => handleToggleVisibility(e, wf)}
                        title={wf.visibility === 'private' ? (zh ? '私有 — 点击设为所有人可见' : 'Private — click to make visible to all') : (zh ? '所有人可见 — 点击设为私有' : 'Visible to all — click to make private')}
                        style={{ fontSize: 13, opacity: wf.visibility === 'private' ? 1 : 0.4 }}
                      >
                        {wf.visibility === 'private' ? <IconLock size={12} /> : <IconGlobe size={12} />}
                      </button>
                      <button
                        className="btn btn-sm"
                        disabled={duplicating === wf.id}
                        onClick={(e) => handleDuplicate(e, wf.id)}
                        title={zh ? '复制此工作流' : 'Duplicate this workflow'}
                      >
                        {duplicating === wf.id ? '…' : (zh ? '⧉ 复制' : '⧉ Duplicate')}
                      </button>
                      <button
                        className="btn btn-sm btn-icon"
                        onClick={(e) => { e.stopPropagation(); setMovingFolder(wf); setFolderInput(wf.folder ?? '') }}
                        title={wf.folder ? (zh ? `文件夹：${wf.folder} — 点击移动` : `Folder: ${wf.folder} — click to move`) : (zh ? '移至文件夹' : 'Move to folder')}
                        style={{ fontSize: 12, opacity: wf.folder ? 0.9 : 0.45 }}
                      >
                        <IconFolder size={14} />
                      </button>
                      {wf.status === 'archived' && (
                        <button
                          className="btn btn-sm"
                          disabled={restoring === wf.id}
                          onClick={(e) => handleRestore(e, wf.id)}
                          title={zh ? '恢复为草稿' : 'Restore to draft'}
                        >
                          {restoring === wf.id ? '…' : (zh ? '↩ 恢复' : '↩ Restore')}
                        </button>
                      )}
                    </td>
                  </tr>
                )
              })}
            </tbody>
          </table>
        )}
      </main>

      {/* ── Activity drawer (right overlay) ── */}
      {showActivity && (
        <div style={{
          position: 'fixed',
          top: 52,
          right: 0,
          bottom: 0,
          width: 320,
          background: 'var(--surface)',
          borderLeft: '1px solid var(--border)',
          boxShadow: '-4px 0 16px rgba(0,0,0,0.1)',
          zIndex: 50,
          display: 'flex',
          flexDirection: 'column',
        }}>
          <div style={{ padding: '10px 14px', borderBottom: '1px solid var(--border)', display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
            <span style={{ fontWeight: 700, fontSize: 13 }}>⚡ {locale === 'zh' ? '最近活动' : 'Recent Activity'}</span>
            <button onClick={() => setShowActivity(false)} style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--muted)', fontSize: 18, lineHeight: 1 }}>×</button>
          </div>
          <div style={{ flex: 1, overflowY: 'auto' }}>
            {execSummaries.length === 0 ? (
              <div style={{ padding: '2rem', textAlign: 'center', color: 'var(--muted)', fontSize: 13 }}>
                {locale === 'zh' ? '暂无运行记录' : 'No executions yet'}
              </div>
            ) : (
              execSummaries.slice(0, 50).map((e) => {
                const wfName = workflows.find((w) => w.id === e.workflow_id)?.name ?? e.workflow_id.slice(0, 8) + '…'
                const ageMin = Math.floor((Math.floor(Date.now() / 1000) - e.started_at) / 60)
                const ageStr = ageMin < 1 ? (locale === 'zh' ? '刚刚' : 'just now')
                  : ageMin < 60 ? (locale === 'zh' ? `${ageMin}分钟前` : `${ageMin}m ago`)
                  : (locale === 'zh' ? `${Math.floor(ageMin/60)}小时前` : `${Math.floor(ageMin/60)}h ago`)
                const statusColor: Record<string, string> = {
                  succeeded: 'var(--success-text, #16a34a)',
                  failed: 'var(--danger-text, #dc2626)',
                  running: 'var(--link)',
                  waiting_approval: 'var(--approval-text, #d97706)',
                  cancelled: 'var(--muted)',
                }
                const statusIcon: Record<string, string> = {
                  succeeded: '✓', failed: '✕', running: '●', waiting_approval: '✋', cancelled: '○',
                }
                return (
                  <button
                    key={e.id}
                    onClick={() => {
                      setShowActivity(false)
                      if (onOpenExecution) onOpenExecution(e.id)
                      else onRuns(wfName)
                    }}
                    title={e.id}
                    style={{
                      display: 'flex', width: '100%', padding: '8px 14px', gap: 10, alignItems: 'flex-start',
                      background: 'none', border: 'none', borderBottom: '1px solid var(--border)', cursor: 'pointer', textAlign: 'left',
                    }}
                    onMouseEnter={(ev) => (ev.currentTarget.style.background = 'var(--hover)')}
                    onMouseLeave={(ev) => (ev.currentTarget.style.background = 'none')}
                  >
                    <span style={{ color: statusColor[e.status] ?? 'var(--muted)', fontWeight: 700, fontSize: 14, minWidth: 14, marginTop: 1 }}>
                      {statusIcon[e.status] ?? '?'}
                    </span>
                    <div style={{ flex: 1, minWidth: 0 }}>
                      <div style={{ fontWeight: 500, fontSize: 12, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                        {wfName}
                      </div>
                      <div style={{ fontSize: 11, color: 'var(--muted)', display: 'flex', gap: 6, marginTop: 1 }}>
                        <span style={{ color: statusColor[e.status] ?? 'var(--muted)', fontWeight: 600 }}>{e.status}</span>
                        {e.label && <span>· {e.label}</span>}
                        {e.finished_at && e.started_at && (
                          <span>· {e.finished_at - e.started_at < 60
                            ? `${e.finished_at - e.started_at}s`
                            : `${Math.floor((e.finished_at - e.started_at) / 60)}m`}</span>
                        )}
                        <span style={{ marginLeft: 'auto' }}>{ageStr}</span>
                      </div>
                    </div>
                  </button>
                )
              })
            )}
          </div>
          {execSummaries.length > 0 && (
            <div style={{ padding: '10px 14px', borderTop: '1px solid var(--border)' }}>
              <button
                onClick={() => { onRuns(); setShowActivity(false) }}
                style={{ display: 'block', width: '100%', padding: '6px', background: 'none', border: '1px solid var(--border)', borderRadius: 4, cursor: 'pointer', fontSize: 12, color: 'var(--link)' }}
              >
                {locale === 'zh' ? '查看所有运行记录 →' : 'View all runs →'}
              </button>
            </div>
          )}
        </div>
      )}

      {showTemplates && (
        <TemplatesModal
          onImport={handleImportTemplate}
          onClose={() => setShowTemplates(false)}
        />
      )}

      {showGenerate && (
        <GenerateWorkflowModal
          onClose={() => setShowGenerate(false)}
          onImport={handleGenerateImport}
          onCreated={(id) => {
            setShowGenerate(false)
            load().then(() => openWorkflow(id))
          }}
        />
      )}

      {showSystemInfo && (
        <SystemInfoModal info={systemInfo} onClose={() => setShowSystemInfo(false)} zh={zh} />
      )}

      {/* ── Global search modal ── */}
      {showGlobalSearch && (
        <div className="modal-backdrop" onClick={() => setShowGlobalSearch(false)}>
          <div className="modal" style={{ width: 520, maxHeight: '80vh', overflow: 'auto' }} onClick={(e) => e.stopPropagation()}>
            <div style={{ display: 'flex', gap: 8, alignItems: 'center', marginBottom: 12 }}>
              <input
                autoFocus
                placeholder={zh ? '搜索工作流、执行记录… (Ctrl+Shift+F)' : 'Search workflows, executions… (Ctrl+Shift+F)'}
                value={globalQuery}
                onChange={(e) => setGlobalQuery(e.target.value)}
                onKeyDown={(e) => e.key === 'Escape' && setShowGlobalSearch(false)}
                style={{ flex: 1 }}
              />
              <button className="btn btn-sm btn-icon" onClick={() => setShowGlobalSearch(false)}>✕</button>
            </div>
            {globalSearching && <p style={{ color: 'var(--muted)', fontSize: 12 }}>{zh ? '搜索中…' : 'Searching…'}</p>}
            {globalResults && (
              <>
                {globalResults.workflows.length > 0 && (
                  <div style={{ marginBottom: 12 }}>
                    <div style={{ fontSize: 10, fontWeight: 700, color: 'var(--muted)', textTransform: 'uppercase', letterSpacing: '0.05em', marginBottom: 6 }}>{zh ? '工作流' : 'Workflows'}</div>
                    {globalResults.workflows.map((w) => (
                      <div
                        key={w.id}
                        style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '6px 8px', borderRadius: 'var(--radius)', cursor: 'pointer', background: 'var(--panel)' }}
                        onClick={() => { setShowGlobalSearch(false); openWorkflow(w.id) }}
                      >
                        <span className={`badge badge-${w.status}`}>{w.status}</span>
                        <span style={{ fontWeight: 600 }}>{w.name}</span>
                        {w.description && <span style={{ fontSize: 11, color: 'var(--muted)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{w.description}</span>}
                      </div>
                    ))}
                  </div>
                )}
                {globalResults.executions.length > 0 && (
                  <div>
                    <div style={{ fontSize: 10, fontWeight: 700, color: 'var(--muted)', textTransform: 'uppercase', letterSpacing: '0.05em', marginBottom: 6 }}>{zh ? '执行记录' : 'Executions'}</div>
                    {globalResults.executions.map((e) => (
                      <div
                        key={e.id}
                        style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '6px 8px', borderRadius: 'var(--radius)', cursor: 'pointer', background: 'var(--panel)' }}
                        onClick={() => { setShowGlobalSearch(false); onRuns(e.label ?? e.id.slice(-12)) }}
                      >
                        <span className={`badge badge-${e.status}`}>{e.status}</span>
                        <span style={{ fontFamily: 'monospace', fontSize: 11 }}>{e.id.slice(-16)}</span>
                        {e.label && <span style={{ fontSize: 11, color: 'var(--muted)' }}>{e.label}</span>}
                      </div>
                    ))}
                  </div>
                )}
                {globalResults.workflows.length === 0 && globalResults.executions.length === 0 && (
                  <p style={{ color: 'var(--muted)', fontSize: 12, textAlign: 'center', padding: '20px 0' }}>{zh ? `未找到"${globalQuery}"的结果` : `No results for "${globalQuery}"`}</p>
                )}
              </>
            )}
            {!globalSearching && !globalResults && globalQuery.trim().length >= 2 && (
              <p style={{ color: 'var(--muted)', fontSize: 12 }}>{zh ? '输入以搜索…' : 'Type to search…'}</p>
            )}
            {globalQuery.trim().length < 2 && (
              <p style={{ color: 'var(--muted)', fontSize: 12 }}>{zh ? '请至少输入 2 个字符以搜索工作流和执行记录。' : 'Type at least 2 characters to search across workflows and executions.'}</p>
            )}
          </div>
        </div>
      )}

      {creating && (
        <CreateWorkflowModal onCreate={handleCreate} onClose={() => setCreating(false)} zh={zh} />
      )}

      {editingTags && (
        <div className="modal-backdrop" onClick={() => setEditingTags(null)}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            <h2>{zh ? `编辑标签 — ${editingTags.name}` : `Edit Tags — ${editingTags.name}`}</h2>
            <div className="field">
              <label>{zh ? '标签' : 'Tags'} <span style={{ color: 'var(--muted)', fontWeight: 400 }}>{zh ? '（逗号分隔）' : '(comma-separated)'}</span></label>
              <input
                autoFocus
                placeholder="production, team-a, ml"
                value={tagInput}
                onChange={(e) => setTagInput(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && handleSaveTags()}
              />
              <span style={{ fontSize: 11, color: 'var(--muted)' }}>
                {zh ? '小写字母、数字、连字符和下划线。标签显示为筛选标签。' : 'Lowercase letters, numbers, hyphens and underscores. Tags appear as filter chips.'}
              </span>
            </div>
            <div className="modal-actions">
              <button className="btn" onClick={() => setEditingTags(null)}>{zh ? '取消' : 'Cancel'}</button>
              <button className="btn btn-primary" onClick={handleSaveTags}>{zh ? '保存标签' : 'Save Tags'}</button>
            </div>
          </div>
        </div>
      )}

      {movingFolder && (
        <div className="modal-backdrop" onClick={() => setMovingFolder(null)}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            <h2>{zh ? `移至文件夹 — ${movingFolder.name}` : `Move to Folder — ${movingFolder.name}`}</h2>
            <div className="field">
              <label>{zh ? '文件夹' : 'Folder'} <span style={{ color: 'var(--muted)', fontWeight: 400 }}>{zh ? '（留空以移出文件夹）' : '(leave blank to remove from folder)'}</span></label>
              <input
                autoFocus
                placeholder={zh ? '如 销售、集成、监控' : 'e.g. Sales, Integrations, Monitoring'}
                value={folderInput}
                onChange={(e) => setFolderInput(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && handleMoveFolder()}
                list="af-folder-suggestions"
              />
              <datalist id="af-folder-suggestions">
                {allFolders.map((f) => <option key={f} value={f} />)}
              </datalist>
              <span style={{ fontSize: 11, color: 'var(--muted)' }}>
                {zh ? '文件夹在列表中将工作流分组。已有文件夹会作为自动补全建议显示。' : 'Folders group workflows in the list. Existing folders appear as autocomplete suggestions.'}
              </span>
            </div>
            <div className="modal-actions">
              <button className="btn" onClick={() => setMovingFolder(null)}>{zh ? '取消' : 'Cancel'}</button>
              <button className="btn btn-primary" onClick={handleMoveFolder}>{zh ? '保存' : 'Save'}</button>
            </div>
          </div>
        </div>
      )}

      {quickRunResult && (
        <div
          className={`toast toast-${quickRunResult.status === 'failed' ? 'error' : 'success'}`}
          style={{ position: 'fixed', bottom: 24, right: 24, zIndex: 9999, display: 'flex', alignItems: 'center', gap: 10, cursor: 'pointer' }}
          onClick={() => setQuickRunResult(null)}
        >
          <span>{zh ? '运行已启动：' : 'Run started: '}<code style={{ fontFamily: 'monospace' }}>{quickRunResult.id.slice(0, 8)}</code></span>
          <span style={{ opacity: 0.6, fontSize: 11 }}>{zh ? '点击关闭' : 'click to dismiss'}</span>
        </div>
      )}

      {/* ── Quick Run modal ── */}
      {showQuickRun && (() => {
        const published = workflows.filter((w) => w.status === 'published')
        const qFiltered = qrSearch.trim()
          ? published.filter((w) => w.name.toLowerCase().includes(qrSearch.toLowerCase()))
          : published
        const selectedWf = qFiltered.length === 1 ? qFiltered[0] : published.find((w) => w.name.toLowerCase() === qrSearch.toLowerCase())

        const handleRun = async () => {
          if (!selectedWf) return
          let parsedOk = true
          try { JSON.parse(qrInput) } catch { parsedOk = false }
          if (!parsedOk) { alert(zh ? '输入 JSON 格式无效' : 'Invalid input JSON'); return }
          setQrStarting(true)
          setQrResult(null)
          try {
            const rec = await api.startExecutionFromWorkflow(
              auth!.tenantId, selectedWf.id, qrInput,
              undefined, qrLabel || undefined, undefined, qrDry
            )
            setQrResult({ id: rec.id, status: rec.status })
            if (onOpenExecution) {
              setShowQuickRun(false)
              onOpenExecution(rec.id)
            }
          } catch (e) {
            alert(String(e))
          } finally {
            setQrStarting(false)
          }
        }

        return (
          <div className="modal-backdrop" onClick={() => setShowQuickRun(false)}>
            <div className="modal" style={{ width: 480 }} onClick={(e) => e.stopPropagation()}>
              <div className="modal-header">
                <h3>▶ {zh ? '快速运行工作流' : 'Quick Run Workflow'}</h3>
                <button className="btn btn-sm btn-icon" onClick={() => setShowQuickRun(false)}>✕</button>
              </div>

              <div style={{ display: 'flex', flexDirection: 'column', gap: 12, padding: '0 0 4px' }}>
                {/* Workflow selector */}
                <div>
                  <label style={{ fontSize: 12, color: 'var(--muted)', display: 'block', marginBottom: 4 }}>
                    {zh ? '选择工作流（已发布）' : 'Select workflow (published)'}
                  </label>
                  <input
                    ref={qrSearchRef}
                    value={qrSearch}
                    onChange={(e) => setQrSearch(e.target.value)}
                    onKeyDown={(e) => { if (e.key === 'Enter' && selectedWf) { e.preventDefault(); void handleRun() } if (e.key === 'Escape') setShowQuickRun(false) }}
                    placeholder={zh ? '输入工作流名称…' : 'Type workflow name…'}
                    list="qr-wf-list"
                    style={{ width: '100%', boxSizing: 'border-box' }}
                  />
                  <datalist id="qr-wf-list">
                    {published.map((w) => <option key={w.id} value={w.name} />)}
                  </datalist>
                  {qrSearch && !selectedWf && qFiltered.length > 0 && (
                    <div style={{ marginTop: 4, background: 'var(--surface)', border: '1px solid var(--border)', borderRadius: 6, maxHeight: 140, overflowY: 'auto' }}>
                      {qFiltered.slice(0, 6).map((w) => (
                        <button
                          key={w.id}
                          onClick={() => setQrSearch(w.name)}
                          style={{ display: 'block', width: '100%', textAlign: 'left', padding: '6px 12px', background: 'none', border: 'none', cursor: 'pointer', fontSize: 13, color: 'var(--text)' }}
                          onMouseEnter={(e) => { (e.target as HTMLElement).style.background = 'var(--hover)' }}
                          onMouseLeave={(e) => { (e.target as HTMLElement).style.background = 'none' }}
                        >
                          {w.name}
                        </button>
                      ))}
                    </div>
                  )}
                  {selectedWf && (
                    <div style={{ marginTop: 4, fontSize: 12, color: 'var(--success, #16a34a)' }}>✓ {selectedWf.name}</div>
                  )}
                  {qrSearch && qFiltered.length === 0 && (
                    <div style={{ marginTop: 4, fontSize: 12, color: 'var(--danger-text, #dc2626)' }}>{zh ? '无匹配的已发布工作流' : 'No published workflows match'}</div>
                  )}
                </div>

                {/* Input JSON */}
                <div>
                  <label style={{ fontSize: 12, color: 'var(--muted)', display: 'block', marginBottom: 4 }}>
                    {zh ? '输入 JSON（可选）' : 'Input JSON (optional)'}
                  </label>
                  <textarea
                    value={qrInput}
                    onChange={(e) => setQrInput(e.target.value)}
                    rows={4}
                    style={{ width: '100%', boxSizing: 'border-box', fontFamily: 'monospace', fontSize: 12, resize: 'vertical' }}
                    spellCheck={false}
                  />
                  {(() => {
                    try { JSON.parse(qrInput); return null } catch {
                      return <span style={{ fontSize: 11, color: 'var(--danger-text, #dc2626)' }}>{zh ? '无效 JSON' : 'Invalid JSON'}</span>
                    }
                  })()}
                </div>

                {/* Label + Dry run row */}
                <div style={{ display: 'flex', gap: 12, alignItems: 'center' }}>
                  <div style={{ flex: 1 }}>
                    <label style={{ fontSize: 12, color: 'var(--muted)', display: 'block', marginBottom: 4 }}>
                      {zh ? '标签（可选）' : 'Label (optional)'}
                    </label>
                    <input
                      value={qrLabel}
                      onChange={(e) => setQrLabel(e.target.value)}
                      placeholder={zh ? '如：测试、生产…' : 'e.g. test, prod…'}
                      style={{ width: '100%', boxSizing: 'border-box', fontSize: 12 }}
                    />
                  </div>
                  <div style={{ display: 'flex', alignItems: 'center', gap: 6, paddingTop: 18 }}>
                    <input type="checkbox" id="qr-dry" checked={qrDry} onChange={(e) => setQrDry(e.target.checked)} />
                    <label htmlFor="qr-dry" style={{ fontSize: 12, cursor: 'pointer' }}>{zh ? '试运行' : 'Dry run'}</label>
                  </div>
                </div>

                {/* Result */}
                {qrResult && (
                  <div style={{ padding: '8px 12px', background: 'rgba(22,163,74,0.08)', border: '1px solid var(--success, #16a34a)', borderRadius: 6, fontSize: 13 }}>
                    <span style={{ color: 'var(--success, #16a34a)', fontWeight: 600 }}>✓ {zh ? '已启动：' : 'Started: '}</span>
                    <code style={{ fontFamily: 'monospace' }}>{qrResult.id}</code>
                  </div>
                )}
              </div>

              <div className="modal-actions">
                <button className="btn" onClick={() => setShowQuickRun(false)}>{zh ? '取消' : 'Cancel'}</button>
                <button
                  className="btn btn-primary"
                  disabled={!selectedWf || qrStarting}
                  onClick={handleRun}
                >
                  {qrStarting ? (zh ? '启动中…' : 'Starting…') : `▶ ${zh ? '运行' : 'Run'}${qrDry ? ` (${zh ? '试运行' : 'dry'})` : ''}`}
                </button>
              </div>
            </div>
          </div>
        )
      })()}

      {showShortcuts && (
        <ShortcutsModal onClose={() => setShowShortcuts(false)} zh={zh} />
      )}
    </div>
  )
}
