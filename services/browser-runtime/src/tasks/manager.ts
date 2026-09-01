import { ulid } from 'ulid'
import type { Page } from 'playwright'
import type { BrowserRuntimeConfig } from '../config.js'
import { BrowserRuntimeError, safeError } from '../errors.js'
import type { BrowserPool, AllocatedContext } from '../browser/pool.js'
import type { ActionDispatcher } from '../actions/dispatcher.js'
import type { BrowserActionResult, BrowserTask, BrowserTaskStatus, CreateBrowserTaskRequest } from '../types.js'
import { terminalStatuses } from '../types.js'
import type { SessionManager, BrowserSessionRecord } from '../sessions/manager.js'
import type { RuntimeMetrics } from '../telemetry/metrics.js'
import type { EventBus, BrowserEvent } from '../events/event-bus.js'
import { inSpan } from '../telemetry/tracing.js'

interface RunningTask { controller: AbortController; tenantId: string; page?: Page; allocation?: AllocatedContext; session?: BrowserSessionRecord }

export class TaskManager {
  private readonly tasks = new Map<string, BrowserTask>()
  private readonly running = new Map<string, RunningTask>()
  private accepting = true
  private readonly cleanupTimer: NodeJS.Timeout

  constructor(
    private readonly config: BrowserRuntimeConfig,
    private readonly pool: BrowserPool,
    private readonly sessions: SessionManager,
    private readonly dispatcher: ActionDispatcher,
    private readonly metrics: RuntimeMetrics,
    private readonly events: EventBus,
  ) {
    this.cleanupTimer = setInterval(() => this.cleanupRetainedTasks(), Math.min(60_000, config.BROWSER_TASK_RETENTION_MS))
    this.cleanupTimer.unref()
  }

  create(request: CreateBrowserTaskRequest) {
    if (!this.accepting) throw new BrowserRuntimeError('BROWSER_RESOURCE_LIMIT', 'Runtime is shutting down', 503)
    if (request.actions.length === 0 || request.actions.length > this.config.BROWSER_MAX_ACTIONS_PER_TASK) {
      throw new BrowserRuntimeError('BROWSER_INVALID_REQUEST', `actions must contain 1-${this.config.BROWSER_MAX_ACTIONS_PER_TASK} entries`)
    }
    const activeForTenant = [...this.running.values()].filter(({ tenantId }) => tenantId === request.tenant_id).length
      + [...this.tasks.values()].filter((task) => task.tenant_id === request.tenant_id && task.status === 'queued').length
    if (activeForTenant >= this.config.BROWSER_TENANT_MAX_RUNNING_TASKS) {
      throw new BrowserRuntimeError('BROWSER_RESOURCE_LIMIT', 'Tenant running task quota reached', 429)
    }
    const now = new Date().toISOString()
    const task: BrowserTask = {
      id: `bt_${ulid()}`,
      tenant_id: request.tenant_id,
      status: 'queued',
      actions: structuredClone(request.actions),
      timeout_ms: Math.min(request.timeout_ms ?? this.config.BROWSER_TASK_TIMEOUT_MS, this.config.BROWSER_TASK_TIMEOUT_MS),
      created_at: now,
      ...(request.workflow_id ? { workflow_id: request.workflow_id } : {}),
      ...(request.execution_id ? { execution_id: request.execution_id } : {}),
      ...(request.node_id ? { node_id: request.node_id } : {}),
      ...(request.session_id ? { session_id: request.session_id } : {}),
    }
    this.tasks.set(task.id, task)
    void this.emit(task, 'queued')
    setImmediate(() => { void this.run(task.id) })
    return structuredClone(task)
  }

  get(id: string, tenantId: string) {
    const task = this.tasks.get(id)
    if (!task || task.tenant_id !== tenantId) throw new BrowserRuntimeError('BROWSER_INVALID_REQUEST', 'Browser task was not found', 404)
    return structuredClone(task)
  }

  async cancel(id: string, tenantId: string) {
    const task = this.tasks.get(id)
    if (!task || task.tenant_id !== tenantId) throw new BrowserRuntimeError('BROWSER_INVALID_REQUEST', 'Browser task was not found', 404)
    if (terminalStatuses.has(task.status)) return structuredClone(task)
    this.running.get(id)?.controller.abort()
    await this.forceCleanup(this.running.get(id))
    this.finish(task, 'cancelled', new BrowserRuntimeError('BROWSER_TASK_CANCELLED', 'Task was cancelled', 409))
    return structuredClone(task)
  }

