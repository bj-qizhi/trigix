// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { test, expect, type Page } from '@playwright/test'

// Behavioural safety net for the WorkflowEditor graph-mutation surface BEFORE it
// is refactored. These pin the contract that survives a rewrite: palette add,
// config edits, node deletion, edge round-trip, and run dispatch must all flow
// through to the serialized graph (POST .../versions) and execution
// (POST .../executions). Backend is route-mocked; we also assert no pageerror.

const AUTH = {
  token: 'h.' + Buffer.from(JSON.stringify({ tenant_id: 't' })).toString('base64') + '.s',
  tenantId: 't', workspaceId: 'w', projectId: 'p', role: 'admin',
  email: 'a@example.com', emailVerified: true,
}

const WF = {
  id: 'wf1', tenant_id: 't', workspace_id: 'w', project_id: 'p',
  name: 'Editor WF', status: 'published', latest_version_id: 'v1',
  updated_at: 1, created_at: 1,
}

const VERSION = {
  id: 'v1', tenant_id: 't', workflow_id: 'wf1', version: 1, status: 'published',
  graph: {
    workflow_version_id: 'v1',
    nodes: [
      { id: 'trigger', type: 'trigger', config: {} },
      { id: 'structured_output', type: 'structured_output', config: {} },
      { id: 'http', type: 'http', config: { url: 'https://example.com' } },
      { id: 'slack', type: 'slack', config: {} },
    ],
    edges: [
      { source: 'trigger', target: 'structured_output' },
      { source: 'structured_output', target: 'slack' },
    ],
  },
}

type Graph = { nodes: { id: string; type: string; config: Record<string, unknown> }[]; edges: { source: string; target: string }[] }
type VersionPost = { tenant_id: string; graph: Graph; message?: string }
type ExecPost = { tenant_id: string; input_json: string }

function trackErrors(page: Page): string[] {
  const errors: string[] = []
  page.on('pageerror', (e) => errors.push(String(e)))
  return errors
}

// Auth + the WorkflowList critical-path reads so the list renders, plus the
// editor's workflow + version GETs. Everything else falls through to 403.
async function mockBackend(page: Page) {
  await page.addInitScript((auth) => localStorage.setItem('af_auth', JSON.stringify(auth)), AUTH)
  await page.route('**/v1/**', (r) => r.fulfill({ status: 403, json: {} }))
  await page.route(/\/v1\/schedules/, (r) => r.fulfill({ json: [] }))
  await page.route(/\/v1\/executions\/stats/, (r) => r.fulfill({ json: {} }))
  await page.route(/\/v1\/executions(\?|$)/, (r) => r.fulfill({ json: [] }))
  await page.route(/\/v1\/workflows(\?|$)/, (r) => r.fulfill({ json: [WF] }))
  await page.route(/\/v1\/workflows\/wf1(\?|$)/, (r) => r.fulfill({ json: WF }))
  await page.route(/\/v1\/workflow-versions\/v1/, (r) => r.fulfill({ json: VERSION }))
}

// Capture every POST .../versions body and answer with a fresh draft version.
function captureVersionPosts(page: Page): VersionPost[] {
  const calls: VersionPost[] = []
  page.route(/\/v1\/workflows\/wf1\/versions/, async (r) => {
    if (r.request().method() === 'POST') calls.push(r.request().postDataJSON() as VersionPost)
    await r.fulfill({ json: { ...VERSION, id: 'v2', version: 2, status: 'draft' } })
  })
  return calls
}

// Capture every POST .../executions body and answer with a queued execution.
function captureExecutionPosts(page: Page): ExecPost[] {
  const calls: ExecPost[] = []
  page.route(/\/v1\/workflows\/wf1\/executions/, async (r) => {
    if (r.request().method() === 'POST') calls.push(r.request().postDataJSON() as ExecPost)
    await r.fulfill({
      json: {
        id: 'ex1', tenant_id: 't', workflow_id: 'wf1', workflow_version_id: 'v1',
        status: 'queued', started_at: 1, created_at: 1, node_results: [],
      },
    })
  })
  return calls
}

