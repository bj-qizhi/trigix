// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { describe, it, expect } from 'vitest'
import { friendlyError } from './errorMessage'

describe('friendlyError', () => {
  it('maps common HTTP status codes to plain language', () => {
    expect(friendlyError(new Error('401 Unauthorized: '), false)).toMatch(/sign in again/)
    expect(friendlyError(new Error('403 Forbidden: '), false)).toMatch(/permission/)
    expect(friendlyError(new Error('404 Not Found: '), false)).toMatch(/not found/i)
    expect(friendlyError(new Error('500 Internal Server Error: oops'), false)).toMatch(/Server error/)
    expect(friendlyError(new Error('429 Too Many Requests: '), false)).toMatch(/Too many requests/)
  })

  it('localizes to Chinese', () => {
    expect(friendlyError(new Error('403 Forbidden: '), true)).toBe('您没有权限执行此操作。')
    expect(friendlyError(new Error('500 x: '), true)).toBe('服务器内部错误，请稍后重试。')
  })

  it('appends the server body for validation-style codes', () => {
    expect(friendlyError(new Error('422 Unprocessable Entity: name is required'), false))
      .toBe('The submitted data is invalid — please review and retry.（name is required）')
    // ...but not for a 403, where the body is noise.
    expect(friendlyError(new Error('403 Forbidden: tenant mismatch'), false)).toBe("You don't have permission to do this.")
  })

  it('detects network failures', () => {
    expect(friendlyError(new TypeError('Failed to fetch'), false)).toMatch(/Network error/)
    expect(friendlyError(new TypeError('Failed to fetch'), true)).toMatch(/网络连接失败/)
  })

  it('falls back to a cleaned message for unknown shapes', () => {
    expect(friendlyError(new Error('something odd happened'), false)).toBe('something odd happened')
    expect(friendlyError('plain string', false)).toBe('plain string')
  })

  it('does not append an over-long body', () => {
    const long = 'x'.repeat(300)
    expect(friendlyError(new Error(`400 Bad Request: ${long}`), false)).toBe('Invalid request — check your input and try again.')
  })
})
