import { createRuntime } from './runtime.js'

const runtime = await createRuntime()
await runtime.app.listen({ host: runtime.config.BROWSER_RUNTIME_HOST, port: runtime.config.BROWSER_RUNTIME_PORT })

let shuttingDown = false
async function shutdown() {
  if (shuttingDown) return
  shuttingDown = true
  await runtime.close()
  process.exitCode = 0
}
process.once('SIGTERM', () => { void shutdown() })
process.once('SIGINT', () => { void shutdown() })
