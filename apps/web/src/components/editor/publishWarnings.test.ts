// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { describe, it, expect } from 'vitest'
import { collectPublishWarnings } from './publishWarnings'
import type { FlowNode, FlowEdge } from '../Canvas'

const node = (id: string, nodeType: string, config: Record<string, unknown> = {}): FlowNode =>
  ({ id, type: nodeType, position: { x: 0, y: 0 }, data: { label: id, nodeType, config } } as unknown as FlowNode)
const edge = (source: string, target: string): FlowEdge =>
  ({ id: `${source}-${target}`, source, target } as unknown as FlowEdge)

const messages = (w: ReturnType<typeof collectPublishWarnings>) => w.map((x) => x.message)

describe('collectPublishWarnings', () => {
  it('flags a missing trigger (structural — no nodeId)', () => {
    const w = collectPublishWarnings([node('http1', 'http', { url: 'x' })], [])
    const missing = w.find((m) => m.message.includes('No trigger node'))
    expect(missing).toBeTruthy()
    expect(missing!.nodeId).toBeUndefined()
  })

  it('flags multiple triggers', () => {
    const w = collectPublishWarnings([node('t1', 'trigger'), node('t2', 'trigger')], [])
    expect(messages(w).some((m) => m.includes('Multiple trigger nodes'))).toBe(true)
  })

  it('flags orphaned nodes (no incoming / no outgoing) and tags them with the node id', () => {
    const w = collectPublishWarnings(
      [node('trigger', 'trigger'), node('http1', 'http', { url: 'x' })],
      [], // http1 has no edges
    )
    const incoming = w.find((m) => m.message.includes('"http1" has no incoming connections'))
    const outgoing = w.find((m) => m.message.includes('"http1" has no outgoing connections'))
    expect(incoming?.nodeId).toBe('http1')
    expect(outgoing?.nodeId).toBe('http1')
  })

  it('applies the data-driven required-field checks (with nodeId)', () => {
    const w = collectPublishWarnings(
      [node('trigger', 'trigger'), node('s', 'slack', {})],
      [edge('trigger', 's')],
    )
    const slack = w.find((m) => m.message === 'Slack node "s" has no Webhook URL')
    expect(slack?.nodeId).toBe('s')
  })

  it('uses node_label in the message when present', () => {
    const w = collectPublishWarnings(
      [node('trigger', 'trigger'), node('h', 'http', { node_label: 'Fetch' })],
      [edge('trigger', 'h')],
    )
    expect(messages(w)).toContain('HTTP node "Fetch" has no URL')
  })

  it('flags credential/connection nodes with empty config (newly covered)', () => {
    const w = collectPublishWarnings(
      [node('trigger', 'trigger'), node('db', 'mysql', {})],
      [edge('trigger', 'db')],
    )
    const msgs = messages(w)
    expect(msgs).toContain('MySQL node "db" has no connection URL')
    expect(msgs).toContain('MySQL node "db" has no SQL query')
  })

  it('does not warn when required config is present', () => {
    const w = collectPublishWarnings(
      [node('trigger', 'trigger'), node('h', 'http', { url: 'https://x' })],
      [edge('trigger', 'h')],
    )
    expect(messages(w).some((m) => m.includes('has no URL'))).toBe(false)
  })
})
