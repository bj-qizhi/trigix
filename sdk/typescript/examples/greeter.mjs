// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

// Run with:  node examples/greeter.mjs
// Then register http://localhost:9000 in Trigix → Custom Nodes (Import All).

import { defineNode, serve } from '../index.js'

defineNode({
  slug: 'greet',
  label: 'Greeter',
  description: 'Greets a name from config or input.',
  configSchema: { type: 'object', properties: { name: { type: 'string', title: 'Name' } } },
  handler: (config, input) => ({
    greeting: `Hello, ${config.name ?? input.name ?? 'world'}!`,
  }),
})

defineNode({
  slug: 'word_count',
  label: 'Word Count',
  description: 'Counts words in the input text.',
  configSchema: { type: 'object', properties: { field: { type: 'string', default: 'text' } } },
  handler: (config, input) => {
    const text = String(input[config.field ?? 'text'] ?? '')
    return { words: text.split(/\s+/).filter(Boolean).length, chars: text.length }
  },
})

serve(9000)
