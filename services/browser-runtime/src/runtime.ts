import { loadConfig, type BrowserRuntimeConfig } from './config.js'
import { RuntimeMetrics } from './telemetry/metrics.js'
import { startTelemetry, stopTelemetry } from './telemetry/tracing.js'
import { UrlPolicy } from './security/url-policy.js'
import { SecureBrowserProxy } from './security/secure-proxy.js'
import { BrowserPool } from './browser/pool.js'
import { SessionManager } from './sessions/manager.js'
import { createArtifactStore } from './artifacts/store.js'
import { ArtifactManager } from './artifacts/manager.js'
import { ActionDispatcher } from './actions/dispatcher.js'
import { createEventBus } from './events/event-bus.js'
import { TaskManager } from './tasks/manager.js'
import { buildApi } from './api.js'

export async function createRuntime(config: BrowserRuntimeConfig = loadConfig()) {
  await startTelemetry(config.OTEL_EXPORTER_OTLP_ENDPOINT)
  const metrics = new RuntimeMetrics(config.PROMETHEUS_ENABLED)
  const policy = new UrlPolicy(config.BROWSER_BLOCK_PRIVATE_NETWORK, config.allowedHosts)
  const proxy = new SecureBrowserProxy(policy, config.BROWSER_PROXY_PORT)
  await proxy.start()
  const pool = new BrowserPool(config, proxy.address, metrics)
  await pool.start()
  const sessions = new SessionManager(pool, config, metrics)
  sessions.start()
  const artifacts = new ArtifactManager(createArtifactStore(config, metrics))
  const events = createEventBus(config)
  await events.start()
  const dispatcher = new ActionDispatcher(config, policy, artifacts, sessions)
  const tasks = new TaskManager(config, pool, sessions, dispatcher, metrics, events)
  const app = buildApi({ config, pool, sessions, artifacts, tasks, metrics })
  let stopped = false
  return {
    config, app, pool, sessions, tasks,
    async close() {
      if (stopped) return
      stopped = true
      await app.close()
      await tasks.shutdown(config.BROWSER_SHUTDOWN_GRACE_MS)
      await sessions.shutdown()
      await pool.close()
      await proxy.close()
      await events.close()
      await stopTelemetry()
    },
  }
}
