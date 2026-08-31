// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { expect, test } from '@playwright/test'

const AUTH = {
  token: 'h.' + Buffer.from(JSON.stringify({ tenant_id: 't' })).toString('base64') + '.s',
  tenantId: 't', workspaceId: 'w', projectId: 'p', role: 'admin',
  email: 'operator@example.com', emailVerified: true,
}

test('operator approves a sanitized Desktop command and exports evidence metadata', async ({ page }) => {
  await page.addInitScript((auth) => localStorage.setItem('af_auth', JSON.stringify(auth)), AUTH)
  await page.route('**/v1/**', (route) => route.fulfill({ status: 403, json: {} }))
  await page.route(/\/v1\/schedules/, (route) => route.fulfill({ json: [] }))
  await page.route(/\/v1\/executions\/stats/, (route) => route.fulfill({ json: {} }))
  await page.route(/\/v1\/executions(\?|$)/, (route) => route.fulfill({ json: [], headers: { 'x-total-count': '0' } }))
  await page.route(/\/v1\/workflows(\?|$)/, (route) => route.fulfill({ json: [] }))
  await page.route(/\/v1\/desktop\/approvals(\?|$)/, (route) => route.fulfill({ json: [{
    command_id: 'desktop-command-1', execution_id: 'execution-1', device_id: 'device-1', workflow_id: 'workflow-1',
    action_kind: 'type_text', risk: 'high', reason: 'high risk desktop action requires command-specific Approval',
    requested_by: 'requester-1', created_at_unix_ms: Date.now(), expires_at_unix_ms: Date.now() + 60_000,
  }] }))

  let decision: unknown
  await page.route(/\/v1\/desktop\/approvals\/desktop-command-1$/, async (route) => {
    decision = route.request().postDataJSON()
    await route.fulfill({ json: { command: { command_id: 'desktop-command-1', execution_id: 'execution-1' }, device_id: 'device-1', workflow_id: 'workflow-1', status: 'queued' } })
  })
  await page.route(/\/v1\/desktop\/evidence\/export/, (route) => route.fulfill({ json: {
    execution_id: 'execution-1', exported_at_unix_ms: Date.now(), records: [],
  } }))
  await page.route(/\/v1\/desktop\/evidence(\?|$)/, (route) => route.fulfill({ json: [{
    evidence_id: 'evidence-1', tenant_id: 't', project_id: 'p', execution_id: 'execution-1',
    command_id: 'desktop-command-1', device_id: 'device-1', kind: 'adapter_audit',
    selector_strategy: 'automation_id', application_id: 'fixture', started_at_unix_ms: 1,
    completed_at_unix_ms: 2, outcome: 'succeeded', policy_version: 'v1', redacted_regions: 1,
    byte_size: 0, expires_at_unix_ms: Date.now() + 86_400_000, created_at_unix_ms: Date.now(),
  }] }))

  await page.goto('/')
  await page.locator('button[title="Navigation"]').click()
  await page.getByText(/审批队列|Approvals/, { exact: true }).last().click()

  await expect(page.getByRole('heading', { name: /审批与桌面证据|Approvals & desktop evidence/ })).toBeVisible()
  await expect(page.getByText('type_text', { exact: true })).toBeVisible()
  await expect(page.getByText('HIGH', { exact: true })).toBeVisible()
  await expect(page.getByText('must-not-appear-in-queue')).toHaveCount(0)

  await page.getByRole('button', { name: /审查并批准|Review & approve/ }).click()
  await expect(page.getByRole('alertdialog')).toBeVisible()
  await page.getByRole('alertdialog').getByRole('button', { name: /确认|Confirm/ }).click()
  await expect.poll(() => decision).toEqual({ tenant_id: 't', decision: 'approve' })

  await page.getByPlaceholder(/输入执行 ID|Enter an Execution ID/).fill('execution-1')
  await page.getByRole('button', { name: /查看证据|View evidence/ }).click()
  await expect(page.getByRole('region', { name: /桌面证据结果|Desktop evidence results/ })).toContainText('automation_id')

  const download = page.waitForEvent('download')
  await page.getByRole('button', { name: /导出安全元数据|Export safe metadata/ }).click()
  expect((await download).suggestedFilename()).toBe('desktop-evidence-execution-1.json')
})
