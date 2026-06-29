// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useEffect, useState } from 'react'
import type { Dispatch, SetStateAction } from 'react'
import { useAuth } from '../../AuthContext'
import * as api from '../../api/client'
import { useLocale } from '../../useLocale'
import { friendlyError } from '../../errorMessage'
import type { WorkflowRecord, ScheduleSummary, ExecutionSummary } from '../../types'

// Bootstrap data layer for the workflow list: loads the workflows, schedules,
// execution summaries/stats, billing, notifications and expiring credentials,
// and exposes the slices (plus the setters the page mutates) so WorkflowList
// itself stays free of the loading machinery.
export interface WorkflowListData {
  workflows: WorkflowRecord[]
  setWorkflows: Dispatch<SetStateAction<WorkflowRecord[]>>
  schedules: ScheduleSummary[]
  setSchedules: Dispatch<SetStateAction<ScheduleSummary[]>>
  execSummaries: ExecutionSummary[]
  setExecSummaries: Dispatch<SetStateAction<ExecutionSummary[]>>
  execStats: api.ExecutionStats | null
  billingStatus: api.BillingStatus | null
  serverNotifs: api.AppNotification[]
  setServerNotifs: Dispatch<SetStateAction<api.AppNotification[]>>
  serverUnread: number
  setServerUnread: Dispatch<SetStateAction<number>>
  expiringCreds: api.CredentialSummary[]
  loading: boolean
  error: string | null
  reload: () => Promise<unknown>
}

export function useWorkflowListData(): WorkflowListData {
  const { auth } = useAuth()
  const { locale } = useLocale()
  const zh = locale === 'zh'
  const [workflows, setWorkflows]       = useState<WorkflowRecord[]>([])
  const [schedules, setSchedules]       = useState<ScheduleSummary[]>([])
  const [execSummaries, setExecSummaries] = useState<ExecutionSummary[]>([])
  const [loading, setLoading]           = useState(true)
  const [error, setError]               = useState<string | null>(null)
  const [execStats, setExecStats]       = useState<api.ExecutionStats | null>(null)
  const [billingStatus, setBillingStatus] = useState<api.BillingStatus | null>(null)
  const [serverNotifs, setServerNotifs] = useState<api.AppNotification[]>([])
  const [serverUnread, setServerUnread] = useState(0)
  const [expiringCreds, setExpiringCreds] = useState<api.CredentialSummary[]>([])

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
      .catch((e: unknown) => setError(friendlyError(e, zh)))
      .finally(() => setLoading(false))
    api.getBillingStatus().then(setBillingStatus).catch(() => {})
    api.listNotifications(auth!.tenantId, 20).then((r) => { setServerNotifs(r.notifications); setServerUnread(r.unread_count) }).catch(() => {})
    api.listExpiringCredentials(auth!.tenantId, 7).then(setExpiringCreds).catch(() => {})
    return loaded
  }

  useEffect(() => { void load() }, [])

  return {
    workflows, setWorkflows,
    schedules, setSchedules,
    execSummaries, setExecSummaries,
    execStats, billingStatus,
    serverNotifs, setServerNotifs,
    serverUnread, setServerUnread,
    expiringCreds,
    loading, error,
    reload: load,
  }
}
