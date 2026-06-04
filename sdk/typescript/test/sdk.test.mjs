// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import assert from 'node:assert/strict'
import test from 'node:test'

import { defineNode, handle, manifest, runNode } from '../index.js'

defineNode({
  slug: 'upper',
  label: 'Uppercase',
  handler: (config, input) => ({ out: String(input.text ?? '').toUpperCase() }),
})

test('runNode parses input and returns output_json', async () => {
  const out = await runNode('upper', {}, JSON.stringify({ text: 'hi' }), {})
  assert.deepEqual(JSON.parse(out), { out: 'HI' })
})

test('runNode tolerates bad input json', async () => {
  const out = await runNode('upper', {}, 'not json', {})
  assert.deepEqual(JSON.parse(out), { out: '' })
})

test('manifest lists the node with a resolved endpoint', () => {
  const m = manifest('http://node:9000')
  const upper = m.nodes.find((n) => n.slug === 'upper')
  assert.ok(upper)
  assert.equal(upper.endpoint, 'http://node:9000/nodes/upper')
})

test('handle implements the executor contract', async () => {
  const res = await handle('POST', '/nodes/upper', {
    node_id: 'n1',
    config: {},
    input_json: JSON.stringify({ text: 'abc' }),
    node_outputs: {},
  })
  assert.equal(res.status, 200)
  assert.deepEqual(JSON.parse(res.body.output_json), { out: 'ABC' })
})

test('unknown node returns 404', async () => {
  const res = await handle('POST', '/nodes/ghost', { input_json: '{}' })
  assert.equal(res.status, 404)
})

test('handler error returns 500', async () => {
  defineNode({
    slug: 'boom',
    handler: () => {
      throw new Error('kaboom')
    },
  })
  const res = await handle('POST', '/nodes/boom', { input_json: '{}' })
  assert.equal(res.status, 500)
  assert.match(res.body.detail, /kaboom/)
})

test('healthz responds ok', async () => {
  const res = await handle('GET', '/healthz', {})
  assert.equal(res.status, 200)
  assert.deepEqual(res.body, { status: 'ok' })
})
