import { lookup } from 'node:dns/promises'
import ipaddr from 'ipaddr.js'
import { BrowserRuntimeError } from '../errors.js'

const forbiddenNames = new Set(['localhost', 'localhost.localdomain', 'metadata.google.internal'])

export interface ValidatedTarget { url: URL; addresses: string[] }

export class UrlPolicy {
  constructor(
    private readonly blockPrivateNetwork: boolean,
    private readonly allowedHosts: string[],
  ) {}

  async validate(raw: string): Promise<ValidatedTarget> {
    let url: URL
    try { url = new URL(raw) } catch { throw new BrowserRuntimeError('BROWSER_URL_BLOCKED', 'URL is invalid') }
    if (!['http:', 'https:'].includes(url.protocol)) {
      throw new BrowserRuntimeError('BROWSER_URL_BLOCKED', 'Only HTTP and HTTPS URLs are allowed')
    }
    if (url.username || url.password) {
      throw new BrowserRuntimeError('BROWSER_URL_BLOCKED', 'Credentials in URLs are not allowed')
    }
    const hostname = url.hostname.toLowerCase().replace(/\.$/, '')
    if (!hostname || forbiddenNames.has(hostname) || hostname.endsWith('.localhost')) {
      throw new BrowserRuntimeError('BROWSER_URL_BLOCKED', 'Local hostnames are blocked')
    }
    let addresses: string[]
    if (ipaddr.isValid(hostname)) {
      addresses = [hostname]
    } else {
      try {
        addresses = [...new Set((await lookup(hostname, { all: true, verbatim: true })).map(({ address }) => address))]
      } catch {
        throw new BrowserRuntimeError('BROWSER_URL_BLOCKED', 'Hostname could not be resolved')
      }
    }
    if (addresses.length === 0) throw new BrowserRuntimeError('BROWSER_URL_BLOCKED', 'Hostname has no address')
    if (this.blockPrivateNetwork && !this.hostAllowed(hostname)) {
      for (const address of addresses) {
        if (!isPublicAddress(address)) {
          throw new BrowserRuntimeError('BROWSER_URL_BLOCKED', 'Destination resolves to a blocked network')
        }
      }
    }
    return { url, addresses }
  }

  private hostAllowed(hostname: string) {
    return this.allowedHosts.some((rule) => rule.startsWith('*.')
      ? hostname.endsWith(rule.slice(1)) && hostname !== rule.slice(2)
      : hostname === rule)
  }
}

export function isPublicAddress(address: string): boolean {
  let parsed: ipaddr.IPv4 | ipaddr.IPv6
  try { parsed = ipaddr.process(address) } catch { return false }
  const range = parsed.range()
  return range === 'unicast'
}
