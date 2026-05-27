import { useState } from 'react'
import { AuthProvider, useAuth } from './AuthContext'
import { LoginPage } from './components/LoginPage'
import { WorkflowList } from './components/WorkflowList'
import { WorkflowEditor } from './components/WorkflowEditor'
import { CredentialsPage } from './components/CredentialsPage'
import { AuditLogPage } from './components/AuditLogPage'
import { RunsPage } from './components/RunsPage'
import { ExecutionDetailPage } from './components/ExecutionDetailPage'

type Page =
  | { name: 'list' }
  | { name: 'editor'; workflowId: string }
  | { name: 'credentials' }
  | { name: 'audit' }
  | { name: 'runs' }
  | { name: 'execution'; executionId: string; fromRuns?: boolean }

function AppInner() {
  const { auth } = useAuth()
  const [page, setPage] = useState<Page>({ name: 'list' })

  if (!auth) {
    return <LoginPage />
  }

  if (page.name === 'editor') {
    return (
      <WorkflowEditor
        workflowId={page.workflowId}
        onBack={() => setPage({ name: 'list' })}
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
      />
    )
  }

  if (page.name === 'execution') {
    return (
      <ExecutionDetailPage
        executionId={page.executionId}
        onBack={() => setPage(page.fromRuns ? { name: 'runs' } : { name: 'list' })}
        onOpenWorkflow={(id) => setPage({ name: 'editor', workflowId: id })}
        onRetry={(newId) => setPage({ name: 'execution', executionId: newId, fromRuns: page.fromRuns })}
      />
    )
  }

  return (
    <WorkflowList
      onOpen={(id) => setPage({ name: 'editor', workflowId: id })}
      onCredentials={() => setPage({ name: 'credentials' })}
      onAuditLog={() => setPage({ name: 'audit' })}
      onRuns={() => setPage({ name: 'runs' })}
    />
  )
}

export function App() {
  return (
    <AuthProvider>
      <AppInner />
    </AuthProvider>
  )
}
