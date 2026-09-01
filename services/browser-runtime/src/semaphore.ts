import { BrowserRuntimeError } from './errors.js'

interface Waiter { resolve: () => void; reject: (error: Error) => void; signal?: AbortSignal }

export class Semaphore {
  private activeCount = 0
  private readonly waiters: Waiter[] = []

  constructor(private readonly max: number, private readonly maxQueue: number) {}

  get active() { return this.activeCount }
  get available() { return Math.max(0, this.max - this.activeCount) }
  get queued() { return this.waiters.length }

  async acquire(signal?: AbortSignal): Promise<() => void> {
    if (signal?.aborted) throw new BrowserRuntimeError('BROWSER_TASK_CANCELLED', 'Allocation cancelled', 409)
    if (this.activeCount < this.max) {
      this.activeCount += 1
      return this.releaseOnce()
    }
    if (this.waiters.length >= this.maxQueue) {
      throw new BrowserRuntimeError('BROWSER_RESOURCE_LIMIT', 'Browser queue capacity reached', 429)
    }
    await new Promise<void>((resolve, reject) => {
      const waiter: Waiter = { resolve, reject }
      if (signal) waiter.signal = signal
      this.waiters.push(waiter)
      signal?.addEventListener('abort', () => {
        const index = this.waiters.indexOf(waiter)
        if (index >= 0) this.waiters.splice(index, 1)
        reject(new BrowserRuntimeError('BROWSER_TASK_CANCELLED', 'Allocation cancelled', 409))
      }, { once: true })
    })
    this.activeCount += 1
    return this.releaseOnce()
  }

  close() {
    const error = new BrowserRuntimeError('BROWSER_BROWSER_CRASHED', 'Browser pool is closing', 503)
    for (const waiter of this.waiters.splice(0)) waiter.reject(error)
  }

  private releaseOnce() {
    let released = false
    return () => {
      if (released) return
      released = true
      this.activeCount = Math.max(0, this.activeCount - 1)
      this.waiters.shift()?.resolve()
    }
  }
}
