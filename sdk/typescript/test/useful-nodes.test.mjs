// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import assert from 'node:assert/strict'
import test from 'node:test'

import { htmlToText, redactPii } from '../examples/useful-nodes.mjs'

test('htmlToText strips tags and scripts, decodes entities', () => {
  const out = htmlToText('<h1>Title</h1><p>Hello <b>world</b> &amp; more</p><script>evil()</script>')
  assert.match(out, /Title/)
  assert.match(out, /Hello world & more/)
  assert.doesNotMatch(out, /evil/)
})

test('htmlToText keeps links when asked', () => {
  const out = htmlToText('<a href="https://x.com">site</a>', true)
  assert.match(out, /site/)
  assert.match(out, /https:\/\/x\.com/)
})

test('redactPii masks each category', () => {
  const { redacted, counts } = redactPii('mail a@b.com card 4111 1111 1111 1111 ip 10.0.0.1')
  assert.doesNotMatch(redacted, /a@b\.com/)
  assert.match(redacted, /\[EMAIL\]/)
  assert.match(redacted, /\[CREDIT_CARD\]/)
  assert.match(redacted, /\[IPV4\]/)
  assert.equal(counts.EMAIL, 1)
})

test('redactPii respects category filter', () => {
  const { redacted, counts } = redactPii('a@b.com 10.0.0.1', ['EMAIL'])
  assert.match(redacted, /\[EMAIL\]/)
  assert.match(redacted, /10\.0\.0\.1/) // IP untouched
  assert.deepEqual(Object.keys(counts), ['EMAIL'])
})