  async shutdown(graceMs: number) {
    this.accepting = false
    clearInterval(this.cleanupTimer)
    const deadline = Date.now() + graceMs
    while (this.running.size > 0 && Date.now() < deadline) await new Promise((resolve) => setTimeout(resolve, 50))
    await Promise.allSettled([...this.running.keys()].map((id) => this.cancel(id, this.tasks.get(id)!.tenant_id)))
  }

  private async run(id: string) {
    const task = this.tasks.get(id)
    if (!task || terminalStatuses.has(task.status)) return
    const controller = new AbortController()
    const running: RunningTask = { controller, tenantId: task.tenant_id }
    this.running.set(id, running)
    task.status = 'running'
    task.started_at = new Date().toISOString()
    this.metrics.running.inc()
    await this.emit(task, 'started')
    const timeout = setTimeout(() => controller.abort(new BrowserRuntimeError('BROWSER_TASK_TIMEOUT', 'Task timeout elapsed', 408)), task.timeout_ms)
    const started = Date.now()
    try {
      await inSpan('browser.task', spanAttributes(task), async () => {
        if (task.session_id) {
          running.session = this.sessions.get(task.session_id, task.tenant_id)
          running.page = running.session.activePage
        } else {
          running.allocation = await this.pool.allocate(controller.signal)
          running.page = await running.allocation.context.newPage()
          this.metrics.pages.inc()
        }
        const results: BrowserActionResult[] = []
        for (let index = 0; index < task.actions.length; index += 1) {
          if (controller.signal.aborted) throw controller.signal.reason
          const action = task.actions[index]!
          await this.emit(task, 'action_started', action.type)
          const actionStarted = Date.now()
          const startedAt = new Date(actionStarted).toISOString()
          try {
            const timeoutMs = Math.min(action.timeout_ms ?? defaultActionTimeout(action.type, this.config), task.timeout_ms)
            const output = await withTimeout(
              inSpan(`browser.${action.type}`, { ...spanAttributes(task), 'browser.action': action.type, 'browser.url.host': safeHost(running.page.url()) },
                () => this.dispatcher.execute(action, { page: running.page!, context: running.page!.context(), task, signal: controller.signal, ...(running.session ? { session: running.session } : {}) })),
              timeoutMs,
              controller,
            )
            const completedAt = new Date().toISOString()
            results.push({
              ...(action.id ? { action_id: action.id } : {}), type: action.type, success: true, started_at: startedAt, completed_at: completedAt,
              duration_ms: Date.now() - actionStarted,
              ...(output.data !== undefined ? { data: output.data } : {}),
              ...(output.artifacts ? { artifact_ids: output.artifacts.map(({ id: artifactId }) => artifactId) } : {}),
            })
            for (let artifactIndex = 0; artifactIndex < (output.artifacts?.length ?? 0); artifactIndex += 1) await this.emit(task, 'artifact_created', action.type)
            this.metrics.actionDuration.observe({ action: action.type }, (Date.now() - actionStarted) / 1_000)
            await this.emit(task, 'action_completed', action.type)
          } catch (error) {
            const runtimeError = classifyActionError(controller.signal.aborted ? controller.signal.reason : error, action.type)
            results.push({
              ...(action.id ? { action_id: action.id } : {}), type: action.type, success: false, started_at: startedAt,
              completed_at: new Date().toISOString(), duration_ms: Date.now() - actionStarted,
              error: { code: runtimeError.code, message: runtimeError.message },
            })
            task.result = { actions: results, duration_ms: Date.now() - started }
            task.error = { code: runtimeError.code, message: runtimeError.message, action_index: index, action_type: action.type }
            throw runtimeError
          }
        }
        task.result = { actions: results, duration_ms: Date.now() - started, final_url: running.page.url(), title: await running.page.title() }
      })
      this.finish(task, 'completed')
    } catch (error) {
      const runtimeError = error instanceof BrowserRuntimeError ? error : safeError(controller.signal.aborted ? controller.signal.reason : error)
      const status: BrowserTaskStatus = runtimeError.code === 'BROWSER_TASK_CANCELLED' ? 'cancelled' : runtimeError.code === 'BROWSER_TASK_TIMEOUT' ? 'timeout' : 'failed'
      this.finish(task, status, runtimeError)
    } finally {
      clearTimeout(timeout)
      await this.forceCleanup(running, task.status !== ('completed' as BrowserTaskStatus))
      this.running.delete(id)
      this.metrics.running.dec()
      this.metrics.taskDuration.observe((Date.now() - started) / 1_000)
    }
  }

