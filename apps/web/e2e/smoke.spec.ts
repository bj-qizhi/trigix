// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { test, expect, type Page } from '@playwright/test'

// Smoke regression for the icon migration (emoji → react-icons), the node-header
// colour fix, palette/menu icons, drag-drop wiring and theme toggle. Backend is
// route-mocked; we assert pages render without an uncaught exception (pageerror)
// and that the new icon/colour wiring is actually present in the DOM.

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

const VERSION = {
  id: 'v1', tenant_id: 't', workflow_id: 'wf1', version: 1, status: 'published',
  graph: {
    workflow_version_id: 'v1',
    nodes: [
      { id: 'trigger', type: 'trigger', config: {} },
      // structured_output previously had no header-background CSS → title bug
      { id: 'structured_output', type: 'structured_output', config: {} },
      { id: 'slack', type: 'slack', config: {} },
      { id: 'http', type: 'http', config: { url: 'https://example.com' } },
    ],
    edges: [
      { source: 'trigger', target: 'structured_output' },
      { source: 'structured_output', target: 'slack' },
    ],
  },
}

function trackErrors(page: Page): string[] {
  const errors: string[] = []
  page.on('pageerror', (e) => errors.push(String(e)))
  return errors
}

// Auth + the WorkflowList critical-path Promise.all (workflows/schedules/
// executions/stats) so the list renders rows; everything else falls to 403.
async function authedList(page: Page) {
  await page.addInitScript((auth) => localStorage.setItem('af_auth', JSON.stringify(auth)), AUTH)
  await page.route('**/v1/**', (r) => r.fulfill({ status: 403, json: {} }))
  await page.route(/\/v1\/schedules/, (r) => r.fulfill({ json: [] }))
  await page.route(/\/v1\/executions\/stats/, (r) => r.fulfill({ json: {} }))
  await page.route(/\/v1\/executions(\?|$)/, (r) => r.fulfill({ json: [] }))
  await page.route(/\/v1\/workflows(\?|$)/, (r) => r.fulfill({ json: [WF] }))
}

test('workflow list + nav menu render with icons, theme toggles, no crash', async ({ page }) => {
  const errors = trackErrors(page)
  await authedList(page)
  await page.goto('/')

  await expect(page.getByText('Smoke WF')).toBeVisible()

  // Nav menu items render with their react-icons SVG icons.
  await page.locator('button[title="Navigation"]').click()
  await expect(page.getByText('监控中心', { exact: true })).toBeVisible()
  await expect(page.locator('button:has-text("监控中心") svg').first()).toBeVisible()
  await page.keyboard.press('Escape')

  // Theme toggle (Phosphor sun/moon) flips data-theme without crashing.
  const themeBtn = page.locator('button[title="Toggle dark/light theme"]')
  await expect(themeBtn.locator('svg')).toBeVisible()
  const before = await page.evaluate(() => document.documentElement.getAttribute('data-theme'))
  await themeBtn.click()
  const after = await page.evaluate(() => document.documentElement.getAttribute('data-theme'))
  expect(after).not.toBe(before)

  expect(errors, errors.join('\n')).toHaveLength(0)
})

test('editor renders nodes with coloured headers + palette icons, no crash', async ({ page }) => {
  const errors = trackErrors(page)
  await authedList(page)
  await page.route(/\/v1\/workflows\/wf1(\?|$)/, (r) => r.fulfill({ json: WF }))
  await page.route(/\/v1\/workflow-versions\/v1/, (r) => r.fulfill({ json: VERSION }))
  await page.goto('/')

  await page.getByText('Smoke WF').click()

  // Canvas node renders (previously-broken title now shows on the node block).
  const node = page.getByTestId('rf__node-structured_output')
  await expect(node).toBeVisible({ timeout: 10_000 })
  await expect(node.getByText('Structured Output')).toBeVisible()

  // The header fix: node headers have a non-transparent background colour.
  const headerBg = await page.locator('.flow-node-header').first().evaluate(
    (el) => getComputedStyle(el).backgroundColor,
  )
  expect(headerBg).not.toBe('rgba(0, 0, 0, 0)')
  expect(headerBg).not.toBe('transparent')

  // Node header icon is a react-icons SVG (not emoji text).
  await expect(page.locator('.flow-node-header svg').first()).toBeVisible()

  // Left palette renders draggable node items with icons.
  await expect(page.locator('.palette-node-label svg').first()).toBeVisible()
  await expect(page.locator('.palette-node[draggable="true"]').first()).toBeVisible()

  expect(errors, errors.join('\n')).toHaveLength(0)
})
