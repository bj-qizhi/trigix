// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { lazy, Suspense, useState, useCallback, useEffect } from 'react'
import { AuthProvider, useAuth } from './AuthContext'
import * as api from './api/client'
import { useLocale } from './useLocale'
import { LoginPage } from './components/LoginPage'
import { WorkflowList } from './components/WorkflowList'
import { usePageRouter } from './routing'
import { ToastProvider } from './toast'
import { IconX } from './components/uiIcons'
import logoIcon from './assets/logo.svg'

const WorkflowEditor = lazy(() => import('./components/WorkflowEditor').then((m) => ({ default: m.WorkflowEditor })))
const CredentialsPage = lazy(() => import('./components/CredentialsPage').then((m) => ({ default: m.CredentialsPage })))
const AuditLogPage = lazy(() => import('./components/AuditLogPage').then((m) => ({ default: m.AuditLogPage })))
const RunsPage = lazy(() => import('./components/RunsPage').then((m) => ({ default: m.RunsPage })))
const ExecutionDetailPage = lazy(() => import('./components/ExecutionDetailPage').then((m) => ({ default: m.ExecutionDetailPage })))
const AnalyticsPage = lazy(() => import('./components/AnalyticsPage').then((m) => ({ default: m.AnalyticsPage })))
const EnvironmentPage = lazy(() => import('./components/EnvironmentPage').then((m) => ({ default: m.EnvironmentPage })))
const WorkspacePage = lazy(() => import('./components/WorkspacePage').then((m) => ({ default: m.WorkspacePage })))
const WebhookPage = lazy(() => import('./components/WebhookPage').then((m) => ({ default: m.WebhookPage })))
const ApiKeysPage = lazy(() => import('./components/ApiKeysPage').then((m) => ({ default: m.ApiKeysPage })))
const SsoSettingsPage = lazy(() => import('./components/SsoSettingsPage').then((m) => ({ default: m.SsoSettingsPage })))
const KnowledgeBasePage = lazy(() => import('./components/KnowledgeBasePage').then((m) => ({ default: m.KnowledgeBasePage })))
const CustomNodesPage = lazy(() => import('./components/CustomNodesPage').then((m) => ({ default: m.CustomNodesPage })))
const EventSubscriptionsPage = lazy(() => import('./components/EventSubscriptionsPage').then((m) => ({ default: m.EventSubscriptionsPage })))
const FormPage = lazy(() => import('./components/FormPage').then((m) => ({ default: m.FormPage })))
const OrgPage = lazy(() => import('./components/OrgPage').then((m) => ({ default: m.OrgPage })))
const AccountPage = lazy(() => import('./components/AccountPage').then((m) => ({ default: m.AccountPage })))
const AffiliatePage = lazy(() => import('./components/AffiliatePage').then((m) => ({ default: m.AffiliatePage })))
const AdminPayoutsPage = lazy(() => import('./components/AdminPayoutsPage').then((m) => ({ default: m.AdminPayoutsPage })))
const UsersPage = lazy(() => import('./components/UsersPage').then((m) => ({ default: m.UsersPage })))
const SchedulesPage = lazy(() => import('./components/SchedulesPage').then((m) => ({ default: m.SchedulesPage })))
const MonitoringPage = lazy(() => import('./components/MonitoringPage').then((m) => ({ default: m.MonitoringPage })))
const ApprovalsPage = lazy(() => import('./components/ApprovalsPage').then((m) => ({ default: m.ApprovalsPage })))
const WorkflowDepsPage = lazy(() => import('./components/WorkflowDepsPage').then((m) => ({ default: m.WorkflowDepsPage })))

function PageFallback() {
  return <div role="status" aria-live="polite" style={{ padding: '2rem', color: 'var(--muted)' }}>Loading…</div>
}

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
        <IconX aria-hidden />
      </button>
    </div>
  )
}

function AppInner() {
  const { auth } = useAuth()
  const [page, setPage] = usePageRouter()

  useEffect(() => {
    const params = new URLSearchParams(window.location.search)
    if (params.get('billing') === 'success') {
      setPage({ name: 'account' })
    }
  }, [setPage])

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
  return (
    <ToastProvider>
      <Suspense fallback={<PageFallback />}>
        {window.location.pathname.startsWith('/forms/') ? <PublicFormRoute /> : <AuthedApp />}
      </Suspense>
    </ToastProvider>
  )
}

function AuthedApp() {
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
