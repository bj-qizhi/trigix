import type { BrowserArtifact } from '../types.js'
import { BrowserRuntimeError } from '../errors.js'
import type { ArtifactStore } from './store.js'

export class ArtifactManager {
  private readonly metadata = new Map<string, BrowserArtifact>()
  constructor(private readonly store: ArtifactStore) {}

  async create(input: Parameters<ArtifactStore['put']>[0]) {
    const artifact = await this.store.put(input)
    await this.store.saveMetadata(artifact)
    this.metadata.set(artifact.id, artifact)
    return artifact
  }

  async getMetadata(id: string, tenantId: string) {
    const artifact = this.metadata.get(id) ?? await this.store.loadMetadata(id, tenantId)
    if (!artifact || artifact.tenant_id !== tenantId) throw new BrowserRuntimeError('BROWSER_ARTIFACT_FAILED', 'Artifact was not found', 404)
    this.metadata.set(id, artifact)
    return artifact
  }

  async read(id: string, tenantId: string) {
    const artifact = await this.getMetadata(id, tenantId)
    return { artifact, body: await this.store.get(artifact) }
  }
}
