import { chromium, type Browser, type BrowserContext } from 'playwright'
import type { BrowserRuntimeConfig } from '../config.js'
import { BrowserRuntimeError } from '../errors.js'
import { Semaphore } from '../semaphore.js'
import type { RuntimeMetrics } from '../telemetry/metrics.js'

interface Slot { browser: Browser; contexts: number; replacing: boolean }

export interface AllocatedContext {
  context: BrowserContext
  release(): Promise<void>
}

export class BrowserPool {
  private readonly slots: Slot[] = []
  private readonly semaphore: Semaphore
  private accepting = false

  constructor(
    private readonly config: BrowserRuntimeConfig,
    private readonly proxyAddress: string,
    private readonly metrics: RuntimeMetrics,
  ) {
    this.semaphore = new Semaphore(config.capacity, config.BROWSER_QUEUE_CAPACITY)
    metrics.poolSize.set(config.BROWSER_POOL_SIZE)
    metrics.poolAvailable.set(config.capacity)
  }

  get state() {
    return {
      size: this.slots.filter(({ browser }) => browser.isConnected()).length,
      available: this.semaphore.available,
      active_contexts: this.semaphore.active,
      queue_depth: this.semaphore.queued,
      ready: this.accepting && this.slots.some(({ browser }) => browser.isConnected()),
    }
  }

  async start() {
    for (let index = 0; index < this.config.BROWSER_POOL_SIZE; index += 1) {
      this.slots.push(await this.launchSlot())
    }
    this.accepting = true
  }

  async allocate(signal?: AbortSignal): Promise<AllocatedContext> {
    if (!this.accepting) throw new BrowserRuntimeError('BROWSER_BROWSER_CRASHED', 'Browser pool is not accepting work', 503)
    const releasePermit = await this.semaphore.acquire(signal)
    this.refreshMetrics()
    const slot = this.slots
      .filter(({ browser, contexts }) => browser.isConnected() && contexts < this.config.BROWSER_MAX_CONTEXTS_PER_BROWSER)
      .sort((left, right) => left.contexts - right.contexts)[0]
    if (!slot) {
      releasePermit()
      this.refreshMetrics()
      throw new BrowserRuntimeError('BROWSER_BROWSER_CRASHED', 'No healthy browser process is available', 503)
    }
    slot.contexts += 1
    try {
      const context = await slot.browser.newContext({
        proxy: { server: this.proxyAddress },
        acceptDownloads: true,
        serviceWorkers: 'block',
      })
      this.metrics.contexts.inc()
      let released = false
      return {
        context,
        release: async () => {
          if (released) return
          released = true
          await context.close().catch(() => undefined)
          slot.contexts = Math.max(0, slot.contexts - 1)
          releasePermit()
          this.metrics.contexts.dec()
          this.refreshMetrics()
        },
      }
    } catch (error) {
      slot.contexts = Math.max(0, slot.contexts - 1)
      releasePermit()
      this.refreshMetrics()
      throw error
    }
  }

  async close() {
    this.accepting = false
    this.semaphore.close()
    await Promise.allSettled(this.slots.map(({ browser }) => browser.close()))
    this.slots.length = 0
    this.refreshMetrics()
  }

  private async launchSlot(): Promise<Slot> {
    const slot: Slot = { browser: await this.launchBrowser(), contexts: 0, replacing: false }
    this.watch(slot)
    return slot
  }

  private launchBrowser() {
    return chromium.launch({
      headless: this.config.BROWSER_CHROMIUM_HEADLESS,
      ...(this.config.BROWSER_CHROMIUM_EXECUTABLE_PATH ? { executablePath: this.config.BROWSER_CHROMIUM_EXECUTABLE_PATH } : {}),
      args: ['--disable-dev-shm-usage=false', '--disable-background-networking', '--disable-component-update', '--no-default-browser-check'],
    })
  }

  private watch(slot: Slot) {
    slot.browser.on('disconnected', () => { void this.replace(slot) })
  }

  private async replace(slot: Slot) {
    if (!this.accepting || slot.replacing) return
    slot.replacing = true
    this.metrics.crashes.inc()
    let delayMs = 250
    while (this.accepting) {
      try {
        slot.browser = await this.launchBrowser()
        slot.contexts = 0
        this.watch(slot)
        this.metrics.restarts.inc()
        break
      } catch {
        await new Promise((resolve) => setTimeout(resolve, delayMs))
        delayMs = Math.min(delayMs * 2, 5_000)
      }
    }
    slot.replacing = false
    this.refreshMetrics()
  }

  private refreshMetrics() {
    this.metrics.poolAvailable.set(this.semaphore.available)
    this.metrics.queueDepth.set(this.semaphore.queued)
  }
}
