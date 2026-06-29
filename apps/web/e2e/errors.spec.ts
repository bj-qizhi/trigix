// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { test, expect, type Page } from '@playwright/test'

// Friendly error messages + skeleton loaders on data pages. Default locale is
// zh. Pages are deep-linked via the URL router added earlier.

const AUTH = {
  token: 'h.' + Buffer.from(JSON.stringify({ tenant_id: 't' })).toString('base64') + '.s',
  tenantId: 't', workspaceId: 'w', projectId: 'p', role: 'admin',
  email: 'a@example.com', emailVerified: true,
}

async function auth(page: Page) {
  await page.addInitScript((a) => localStorage.setItem('af_auth', JSON.stringify(a)), AUTH)
  await page.route('**/v1/**', (r) => r.fulfill({ status: 403, json: {} }))
}

test('a forbidden load shows a friendly message, not a raw status code', async ({ page }) => {
  await auth(page)
  // Credentials list returns 403 → the page should render plain language.
  await page.goto('/credentials')
  await expect(page.getByText('您没有权限执行此操作。')).toBeVisible({ timeout: 10_000 })
  // And not the raw "403 Forbidden: ..." string.
  await expect(page.getByText(/403 Forbidden/)).toHaveCount(0)
})

test('a slow load shows a skeleton placeholder', async ({ page }) => {
  await auth(page)
  // Delay the credentials read so the skeleton is on screen before data lands.
  await page.route(/\/v1\/credentials/, async (r) => {
    await new Promise((res) => setTimeout(res, 1200))
    await r.fulfill({ json: [] })
  })
  await page.goto('/credentials')
  await expect(page.getByTestId('skeleton')).toBeVisible({ timeout: 5_000 })
})
