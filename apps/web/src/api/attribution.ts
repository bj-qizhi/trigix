// First-touch acquisition attribution, captured in the browser and sent at
// signup so the backend can credit a paid conversion to its acquisition channel
// (and forward it to PostHog server-side).
//
// First touch wins: once captured it is never overwritten, so the original
// channel that brought the visitor is preserved across later navigations.

const STORAGE_KEY = 'trigix_attribution_v1'

export interface Attribution {
  utm_source?: string
  utm_medium?: string
  utm_campaign?: string
  utm_term?: string
  utm_content?: string
  referrer?: string
  landing_page?: string
  distinct_id?: string
}

function readFromUrl(): Attribution {
  const p = new URLSearchParams(window.location.search)
  const get = (k: string) => p.get(k) || undefined
  const attr: Attribution = {
    utm_source: get('utm_source'),
    utm_medium: get('utm_medium'),
    utm_campaign: get('utm_campaign'),
    utm_term: get('utm_term'),
    utm_content: get('utm_content'),
    landing_page: window.location.href,
  }
  const ref = document.referrer || undefined
  if (ref && !ref.startsWith(window.location.origin)) attr.referrer = ref
  return attr
}

function hasSignal(a: Attribution): boolean {
  return Boolean(
    a.utm_source || a.utm_medium || a.utm_campaign || a.utm_term || a.utm_content || a.referrer,
  )
}

/** Captures first-touch attribution once; a no-op if already stored or no signal. */
export function captureFirstTouch(): void {
  try {
    if (localStorage.getItem(STORAGE_KEY)) return
    const attr = readFromUrl()
    if (hasSignal(attr)) localStorage.setItem(STORAGE_KEY, JSON.stringify(attr))
  } catch {
    /* localStorage unavailable (private mode) — attribution is best-effort */
  }
}

/** Returns stored attribution, enriched with the PostHog distinct id if present. */
export function getAttribution(): Attribution | undefined {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return undefined
    const attr = JSON.parse(raw) as Attribution
    const ph = (window as unknown as { posthog?: { get_distinct_id?: () => string } }).posthog
    if (ph && typeof ph.get_distinct_id === 'function') {
      attr.distinct_id = ph.get_distinct_id()
    }
    return attr
  } catch {
    return undefined
  }
}
