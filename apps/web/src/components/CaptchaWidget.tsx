// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useEffect, useRef } from 'react'

// Cloudflare Turnstile and hCaptcha expose the same explicit-render JS API
// (render → widget id, reset, remove), so one component drives both. The
// provider + public site key come from the backend at runtime (/v1/system/info),
// so toggling captcha never needs a frontend rebuild.

export type CaptchaProvider = 'turnstile' | 'hcaptcha'

interface CaptchaApi {
  render: (el: HTMLElement, opts: Record<string, unknown>) => string
  reset: (id?: string) => void
  remove: (id: string) => void
}

const SCRIPTS: Record<CaptchaProvider, { src: string; global: string }> = {
  turnstile: {
    src: 'https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit',
    global: 'turnstile',
  },
  hcaptcha: {
    src: 'https://js.hcaptcha.com/1/api.js?render=explicit',
    global: 'hcaptcha',
  },
}

function loadScript(src: string): Promise<void> {
  return new Promise((resolve, reject) => {
    if (document.querySelector(`script[src="${src}"]`)) {
      resolve()
      return
    }
    const s = document.createElement('script')
    s.src = src
    s.async = true
    s.defer = true
    s.onload = () => resolve()
    s.onerror = () => reject(new Error('failed to load captcha script'))
    document.head.appendChild(s)
  })
}

function captchaApi(global: string): CaptchaApi | undefined {
  return (window as unknown as Record<string, CaptchaApi | undefined>)[global]
}

export function CaptchaWidget({
  provider,
  siteKey,
  onToken,
}: {
  provider: CaptchaProvider
  siteKey: string
  /** Called with the solved token, or `null` when it expires/errors. */
  onToken: (token: string | null) => void
}) {
  const ref = useRef<HTMLDivElement>(null)
  const widgetId = useRef<string | null>(null)
  // Keep the latest callback in a ref so re-renders don't re-mount the widget.
  const onTokenRef = useRef(onToken)
  onTokenRef.current = onToken

  useEffect(() => {
    let cancelled = false
    const { src, global } = SCRIPTS[provider]

    loadScript(src)
      .then(() => {
        const started = Date.now()
        const tryRender = () => {
          if (cancelled) return
          const api = captchaApi(global)
          if (api && ref.current && widgetId.current === null) {
            widgetId.current = api.render(ref.current, {
              sitekey: siteKey,
              callback: (token: string) => onTokenRef.current(token),
              'expired-callback': () => onTokenRef.current(null),
              'error-callback': () => onTokenRef.current(null),
            })
          } else if (!api && Date.now() - started < 10000) {
            setTimeout(tryRender, 100)
          }
        }
        tryRender()
      })
      .catch(() => onTokenRef.current(null))

    return () => {
      cancelled = true
      const api = captchaApi(SCRIPTS[provider].global)
      if (api && widgetId.current !== null) {
        try {
          api.remove(widgetId.current)
        } catch {
          /* widget already gone */
        }
        widgetId.current = null
      }
    }
  }, [provider, siteKey])

  return <div ref={ref} />
}