async function openEditor(page: Page) {
  await page.goto('/')
  await page.getByText('Editor WF').click()
  // Canvas mounted once a loaded node is on screen.
  await expect(page.getByTestId('rf__node-http')).toBeVisible({ timeout: 10_000 })
}

const rfNodes = (page: Page) => page.locator('[data-testid^="rf__node-"]')

// Blur any focused config field — the editor's Ctrl+S / Ctrl+Enter / Delete
// keymap intentionally ignores keystrokes while an INPUT/TEXTAREA/SELECT holds
// focus, so we click the canvas pane first.
async function blurToCanvas(page: Page) {
  await page.locator('.react-flow__pane').click({ position: { x: 5, y: 5 } })
}

test('palette click adds a node to the canvas', async ({ page }) => {
  const errors = trackErrors(page)
  await mockBackend(page)
  await openEditor(page)

  const before = await rfNodes(page).count()
  await page.locator('.palette-node').first().click()
  await expect(rfNodes(page)).toHaveCount(before + 1, { timeout: 10_000 })

  expect(errors, errors.join('\n')).toHaveLength(0)
})

test('editing a node config and saving sends the new value in the version graph', async ({ page }) => {
  const errors = trackErrors(page)
  await mockBackend(page)
  const posts = captureVersionPosts(page)
  await openEditor(page)

  // Open the HTTP node config and rewrite its URL field (located by its current
  // value, so the test is independent of field labels/order).
  await page.getByTestId('rf__node-http').click({ position: { x: 10, y: 10 } })
  const inputs = page.locator('.config-panel-body input')
  await expect(inputs.first()).toBeVisible()
  let urlField = null
  for (let i = 0; i < (await inputs.count()); i++) {
    if ((await inputs.nth(i).inputValue()).includes('example.com')) { urlField = inputs.nth(i); break }
  }
  expect(urlField, 'HTTP node URL field should be present').not.toBeNull()
  await urlField!.fill('https://changed.example.com/new')

  await blurToCanvas(page)
  await page.keyboard.press('Control+s')

  await expect.poll(() => posts.length).toBeGreaterThan(0)
  const httpNode = posts.at(-1)!.graph.nodes.find((n) => n.id === 'http')
  expect(httpNode?.config.url).toBe('https://changed.example.com/new')

  expect(errors, errors.join('\n')).toHaveLength(0)
})

test('deleting a node removes it (and its edges) from the saved graph', async ({ page }) => {
  const errors = trackErrors(page)
  await mockBackend(page)
  const posts = captureVersionPosts(page)
  await openEditor(page)

  // Select the slack node and delete it.
  await page.getByTestId('rf__node-slack').click({ position: { x: 10, y: 10 } })
  await blurToCanvas(page)
  // Re-select (pane click cleared selection) then delete.
  await page.getByTestId('rf__node-slack').click({ position: { x: 10, y: 10 } })
  await page.keyboard.press('Delete')
  await expect(page.getByTestId('rf__node-slack')).toHaveCount(0)

  await page.keyboard.press('Control+s')

  await expect.poll(() => posts.length).toBeGreaterThan(0)
  const graph = posts.at(-1)!.graph
  expect(graph.nodes.find((n) => n.id === 'slack')).toBeUndefined()
  expect(graph.nodes.find((n) => n.id === 'http')).toBeDefined()
  // The edge into slack must be gone; the trigger→structured_output edge stays.
  expect(graph.edges.some((e) => e.target === 'slack')).toBe(false)
  expect(graph.edges.some((e) => e.source === 'trigger' && e.target === 'structured_output')).toBe(true)

  expect(errors, errors.join('\n')).toHaveLength(0)
})

