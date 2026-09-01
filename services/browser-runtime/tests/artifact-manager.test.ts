import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { describe, expect, it } from 'vitest'
import { ArtifactManager } from '../src/artifacts/manager.js'
import { createArtifactStore, type ArtifactStore } from '../src/artifacts/store.js'
import { loadConfig } from '../src/config.js'
import { RuntimeMetrics } from '../src/telemetry/metrics.js'
import type { BrowserArtifact } from '../src/types.js'

class DurableMemoryStore implements ArtifactStore {
  readonly metadata = new Map<string, BrowserArtifact>()
  readonly bodies = new Map<string, Buffer>()

  async put(input: Parameters<ArtifactStore['put']>[0]) {
    const artifact: BrowserArtifact = {
      id: 'ba_test', tenant_id: input.tenantId, type: input.type,
      content_type: input.contentType, size: input.body.byteLength,
      storage_key: `browser/${input.tenantId}/ba_test`, created_at: new Date().toISOString(),
    }
    this.bodies.set(artifact.storage_key, input.body)
    return artifact
  }

  async get(artifact: BrowserArtifact) { return this.bodies.get(artifact.storage_key)! }
  async saveMetadata(artifact: BrowserArtifact) { this.metadata.set(`${artifact.tenant_id}/${artifact.id}`, artifact) }
  async loadMetadata(id: string, tenantId: string) { return this.metadata.get(`${tenantId}/${id}`) }
}

describe('ArtifactManager', () => {
  it('reloads durable metadata and preserves tenant isolation', async () => {
    const store = new DurableMemoryStore()
    const first = new ArtifactManager(store)
    const artifact = await first.create({ tenantId: 'tenant-a', type: 'screenshot', contentType: 'image/png', body: Buffer.from('png') })
    const restarted = new ArtifactManager(store)
    expect((await restarted.read(artifact.id, 'tenant-a')).body.toString()).toBe('png')
    await expect(restarted.getMetadata(artifact.id, 'tenant-b')).rejects.toMatchObject({ httpStatus: 404 })
  })

  it('rejects traversal identifiers before local filesystem access', async () => {
    const directory = await mkdtemp(path.join(tmpdir(), 'trigix-artifact-path-'))
    try {
      const config = loadConfig({ NODE_ENV: 'test', BROWSER_ARTIFACT_DIR: directory })
      const store = createArtifactStore(config, new RuntimeMetrics(false))
      await expect(store.loadMetadata('../artifact', 'tenant-a')).rejects.toMatchObject({
        code: 'BROWSER_ARTIFACT_FAILED',
        httpStatus: 400,
      })
      await expect(store.loadMetadata('artifact', '../tenant-a')).rejects.toMatchObject({
        code: 'BROWSER_ARTIFACT_FAILED',
        httpStatus: 400,
      })
    } finally {
      await rm(directory, { recursive: true, force: true })
    }
  })
})
