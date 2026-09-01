import { describe, expect, it } from 'vitest'
import { loadConfig } from '../src/config.js'

describe('configuration', () => {
  it('derives bounded capacity and host allowlist', () => {
    const config = loadConfig({ NODE_ENV: 'test', BROWSER_POOL_SIZE: '2', BROWSER_MAX_CONTEXTS_PER_BROWSER: '4', BROWSER_ALLOWED_HOSTS: 'internal.example, *.corp.example' })
    expect(config.capacity).toBe(8)
    expect(config.allowedHosts).toEqual(['internal.example', '*.corp.example'])
  })

  it('requires service authentication in production', () => {
    expect(() => loadConfig({ NODE_ENV: 'production' })).toThrow(/BROWSER_RUNTIME_AUTH_TOKEN/)
  })

  it('accepts empty optional values from Compose and Helm environments', () => {
    const config = loadConfig({ NODE_ENV: 'test', BROWSER_ARTIFACT_ENDPOINT: '', BROWSER_ARTIFACT_BUCKET: '', OTEL_EXPORTER_OTLP_ENDPOINT: '' })
    expect(config.BROWSER_ARTIFACT_ENDPOINT).toBeUndefined()
    expect(config.BROWSER_ARTIFACT_BUCKET).toBeUndefined()
    expect(config.OTEL_EXPORTER_OTLP_ENDPOINT).toBeUndefined()
  })
})
