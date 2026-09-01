import http from 'node:http'
import net from 'node:net'
import { once } from 'node:events'
import type { AddressInfo } from 'node:net'
import type { Duplex } from 'node:stream'
import { UrlPolicy } from './url-policy.js'

export class SecureBrowserProxy {
  private readonly server: http.Server
  private addressValue = ''

  constructor(private readonly policy: UrlPolicy, private readonly requestedPort = 0) {
    this.server = http.createServer((request, response) => { void this.forwardHttp(request, response) })
    this.server.on('connect', (request, socket, head) => { void this.forwardConnect(request, socket, head) })
    this.server.on('clientError', (_error, socket) => socket.end('HTTP/1.1 400 Bad Request\r\n\r\n'))
  }

  get address() { return this.addressValue }

  async start() {
    this.server.listen(this.requestedPort, '127.0.0.1')
    await once(this.server, 'listening')
    const address = this.server.address() as AddressInfo
    this.addressValue = `http://127.0.0.1:${address.port}`
  }

  async close() {
    if (!this.server.listening) return
    this.server.close()
    await once(this.server, 'close')
  }

  private async forwardConnect(request: http.IncomingMessage, client: Duplex, head: Buffer) {
    try {
      const authority = new URL(`https://${request.url ?? ''}`)
      const target = await this.policy.validate(authority.href)
      const port = Number(authority.port || 443)
      const upstream = net.connect({ host: target.addresses[0]!, port })
      upstream.setTimeout(60_000, () => upstream.destroy())
      upstream.once('connect', () => {
        client.write('HTTP/1.1 200 Connection Established\r\n\r\n')
        if (head.length) upstream.write(head)
        client.pipe(upstream)
        upstream.pipe(client)
      })
      upstream.once('error', () => client.end('HTTP/1.1 502 Bad Gateway\r\n\r\n'))
    } catch {
      client.end('HTTP/1.1 403 Forbidden\r\n\r\n')
    }
  }

  private async forwardHttp(request: http.IncomingMessage, response: http.ServerResponse) {
    try {
      const target = await this.policy.validate(request.url ?? '')
      const headers: Record<string, string | string[] | undefined> = { ...request.headers, host: target.url.host }
      delete headers['proxy-authorization']
      delete headers['proxy-connection']
      const upstream = http.request({
        host: target.addresses[0]!,
        port: Number(target.url.port || 80),
        path: `${target.url.pathname}${target.url.search}`,
        method: request.method,
        headers,
      }, (upstreamResponse) => {
        response.writeHead(upstreamResponse.statusCode ?? 502, upstreamResponse.headers)
        upstreamResponse.pipe(response)
      })
      upstream.setTimeout(60_000, () => upstream.destroy())
      upstream.once('error', () => { if (!response.headersSent) response.writeHead(502); response.end() })
      request.pipe(upstream)
    } catch {
      response.writeHead(403, { 'content-type': 'text/plain' })
      response.end('Destination blocked')
    }
  }
}
