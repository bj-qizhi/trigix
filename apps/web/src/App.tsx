// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useState, useCallback, useEffect } from 'react'
import { AuthProvider, useAuth } from './AuthContext'
import * as api from './api/client'
import { useLocale } from './useLocale'
import { LoginPage } from './components/LoginPage'
import { WorkflowList } from './components/WorkflowList'
import { WorkflowEditor } from './components/WorkflowEditor'
import { CredentialsPage } from './components/CredentialsPage'
import { AuditLogPage } from './components/AuditLogPage'
import { RunsPage } from './components/RunsPage'
import { ExecutionDetailPage } from './components/ExecutionDetailPage'
import { AnalyticsPage } from './components/AnalyticsPage'
import { EnvironmentPage } from './components/EnvironmentPage'
import { WorkspacePage } from './components/WorkspacePage'
import { WebhookPage } from './components/WebhookPage'
import { ApiKeysPage } from './components/ApiKeysPage'
import { SsoSettingsPage } from './components/SsoSettingsPage'
import { KnowledgeBasePage } from './components/KnowledgeBasePage'
import { CustomNodesPage } from './components/CustomNodesPage'
import { EventSubscriptionsPage } from './components/EventSubscriptionsPage'
import { FormPage } from './components/FormPage'
import { OrgPage } from './components/OrgPage'
import { AccountPage } from './components/AccountPage'
import { AffiliatePage } from './components/AffiliatePage'
import { AdminPayoutsPage } from './components/AdminPayoutsPage'
import { UsersPage } from './components/UsersPage'
import { SchedulesPage } from './components/SchedulesPage'
import { MonitoringPage } from './components/MonitoringPage'
import { ApprovalsPage } from './components/ApprovalsPage'
import { WorkflowDepsPage } from './components/WorkflowDepsPage'
import logoIcon from './assets/logo.svg'

type Page =
  | { name: 'list' }
  | { name: 'editor'; workflowId: string; initialInput?: string }
  | { name: 'credentials' }
  | { name: 'audit' }
  | { name: 'runs'; workflowFilter?: string }
  | { name: 'analytics' }
  | { name: 'environment' }
  | { name: 'workspaces' }
  | { name: 'webhooks' }
  | { name: 'apikeys' }
  | { name: 'sso' }
  | { name: 'knowledge' }
  | { name: 'custom-nodes' }
  | { name: 'event-subscriptions' }
  | { name: 'orgs' }
  | { name: 'account' }
  | { name: 'affiliate' }
  | { name: 'payouts' }
  | { name: 'users' }
  | { name: 'schedules' }
  | { name: 'monitoring' }
  | { name: 'approvals' }
  | { name: 'workflow-deps' }
  | { name: 'execution'; executionId: string; fromRuns?: boolean }

function EmailVerificationBanner({ email }: { email?: string }) {
  const [dismissed, setDismissed] = useState(false)
  const [sent, setSent] = useState(false)
  const { t } = useLocale()
  const handleResend = useCallback(() => {
    if (!email) return
    api.resendVerification(email).then(() => setSent(true)).catch(() => {})
  }, [email])
  if (dismissed) return null
  return (
    <div style={{ background: '#7c3aed', color: '#fff', padding: '0.5rem 1rem', display: 'flex', alignItems: 'center', gap: '0.75rem', fontSize: '0.85rem' }}>
      <span>{t('verify.banner')}</span>
      {!sent && email && (
        <button onClick={handleResend} style={{ background: 'rgba(255,255,255,0.2)', border: 'none', color: '#fff', padding: '0.2rem 0.6rem', borderRadius: '4px', cursor: 'pointer', fontSize: '0.8rem' }}>
          {t('verify.resend')}
        </button>
      )}
      {sent && <span style={{ opacity: 0.8 }}>{t('verify.sent')}</span>}
      <button onClick={() => setDismissed(true)} style={{ marginLeft: 'auto', background: 'none', border: 'none', color: '#fff', cursor: 'pointer', fontSize: '1rem', lineHeight: 1 }}>
        ×
      </button>
    </div>
  )
}

