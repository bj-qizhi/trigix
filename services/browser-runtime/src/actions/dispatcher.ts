import { mkdir, readFile, rm, stat, writeFile } from 'node:fs/promises'
import path from 'node:path'
import os from 'node:os'
import type { BrowserContext, Page } from 'playwright'
import type { BrowserRuntimeConfig } from '../config.js'
import { BrowserRuntimeError } from '../errors.js'
import type { ArtifactManager } from '../artifacts/manager.js'
import type { BrowserAction, BrowserArtifact, BrowserTask } from '../types.js'
import type { BrowserSessionRecord, SessionManager } from '../sessions/manager.js'
import type { UrlPolicy } from '../security/url-policy.js'

export interface ActionEnvironment {
  page: Page
  context: BrowserContext
  task: BrowserTask
  session?: BrowserSessionRecord
  signal: AbortSignal
}

export interface ActionOutput { data?: unknown; artifacts?: BrowserArtifact[] }

export class ActionDispatcher {
  private readonly networkEntries = new WeakMap<Page, Array<Record<string, unknown>>>()
  constructor(
    private readonly config: BrowserRuntimeConfig,
    private readonly urlPolicy: UrlPolicy,
    private readonly artifacts: ArtifactManager,
    private readonly sessions: SessionManager,
  ) {}

