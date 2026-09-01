import { ulid } from 'ulid'
import type { Page } from 'playwright'
import type { BrowserRuntimeConfig } from '../config.js'
import { BrowserRuntimeError } from '../errors.js'
import type { BrowserPool, AllocatedContext } from '../browser/pool.js'
import type { BrowserSessionView } from '../types.js'
import type { RuntimeMetrics } from '../telemetry/metrics.js'

interface SessionRecord extends BrowserSessionView {
  allocation: AllocatedContext
  pages: Page[]
  activePage: Page
}

export class SessionManager {
  private readonly sessions = new Map<string, SessionRecord>()
  private readonly pendingByTenant = new Map<string, number>()
  private cleanupTimer?: NodeJS.Timeout

  constructor(private readonly pool: BrowserPool, private readonly config: BrowserRuntimeConfig, private readonly metrics: RuntimeMetrics) {}

  start() {
    this.cleanupTimer = setInterval(() => { void this.expireIdle() }, Math.min(30_000, this.config.BROWSER_IDLE_SESSION_TIMEOUT_MS))
    this.cleanupTimer.unref()
  }

  async create(tenantId: string, executionId?: string, signal?: AbortSignal): Promise<BrowserSessionView> {
    const tenantCount = [...this.sessions.values()].filter((session) => session.tenant_id === tenantId && session.status === 'active').length
      + (this.pendingByTenant.get(tenantId) ?? 0)
    if (tenantCount >= this.config.BROWSER_TENANT_MAX_SESSIONS) {
      throw new BrowserRuntimeError('BROWSER_RESOURCE_LIMIT', 'Tenant session quota reached', 429)
    }
    this.pendingByTenant.set(tenantId, (this.pendingByTenant.get(tenantId) ?? 0) + 1)
    let allocation: AllocatedContext | undefined
    const controller = new AbortController()
    const timeout = setTimeout(() => controller.abort(new BrowserRuntimeError('BROWSER_TASK_TIMEOUT', 'Session allocation timeout elapsed', 408)), this.config.BROWSER_TASK_TIMEOUT_MS)
    const forwardAbort = () => controller.abort(signal?.reason)
    signal?.addEventListener('abort', forwardAbort, { once: true })
    try {
      allocation = await this.pool.allocate(controller.signal)
      const page = await allocation.context.newPage()
      const now = Date.now()
      const record: SessionRecord = {
        id: `bs_${ulid()}`,
        tenant_id: tenantId,
        status: 'active',
        created_at: new Date(now).toISOString(),
        last_activity_at: new Date(now).toISOString(),
        expires_at: new Date(now + this.config.BROWSER_IDLE_SESSION_TIMEOUT_MS).toISOString(),
        allocation,
        pages: [page],
        activePage: page,
        ...(executionId ? { execution_id: executionId } : {}),
      }
      this.sessions.set(record.id, record)
      this.metrics.sessions.inc()
      this.metrics.pages.inc()
      return view(record)
    } catch (error) {
      await allocation?.release().catch(() => undefined)
      throw error
    } finally {
      clearTimeout(timeout)
      signal?.removeEventListener('abort', forwardAbort)
      const pending = Math.max(0, (this.pendingByTenant.get(tenantId) ?? 1) - 1)
      if (pending === 0) this.pendingByTenant.delete(tenantId)
      else this.pendingByTenant.set(tenantId, pending)
    }
  }

  get(id: string, tenantId: string): SessionRecord {
    const record = this.sessions.get(id)
    if (!record || record.tenant_id !== tenantId) throw new BrowserRuntimeError('BROWSER_SESSION_NOT_FOUND', 'Browser session was not found', 404)
    if (record.status !== 'active') throw new BrowserRuntimeError('BROWSER_SESSION_EXPIRED', 'Browser session is no longer active', 410)
    const now = Date.now()
    record.last_activity_at = new Date(now).toISOString()
    record.expires_at = new Date(now + this.config.BROWSER_IDLE_SESSION_TIMEOUT_MS).toISOString()
    return record
  }

  inspect(id: string, tenantId: string) { return view(this.get(id, tenantId)) }

  async addPage(record: SessionRecord, page: Page) {
    if (record.pages.length >= this.config.BROWSER_MAX_PAGES_PER_CONTEXT) {
      await page.close().catch(() => undefined)
      throw new BrowserRuntimeError('BROWSER_RESOURCE_LIMIT', 'Session page quota reached', 429)
    }
    record.pages.push(page)
    record.activePage = page
    this.metrics.pages.inc()
  }

  selectPage(record: SessionRecord, index: number) {
    const page = record.pages[index]
    if (!page || page.isClosed()) throw new BrowserRuntimeError('BROWSER_INVALID_REQUEST', 'Page index is unavailable')
    record.activePage = page
    return page
  }

  async closePage(record: SessionRecord, index: number) {
    if (record.pages.length <= 1) {
      throw new BrowserRuntimeError('BROWSER_RESOURCE_LIMIT', 'A session must retain at least one Page')
    }
    const page = this.selectPage(record, index)
    await page.close()
    record.pages.splice(index, 1)
    record.activePage = record.pages[Math.min(index, record.pages.length - 1)]!
    this.metrics.pages.dec()
  }

  async close(id: string, tenantId: string, status: BrowserSessionView['status'] = 'closed') {
    const record = this.sessions.get(id)
    if (!record || record.tenant_id !== tenantId) throw new BrowserRuntimeError('BROWSER_SESSION_NOT_FOUND', 'Browser session was not found', 404)
    if (record.status !== 'active') return view(record)
    record.status = status === 'expired' ? 'expired' : 'closing'
    const openPages = record.pages.filter((page) => !page.isClosed()).length
    await record.allocation.release()
    record.status = status
    const now = Date.now()
    record.last_activity_at = new Date(now).toISOString()
    record.expires_at = new Date(now + this.config.BROWSER_IDLE_SESSION_TIMEOUT_MS).toISOString()
    this.metrics.sessions.dec()
    this.metrics.pages.dec(openPages)
    return view(record)
  }

  async shutdown() {
    if (this.cleanupTimer) clearInterval(this.cleanupTimer)
    await Promise.allSettled([...this.sessions.values()].filter(({ status }) => status === 'active').map((record) => this.close(record.id, record.tenant_id)))
  }

  private async expireIdle() {
    const now = Date.now()
    const expired = [...this.sessions.values()].filter((record) => record.status === 'active' && Date.parse(record.expires_at) <= now)
    await Promise.allSettled(expired.map((record) => this.close(record.id, record.tenant_id, 'expired')))
    for (const record of this.sessions.values()) {
      if (record.status !== 'active' && Date.parse(record.expires_at) <= now) this.sessions.delete(record.id)
    }
  }
}

function view(record: SessionRecord): BrowserSessionView {
  const { id, tenant_id, execution_id, status, created_at, last_activity_at, expires_at } = record
  return { id, tenant_id, status, created_at, last_activity_at, expires_at, ...(execution_id ? { execution_id } : {}) }
}

export type BrowserSessionRecord = ReturnType<SessionManager['get']>
