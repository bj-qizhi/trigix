import { mkdir, readFile, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { GetObjectCommand, PutObjectCommand, S3Client } from '@aws-sdk/client-s3'
import { ulid } from 'ulid'
import type { BrowserRuntimeConfig } from '../config.js'
import { BrowserRuntimeError } from '../errors.js'
import type { BrowserArtifact, BrowserArtifactType } from '../types.js'
import type { RuntimeMetrics } from '../telemetry/metrics.js'

export interface ArtifactStore {
  put(input: { tenantId: string; executionId?: string; taskId?: string; type: BrowserArtifactType; contentType: string; body: Buffer }): Promise<BrowserArtifact>
  get(artifact: BrowserArtifact): Promise<Buffer>
  saveMetadata(artifact: BrowserArtifact): Promise<void>
  loadMetadata(id: string, tenantId: string): Promise<BrowserArtifact | undefined>
}

abstract class BaseStore implements ArtifactStore {
  constructor(protected readonly config: BrowserRuntimeConfig, protected readonly metrics: RuntimeMetrics) {}
  abstract write(key: string, body: Buffer, contentType: string): Promise<void>
  abstract read(key: string): Promise<Buffer>

  async put(input: { tenantId: string; executionId?: string; taskId?: string; type: BrowserArtifactType; contentType: string; body: Buffer }) {
    if (input.body.byteLength > this.config.BROWSER_MAX_ARTIFACT_BYTES) {
      throw new BrowserRuntimeError('BROWSER_ARTIFACT_FAILED', 'Artifact exceeds configured size limit', 413)
    }
    const id = `ba_${ulid()}`
    const key = ['browser', safeSegment(input.tenantId), safeSegment(input.executionId ?? 'unbound'), safeSegment(input.taskId ?? 'unbound'), id].join('/')
    await this.write(key, input.body, input.contentType)
    this.metrics.artifacts.inc({ type: input.type })
    this.metrics.artifactBytes.inc({ type: input.type }, input.body.byteLength)
    return {
      id, tenant_id: input.tenantId, type: input.type, content_type: input.contentType,
      size: input.body.byteLength, storage_key: key, created_at: new Date().toISOString(),
      ...(input.executionId ? { execution_id: input.executionId } : {}),
      ...(input.taskId ? { task_id: input.taskId } : {}),
    }
  }
  get(artifact: BrowserArtifact) { return this.read(artifact.storage_key) }
  abstract saveMetadata(artifact: BrowserArtifact): Promise<void>
  abstract loadMetadata(id: string, tenantId: string): Promise<BrowserArtifact | undefined>
}

class LocalArtifactStore extends BaseStore {
  async write(key: string, body: Buffer) {
    const target = artifactPath(this.config.BROWSER_ARTIFACT_DIR, key)
    await mkdir(path.dirname(target), { recursive: true, mode: 0o700 })
    await writeFile(target, body, { mode: 0o600 })
  }
  read(key: string) { return readFile(artifactPath(this.config.BROWSER_ARTIFACT_DIR, key)) }
  async saveMetadata(artifact: BrowserArtifact) {
    const target = metadataPath(this.config.BROWSER_ARTIFACT_DIR, artifact.tenant_id, artifact.id)
    await mkdir(path.dirname(target), { recursive: true, mode: 0o700 })
    await writeFile(target, JSON.stringify(artifact), { mode: 0o600 })
  }
  async loadMetadata(id: string, tenantId: string) {
    try {
      return validateMetadata(JSON.parse(await readFile(metadataPath(this.config.BROWSER_ARTIFACT_DIR, tenantId, id), 'utf8')), id, tenantId)
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === 'ENOENT') return undefined
      throw error
    }
  }
}

class S3ArtifactStore extends BaseStore {
  private readonly client = new S3Client({
    region: this.config.BROWSER_ARTIFACT_REGION,
    ...(this.config.BROWSER_ARTIFACT_ENDPOINT ? { endpoint: this.config.BROWSER_ARTIFACT_ENDPOINT, forcePathStyle: true } : {}),
  })
  async write(key: string, body: Buffer, contentType: string) {
    await this.client.send(new PutObjectCommand({ Bucket: this.config.BROWSER_ARTIFACT_BUCKET!, Key: key, Body: body, ContentType: contentType }))
  }
  async read(key: string) {
    const response = await this.client.send(new GetObjectCommand({ Bucket: this.config.BROWSER_ARTIFACT_BUCKET!, Key: key }))
    if (!response.Body) throw new BrowserRuntimeError('BROWSER_ARTIFACT_FAILED', 'Artifact body is unavailable', 404)
    return Buffer.from(await response.Body.transformToByteArray())
  }
  async saveMetadata(artifact: BrowserArtifact) {
    await this.client.send(new PutObjectCommand({
      Bucket: this.config.BROWSER_ARTIFACT_BUCKET!, Key: metadataKey(artifact.tenant_id, artifact.id),
      Body: JSON.stringify(artifact), ContentType: 'application/json',
    }))
  }
  async loadMetadata(id: string, tenantId: string) {
    try {
      const response = await this.client.send(new GetObjectCommand({ Bucket: this.config.BROWSER_ARTIFACT_BUCKET!, Key: metadataKey(tenantId, id) }))
      if (!response.Body) return undefined
      return validateMetadata(JSON.parse(await response.Body.transformToString()), id, tenantId)
    } catch (error) {
      const status = (error as { $metadata?: { httpStatusCode?: number } }).$metadata?.httpStatusCode
      if (status === 404) return undefined
      throw error
    }
  }
}

function safeSegment(value: string) {
  const segment = path.basename(value)
  if (segment !== value || segment === '.' || segment === '..' || !/^[A-Za-z0-9._:-]{1,128}$/.test(segment)) {
    throw new BrowserRuntimeError('BROWSER_ARTIFACT_FAILED', 'Artifact path identifier is invalid', 400)
  }
  return segment
}
function metadataKey(tenantId: string, id: string) { return `browser-metadata/${safeSegment(tenantId)}/${safeSegment(id)}.json` }
function metadataPath(root: string, tenantId: string, id: string) { return withinRoot(root, 'browser-metadata', safeSegment(tenantId), `${safeSegment(id)}.json`) }
function artifactPath(root: string, key: string) { return withinRoot(root, ...key.split('/').map(safeSegment)) }
function withinRoot(root: string, ...segments: string[]) {
  const resolvedRoot = path.resolve(root)
  const target = path.resolve(resolvedRoot, ...segments)
  if (target === resolvedRoot || !target.startsWith(`${resolvedRoot}${path.sep}`)) {
    throw new BrowserRuntimeError('BROWSER_ARTIFACT_FAILED', 'Artifact path escapes the storage root', 400)
  }
  return target
}
function validateMetadata(value: unknown, id: string, tenantId: string) {
  if (!value || typeof value !== 'object') return undefined
  const artifact = value as BrowserArtifact
  return artifact.id === id && artifact.tenant_id === tenantId ? artifact : undefined
}

export function createArtifactStore(config: BrowserRuntimeConfig, metrics: RuntimeMetrics): ArtifactStore {
  return config.BROWSER_ARTIFACT_PROVIDER === 's3' ? new S3ArtifactStore(config, metrics) : new LocalArtifactStore(config, metrics)
}