  async execute(action: BrowserAction, environment: ActionEnvironment): Promise<ActionOutput> {
    const { page, context, task, session, signal } = environment
    throwIfAborted(signal)
    switch (action.type) {
      case 'navigate': {
        const url = requiredString(action.params, 'url')
        await this.urlPolicy.validate(url)
        const waitUntil = optionalEnum(action.params, 'wait_until', ['load', 'domcontentloaded', 'networkidle', 'commit'] as const) ?? 'domcontentloaded'
        const response = await page.goto(url, { waitUntil })
        return { data: { url: page.url(), title: await page.title(), status: response?.status() ?? null } }
      }
      case 'click': {
        const selector = requiredString(action.params, 'selector')
        assertNotAntiBotSelector(selector)
        await page.locator(selector).click()
        return { data: { url: page.url() } }
      }
      case 'input': {
        const selector = requiredString(action.params, 'selector')
        assertNotAntiBotSelector(selector)
        const locator = page.locator(selector)
        const value = requiredString(action.params, 'value')
        if (action.params.clear_first === false) await locator.pressSequentially(value)
        else await locator.fill(value)
        return { data: { input_applied: true } }
      }
      case 'wait': {
        if (typeof action.params.milliseconds === 'number') {
          await abortableDelay(Math.min(action.params.milliseconds, 60_000), signal)
        } else if (typeof action.params.url === 'string') {
          await page.waitForURL(action.params.url)
        } else if (typeof action.params.load_state === 'string') {
          const state = optionalEnum(action.params, 'load_state', ['load', 'domcontentloaded', 'networkidle'] as const)!
          await page.waitForLoadState(state)
        } else {
          const state = optionalEnum(action.params, 'state', ['attached', 'detached', 'visible', 'hidden'] as const) ?? 'visible'
          await page.locator(requiredString(action.params, 'selector')).waitFor({ state })
        }
        return { data: { waited: true, url: page.url() } }
      }
      case 'extract': return { data: await extract(page, action.params) }
      case 'screenshot': {
        const body = await page.screenshot({ fullPage: action.params.full_page !== false, type: 'png' })
        const artifact = await this.artifacts.create(artifactInput(task, 'screenshot', 'image/png', body))
        return { data: artifact, artifacts: [artifact] }
      }
      case 'cookies': {
        const operation = optionalEnum(action.params, 'operation', ['get', 'set', 'clear'] as const) ?? 'get'
        if (operation === 'clear') { await context.clearCookies(); return { data: { cleared: true } } }
        if (operation === 'set') {
          if (!Array.isArray(action.params.cookies)) throw new BrowserRuntimeError('BROWSER_INVALID_REQUEST', 'cookies must be an array')
          await context.addCookies(action.params.cookies as Parameters<BrowserContext['addCookies']>[0])
          return { data: { count: action.params.cookies.length } }
        }
        const cookies = await context.cookies()
        return { data: cookies.map(({ name, domain, path: cookiePath, expires, httpOnly, secure, sameSite }) => ({ name, domain, path: cookiePath, expires, http_only: httpOnly, secure, same_site: sameSite })) }
      }
      case 'upload': {
        const selector = requiredString(action.params, 'selector')
        const filename = path.basename(requiredString(action.params, 'filename'))
        const content = Buffer.from(requiredString(action.params, 'content_base64'), 'base64')
        if (content.byteLength > this.config.BROWSER_MAX_DOWNLOAD_BYTES) throw new BrowserRuntimeError('BROWSER_RESOURCE_LIMIT', 'Upload exceeds configured size limit', 413)
        const directory = await makeTaskTemp(task.id)
        const target = path.join(directory, filename)
        try { await writeFile(target, content, { mode: 0o600 }); await page.locator(selector).setInputFiles(target) }
        finally { await rm(directory, { recursive: true, force: true }) }
        return { data: { filename, size: content.byteLength } }
      }
      case 'download': {
        const [download] = await Promise.all([page.waitForEvent('download'), page.locator(requiredString(action.params, 'selector')).click()])
        const downloadPath = await download.path()
        if (!downloadPath) throw new BrowserRuntimeError('BROWSER_ARTIFACT_FAILED', 'Download did not produce a file')
        if ((await stat(downloadPath)).size > this.config.BROWSER_MAX_DOWNLOAD_BYTES) throw new BrowserRuntimeError('BROWSER_RESOURCE_LIMIT', 'Download exceeds configured size limit', 413)
        const body = await readFile(downloadPath)
        const artifact = await this.artifacts.create(artifactInput(task, 'download', 'application/octet-stream', body))
        return { data: { ...artifact, suggested_filename: download.suggestedFilename() }, artifacts: [artifact] }
      }
      case 'pdf': {
        const body = await page.pdf({ format: typeof action.params.format === 'string' ? action.params.format : 'A4', printBackground: true })
        const artifact = await this.artifacts.create(artifactInput(task, 'pdf', 'application/pdf', body))
        return { data: artifact, artifacts: [artifact] }
      }
      case 'network': {
        const operation = optionalEnum(action.params, 'operation', ['start', 'get', 'clear'] as const) ?? 'get'
        if (operation === 'start') {
          if (!this.networkEntries.has(page)) {
            const entries: Array<Record<string, unknown>> = []
            this.networkEntries.set(page, entries)
            page.on('response', async (response) => {
              const request = response.request()
              entries.push({ method: request.method(), resource_type: request.resourceType(), url: redactUrl(response.url()), status: response.status(), content_type: response.headers()['content-type'] ?? null, timing_ms: Math.max(0, request.timing().responseEnd) })
              if (entries.length > 1_000) entries.shift()
            })
          }
          return { data: { recording: true } }
        }
        const entries = this.networkEntries.get(page) ?? []
        if (operation === 'clear') { entries.length = 0; return { data: { cleared: true } } }
        return { data: { url: page.url(), entries: structuredClone(entries), count: entries.length } }
      }
      case 'har': {
        const entries = this.networkEntries.get(page) ?? []
        const body = Buffer.from(JSON.stringify({ log: { version: '1.2', creator: { name: 'Trigix Browser Runtime', version: '1.6.0-rc.1' }, entries } }))
        const artifact = await this.artifacts.create(artifactInput(task, 'har', 'application/json', body))
        return { data: artifact, artifacts: [artifact] }
      }
      case 'trace': {
        const operation = optionalEnum(action.params, 'operation', ['start', 'stop'] as const) ?? 'start'
        if (operation === 'start') { await context.tracing.start({ screenshots: true, snapshots: true, sources: false }); return { data: { tracing: true } } }
        const directory = await makeTaskTemp(task.id)
        const target = path.join(directory, 'trace.zip')
        try {
          await context.tracing.stop({ path: target })
          const artifact = await this.artifacts.create(artifactInput(task, 'trace', 'application/zip', await readFile(target)))
          return { data: artifact, artifacts: [artifact] }
        } finally { await rm(directory, { recursive: true, force: true }) }
      }
      case 'page': {
        if (!session) throw new BrowserRuntimeError('BROWSER_INVALID_REQUEST', 'Page actions require a session')
        const operation = optionalEnum(action.params, 'operation', ['new', 'select', 'close'] as const) ?? 'new'
        if (operation === 'new') { const newPage = await context.newPage(); await this.sessions.addPage(session, newPage); return { data: { page_index: session.pages.indexOf(newPage) } } }
        const index = Number(action.params.index ?? session.pages.indexOf(session.activePage))
        const selected = this.sessions.selectPage(session, index)
        if (operation === 'close') { await this.sessions.closePage(session, index); return { data: { closed: index } } }
        return { data: { page_index: index, url: selected.url() } }
      }
      case 'evaluate': {
        if (!this.config.BROWSER_ENABLE_EVALUATE) throw new BrowserRuntimeError('BROWSER_INVALID_REQUEST', 'JavaScript evaluation is disabled by policy', 403)
        return { data: await page.evaluate(requiredString(action.params, 'script')) }
      }
    }
  }
}

