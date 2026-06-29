// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useState, useEffect, useCallback } from 'react'

// The single source of truth for which screen is shown. Most variants are
// payload-free; three carry an id (editor/execution) or an optional filter
// (runs). `initialInput` on the editor is a transient prefill and is
// deliberately NOT encoded in the URL.
export type Page =
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

// Payload-free pages: a 1:1 mapping between page name and URL path.
const STATIC_PATHS: Record<string, string> = {
  list: '/',
  credentials: '/credentials',
  audit: '/audit',
  analytics: '/analytics',
  environment: '/environment',
  workspaces: '/workspaces',
  webhooks: '/webhooks',
  apikeys: '/api-keys',
  sso: '/sso',
  knowledge: '/knowledge',
  'custom-nodes': '/custom-nodes',
  'event-subscriptions': '/event-subscriptions',
  orgs: '/orgs',
  account: '/account',
  affiliate: '/affiliate',
  payouts: '/payouts',
  users: '/users',
  schedules: '/schedules',
  monitoring: '/monitoring',
  approvals: '/approvals',
  'workflow-deps': '/workflow-deps',
}

// Serialize a page to a path (+ query string). Deep-linkable: anything a user
// might want to share — a specific workflow, run, or filtered runs view — round
// trips through the URL.
export function pageToPath(page: Page): string {
  switch (page.name) {
    case 'editor':
      return `/workflows/${encodeURIComponent(page.workflowId)}`
    case 'execution':
      return `/executions/${encodeURIComponent(page.executionId)}${page.fromRuns ? '?from=runs' : ''}`
    case 'runs':
      return page.workflowFilter ? `/runs?workflow=${encodeURIComponent(page.workflowFilter)}` : '/runs'
    default:
      return STATIC_PATHS[page.name] ?? '/'
  }
}

// Parse a path (+ query string) back into a page. Unknown paths fall back to
// the workflow list so a stale/garbled link never dead-ends.
export function pathToPage(pathname: string, search = ''): Page {
  const params = new URLSearchParams(search)

  const editor = pathname.match(/^\/workflows\/([^/]+)\/?$/)
  if (editor) return { name: 'editor', workflowId: decodeURIComponent(editor[1]) }

  const exec = pathname.match(/^\/executions\/([^/]+)\/?$/)
  if (exec) return { name: 'execution', executionId: decodeURIComponent(exec[1]), fromRuns: params.get('from') === 'runs' }

  const normalized = pathname.length > 1 ? pathname.replace(/\/$/, '') : pathname

  if (normalized === '/runs') {
    const wf = params.get('workflow')
    return wf ? { name: 'runs', workflowFilter: wf } : { name: 'runs' }
  }

  for (const [name, path] of Object.entries(STATIC_PATHS)) {
    if (path === normalized) return { name } as Page
  }
  return { name: 'list' }
}

// Page state backed by the browser history. Initializes from the current URL
// (deep-linking), pushes a new entry on every in-app navigation, and follows
// the back/forward buttons via popstate.
export function usePageRouter(): [Page, (page: Page) => void] {
  const [page, setPageState] = useState<Page>(() =>
    pathToPage(window.location.pathname, window.location.search),
  )

  useEffect(() => {
    const onPop = () => setPageState(pathToPage(window.location.pathname, window.location.search))
    window.addEventListener('popstate', onPop)
    return () => window.removeEventListener('popstate', onPop)
  }, [])

  const setPage = useCallback((next: Page) => {
    const path = pageToPath(next)
    if (path !== window.location.pathname + window.location.search) {
      window.history.pushState(null, '', path)
    }
    setPageState(next)
  }, [])

  return [page, setPage]
}
