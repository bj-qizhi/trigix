// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

// Trigix custom node SDK (zero dependencies; runs on Node's built-in http).
//
//   import { defineNode, serve } from '@trigix/node-sdk'
//
//   defineNode({
//     slug: 'greet', label: 'Greeter',
//     configSchema: { type: 'object', properties: { name: { type: 'string' } } },
//     handler: (config, input) => ({ greeting: `Hello, ${config.name ?? input.name ?? 'world'}!` }),
//   })
//
//   serve(9000)
//
// The executor POSTs { node_id, config, input_json, node_outputs } to
// /nodes/<slug> and expects { output_json }. GET /manifest lists every node.

import http from 'node:http'

const registry = new Map()

export function defineNode({ slug, label, description = '', configSchema, handler }) {
  if (!slug || typeof handler !== 'function') {
    throw new Error('defineNode requires a slug and a handler function')
  }
  registry.set(slug, {
    slug,
    label: label || slug,
    description,
    configSchema: configSchema || { type: 'object' },
    handler,
  })
}

export function registry_() {
  return new Map(registry)
}

export function manifest(baseUrl = '') {
  return {
    nodes: [...registry.values()].map((n) => ({
      slug: n.slug,
      label: n.label,
      description: n.description,
      config_schema: n.configSchema,
      endpoint: `${baseUrl}/nodes/${n.slug}`,
    })),
  }
}

export async function runNode(slug, config, inputJson, nodeOutputs) {
  const n = registry.get(slug)
  if (!n) throw new Error(`unknown node '${slug}'`)
  let input
  try {
    input = JSON.parse(inputJson || '{}')
  } catch {
    input = {}
  }
  const result = await n.handler(config || {}, input, nodeOutputs || {})
  return JSON.stringify(result)
}

// Framework-agnostic request handler — used by the server and by tests.
export async function handle(method, path, body = {}, baseUrl = '') {
  if (method === 'GET' && path === '/healthz') return { status: 200, body: { status: 'ok' } }
  if (method === 'GET' && path === '/manifest') return { status: 200, body: manifest(baseUrl) }
  const m = path.match(/^\/nodes\/([^/]+)$/)
  if (method === 'POST' && m) {
    const slug = decodeURIComponent(m[1])
    if (!registry.has(slug)) return { status: 404, body: { detail: `unknown node '${slug}'` } }
    try {
      const output_json = await runNode(slug, body.config, body.input_json, body.node_outputs)
      return { status: 200, body: { output_json } }
    } catch (e) {
      return { status: 500, body: { detail: `node '${slug}' failed: ${e.message}` } }
    }
  }
  return { status: 404, body: { detail: 'not found' } }
}

export function createServer({ baseUrl = '' } = {}) {
  return http.createServer((req, res) => {
    const chunks = []
    req.on('data', (c) => chunks.push(c))
    req.on('end', async () => {
      let parsed = {}
      if (chunks.length) {
        try {
          parsed = JSON.parse(Buffer.concat(chunks).toString('utf8'))
        } catch {
          parsed = {}
        }
      }
      const url = (req.url || '/').split('?')[0]
      const { status, body } = await handle(req.method || 'GET', url, parsed, baseUrl)
      res.writeHead(status, { 'content-type': 'application/json' })
      res.end(JSON.stringify(body))
    })
  })
}

export function serve(port, opts = {}) {
  const server = createServer(opts)
  server.listen(port, () => {
    // eslint-disable-next-line no-console
    console.log(`Trigix custom nodes listening on :${port}`)
  })
  return server
}
