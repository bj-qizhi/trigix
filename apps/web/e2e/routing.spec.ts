// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { test, expect, type Page } from '@playwright/test'

// URL routing: pages are deep-linkable, in-app navigation pushes history, and
// the browser back/forward buttons work. Backend is route-mocked so '/' lands
// on the workflow list and each page mounts on the no-data path.

const AUTH = {
  token: 'h.' + Buffer.from(JSON.stringify({ tenant_id: 't' })).toString('base64') + '.s',
  tenantId: 't', workspaceId: 'w', projectId: 'p', role: 'admin',
  email: 'a@example.com', emailVerified: true,
}

async function mockBackend(page: Page) {
  await page.addInitScript((auth) => localStorage.setItem('af_auth', JSON.stringify(auth)), AUTH)
  await page.route('**/v1/**', (r) => r.fulfill({ status: 403, json: {} }))
  await page.route(/\/v1\/schedules/, (r) => r.fulfill({ json: [] }))
  await page.route(/\/v1\/executions\/stats/, (r) => r.fulfill({ json: {} }))
  await page.route(/\/v1\/executions(\?|$)/, (r) => r.fulfill({ json: [] }))
  await page.route(/\/v1\/workflows(\?|$)/, (r) => r.fulfill({ json: [] }))
}

const nav = (page: Page) => page.locator('button[title="Navigation"]')

test('deep-links straight to a page', async ({ page }) => {
  await mockBackend(page)

  // A shared/bookmarked URL opens that page directly, not the list.
  await page.goto('/runs')
  await expect(nav(page)).toHaveCount(0, { timeout: 10_000 })
  await expect(page).toHaveURL(/\/runs$/)
})

test('unknown paths fall back to the list', async ({ page }) => {
  await mockBackend(page)
  await page.goto('/this/does/not/exist')
  // Falls back to the list, whose Navigation button is present.
  await expect(nav(page)).toBeVisible({ timeout: 10_000 })
})

test('in-app navigation updates the URL and back returns to the list', async ({ page }) => {
  await mockBackend(page)

  await page.goto('/')
  await expect(page).toHaveURL(/\/$/)
  await expect(nav(page)).toBeVisible()

  await nav(page).click()
  await page.getByRole('button', { name: '运行记录', exact: true }).click()

  // The URL reflects the new page, and the list's nav button is gone.
  await expect(page).toHaveURL(/\/runs$/)
  await expect(nav(page)).toHaveCount(0, { timeout: 10_000 })

  // Browser back returns to the list.
  await page.goBack()
  await expect(page).toHaveURL(/\/$/)
  await expect(nav(page)).toBeVisible({ timeout: 10_000 })

  // Forward re-enters the runs page.
  await page.goForward()
  await expect(page).toHaveURL(/\/runs$/)
  await expect(nav(page)).toHaveCount(0, { timeout: 10_000 })
})
