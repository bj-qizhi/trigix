// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import type { Server } from 'node:http'

/** A node handler: receives the node config, the parsed input, and upstream
 *  node outputs; returns a JSON-serializable result. */
export type NodeHandler = (
  config: Record<string, unknown>,
  input: Record<string, unknown>,
  nodeOutputs: Record<string, string>,
) => unknown | Promise<unknown>

export interface NodeDefinition {
  slug: string
  label?: string
  description?: string
  configSchema?: Record<string, unknown>
  handler: NodeHandler
}

export interface ManifestNode {
  slug: string
  label: string
  description: string
  config_schema: Record<string, unknown>
  endpoint: string
}

export interface HandleResult {
  status: number
  body: unknown
}

export function defineNode(def: NodeDefinition): void
export function manifest(baseUrl?: string): { nodes: ManifestNode[] }
export function runNode(
  slug: string,
  config: Record<string, unknown>,
  inputJson: string,
  nodeOutputs: Record<string, string>,
): Promise<string>
export function handle(
  method: string,
  path: string,
  body?: Record<string, unknown>,
  baseUrl?: string,
): Promise<HandleResult>
export function createServer(opts?: { baseUrl?: string }): Server
export function serve(port: number, opts?: { baseUrl?: string }): Server
