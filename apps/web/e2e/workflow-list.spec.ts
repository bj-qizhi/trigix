// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { test, expect, type Page } from '@playwright/test'

// Behavioural safety net for the WorkflowList page before it is refactored:
// the list renders, the create-workflow flow posts a new workflow, and global
// search opens. Backend is route-mocked.

const AUTH = {
  token: 'h.' + Buffer.from(JSON.stringify({ tenant_id: 't' })).toString('base64') + '.s',
  tenantId: 't', workspaceId: 'w', projectId: 'p', role: 'admin',
  email: 'a@example.com', emailVerified: true,
}

const WF = {
  id: 'wf1', tenant_id: 't', workspace_id: 'w', project_id: 'p',
  name: 'Smoke WF', status: 'published', latest_version_id: 'v1',
  updated_at: 1, created_at: 1,
}

function trackErrors(page: Page): string[] {
  const errors: string[] = []
  page.on('pageerror', (e) => errors.push(String(e)))
  return errors
}

async function mockBackend(page: Page) {
  await page.addInitScript((auth) => localStorage.setItem('af_auth', JSON.stringify(auth)), AUTH)
  await page.route('**/v1/**', (r) => r.fulfill({ status: 403, json: {} }))
  await page.route(/\/v1\/schedules/, (r) => r.fulfill({ json: [] }))
  await page.route(/\/v1\/executions\/stats/, (r) => r.fulfill({ json: {} }))
  await page.route(/\/v1\/executions(\?|$)/, (r) => r.fulfill({ json: [] }))
  await page.route(/\/v1\/workflows(\?|$)/, (r) => r.fulfill({ json: [WF] }))
}

test('workflow list renders the workflow rows', async ({ page }) => {
  const errors = trackErrors(page)
  await mockBackend(page)
  await page.goto('/')
  await expect(page.getByText('Smoke WF')).toBeVisible()
  expect(errors, errors.join('\n')).toHaveLength(0)
})

test('create-workflow modal posts a new workflow', async ({ page }) => {
  const errors = trackErrors(page)
  await mockBackend(page)
  const posts: { name: string }[] = []
  // POST /v1/workflows creates; route after the catch-all/list so it wins.
  await page.route(/\/v1\/workflows(\?|$)/, async (r) => {
    if (r.request().method() === 'POST') {
      posts.push(r.request().postDataJSON() as { name: string })
      return r.fulfill({ json: { ...WF, id: 'wf-new', name: 'My New Flow', latest_version_id: null } })
    }
    return r.fulfill({ json: [WF] })
  })
  // The post-create navigation opens the editor for the new workflow.
  await page.route(/\/v1\/workflows\/wf-new(\?|$)/, (r) => r.fulfill({ json: { ...WF, id: 'wf-new', latest_version_id: null } }))
  await page.route(/\/v1\/workflows\/wf-new\/versions/, (r) => r.fulfill({ json: [] }))

  await page.goto('/')
  await page.getByRole('button', { name: /创建工作流|Create Workflow/ }).first().click()
  await expect(page.getByRole('heading', { name: /新建工作流|New Workflow/ })).toBeVisible()
  await page.locator('.modal input').first().fill('My New Flow')
  await page.getByRole('button', { name: /^创建$|^Create$/ }).click()

  await expect.poll(() => posts.length).toBeGreaterThan(0)
  expect(posts.at(-1)!.name).toBe('My New Flow')
  expect(errors, errors.join('\n')).toHaveLength(0)
})

test('a failed action surfaces a global error toast (not a native alert)', async ({ page }) => {
  const errors = trackErrors(page)
  await mockBackend(page)
  // Make the create POST fail so the submit handler hits its catch → toast.
  await page.route(/\/v1\/workflows(\?|$)/, async (r) => {
    if (r.request().method() === 'POST') return r.fulfill({ status: 500, json: { message: 'boom' } })
    return r.fulfill({ json: [WF] })
  })

  await page.goto('/')
  await page.getByRole('button', { name: /创建工作流|Create Workflow/ }).first().click()
  await page.locator('.modal input').first().fill('Doomed Flow')
  await page.getByRole('button', { name: /^创建$|^Create$/ }).click()

  // The unified toast surface shows the error (the old code used window.alert).
  await expect(page.locator('.toast.toast-error')).toBeVisible({ timeout: 10_000 })
  expect(errors, errors.join('\n')).toHaveLength(0)
})

test('global search modal opens', async ({ page }) => {
  const errors = trackErrors(page)
  await mockBackend(page)
  await page.goto('/')
  await expect(page.getByText('Smoke WF')).toBeVisible()
  await page.keyboard.press('Control+Shift+F')
  // The global-search modal input is the one mentioning executions (the inline
  // list filter only mentions workflows).
  await expect(page.getByPlaceholder(/执行记录|executions/)).toBeVisible()
  expect(errors, errors.join('\n')).toHaveLength(0)
})