async function extract(page: Page, params: Record<string, unknown>) {
  const selector = requiredString(params, 'selector')
  const mode = optionalEnum(params, 'mode', ['text', 'html', 'attribute', 'json', 'list', 'table'] as const) ?? 'text'
  const locator = page.locator(selector)
  if (mode === 'text') return { data: await locator.first().textContent(), count: await locator.count() }
  if (mode === 'html') return { data: await locator.first().innerHTML(), count: await locator.count() }
  if (mode === 'attribute') return { data: await locator.first().getAttribute(requiredString(params, 'attribute')), count: await locator.count() }
  if (mode === 'list') return { data: await locator.allTextContents(), count: await locator.count() }
  if (mode === 'table') {
    const rows = await locator.locator('tr').evaluateAll((elements) => elements.map((row) => [...row.querySelectorAll('th,td')].map((cell) => cell.textContent?.trim() ?? '')))
    return { data: rows, count: rows.length }
  }
  const text = await locator.first().textContent()
  try { return { data: JSON.parse(text ?? 'null') as unknown, count: await locator.count() } }
  catch { throw new BrowserRuntimeError('BROWSER_ACTION_FAILED', 'Extracted content is not valid JSON') }
}

function artifactInput(task: BrowserTask, type: BrowserArtifact['type'], contentType: string, body: Buffer) {
  return { tenantId: task.tenant_id, type, contentType, body, taskId: task.id, ...(task.execution_id ? { executionId: task.execution_id } : {}) }
}
function requiredString(params: Record<string, unknown>, key: string) {
  const value = params[key]
  if (typeof value !== 'string' || !value.trim()) throw new BrowserRuntimeError('BROWSER_INVALID_REQUEST', `${key} is required`)
  return value
}
function optionalEnum<const T extends readonly string[]>(params: Record<string, unknown>, key: string, values: T): T[number] | undefined {
  const value = params[key]
  if (value === undefined) return undefined
  if (typeof value !== 'string' || !values.includes(value)) throw new BrowserRuntimeError('BROWSER_INVALID_REQUEST', `${key} is invalid`)
  return value
}
function throwIfAborted(signal: AbortSignal) { if (signal.aborted) throw new BrowserRuntimeError('BROWSER_TASK_CANCELLED', 'Task was cancelled', 409) }
async function abortableDelay(ms: number, signal: AbortSignal) {
  await new Promise<void>((resolve, reject) => {
    const timer = setTimeout(resolve, ms)
    signal.addEventListener('abort', () => { clearTimeout(timer); reject(new BrowserRuntimeError('BROWSER_TASK_CANCELLED', 'Task was cancelled', 409)) }, { once: true })
  })
}
async function makeTaskTemp(taskId: string) { const directory = path.join(os.tmpdir(), `trigix-browser-${taskId}-${Date.now()}`); await mkdir(directory, { recursive: true, mode: 0o700 }); return directory }
function redactUrl(raw: string) { try { const url = new URL(raw); url.username = ''; url.password = ''; url.search = ''; url.hash = ''; return url.href } catch { return '' } }
function assertNotAntiBotSelector(selector: string) {
  if (/(captcha|turnstile|hcaptcha|recaptcha|challenges\.cloudflare)/i.test(selector)) {
    throw new BrowserRuntimeError('BROWSER_INVALID_REQUEST', 'Automating CAPTCHA or anti-bot challenges is not supported', 403)
  }
}