function AppInner() {
  const { auth } = useAuth()
  const [page, setPage] = useState<Page>({ name: 'list' })

  useEffect(() => {
    const params = new URLSearchParams(window.location.search)
    if (params.get('billing') === 'success') {
      setPage({ name: 'account' })
      window.history.replaceState({}, '', window.location.pathname)
    }
  }, [])

  if (!auth) {
    return <LoginPage />
  }

  const showVerifyBanner = auth.emailVerified === false

  if (page.name === 'editor') {
    return (
      <WorkflowEditor
        workflowId={page.workflowId}
        onBack={() => setPage({ name: 'list' })}
        initialInput={page.initialInput}
      />
    )
  }

  if (page.name === 'credentials') {
    return <CredentialsPage onBack={() => setPage({ name: 'list' })} />
  }

  if (page.name === 'audit') {
    return <AuditLogPage onBack={() => setPage({ name: 'list' })} />
  }

  if (page.name === 'runs') {
    return (
      <RunsPage
        onBack={() => setPage({ name: 'list' })}
        onOpenExecution={(id) => setPage({ name: 'execution', executionId: id, fromRuns: true })}
        onOpenWorkflow={(id) => setPage({ name: 'editor', workflowId: id })}
        initialWorkflowFilter={page.workflowFilter}
      />
    )
  }

  if (page.name === 'analytics') {
    return <AnalyticsPage onBack={() => setPage({ name: 'list' })} />
  }

  if (page.name === 'environment') {
    return <EnvironmentPage onBack={() => setPage({ name: 'list' })} />
  }

  if (page.name === 'workspaces') {
    return <WorkspacePage onBack={() => setPage({ name: 'list' })} />
  }

  if (page.name === 'webhooks') {
    return (
      <WebhookPage
        onBack={() => setPage({ name: 'list' })}
        onOpenWorkflow={(id) => setPage({ name: 'editor', workflowId: id })}
      />
    )
  }

  if (page.name === 'apikeys') {
    return <ApiKeysPage onBack={() => setPage({ name: 'list' })} />
  }

  if (page.name === 'sso') {
    return <SsoSettingsPage onBack={() => setPage({ name: 'list' })} />
  }

  if (page.name === 'knowledge') {
    return <KnowledgeBasePage onBack={() => setPage({ name: 'list' })} />
  }

  if (page.name === 'custom-nodes') {
    return <CustomNodesPage onBack={() => setPage({ name: 'list' })} />
  }

  if (page.name === 'event-subscriptions') {
    return <EventSubscriptionsPage onBack={() => setPage({ name: 'list' })} />
  }

  if (page.name === 'orgs') {
    return <OrgPage onBack={() => setPage({ name: 'list' })} />
  }

  if (page.name === 'account') {
    return <AccountPage onBack={() => setPage({ name: 'list' })} />
  }

  if (page.name === 'affiliate') {
    return <AffiliatePage onBack={() => setPage({ name: 'list' })} />
  }

  if (page.name === 'payouts') {
    return <AdminPayoutsPage onBack={() => setPage({ name: 'list' })} />
  }

  if (page.name === 'users') {
    return <UsersPage onBack={() => setPage({ name: 'list' })} />
  }

  if (page.name === 'schedules') {
    return (
      <SchedulesPage
        onBack={() => setPage({ name: 'list' })}
        onOpenWorkflow={(id) => setPage({ name: 'editor', workflowId: id })}
        onOpenExecution={(id) => setPage({ name: 'execution', executionId: id })}
      />
    )
  }

  if (page.name === 'monitoring') {
    return (
      <MonitoringPage
        onBack={() => setPage({ name: 'list' })}
        onOpenExecution={(id) => setPage({ name: 'execution', executionId: id })}
        onOpenWorkflow={(id) => setPage({ name: 'editor', workflowId: id })}
      />
    )
  }

  if (page.name === 'approvals') {
    return (
      <ApprovalsPage
        onBack={() => setPage({ name: 'list' })}
        onOpenExecution={(id) => setPage({ name: 'execution', executionId: id, fromRuns: false })}
        onOpenWorkflow={(id) => setPage({ name: 'editor', workflowId: id })}
      />
    )
  }

  if (page.name === 'workflow-deps') {
    return (
      <WorkflowDepsPage
        onBack={() => setPage({ name: 'list' })}
        onOpenWorkflow={(id) => setPage({ name: 'editor', workflowId: id })}
      />
    )
  }

  if (page.name === 'execution') {
    return (
      <ExecutionDetailPage
        executionId={page.executionId}
        onBack={() => setPage(page.fromRuns ? { name: 'runs' } : { name: 'list' })}
        onOpenWorkflow={(id, input) => setPage({ name: 'editor', workflowId: id, initialInput: input })}
        onRetry={(newId) => setPage({ name: 'execution', executionId: newId, fromRuns: page.fromRuns })}
        onOpenExecution={(id) => setPage({ name: 'execution', executionId: id, fromRuns: page.fromRuns })}
      />
    )
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
      {showVerifyBanner && <EmailVerificationBanner email={auth.email} />}
      <WorkflowList
        onOpen={(id) => setPage({ name: 'editor', workflowId: id })}
        onOpenExecution={(id) => setPage({ name: 'execution', executionId: id })}
        onCredentials={() => setPage({ name: 'credentials' })}
        onAuditLog={() => setPage({ name: 'audit' })}
        onRuns={(wf) => setPage({ name: 'runs', workflowFilter: wf })}
        onAnalytics={() => setPage({ name: 'analytics' })}
        onEnvironment={() => setPage({ name: 'environment' })}
        onWorkspaces={() => setPage({ name: 'workspaces' })}
        onWebhooks={() => setPage({ name: 'webhooks' })}
        onApiKeys={() => setPage({ name: 'apikeys' })}
        onSso={() => setPage({ name: 'sso' })}
        onKnowledge={() => setPage({ name: 'knowledge' })}
        onCustomNodes={() => setPage({ name: 'custom-nodes' })}
        onEventSubscriptions={() => setPage({ name: 'event-subscriptions' })}
        onOrgs={() => setPage({ name: 'orgs' })}
        onAccount={() => setPage({ name: 'account' })}
        onAffiliate={() => setPage({ name: 'affiliate' })}
        onPayouts={() => setPage({ name: 'payouts' })}
        onUsers={() => setPage({ name: 'users' })}
        onSchedules={() => setPage({ name: 'schedules' })}
        onMonitoring={() => setPage({ name: 'monitoring' })}
        onApprovals={() => setPage({ name: 'approvals' })}
        onWorkflowDeps={() => setPage({ name: 'workflow-deps' })}
      />
    </div>
  )
}