test('saving an unchanged graph round-trips the loaded nodes and edges', async ({ page }) => {
  const errors = trackErrors(page)
  await mockBackend(page)
  const posts = captureVersionPosts(page)
  await openEditor(page)

  await blurToCanvas(page)
  await page.keyboard.press('Control+s')

  await expect.poll(() => posts.length).toBeGreaterThan(0)
  const graph = posts.at(-1)!.graph
  expect(graph.nodes.map((n) => n.id).sort()).toEqual(['http', 'slack', 'structured_output', 'trigger'])
  expect(graph.edges).toEqual(
    expect.arrayContaining([
      expect.objectContaining({ source: 'trigger', target: 'structured_output' }),
      expect.objectContaining({ source: 'structured_output', target: 'slack' }),
    ]),
  )

  expect(errors, errors.join('\n')).toHaveLength(0)
})

test('clicking the workflow title renames it', async ({ page }) => {
  const errors = trackErrors(page)
  await mockBackend(page)
  const patches: { name?: string }[] = []
  // PATCH /v1/workflows/wf1 renames; route after the catch-all so it wins.
  await page.route(/\/v1\/workflows\/wf1(\?|$)/, async (r) => {
    if (r.request().method() === 'PATCH') {
      patches.push(r.request().postDataJSON() as { name?: string })
      return r.fulfill({ json: { ...WF, name: 'Renamed WF' } })
    }
    return r.fulfill({ json: WF })
  })
  await openEditor(page)

  await expect(page.locator('.topbar-title')).toHaveText('Editor WF')
  // Click-to-rename enters an inline editor (regression guard: a stray rename
  // modal used to steal autofocus and immediately revert this).
  await page.locator('.topbar-title').click()
  const input = page.locator('.topbar input').first()
  await expect(input).toBeVisible()
  await input.fill('Renamed WF')
  await input.press('Enter')

  await expect.poll(() => patches.length).toBeGreaterThan(0)
  expect(patches.at(-1)!.name).toBe('Renamed WF')
  expect(errors, errors.join('\n')).toHaveLength(0)
})

test('canvas node shows its data-driven preview', async ({ page }) => {
  const errors = trackErrors(page)
  await mockBackend(page)
  await openEditor(page)
  // nodePreview() renders the http node's URL as its on-canvas subtitle.
  await expect(page.getByTestId('rf__node-http').getByText('https://example.com')).toBeVisible()
  expect(errors, errors.join('\n')).toHaveLength(0)
})

test('integration node config panel renders after the domain split', async ({ page }) => {
  const errors = trackErrors(page)
  await mockBackend(page)
  await openEditor(page)

  // structured_output's panel (StructuredOutputConfig) now lives in
  // integrations/ai.tsx and pulls llmEndpointFields from integrations/_helpers.
  // Selecting it exercises the barrel → domain-file → helpers wiring at runtime.
  await page.getByTestId('rf__node-structured_output').click({ position: { x: 10, y: 10 } })
  await expect(page.locator('.config-panel-body textarea, .config-panel-body input').first()).toBeVisible()

  expect(errors, errors.join('\n')).toHaveLength(0)
})

test('typing {{credential. autocompletes the saved credentials', async ({ page }) => {
  const errors = trackErrors(page)
  await mockBackend(page)
  await page.route(/\/v1\/credentials(\?|$)/, (r) => r.fulfill({ json: [{ id: 'c1', name: 'my-api-key', created_at: 1, updated_at: 1 }] }))
  await openEditor(page)

  // Opening a node's config mounts the autocomplete, which fetches credentials.
  const credResp = page.waitForResponse((r) => /\/v1\/credentials/.test(r.url()))
  await page.getByTestId('rf__node-http').click({ position: { x: 10, y: 10 } })
  await credResp
  const inputs = page.locator('.config-panel-body input')
  await expect(inputs.first()).toBeVisible()
  let urlField = null
  for (let i = 0; i < (await inputs.count()); i++) {
    if ((await inputs.nth(i).inputValue()).includes('example.com')) { urlField = inputs.nth(i); break }
  }
  await urlField!.fill('')
  await urlField!.click()
  await urlField!.pressSequentially('{{credential.my')

  // The platform resolves {{credential.<name>}} before execution, so the field
  // autocomplete now offers the user's saved credential names.
  await expect(page.getByText('my-api-key')).toBeVisible({ timeout: 10_000 })
  expect(errors, errors.join('\n')).toHaveLength(0)
})

