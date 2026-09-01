import { createClient, type RedisClientType } from 'redis'
import type { BrowserRuntimeConfig } from '../config.js'

export interface BrowserEvent {
  event: 'queued' | 'started' | 'action_started' | 'action_completed' | 'artifact_created' | 'completed' | 'failed' | 'timeout' | 'cancelled'
  task_id: string
  tenant_id: string
  execution_id?: string
  node_id?: string
  action_type?: string
  occurred_at: string
}

export interface EventBus { start(): Promise<void>; publish(event: BrowserEvent): Promise<void>; close(): Promise<void> }

class InMemoryEventBus implements EventBus {
  async start() {}
  async publish() {}
  async close() {}
}

class RedisStreamEventBus implements EventBus {
  private client?: RedisClientType
  constructor(private readonly url: string) {}
  async start() { this.client = createClient({ url: this.url }); await this.client.connect() }
  async publish(event: BrowserEvent) {
    await this.client?.xAdd('browser.events', '*', {
      event: event.event,
      task_id: event.task_id,
      tenant_id: event.tenant_id,
      occurred_at: event.occurred_at,
      ...(event.execution_id ? { execution_id: event.execution_id } : {}),
      ...(event.node_id ? { node_id: event.node_id } : {}),
      ...(event.action_type ? { action_type: event.action_type } : {}),
    }, { TRIM: { strategy: 'MAXLEN', strategyModifier: '~', threshold: 100_000 } })
  }
  async close() { if (this.client?.isOpen) await this.client.quit() }
}

export function createEventBus(config: BrowserRuntimeConfig): EventBus {
  return config.REDIS_URL ? new RedisStreamEventBus(config.REDIS_URL) : new InMemoryEventBus()
}
