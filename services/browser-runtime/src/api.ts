import Fastify, { type FastifyInstance } from 'fastify'
import { timingSafeEqual } from 'node:crypto'
import { z } from 'zod'
import { actionTypes, type CreateBrowserTaskRequest } from './types.js'
import type { BrowserRuntimeConfig } from './config.js'
import { BrowserRuntimeError, safeError } from './errors.js'
import type { BrowserPool } from './browser/pool.js'
import type { TaskManager } from './tasks/manager.js'
import type { SessionManager } from './sessions/manager.js'
import type { ArtifactManager } from './artifacts/manager.js'
import type { RuntimeMetrics } from './telemetry/metrics.js'

const identifier = z.string().min(1).max(128).regex(/^[A-Za-z0-9._:-]+$/)
const actionSchema = z.object({ id: identifier.optional(), type: z.enum(actionTypes), params: z.record(z.string(), z.unknown()), timeout_ms: z.number().int().positive().optional() }).strict()
const taskSchema = z.object({
  tenant_id: identifier, workflow_id: identifier.optional(), execution_id: identifier.optional(), node_id: identifier.optional(),
  session_id: identifier.optional(), timeout_ms: z.number().int().positive().optional(), actions: z.array(actionSchema).min(1),
}).strict()
const sessionSchema = z.object({ tenant_id: identifier, execution_id: identifier.optional() }).strict()

interface Dependencies { config: BrowserRuntimeConfig; pool: BrowserPool; tasks: TaskManager; sessions: SessionManager; artifacts: ArtifactManager; metrics: RuntimeMetrics }

export function buildApi(deps: Dependencies): FastifyInstance {
  const app = Fastify({ logger: { level: deps.config.LOG_LEVEL, redact: { paths: ['req.headers.authorization', 'req.headers.cookie', 'req.body.password', 'req.body.token', 'req.body.secret', 'req.body.api_key', '*.params.value', '*.params.content_base64'], censor: '[REDACTED]' } }, bodyLimit: 2 * 1024 * 1024 })

  app.setErrorHandler((error, _request, reply) => {
    const runtimeError = error instanceof z.ZodError
      ? new BrowserRuntimeError('BROWSER_INVALID_REQUEST', error.issues.map(({ path, message }) => `${path.join('.')}: ${message}`).join('; '))
      : safeError(error)
    void reply.status(runtimeError.httpStatus).send({ error: { code: runtimeError.code, message: runtimeError.message } })
  })

  app.get('/health', async () => ({ status: 'ok' }))
  app.get('/healthz', async () => ({ status: 'ok' }))
  app.get('/ready', async (_request, reply) => {
    const state = deps.pool.state
    return reply.status(state.ready ? 200 : 503).send({ ready: state.ready, browser_pool: state })
  })
  app.get('/readyz', async (_request, reply) => {
    const state = deps.pool.state
    return reply.status(state.ready ? 200 : 503).send({ ready: state.ready, browser_pool: state })
  })
  app.get('/metrics', async (_request, reply) => {
    if (!deps.config.PROMETHEUS_ENABLED) return reply.status(404).send()
    return reply.header('content-type', deps.metrics.registry.contentType).send(await deps.metrics.registry.metrics())
  })

  app.addHook('onRequest', async (request) => {
    if (request.url === '/health' || request.url === '/healthz' || request.url === '/ready' || request.url === '/readyz' || request.url === '/metrics') return
    if (deps.config.BROWSER_RUNTIME_AUTH_TOKEN && !validBearer(request.headers.authorization, deps.config.BROWSER_RUNTIME_AUTH_TOKEN)) {
      throw new BrowserRuntimeError('BROWSER_UNAUTHORIZED', 'Service authentication failed', 401)
    }
  })

  app.post('/v1/tasks', async (request, reply) => {
    const body = taskSchema.parse(request.body) as CreateBrowserTaskRequest
    assertTenant(request.headers['x-trigix-tenant-id'], body.tenant_id)
    const task = deps.tasks.create(body)
    return reply.status(202).send({ task_id: task.id, status: task.status })
  })
  app.get<{ Params: { taskId: string } }>('/v1/tasks/:taskId', async (request) => deps.tasks.get(request.params.taskId, tenantHeader(request.headers['x-trigix-tenant-id'])))
  app.delete<{ Params: { taskId: string } }>('/v1/tasks/:taskId', async (request) => deps.tasks.cancel(request.params.taskId, tenantHeader(request.headers['x-trigix-tenant-id'])))

  app.post('/v1/sessions', async (request, reply) => {
    const body = sessionSchema.parse(request.body)
    assertTenant(request.headers['x-trigix-tenant-id'], body.tenant_id)
    const controller = new AbortController()
    const abort = () => controller.abort(new BrowserRuntimeError('BROWSER_TASK_CANCELLED', 'Session request was cancelled', 409))
    request.raw.once('aborted', abort)
    try {
      return reply.status(201).send(await deps.sessions.create(body.tenant_id, body.execution_id, controller.signal))
    } finally {
      request.raw.removeListener('aborted', abort)
    }
  })
  app.get<{ Params: { sessionId: string } }>('/v1/sessions/:sessionId', async (request) => deps.sessions.inspect(request.params.sessionId, tenantHeader(request.headers['x-trigix-tenant-id'])))
  app.delete<{ Params: { sessionId: string } }>('/v1/sessions/:sessionId', async (request) => deps.sessions.close(request.params.sessionId, tenantHeader(request.headers['x-trigix-tenant-id'])))

  app.get<{ Params: { artifactId: string } }>('/v1/artifacts/:artifactId', async (request, reply) => {
    const { artifact, body } = await deps.artifacts.read(request.params.artifactId, tenantHeader(request.headers['x-trigix-tenant-id']))
    return reply.header('content-type', artifact.content_type).header('content-length', String(artifact.size)).send(body)
  })
  app.get<{ Params: { artifactId: string } }>('/v1/artifacts/:artifactId/metadata', async (request) => deps.artifacts.getMetadata(request.params.artifactId, tenantHeader(request.headers['x-trigix-tenant-id'])))
  return app
}

function tenantHeader(value: string | string[] | undefined) {
  if (typeof value !== 'string' || !value) throw new BrowserRuntimeError('BROWSER_UNAUTHORIZED', 'Tenant header is required', 401)
  return value
}
function assertTenant(header: string | string[] | undefined, bodyTenant: string) {
  if (tenantHeader(header) !== bodyTenant) throw new BrowserRuntimeError('BROWSER_UNAUTHORIZED', 'Tenant header does not match request', 403)
}
function validBearer(header: string | undefined, expectedToken: string) {
  const provided = Buffer.from(header ?? '')
  const expected = Buffer.from(`Bearer ${expectedToken}`)
  return provided.length === expected.length && timingSafeEqual(provided, expected)
}
