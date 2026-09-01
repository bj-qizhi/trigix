import { z } from 'zod'

const bool = z.enum(['true', 'false']).transform((value) => value === 'true')
const positiveInt = (fallback: number) => z.coerce.number().int().positive().default(fallback)
const nonnegativeInt = (fallback: number) => z.coerce.number().int().nonnegative().default(fallback)
const optionalString = z.preprocess((value) => value === '' ? undefined : value, z.string().optional())
const optionalUrl = z.preprocess((value) => value === '' ? undefined : value, z.string().url().optional())

const schema = z.object({
  NODE_ENV: z.enum(['development', 'test', 'production']).default('development'),
  BROWSER_RUNTIME_HOST: z.string().default('0.0.0.0'),
  BROWSER_RUNTIME_PORT: positiveInt(38100),
  BROWSER_RUNTIME_AUTH_TOKEN: z.string().min(32).optional(),
  BROWSER_POOL_SIZE: positiveInt(3),
  BROWSER_MAX_CONTEXTS_PER_BROWSER: positiveInt(10),
  BROWSER_MAX_PAGES_PER_CONTEXT: positiveInt(3),
  BROWSER_TASK_TIMEOUT_MS: positiveInt(60_000),
  BROWSER_TASK_RETENTION_MS: positiveInt(3_600_000),
  BROWSER_MAX_RETAINED_TASKS: positiveInt(10_000),
  BROWSER_ACTION_TIMEOUT_MS: positiveInt(10_000),
  BROWSER_NAVIGATION_TIMEOUT_MS: positiveInt(15_000),
  BROWSER_IDLE_SESSION_TIMEOUT_MS: positiveInt(300_000),
  BROWSER_SHUTDOWN_GRACE_MS: positiveInt(30_000),
  BROWSER_ARTIFACT_PROVIDER: z.enum(['local', 's3']).default('local'),
  BROWSER_ARTIFACT_DIR: z.string().default('/data/browser'),
  BROWSER_ARTIFACT_BUCKET: optionalString,
  BROWSER_ARTIFACT_ENDPOINT: optionalUrl,
  BROWSER_ARTIFACT_REGION: z.string().default('us-east-1'),
  BROWSER_BLOCK_PRIVATE_NETWORK: bool.default(true),
  BROWSER_ALLOWED_HOSTS: z.string().default(''),
  BROWSER_ENABLE_EVALUATE: bool.default(false),
  BROWSER_TENANT_MAX_RUNNING_TASKS: positiveInt(10),
  BROWSER_TENANT_MAX_SESSIONS: positiveInt(10),
  BROWSER_MAX_DOWNLOAD_BYTES: positiveInt(52_428_800),
  BROWSER_MAX_ARTIFACT_BYTES: positiveInt(52_428_800),
  BROWSER_MAX_ACTIONS_PER_TASK: positiveInt(100),
  BROWSER_QUEUE_CAPACITY: positiveInt(1_000),
  BROWSER_CHROMIUM_HEADLESS: bool.default(true),
  BROWSER_CHROMIUM_EXECUTABLE_PATH: optionalString,
  REDIS_URL: optionalString,
  OTEL_EXPORTER_OTLP_ENDPOINT: optionalUrl,
  PROMETHEUS_ENABLED: bool.default(true),
  LOG_LEVEL: z.enum(['fatal', 'error', 'warn', 'info', 'debug', 'trace', 'silent']).default('info'),
  BROWSER_PROXY_PORT: nonnegativeInt(0),
}).superRefine((value, ctx) => {
  if (value.NODE_ENV === 'production' && !value.BROWSER_RUNTIME_AUTH_TOKEN) {
    ctx.addIssue({ code: 'custom', path: ['BROWSER_RUNTIME_AUTH_TOKEN'], message: 'required in production' })
  }
  if (value.BROWSER_ARTIFACT_PROVIDER === 's3' && !value.BROWSER_ARTIFACT_BUCKET) {
    ctx.addIssue({ code: 'custom', path: ['BROWSER_ARTIFACT_BUCKET'], message: 'required for S3 artifacts' })
  }
})

export type BrowserRuntimeConfig = ReturnType<typeof loadConfig>

export function loadConfig(env: NodeJS.ProcessEnv = process.env) {
  const value = schema.parse(env)
  return {
    ...value,
    allowedHosts: value.BROWSER_ALLOWED_HOSTS.split(',').map((host) => host.trim().toLowerCase()).filter(Boolean),
    capacity: value.BROWSER_POOL_SIZE * value.BROWSER_MAX_CONTEXTS_PER_BROWSER,
  }
}
