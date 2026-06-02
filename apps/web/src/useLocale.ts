// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// Contact: managecode@gmail.com

import { useEffect, useState } from 'react'
import { t as translate, type Locale, type TranslationKey } from './i18n'

const KEY = 'aiworkflow-locale'

function getStored(): Locale {
  try {
    return (localStorage.getItem(KEY) as Locale) ?? 'zh'
  } catch {
    return 'zh'
  }
}

export function useLocale() {
  const [locale, setLocale] = useState<Locale>(getStored)

  useEffect(() => {
    try { localStorage.setItem(KEY, locale) } catch { /* ignore */ }
  }, [locale])

  const toggle = () => setLocale((l) => (l === 'zh' ? 'en' : 'zh'))
  const t = (key: TranslationKey) => translate(locale, key)

  return { locale, toggle, t }
}
