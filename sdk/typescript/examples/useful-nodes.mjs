// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

// Practical example nodes — zero dependencies, fully offline. Run with:
//   node examples/useful-nodes.mjs
// Then register http://localhost:9000 in Trigix → Custom Nodes → Import All.

import { fileURLToPath } from 'node:url'

import { defineNode, serve } from '../index.js'

// ── html → text ────────────────────────────────────────────────────────────
const BLOCK_TAGS = new Set(['p', 'br', 'div', 'li', 'tr', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6'])
const ENTITIES = {
  amp: '&',
  lt: '<',
  gt: '>',
  quot: '"',
  '#39': "'",
  nbsp: ' ',
}

function isSpace(char) {
  return char === ' ' || char === '\t' || char === '\r' || char === '\n'
}

function isNameChar(char) {
  if (!char) return false
  const code = char.charCodeAt(0)
  return (code >= 48 && code <= 57)
    || (code >= 65 && code <= 90)
    || (code >= 97 && code <= 122)
    || char === '-' || char === ':' || char === '_'
}

function parseTag(raw) {
  let cursor = 0
  while (isSpace(raw[cursor])) cursor++
  const closing = raw[cursor] === '/'
  if (closing) cursor++
  while (isSpace(raw[cursor])) cursor++
  const nameStart = cursor
  while (isNameChar(raw[cursor])) cursor++
  if (cursor === nameStart) return null
  return {
    name: raw.slice(nameStart, cursor).toLowerCase(),
    closing,
    attributesStart: cursor,
    selfClosing: raw.trimEnd().endsWith('/'),
  }
}

function readAttribute(raw, start, wantedName) {
  let cursor = start
  while (cursor < raw.length) {
    while (isSpace(raw[cursor]) || raw[cursor] === '/') cursor++
    const nameStart = cursor
    while (isNameChar(raw[cursor])) cursor++
    if (cursor === nameStart) {
      cursor++
      continue
    }
    const name = raw.slice(nameStart, cursor).toLowerCase()
    while (isSpace(raw[cursor])) cursor++
    if (raw[cursor] !== '=') continue
    cursor++
    while (isSpace(raw[cursor])) cursor++
    const quote = raw[cursor]
    let value
    if (quote === '"' || quote === "'") {
      cursor++
      const valueStart = cursor
      while (cursor < raw.length && raw[cursor] !== quote) cursor++
      value = raw.slice(valueStart, cursor)
      if (cursor < raw.length) cursor++
    } else {
      const valueStart = cursor
      while (cursor < raw.length && !isSpace(raw[cursor])) cursor++
      value = raw.slice(valueStart, cursor)
    }
    if (name === wantedName) return value
  }
  return null
}

function trustedLink(value) {
  if (!value) return null
  try {
    const parsed = new URL(value)
    return parsed.protocol === 'http:' || parsed.protocol === 'https:' ? value.trim() : null
  } catch {
    return null
  }
}

function decodeEntitiesOnce(text) {
  return text.replace(/&(amp|lt|gt|quot|#39|nbsp);/g, (_match, name) => ENTITIES[name])
}

export function htmlToText(html, keepLinks = false) {
  const source = String(html)
  const output = []
  let cursor = 0
  let skippedTag = null
  let activeLink = null

  while (cursor < source.length) {
    if (source[cursor] !== '<') {
      if (!skippedTag) output.push(source[cursor])
      cursor++
      continue
    }
    // Treat a repeated opener as literal text, then parse the following tag.
    if (source[cursor + 1] === '<') {
      if (!skippedTag) output.push('<')
      cursor++
      continue
    }
    const end = source.indexOf('>', cursor + 1)
    if (end < 0) {
      if (!skippedTag) output.push(source.slice(cursor))
      break
    }
    const rawTag = source.slice(cursor + 1, end)
    const tag = parseTag(rawTag)
    cursor = end + 1
    if (!tag) continue

    if (skippedTag) {
      if (tag.closing && tag.name === skippedTag) skippedTag = null
      continue
    }
    if (!tag.closing && !tag.selfClosing && (tag.name === 'script' || tag.name === 'style')) {
      skippedTag = tag.name
      continue
    }
    if (tag.name === 'a' && keepLinks) {
      if (tag.closing) {
        if (activeLink) output.push(` (${activeLink})`)
        activeLink = null
      } else {
        activeLink = trustedLink(readAttribute(rawTag, tag.attributesStart, 'href'))
      }
    }
    if (!tag.closing && BLOCK_TAGS.has(tag.name)) output.push('\n')
  }

  return decodeEntitiesOnce(output.join(''))
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
