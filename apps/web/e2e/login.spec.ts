// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { test, expect, type Page } from '@playwright/test'

// The backend is fully mocked via route interception so these tests are
// deterministic. Default UI locale is Chinese (tabs: 邮箱 / 注册).

interface RegisterBody {
  attribution?: { utm_source?: string; utm_campaign?: string }
  captcha_token?: string
}

function sysinfo(captcha: Record<string, unknown>) {
  return {
    version: 'test',
    node_types: 1,
    auth_required: false,
    rust_edition: '2021',
    features: [],
    max_concurrent_executions: 1,
    max_executions_per_tenant: 1,
    running_executions: 0,
    ...captcha,
  }
}

async function mockSystem(page: Page, captcha: Record<string, unknown>) {
  await page.route('**/v1/system/info', (r) => r.fulfill({ json: sysinfo(captcha) }))
  await page.route('**/v1/sso/public', (r) => r.fulfill({ json: [] }))
}

const fakeAuth = {
  token: 'h.' + Buffer.from(JSON.stringify({ tenant_id: 't' })).toString('base64') + '.s',
  user: { email: 'a@example.com', tenant_id: 't', email_verified: false },
}

async function gotoEmailRegister(page: Page, query = '') {
  await page.goto('/' + query)
  await page.getByRole('button', { name: '邮箱' }).click()
  await page.getByRole('button', { name: '注册' }).click()
}

test('no captcha: email form renders and no widget script is injected', async ({ page }) => {
  await mockSystem(page, { captcha_provider: null, captcha_site_key: null })
  await gotoEmailRegister(page)
  await expect(page.locator('input[type=email]')).toBeVisible()
  await expect(page.locator('input[type=password]')).toBeVisible()
  await expect(page.locator('script[src*="challenges.cloudflare.com"]')).toHaveCount(0)
  await expect(page.locator('script[src*="hcaptcha.com"]')).toHaveCount(0)
})

test('captcha configured: gate blocks submit until solved', async ({ page }) => {
  await mockSystem(page, { captcha_provider: 'turnstile', captcha_site_key: '0xSITE' })
  let registerCalled = false
  await page.route('**/v1/auth/register', (r) => {
    registerCalled = true
    return r.fulfill({ json: {} })
  })
  await gotoEmailRegister(page)
  await expect(page.locator('script[src*="challenges.cloudflare.com"]')).toHaveCount(1)
  await page.locator('input[type=email]').fill('a@example.com')
  await page.locator('input[type=password]').fill('secret123')
  await page.locator('button[type=submit]').click()
  await expect(page.getByText('请先完成人机验证')).toBeVisible()
  expect(registerCalled).toBe(false)
})

test('attribution: first-touch UTM is stored and forwarded on register', async ({ page }) => {
  await mockSystem(page, { captcha_provider: null, captcha_site_key: null })
  let body: RegisterBody | null = null
  await page.route('**/v1/auth/register', async (r) => {
    body = JSON.parse(r.request().postData() || '{}') as RegisterBody
    await r.fulfill({ json: fakeAuth })
  })
  await gotoEmailRegister(page, '?utm_source=playwright&utm_campaign=launch')

  const stored = await page.evaluate(() => localStorage.getItem('trigix_attribution_v1'))
  expect(stored).toBeTruthy()
  expect(JSON.parse(stored as string).utm_source).toBe('playwright')

  await page.locator('input[type=email]').fill('a@example.com')
  await page.locator('input[type=password]').fill('secret123')
  await page.locator('button[type=submit]').click()
  await expect.poll(() => body?.attribution?.utm_source).toBe('playwright')
  expect(body?.attribution?.utm_campaign).toBe('launch')
})

test('captcha: a solved token is forwarded on register', async ({ page }) => {
  // Stub the provider script so it resolves a token immediately.
  await page.route('**/turnstile/v0/api.js**', (r) =>
    r.fulfill({
      contentType: 'application/javascript',
      body: 'window.turnstile={render:(el,o)=>{setTimeout(()=>o.callback("test-token"),0);return "w"},reset(){},remove(){}}',
    }),
  )
  await mockSystem(page, { captcha_provider: 'turnstile', captcha_site_key: '0xSITE' })
  let body: RegisterBody | null = null
  await page.route('**/v1/auth/register', async (r) => {
    body = JSON.parse(r.request().postData() || '{}') as RegisterBody
    await r.fulfill({ json: fakeAuth })
  })
  await gotoEmailRegister(page)
  await page.locator('input[type=email]').fill('a@example.com')
  await page.locator('input[type=password]').fill('secret123')
  await page.locator('button[type=submit]').click()
  await expect.poll(() => body?.captcha_token).toBe('test-token')
})
