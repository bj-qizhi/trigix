// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { describe, it, expect } from 'vitest'
import { collectPublishWarnings } from './publishWarnings'
import type { FlowNode, FlowEdge } from '../Canvas'

const node = (id: string, nodeType: string, config: Record<string, unknown> = {}): FlowNode =>
  ({ id, type: nodeType, position: { x: 0, y: 0 }, data: { label: id, nodeType, config } } as unknown as FlowNode)
const edge = (source: string, target: string): FlowEdge =>
  ({ id: `${source}-${target}`, source, target } as unknown as FlowEdge)

describe('collectPublishWarnings', () => {
  it('flags a missing trigger', () => {
    const w = collectPublishWarnings([node('http1', 'http', { url: 'x' })], [])
    expect(w.some((m) => m.includes('No trigger node'))).toBe(true)
  })

  it('flags multiple triggers', () => {
    const w = collectPublishWarnings([node('t1', 'trigger'), node('t2', 'trigger')], [])
    expect(w.some((m) => m.includes('Multiple trigger nodes'))).toBe(true)
  })

  it('flags orphaned nodes (no incoming / no outgoing)', () => {
    const w = collectPublishWarnings(
      [node('trigger', 'trigger'), node('http1', 'http', { url: 'x' })],
      [], // http1 has no edges
    )
    expect(w.some((m) => m.includes('"http1" has no incoming connections'))).toBe(true)
    expect(w.some((m) => m.includes('"http1" has no outgoing connections'))).toBe(true)
  })

  it('applies the data-driven required-field checks', () => {
    const w = collectPublishWarnings(
      [node('trigger', 'trigger'), node('s', 'slack', {})],
      [edge('trigger', 's')],
    )
    expect(w).toContain('Slack node "s" has no Webhook URL')
  })

  it('uses node_label in the message when present', () => {
    const w = collectPublishWarnings(
      [node('trigger', 'trigger'), node('h', 'http', { node_label: 'Fetch' })],
      [edge('trigger', 'h')],
    )
    expect(w).toContain('HTTP node "Fetch" has no URL')
  })

  it('does not warn when required config is present', () => {
    const w = collectPublishWarnings(
      [node('trigger', 'trigger'), node('h', 'http', { url: 'https://x' })],
      [edge('trigger', 'h')],
    )
    expect(w.some((m) => m.includes('has no URL'))).toBe(false)
  })
})
