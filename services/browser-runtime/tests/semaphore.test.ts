import { describe, expect, it } from 'vitest'
import { Semaphore } from '../src/semaphore.js'

describe('Semaphore', () => {
  it('bounds active work and wakes queued work', async () => {
    const semaphore = new Semaphore(1, 2)
    const releaseFirst = await semaphore.acquire()
    let acquired = false
    const second = semaphore.acquire().then((release) => { acquired = true; return release })
    await Promise.resolve()
    expect(acquired).toBe(false)
    expect(semaphore.queued).toBe(1)
    releaseFirst()
    const releaseSecond = await second
    expect(acquired).toBe(true)
    releaseSecond()
    expect(semaphore.active).toBe(0)
  })

  it('removes an aborted waiter', async () => {
    const semaphore = new Semaphore(1, 1)
    const release = await semaphore.acquire()
    const controller = new AbortController()
    const waiting = semaphore.acquire(controller.signal)
    controller.abort()
    await expect(waiting).rejects.toMatchObject({ code: 'BROWSER_TASK_CANCELLED' })
    release()
  })
})
