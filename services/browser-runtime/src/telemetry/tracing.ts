import { trace, type Span, SpanStatusCode } from '@opentelemetry/api'
import { NodeSDK } from '@opentelemetry/sdk-node'
import { OTLPTraceExporter } from '@opentelemetry/exporter-trace-otlp-http'
import { getNodeAutoInstrumentations } from '@opentelemetry/auto-instrumentations-node'

let sdk: NodeSDK | undefined

export async function startTelemetry(endpoint?: string) {
  if (!endpoint) return
  sdk = new NodeSDK({
    serviceName: 'trigix-browser-runtime',
    traceExporter: new OTLPTraceExporter({ url: `${endpoint.replace(/\/$/, '')}/v1/traces` }),
    instrumentations: [getNodeAutoInstrumentations()],
  })
  sdk.start()
}

export async function stopTelemetry() { await sdk?.shutdown() }

export async function inSpan<T>(name: string, attributes: Record<string, string | number | boolean | undefined>, fn: (span: Span) => Promise<T>): Promise<T> {
  const clean = Object.fromEntries(Object.entries(attributes).filter((entry): entry is [string, string | number | boolean] => entry[1] !== undefined))
  return trace.getTracer('trigix-browser-runtime').startActiveSpan(name, { attributes: clean }, async (span) => {
    try { return await fn(span) }
    catch (error) {
      span.setStatus({ code: SpanStatusCode.ERROR })
      if (error instanceof Error) span.recordException(error)
      throw error
    } finally { span.end() }
  })
}