  private finish(task: BrowserTask, status: BrowserTaskStatus, error?: BrowserRuntimeError) {
    if (terminalStatuses.has(task.status)) return
    task.status = status
    task.completed_at = new Date().toISOString()
    if (error && !task.error) task.error = { code: error.code, message: error.message }
    if (error) this.metrics.errors.inc({ code: error.code })
    this.metrics.tasks.inc({ status })
    void this.emit(task, status === 'timeout' ? 'timeout' : status === 'cancelled' ? 'cancelled' : status === 'failed' ? 'failed' : 'completed')
  }

  private async forceCleanup(running?: RunningTask, closeSession = false) {
    if (!running) return
    if (running.session && closeSession) await this.sessions.close(running.session.id, running.session.tenant_id, 'failed').catch(() => undefined)
    if (running.page && !running.session) { await running.page.close().catch(() => undefined); this.metrics.pages.dec() }
    await running.allocation?.release().catch(() => undefined)
    delete running.page
    delete running.allocation
  }

  private emit(task: BrowserTask, event: BrowserEvent['event'], actionType?: string) {
    return this.events.publish({
      event, task_id: task.id, tenant_id: task.tenant_id, occurred_at: new Date().toISOString(),
      ...(task.execution_id ? { execution_id: task.execution_id } : {}), ...(task.node_id ? { node_id: task.node_id } : {}),
      ...(actionType ? { action_type: actionType } : {}),
    }).catch(() => undefined)
  }

  private cleanupRetainedTasks() {
    const expiry = Date.now() - this.config.BROWSER_TASK_RETENTION_MS
    const terminal = [...this.tasks.values()]
      .filter((task) => terminalStatuses.has(task.status))
      .sort((left, right) => Date.parse(left.completed_at ?? left.created_at) - Date.parse(right.completed_at ?? right.created_at))
    const overflow = Math.max(0, this.tasks.size - this.config.BROWSER_MAX_RETAINED_TASKS)
    for (let index = 0; index < terminal.length; index += 1) {
      const task = terminal[index]!
      if (Date.parse(task.completed_at ?? task.created_at) <= expiry || index < overflow) this.tasks.delete(task.id)
    }
  }
}

async function withTimeout<T>(promise: Promise<T>, timeoutMs: number, controller: AbortController): Promise<T> {
  let timer: NodeJS.Timeout | undefined
  try {
    return await Promise.race([
      promise,
      new Promise<never>((_resolve, reject) => { timer = setTimeout(() => { const error = new BrowserRuntimeError('BROWSER_TASK_TIMEOUT', 'Action timeout elapsed', 408); controller.abort(error); reject(error) }, timeoutMs) }),
    ])
  } finally { if (timer) clearTimeout(timer) }
}
function defaultActionTimeout(type: string, config: BrowserRuntimeConfig) { return type === 'navigate' ? config.BROWSER_NAVIGATION_TIMEOUT_MS : config.BROWSER_ACTION_TIMEOUT_MS }
function spanAttributes(task: BrowserTask) { return { 'tenant.id': task.tenant_id, 'workflow.id': task.workflow_id, 'execution.id': task.execution_id, 'node.id': task.node_id, 'browser.task.id': task.id, 'browser.session.id': task.session_id } }
function safeHost(raw: string) { try { return new URL(raw).hostname } catch { return '' } }

function classifyActionError(error: unknown, actionType: string) {
  const runtimeError = safeError(error)
  if (runtimeError.code !== 'BROWSER_INTERNAL_ERROR') return runtimeError
  const message = error instanceof Error ? error.message : ''
  if (/browser.*(closed|disconnected)|target page.*closed/i.test(message)) {
    return new BrowserRuntimeError('BROWSER_BROWSER_CRASHED', 'Browser process became unavailable', 503)
  }
  if (actionType === 'navigate') {
    return new BrowserRuntimeError('BROWSER_NAVIGATION_FAILED', 'Navigation failed')
  }
  if (['click', 'input', 'wait', 'extract'].includes(actionType) && /locator|selector|element/i.test(message)) {
    return new BrowserRuntimeError('BROWSER_SELECTOR_NOT_FOUND', 'Selector did not resolve to an actionable element', 404)
  }
  return runtimeError
}
