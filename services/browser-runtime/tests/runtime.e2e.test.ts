import { createServer } from 'node:http'
import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import type { InjectOptions, Response } from 'light-my-request'
import { afterAll, beforeAll, describe, expect, it } from 'vitest'
import { loadConfig } from '../src/config.js'
import { createRuntime } from '../src/runtime.js'
import type { BrowserTask } from '../src/types.js'

const tenant = 'tenant-e2e'
const token = 'browser-runtime-e2e-service-token'
let fixtureUrl = ''
let artifactDirectory = ''
let runtime: Awaited<ReturnType<typeof createRuntime>>
let closeFixture: () => Promise<void>

describe('Browser Runtime', () => {
  beforeAll(async () => {
    artifactDirectory = await mkdtemp(path.join(tmpdir(), 'trigix-browser-e2e-'))
    const fixture = createServer((request, response) => {
      if (request.url === '/slow') {
        setTimeout(() => {
          response.writeHead(200, { 'content-type': 'text/html; charset=utf-8' })
          response.end('<main>slow</main>')
        }, 5_000)
        return
      }
      response.writeHead(200, { 'content-type': 'text/html; charset=utf-8' })
      response.end('<main><h1>Browser Runtime E2E</h1><input id="name"><button id="save" onclick="document.body.dataset.saved=\'yes\'">Save</button></main>')
    })
    await new Promise<void>((resolve) => fixture.listen(0, '127.0.0.1', resolve))
    const address = fixture.address()
    if (!address || typeof address === 'string') throw new Error('Fixture did not bind a TCP port')
    fixtureUrl = `http://127.0.0.1:${address.port}`
    closeFixture = () => new Promise<void>((resolve, reject) => fixture.close((error) => error ? reject(error) : resolve()))

    const config = loadConfig({
      NODE_ENV: 'test',
      BROWSER_RUNTIME_AUTH_TOKEN: token,
      BROWSER_POOL_SIZE: '1',
      BROWSER_MAX_CONTEXTS_PER_BROWSER: '2',
      BROWSER_MAX_PAGES_PER_CONTEXT: '2',
      BROWSER_ALLOWED_HOSTS: '127.0.0.1',
      BROWSER_ARTIFACT_DIR: artifactDirectory,
      BROWSER_TASK_TIMEOUT_MS: '10000',
      BROWSER_ACTION_TIMEOUT_MS: '5000',
      BROWSER_NAVIGATION_TIMEOUT_MS: '5000',
      BROWSER_IDLE_SESSION_TIMEOUT_MS: '30000',
      BROWSER_SHUTDOWN_GRACE_MS: '5000',
      PROMETHEUS_ENABLED: 'true',
      LOG_LEVEL: 'silent',
    })
    runtime = await createRuntime(config)
  }, 60_000)

  afterAll(async () => {
    await runtime?.close()
    await closeFixture?.()
    if (artifactDirectory) await rm(artifactDirectory, { recursive: true, force: true })
  })

  it('executes a tenant-bound session workflow and stores artifacts', async () => {
    expect((await runtime.app.inject({ method: 'GET', url: '/healthz' })).statusCode).toBe(200)
    const sessionResponse = await request('POST', '/v1/sessions', { tenant_id: tenant, execution_id: 'execution-e2e' })
    expect(sessionResponse.statusCode).toBe(201)
    const sessionId = sessionResponse.json<{ id: string }>().id

    const created = await request('POST', '/v1/tasks', {
      tenant_id: tenant,
      execution_id: 'execution-e2e',
      session_id: sessionId,
      actions: [
        { type: 'navigate', params: { url: fixtureUrl } },
        { type: 'input', params: { selector: '#name', value: 'Trigix' } },
        { type: 'click', params: { selector: '#save' } },
        { type: 'extract', params: { selector: 'h1', mode: 'text' } },
        { type: 'screenshot', params: { full_page: true } },
      ],
    })
    expect(created.statusCode).toBe(202)
    const task = await waitForTask(created.json<{ task_id: string }>().task_id)
    expect(task.status).toBe('completed')
    expect(task.result?.actions[3]?.data).toEqual({ data: 'Browser Runtime E2E', count: 1 })
    const artifactId = task.result?.actions[4]?.artifact_ids?.[0]
    expect(artifactId).toBeTruthy()

    const artifact = await request('GET', `/v1/artifacts/${artifactId}`)
    expect(artifact.statusCode).toBe(200)
    expect(artifact.headers['content-type']).toContain('image/png')
    const crossTenant = await runtime.app.inject({
      method: 'GET', url: `/v1/artifacts/${artifactId}`,
      headers: { authorization: `Bearer ${token}`, 'x-trigix-tenant-id': 'other-tenant' },
    })
    expect(crossTenant.statusCode).toBe(404)
    expect((await request('DELETE', `/v1/sessions/${sessionId}`)).statusCode).toBe(200)
  }, 30_000)

  it('blocks private destinations unless explicitly allowlisted', async () => {
    const created = await request('POST', '/v1/tasks', {
      tenant_id: tenant,
      actions: [{ type: 'navigate', params: { url: 'http://169.254.169.254/latest/meta-data/' } }],
    })
    const task = await waitForTask(created.json<{ task_id: string }>().task_id)
    expect(task.status).toBe('failed')
    expect(task.error?.code).toBe('BROWSER_URL_BLOCKED')
  })

  it('cancels an in-flight task', async () => {
    const created = await request('POST', '/v1/tasks', {
      tenant_id: tenant,
      actions: [{ type: 'wait', params: { milliseconds: 5_000 } }],
    })
    const taskId = created.json<{ task_id: string }>().task_id
    await new Promise((resolve) => setTimeout(resolve, 100))
    const cancelled = await request('DELETE', `/v1/tasks/${taskId}`)
    expect(cancelled.statusCode).toBe(200)
    expect((await waitForTask(taskId)).status).toBe('cancelled')
  })

  it('times out an in-flight task and keeps the terminal state immutable', async () => {
    const created = await request('POST', '/v1/tasks', {
      tenant_id: tenant,
      timeout_ms: 100,
      actions: [{ type: 'wait', params: { milliseconds: 5_000 } }],
    })
    const taskId = created.json<{ task_id: string }>().task_id
    const timedOut = await waitForTask(taskId)
    expect(timedOut.status).toBe('timeout')
    expect(timedOut.error?.code).toBe('BROWSER_TASK_TIMEOUT')
    expect((await request('DELETE', `/v1/tasks/${taskId}`)).json<BrowserTask>().status).toBe('timeout')
  })

  it('reuses cookies across tasks in the same tenant session', async () => {
    const sessionResponse = await request('POST', '/v1/sessions', {
      tenant_id: tenant,
      execution_id: 'execution-cookie-e2e',
    })
    const sessionId = sessionResponse.json<{ id: string }>().id
    const setCookie = await request('POST', '/v1/tasks', {
      tenant_id: tenant,
      session_id: sessionId,
      actions: [{
        type: 'cookies',
        params: {
          operation: 'set',
          cookies: [{ name: 'runtime-session', value: 'active', url: fixtureUrl }],
        },
      }],
    })
    expect((await waitForTask(setCookie.json<{ task_id: string }>().task_id)).status).toBe('completed')

    const getCookie = await request('POST', '/v1/tasks', {
      tenant_id: tenant,
      session_id: sessionId,
      actions: [{ type: 'cookies', params: { operation: 'get' } }],
    })
    const task = await waitForTask(getCookie.json<{ task_id: string }>().task_id)
    expect(task.result?.actions[0]?.data).toEqual(expect.arrayContaining([
      expect.objectContaining({ name: 'runtime-session' }),
    ]))
    expect((await request('DELETE', `/v1/sessions/${sessionId}`)).statusCode).toBe(200)
  })
})

async function request(method: 'GET' | 'POST' | 'DELETE', url: string, payload?: unknown): Promise<Response> {
  const options: InjectOptions = {
    method,
    url,
    headers: {
      authorization: `Bearer ${token}`,
      'x-trigix-tenant-id': tenant,
      ...(payload === undefined ? {} : { 'content-type': 'application/json' }),
    },
  }
  if (payload !== undefined) options.payload = JSON.stringify(payload)
  return runtime.app.inject(options)
}

async function waitForTask(taskId: string): Promise<BrowserTask> {
  const deadline = Date.now() + 15_000
  while (Date.now() < deadline) {
    const response = await request('GET', `/v1/tasks/${taskId}`)
    const task = response.json<BrowserTask>()
    if (['completed', 'failed', 'timeout', 'cancelled'].includes(task.status)) return task
    await new Promise((resolve) => setTimeout(resolve, 25))
  }
  throw new Error(`Task ${taskId} did not reach a terminal state`)
}