// Public form route: /forms/:token
function PublicFormRoute() {
  const m = window.location.pathname.match(/^\/forms\/([^/]+)/)
  if (m) return <FormPage token={m[1]} />
  return null
}

function Footer() {
  return (
    <footer style={{
      borderTop: '1px solid var(--border)',
      background: 'var(--surface)',
      color: 'var(--muted)',
      fontSize: '12px',
      textAlign: 'center',
      padding: '8px 16px',
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      gap: '10px',
    }}>
      <img src={logoIcon} alt="Trigix" style={{ height: '22px', verticalAlign: 'middle' }} />
      <span>
        © {new Date().getFullYear()} 北京祺智科技有限公司 · All rights reserved ·{' '}
        <a href="https://www.qzso.com/" target="_blank" rel="noopener noreferrer" style={{ color: 'var(--muted)', textDecoration: 'none' }}>
          www.qzso.com
        </a>
        {' · '}
        <a href="mailto:managecode@gmail.com" style={{ color: 'var(--muted)', textDecoration: 'none' }}>
          managecode@gmail.com
        </a>
      </span>
    </footer>
  )
}

export function App() {
  if (window.location.pathname.startsWith('/forms/')) {
    return <PublicFormRoute />
  }
  return (
    <AuthProvider>
      <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
        <div style={{ flex: 1, minHeight: 0, display: 'flex', flexDirection: 'column' }}>
          <AppInner />
        </div>
        <Footer />
      </div>
    </AuthProvider>
  )
}