test('Ctrl+Enter dispatches an execution with the run input', async ({ page }) => {
  const errors = trackErrors(page)
  await mockBackend(page)
  const posts = captureExecutionPosts(page)
  await openEditor(page)

  await blurToCanvas(page)
  await page.keyboard.press('Control+Enter')

  await expect.poll(() => posts.length).toBeGreaterThan(0)
  expect(posts.at(-1)!.tenant_id).toBe('t')
  expect(() => JSON.parse(posts.at(-1)!.input_json)).not.toThrow()

  expect(errors, errors.join('\n')).toHaveLength(0)
})

test('a running execution shows live progress in the editor exec panel', async ({ page }) => {
  const errors = trackErrors(page)
  await mockBackend(page)
  const RUNNING = {
    id: 'ex1', tenant_id: 't', workflow_id: 'wf1', workflow_version_id: 'v1',
    status: 'running', started_at: 1, created_at: 1,
    node_count: 3, completed_node_count: 1,
    node_results: [
      { node_id: 'trigger', status: 'succeeded' },
      { node_id: 'structured_output', status: 'running' },
    ],
  }
  // Dispatch returns a running execution with progress; the GET keeps it running
  // for the SSE-fallback poll so the bar stays put.
  await page.route(/\/v1\/workflows\/wf1\/executions/, (r) => r.fulfill({ json: RUNNING }))
  await page.route(/\/v1\/executions\/ex1(\?|$)/, (r) => r.fulfill({ json: RUNNING }))
  await openEditor(page)

  await blurToCanvas(page)
  await page.keyboard.press('Control+Enter')

  const progress = page.getByTestId('exec-progress')
  await expect(progress).toBeVisible({ timeout: 10_000 })
  await expect(progress).toContainText('1/3')
  await expect(progress).toContainText('structured_output')
  expect(errors, errors.join('\n')).toHaveLength(0)
})

test('version history modal lists versions and diffs them', async ({ page }) => {
  const errors = trackErrors(page)
  await mockBackend(page)

  const V1 = { ...VERSION, id: 'v1', version: 1, status: 'published', message: 'first' }
  const V2 = {
    ...VERSION, id: 'v2', version: 2, status: 'draft', message: 'add delay',
    graph: { ...VERSION.graph, nodes: [...VERSION.graph.nodes, { id: 'delay', type: 'delay', config: {} }] },
  }
  // GET = version list; POST = save (unused here).
  await page.route(/\/v1\/workflows\/wf1\/versions/, (r) =>
    r.fulfill({ json: r.request().method() === 'GET' ? [V2, V1] : V2 }))

  await openEditor(page)
  await page.locator('button[title="Browse version history"]').click()

  // Modal renders and lists both versions.
  await expect(page.getByText(/版本历史|Version History/)).toBeVisible()
  await expect(page.getByText('v2', { exact: true })).toBeVisible()
  await expect(page.getByText('v1', { exact: true })).toBeVisible()

  // Diffing the latest (v2) against its predecessor (v1) surfaces the added node.
  await page.getByRole('button', { name: /差异|Diff/ }).first().click()
  await expect(page.getByText(/\+ node: delay/)).toBeVisible()

  expect(errors, errors.join('\n')).toHaveLength(0)
})

test('Ctrl+K command palette filters nodes and jumps on Enter', async ({ page }) => {
  const errors = trackErrors(page)
  await mockBackend(page)
  await openEditor(page)

  await blurToCanvas(page)
  await page.keyboard.press('Control+k')

  const search = page.getByPlaceholder(/搜索节点|Search nodes/)
  await expect(search).toBeVisible()

  // Filtering narrows the list to the matching node.
  await search.fill('http')
  await expect(page.locator('.modal code', { hasText: 'http' })).toBeVisible()
  await expect(page.locator('.modal code', { hasText: 'slack' })).toHaveCount(0)

  // Enter jumps to the first match and closes the palette.
  await search.press('Enter')
  await expect(search).toHaveCount(0)

  expect(errors, errors.join('\n')).toHaveLength(0)
})

