// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { test, expect, type Page } from '@playwright/test'

// Drives the authenticated AnalyticsPage. Auth is injected into localStorage and
// every API call is failed (403) except the endpoint under test, so the page
// renders its empty states and we isolate the acquisition-channels card.

const AUTH = {
  token: 'h.' + Buffer.from(JSON.stringify({ tenant_id: 't' })).toString('base64') + '.s',
  tenantId: 't',
  workspaceId: 'w',
  projectId: 'p',
  role: 'admin',
  email: 'a@example.com',
  emailVerified: true,
}

async function authed(page: Page) {
  await page.addInitScript((auth) => {
    localStorage.setItem('af_auth', JSON.stringify(auth))
  }, AUTH)
  // Fail everything gracefully (handlers use .catch); specific routes registered
  // later win, so the endpoint under test still returns data.
  await page.route('**/v1/**', (r) => r.fulfill({ status: 403, json: {} }))
}

async function openAnalytics(page: Page) {
  await page.goto('/')
  await page.locator('button[title="Navigation"]').click()
  await page.getByText('分析', { exact: true }).click()
}

test('analytics: acquisition-channels card renders channel data (admin)', async ({ page }) => {
  await authed(page)
  await page.route('**/v1/analytics/attribution', (r) =>
    r.fulfill({
      json: [
        { channel: 'google', signups: 5, paid: 2, revenue_cents: 9800 },
        { channel: 'direct', signups: 2, paid: 0, revenue_cents: 0 },
      ],
    }),
  )
  await openAnalytics(page)
  await expect(page.getByText('获客渠道 ROI')).toBeVisible()
  await expect(page.getByText('google')).toBeVisible()
  await expect(page.getByText('direct')).toBeVisible()
  // Converted revenue is rendered ($98.00 from 9800 cents) — appears in both the
  // summary line and the channel row.
  await expect(page.getByText('$98.00').first()).toBeVisible()
})

test('analytics: acquisition card hidden when there are no channels', async ({ page }) => {
  // No specific attribution route → catch-all 403 → empty → card hidden.
  await authed(page)
  await openAnalytics(page)
  await expect(page.getByText('获客渠道 ROI')).toHaveCount(0)
})
