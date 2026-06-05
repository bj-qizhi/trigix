// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

// Practical example nodes — zero dependencies, fully offline. Run with:
//   node examples/useful-nodes.mjs
// Then register http://localhost:9000 in Trigix → Custom Nodes → Import All.

import { fileURLToPath } from 'node:url'

import { defineNode, serve } from '../index.js'

// ── html → text ────────────────────────────────────────────────────────────
export function htmlToText(html, keepLinks = false) {
  let s = String(html).replace(/<(script|style)[^>]*>[\s\S]*?<\/\1>/gi, '')
  if (keepLinks) s = s.replace(/<a[^>]*href="([^"]*)"[^>]*>([\s\S]*?)<\/a>/gi, '$2 ($1)')
  s = s.replace(/<(p|br|div|li|tr|h[1-6])[^>]*>/gi, '\n').replace(/<[^>]+>/g, '')
  s = s
    .replace(/&amp;/g, '&')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/&nbsp;/g, ' ')
  return s
    .split('\n')
    .map((l) => l.replace(/[ \t]+/g, ' ').trim())
    .filter(Boolean)
    .join('\n')
}

defineNode({
  slug: 'html_to_text',
  label: 'HTML → Text',
  description: 'Strip HTML to clean plain text (drops script/style, collapses whitespace).',
  configSchema: {
    type: 'object',
    properties: {
      field: { type: 'string', title: 'Input field', default: 'html' },
      keep_links: { type: 'boolean', title: 'Append link URLs' },
    },
  },
  handler: (config, input) => {
    const text = htmlToText(input[config.field ?? 'html'] ?? '', Boolean(config.keep_links))
    return { text, length: text.length }
  },
})

// ── redact PII ─────────────────────────────────────────────────────────────
const PII = [
  ['EMAIL', /[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}/g],
  ['CREDIT_CARD', /\b(?:\d[ -]?){13,16}\b/g],
  ['IPV4', /\b(?:\d{1,3}\.){3}\d{1,3}\b/g],
  ['PHONE', /\+?\d[\d\s().-]{7,}\d/g],
]

export function redactPii(text, categories) {
  const active = categories || PII.map(([c]) => c)
  const counts = {}
  let out = String(text)
  for (const [cat, re] of PII) {
    if (!active.includes(cat)) continue
    let n = 0
    out = out.replace(re, () => {
      n++
      return `[${cat}]`
    })
    if (n) counts[cat] = n
  }
  return { redacted: out, counts }
}

defineNode({
  slug: 'redact_pii',
  label: 'Redact PII',
  description: 'Mask emails, phone numbers, card numbers and IPs in text.',
  configSchema: {
    type: 'object',
    properties: {
      field: { type: 'string', title: 'Input field', default: 'text' },
      categories: { type: 'string', title: 'Categories (comma-separated; blank = all)' },
    },
  },
  handler: (config, input) => {
    const cats = config.categories
      ? String(config.categories).split(',').map((c) => c.trim().toUpperCase())
      : undefined
    const { redacted, counts } = redactPii(input[config.field ?? 'text'] ?? '', cats)
    return { redacted, counts, total: Object.values(counts).reduce((a, b) => a + b, 0) }
  },
})

// Only start the server when run directly (`node examples/useful-nodes.mjs`),
// not when imported by tests.
if (process.argv[1] === fileURLToPath(import.meta.url)) {
  serve(9000)
}