test('Ctrl+K command palette runs an editor command', async ({ page }) => {
  const errors = trackErrors(page)
  await mockBackend(page)
  await openEditor(page)

  await blurToCanvas(page)
  await page.keyboard.press('Control+k')
  const search = page.getByPlaceholder(/搜索节点|command/)
  await expect(search).toBeVisible()

  // Type a command name; Enter runs the highlighted command (validate → modal).
  await search.fill('validate')
  await search.press('Enter')
  await expect(page.getByRole('heading', { name: /工作流校验|Workflow Validation/ })).toBeVisible({ timeout: 10_000 })

  expect(errors, errors.join('\n')).toHaveLength(0)
})

test('toolbar View and More menus open and trigger actions', async ({ page }) => {
  const errors = trackErrors(page)
  await mockBackend(page)
  await openEditor(page)

  // View menu toggles open/closed (located by its title — the label is ambiguous).
  const viewBtn = page.locator('button[title="画布视图选项"], button[title="Canvas view options"]')
  await viewBtn.click()
  await expect(page.getByText(/对齐网格|Snap to grid/)).toBeVisible()
  await viewBtn.click()
  await expect(page.getByText(/对齐网格|Snap to grid/)).toHaveCount(0)

  // More menu → first item (Validate) opens the validation modal, and the
  // data-driven required-field check flags the slack node's missing webhook URL.
  await page.locator('button[title="更多操作"], button[title="More actions"]').click()
  await page.locator('.tb-popover .tb-menu-item').first().click()
  await expect(page.getByText(/工作流校验|Workflow Validation/)).toBeVisible()
  await expect(page.getByText('Slack node "slack" has no Webhook URL')).toBeVisible()

  expect(errors, errors.join('\n')).toHaveLength(0)
})

test('toolbar Limits popover opens and toggles an edit field', async ({ page }) => {
  const errors = trackErrors(page)
  await mockBackend(page)
  await openEditor(page)

  await page.locator('button[title="SLA、速率、并发与 AI 预算"], button[title="SLA, rate limit, concurrency & AI budget"]').click()
  await expect(page.getByText(/限额与预算|Limits & budget/)).toBeVisible()

  // Clicking a "not set" value enters inline-edit mode (a number input appears).
  await page.locator('.tb-popover').getByRole('button', { name: /未设置|not set/ }).first().click()
  await expect(page.locator('.tb-popover input[type="number"]')).toBeVisible()

  expect(errors, errors.join('\n')).toHaveLength(0)
})

test('toolbar tag editor toggles an add input', async ({ page }) => {
  const errors = trackErrors(page)
  await mockBackend(page)
  await openEditor(page)

  await page.getByText('+ tag', { exact: true }).click()
  await expect(page.getByPlaceholder(/标签名|tag name/)).toBeVisible()

  expect(errors, errors.join('\n')).toHaveLength(0)
})

test('validation issues are clickable and jump to the offending node', async ({ page }) => {
  const errors = trackErrors(page)
  await mockBackend(page)
  await openEditor(page)

  // The fixture's slack node has no webhook URL and the http node is orphaned,
  // so the "N issues" badge is present. Click it to open the validation modal.
  await page.locator('span[title="点击校验"], span[title="Click to validate"]').click()
  await expect(page.getByRole('heading', { name: /工作流校验|Workflow Validation/ })).toBeVisible({ timeout: 10_000 })

  // Node-scoped warnings render a jump affordance; clicking one selects+centers
  // the node and closes the modal.
  const jump = page.getByText(/定位 →|Jump →/).first()
  await expect(jump).toBeVisible()
  await jump.click()
  await expect(page.getByRole('heading', { name: /工作流校验|Workflow Validation/ })).toHaveCount(0)

  expect(errors, errors.join('\n')).toHaveLength(0)
})
