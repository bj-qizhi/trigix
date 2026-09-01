import type { BrowserErrorCode } from './types.js'

export class BrowserRuntimeError extends Error {
  constructor(
    public readonly code: BrowserErrorCode,
    message: string,
    public readonly httpStatus = 400,
  ) {
    super(message)
    this.name = 'BrowserRuntimeError'
  }
}

export function safeError(error: unknown): BrowserRuntimeError {
  if (error instanceof BrowserRuntimeError) return error
  const message = error instanceof Error ? error.message : 'Unexpected browser runtime failure'
  if (/abort/i.test(message)) return new BrowserRuntimeError('BROWSER_TASK_CANCELLED', 'Task was cancelled', 409)
  if (/timeout/i.test(message)) return new BrowserRuntimeError('BROWSER_TASK_TIMEOUT', 'Browser operation timed out', 408)
  return new BrowserRuntimeError('BROWSER_INTERNAL_ERROR', 'Browser operation failed', 500)
}
