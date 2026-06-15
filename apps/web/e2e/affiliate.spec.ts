// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { test, expect, type Page } from '@playwright/test'

const AUTH = {
  token: 'h.' + Buffer.from(JSON.stringify({ tenant_id: 't' })).toString('base64') + '.s',
  tenantId: 't',
  workspaceId: 'w',
  projectId: 'p',
  role: 'admin',
  email: 'a@example.com',
  emailVerified: true,
}

interface RegisterBody {
  referral_code?: string
}

test('affiliate dashboard shows code, balance and referral link', async ({ page }) => {
  await page.addInitScript((auth) => localStorage.setItem('af_auth', JSON.stringify(auth)), AUTH)
  await page.route('**/v1/**', (r) => r.fulfill({ status: 403, json: {} }))
  await page.route('**/v1/affiliate/me', (r) =>
    r.fulfill({
      json: {
        code: 'ABCD1234',
        referral_count: 3,
        balance_cents: 12345,
        commission_pct: 20,
        entries: [
          { id: 'e1', referee_tenant: 't2', amount_cents: 12345, kind: 'commission', source_ref: null, created_at: 1 },
        ],
        payout_requests: [],
      },
    }),
  )
  await page.goto('/')
  await page.locator('button[title="Navigation"]').click()
  await page.getByText('推荐返佣', { exact: false }).first().click()

  await expect(page.getByText('ABCD1234').first()).toBeVisible()
  await expect(page.getByText('$123.45').first()).toBeVisible()
  await expect(page.locator('input[readonly]')).toHaveValue(/ref=ABCD1234/)
})

test('affiliate can request a USDT payout', async ({ page }) => {
  await page.addInitScript((auth) => localStorage.setItem('af_auth', JSON.stringify(auth)), AUTH)
  await page.route('**/v1/**', (r) => r.fulfill({ status: 403, json: {} }))
  await page.route('**/v1/affiliate/me', (r) =>
    r.fulfill({
      json: {
        code: 'ABCD1234', referral_count: 1, balance_cents: 50000, commission_pct: 20,
        entries: [], payout_requests: [],
      },
    }),
  )
  let body: { address?: string; amount_cents?: number } | null = null
  await page.route('**/v1/affiliate/payout-request', async (r) => {
    body = JSON.parse(r.request().postData() || '{}')
    await r.fulfill({
      json: { id: 'p1', tenant_id: 't', method: 'usdt', address: 'TWallet', amount_cents: 10000, status: 'requested', note: null, created_at: 1, processed_at: null },
    })
  })

  await page.goto('/')
  await page.locator('button[title="Navigation"]').click()
  await page.getByText('推荐返佣', { exact: false }).first().click()
  await page.getByPlaceholder('USDT 地址').fill('TWallet')
  await page.getByPlaceholder('金额($)').fill('100')
  await page.getByRole('button', { name: '申请' }).click()

  await expect.poll(() => (body as { amount_cents?: number } | null)?.amount_cents).toBe(10000)
  expect((body as { address?: string } | null)?.address).toBe('TWallet')
})

test('referral code from ?ref is forwarded on register', async ({ page }) => {
  const sysinfo = {
    version: 'test', node_types: 1, auth_required: false, rust_edition: '2021', features: [],
    max_concurrent_executions: 1, max_executions_per_tenant: 1, running_executions: 0,
    captcha_provider: null, captcha_site_key: null,
  }
  await page.route('**/v1/system/info', (r) => r.fulfill({ json: sysinfo }))
  await page.route('**/v1/sso/public', (r) => r.fulfill({ json: [] }))
  let body: RegisterBody | null = null
  await page.route('**/v1/auth/register', async (r) => {
    body = JSON.parse(r.request().postData() || '{}') as RegisterBody
    await r.fulfill({
      json: {
        token: 'h.' + Buffer.from(JSON.stringify({ tenant_id: 't' })).toString('base64') + '.s',
        user: { email: 'a@example.com', tenant_id: 't', email_verified: false },
      },
    })
  })

  await page.goto('/?ref=REF99999')
  await page.getByRole('button', { name: '邮箱' }).click()
  await page.getByRole('button', { name: '注册' }).click()
  await page.locator('input[type=email]').fill('a@example.com')
  await page.locator('input[type=password]').fill('secret123')
  await page.locator('button[type=submit]').click()

  await expect.poll(() => (body as RegisterBody | null)?.referral_code).toBe('REF99999')
})
