// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

// Turn a raw thrown error into a human-friendly, localized message. The API
// client throws `Error("<status> <statusText>: <body>")` (see api/client.ts),
// so we parse the leading status code and map the common ones to plain
// language. Network failures (fetch rejects with a TypeError) and unknown
// shapes fall back to a cleaned-up message rather than a raw "Error: …".

const HTTP_MESSAGES: Record<number, { zh: string; en: string }> = {
  400: { zh: '请求无效，请检查输入后重试。', en: 'Invalid request — check your input and try again.' },
  401: { zh: '登录已过期，请重新登录。', en: 'Your session has expired — please sign in again.' },
  403: { zh: '您没有权限执行此操作。', en: "You don't have permission to do this." },
  404: { zh: '找不到请求的资源。', en: 'The requested resource was not found.' },
  409: { zh: '操作冲突：资源已存在或已被他人修改。', en: 'Conflict — the resource already exists or was changed elsewhere.' },
  422: { zh: '提交的数据无效，请检查后重试。', en: 'The submitted data is invalid — please review and retry.' },
  429: { zh: '请求过于频繁，请稍后再试。', en: 'Too many requests — please slow down and try again.' },
  500: { zh: '服务器内部错误，请稍后重试。', en: 'Server error — please try again shortly.' },
  502: { zh: '服务暂时不可用，请稍后重试。', en: 'Service temporarily unavailable — please try again shortly.' },
  503: { zh: '服务暂时不可用，请稍后重试。', en: 'Service temporarily unavailable — please try again shortly.' },
  504: { zh: '请求超时，请稍后重试。', en: 'The request timed out — please try again.' },
}

// Codes where the server body usually carries the actionable detail (a
// validation message) worth appending to the friendly prefix.
const APPEND_BODY = new Set([400, 409, 422])

export function friendlyError(e: unknown, zh: boolean): string {
  const raw = e instanceof Error ? e.message : String(e)

  // Network-layer failure (fetch throws TypeError "Failed to fetch" etc.).
  if (/failed to fetch|networkerror|network request failed|load failed/i.test(raw)) {
    return zh ? '网络连接失败，请检查网络后重试。' : 'Network error — please check your connection and try again.'
  }

  // "<status> <statusText>: <body>", optionally prefixed by "Error:".
  const m = raw.match(/^(?:Error:\s*)?(\d{3})\b[^:]*:?\s*([\s\S]*)$/)
  if (m) {
    const code = parseInt(m[1], 10)
    const body = m[2].trim()
    const mapped = HTTP_MESSAGES[code]
    if (mapped) {
      const base = zh ? mapped.zh : mapped.en
      if (APPEND_BODY.has(code) && body && body.length <= 200) return `${base}（${body}）`
      return base
    }
  }

  // Unknown shape: drop a bare "Error:" prefix so it at least reads cleanly.
  return raw.replace(/^Error:\s*/, '') || (zh ? '发生未知错误。' : 'An unknown error occurred.')
}
