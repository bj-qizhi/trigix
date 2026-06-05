# Trigix Node SDK (TypeScript / JavaScript)

Write a custom Trigix workflow node in TypeScript or JavaScript and serve it over
HTTP. Zero dependencies — runs on Node's built-in `http`. Ships JS runtime plus
`.d.ts` types.

## Quick start

```bash
npm install @trigix/node-sdk
node examples/greeter.mjs   # listens on :9000
```

```ts
import { defineNode, serve } from '@trigix/node-sdk'

defineNode({
  slug: 'greet',
  label: 'Greeter',
  configSchema: { type: 'object', properties: { name: { type: 'string' } } },
  handler: (config, input) => ({
    greeting: `Hello, ${config.name ?? input.name ?? 'world'}!`,
  }),
})

serve(9000)
```

In Trigix → **Custom Nodes**, paste the service URL (`http://your-host:9000`) and
click **Import All** — every node from `GET /manifest` is registered at once.

## The contract

The executor POSTs to `/nodes/<slug>`:

```json
{ "node_id": "n1", "config": { "name": "Ada" },
  "input_json": "{...}", "node_outputs": { "prev": "{...}" } }
```

Your handler `(config, input, nodeOutputs) => result` returns a
JSON-serializable value; the server wraps it as `{ "output_json": "..." }`.

## Testing

```bash
node --test
```

`handle(method, path, body)` is exported so you can unit-test nodes without
opening a socket.
