// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { test, expect, type Page } from '@playwright/test'

// Safety net for RunsPage: the run list renders rows fed through applyRunFilters.
// Backend route-mocked.

const AUTH = {
  token: 'h.' + Buffer.from(JSON.stringify({ tenant_id: 't' })).toString('base64') + '.s',
  tenantId: 't', workspaceId: 'w', projectId: 'p', role: 'admin',
  email: 'a@example.com', emailVerified: true,
}

const RUNS = [
  { id: 'ex-ok', tenant_id: 't', workflow_id: 'wf1', workflow_version_id: 'v1', status: 'succeeded', started_at: 1, label: 'nightly-sync', trigger_type: 'schedule' },
  { id: 'ex-bad', tenant_id: 't', workflow_id: 'wf1', workflow_version_id: 'v1', status: 'failed', started_at: 2, label: 'adhoc-run', trigger_type: 'manual' },
]

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
  await page.route(/\/v1\/workflows(\?|$)/, (r) => r.fulfill({ json: [{ id: 'wf1', tenant_id: 't', workspace_id: 'w', project_id: 'p', name: 'My WF', status: 'published', updated_at: 1, created_at: 1 }] }))
  // Both the paged list and the polling list hit /v1/executions.
  await page.route(/\/v1\/executions(\?|$)/, (r) => r.fulfill({ headers: { 'X-Total-Count': String(RUNS.length) }, json: RUNS }))
}

test('runs page renders run rows', async ({ page }) => {
  const errors = trackErrors(page)
  await mockBackend(page)
  await page.goto('/')
  await page.locator('button[title="Navigation"]').click()
  await page.getByRole('button', { name: '运行记录', exact: true }).click()

  // Rows produced by applyRunFilters → filtered render (assert the table cells,
  // not the label-filter options which share the same text).
  await expect(page.getByRole('cell', { name: 'ex-ok' })).toBeVisible()
  await expect(page.getByRole('cell', { name: 'ex-bad' })).toBeVisible()

  expect(errors, `pageerror:\n${errors.join('\n')}`).toHaveLength(0)
})
