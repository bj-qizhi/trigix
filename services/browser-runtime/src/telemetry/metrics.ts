import { Counter, Gauge, Histogram, Registry, collectDefaultMetrics } from 'prom-client'

export class RuntimeMetrics {
  readonly registry = new Registry()
  readonly tasks = new Counter({ name: 'browser_tasks_total', help: 'Browser tasks by terminal outcome', labelNames: ['status'], registers: [this.registry] })
  readonly errors = new Counter({ name: 'browser_errors_total', help: 'Browser runtime errors by stable code', labelNames: ['code'], registers: [this.registry] })
  readonly running = new Gauge({ name: 'browser_tasks_running', help: 'Currently running browser tasks', registers: [this.registry] })
  readonly queueDepth = new Gauge({ name: 'browser_queue_depth', help: 'Tasks waiting for browser capacity', registers: [this.registry] })
  readonly taskDuration = new Histogram({ name: 'browser_task_duration_seconds', help: 'Browser task duration', buckets: [0.1, 0.5, 1, 2, 5, 10, 30, 60], registers: [this.registry] })
  readonly actionDuration = new Histogram({ name: 'browser_action_duration_seconds', help: 'Browser action duration', labelNames: ['action'], buckets: [0.01, 0.1, 0.5, 1, 5, 15, 30], registers: [this.registry] })
  readonly poolSize = new Gauge({ name: 'browser_pool_size', help: 'Configured browser process count', registers: [this.registry] })
  readonly poolAvailable = new Gauge({ name: 'browser_pool_available', help: 'Available browser context capacity', registers: [this.registry] })
  readonly contexts = new Gauge({ name: 'browser_contexts_active', help: 'Active browser contexts', registers: [this.registry] })
  readonly pages = new Gauge({ name: 'browser_pages_active', help: 'Active browser pages', registers: [this.registry] })
  readonly sessions = new Gauge({ name: 'browser_sessions_active', help: 'Active browser sessions', registers: [this.registry] })
  readonly crashes = new Counter({ name: 'browser_crashes_total', help: 'Browser process disconnects', registers: [this.registry] })
  readonly restarts = new Counter({ name: 'browser_restarts_total', help: 'Browser process replacements', registers: [this.registry] })
  readonly artifacts = new Counter({ name: 'browser_artifacts_total', help: 'Browser artifacts by type', labelNames: ['type'], registers: [this.registry] })
  readonly artifactBytes = new Counter({ name: 'browser_artifact_bytes_total', help: 'Browser artifact bytes', labelNames: ['type'], registers: [this.registry] })

  constructor(enabled: boolean) {
    if (enabled) collectDefaultMetrics({ register: this.registry, prefix: 'browser_runtime_' })
  }
}
