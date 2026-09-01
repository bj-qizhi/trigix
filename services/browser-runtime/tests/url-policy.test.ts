import { describe, expect, it } from 'vitest'
import { isPublicAddress, UrlPolicy } from '../src/security/url-policy.js'

describe('URL policy', () => {
  it.each(['127.0.0.1', '10.0.0.1', '172.16.0.1', '192.168.3.22', '169.254.169.254', '::1', 'fe80::1'])('blocks non-public address %s', (address) => {
    expect(isPublicAddress(address)).toBe(false)
  })

  it.each(['1.1.1.1', '8.8.8.8', '2606:4700:4700::1111'])('accepts public address %s', (address) => {
    expect(isPublicAddress(address)).toBe(true)
  })

  it('rejects non-HTTP protocols and credential URLs', async () => {
    const policy = new UrlPolicy(true, [])
    await expect(policy.validate('file:///etc/passwd')).rejects.toMatchObject({ code: 'BROWSER_URL_BLOCKED' })
    await expect(policy.validate('javascript:alert(1)')).rejects.toMatchObject({ code: 'BROWSER_URL_BLOCKED' })
    await expect(policy.validate('https://user:password@example.com')).rejects.toMatchObject({ code: 'BROWSER_URL_BLOCKED' })
  })

  it('requires an explicit allowlist before private access', async () => {
    await expect(new UrlPolicy(true, []).validate('http://127.0.0.1')).rejects.toMatchObject({ code: 'BROWSER_URL_BLOCKED' })
    await expect(new UrlPolicy(true, ['127.0.0.1']).validate('http://127.0.0.1')).resolves.toMatchObject({ addresses: ['127.0.0.1'] })
  })
})
