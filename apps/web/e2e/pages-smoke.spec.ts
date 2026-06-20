// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { test, expect, type Page } from '@playwright/test'

// Broad regression: every page reachable from the main navigation menu must
// mount without throwing. Backend is route-mocked to 403 (plus the few reads
// the list needs), so this asserts each page handles the "no data / forbidden"
// path cleanly — the most common crash source — rather than deep behaviour.

const AUTH = {
  token: 'h.' + Buffer.from(JSON.stringify({ tenant_id: 't' })).toString('base64') + '.s',
  tenantId: 't', workspaceId: 'w', projectId: 'p', role: 'admin',
  email: 'a@example.com', emailVerified: true,
}

// Nav menu labels (default locale is zh). Each opens a distinct top-level page.
const NAV_PAGES = [
  '运行记录', '审批队列', '依赖图', '分析', '推荐返佣', '计划任务', '监控中心',
  '审计日志', '环境变量', '工作空间', 'Webhooks', 'API 密钥', '企业 SSO',
  '知识库', '自定义节点', '事件 Webhooks', '组织', '凭证',
]

function trackErrors(page: Page): string[] {
  const errors: string[] = []
  page.on('pageerror', (e) => errors.push(String(e)))
  return errors
}

async function mockBackend(page: Page) {
  await page.addInitScript((auth) => localStorage.setItem('af_auth', JSON.stringify(auth)), AUTH)
  await page.route('**/v1/**', (r) => r.fulfill({ status: 403, json: {} }))
  // The reads the workflow list issues on mount, so '/' lands on the list.
  await page.route(/\/v1\/schedules/, (r) => r.fulfill({ json: [] }))
  await page.route(/\/v1\/executions\/stats/, (r) => r.fulfill({ json: {} }))
  await page.route(/\/v1\/executions(\?|$)/, (r) => r.fulfill({ json: [] }))
  await page.route(/\/v1\/workflows(\?|$)/, (r) => r.fulfill({ json: [] }))
}

test('every navigation page mounts without a crash', async ({ page }) => {
  const errors = trackErrors(page)
  await mockBackend(page)

  for (const label of NAV_PAGES) {
    await page.goto('/')
    await page.locator('button[title="Navigation"]').click()
    await page.getByRole('button', { name: label, exact: true }).click()

    // Leaving the list removes its Navigation button — proves the page mounted.
    await expect(page.locator('button[title="Navigation"]')).toHaveCount(0, { timeout: 10_000 })
    // Each page must also handle the 403/no-data path without an uncaught error
    // (a render crash or an unhandled fetch rejection both surface here).
    expect(errors, `error after opening "${label}":\n${errors.join('\n')}`).toHaveLength(0)
  }
})
