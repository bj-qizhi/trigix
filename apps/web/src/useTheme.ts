// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useEffect, useState } from 'react'

const KEY = 'velara-theme'

function getStored(): 'dark' | 'light' {
  try {
    return (localStorage.getItem(KEY) as 'dark' | 'light') ?? 'dark'
  } catch {
    return 'dark'
  }
}

function apply(theme: 'dark' | 'light') {
  if (theme === 'light') {
    document.documentElement.setAttribute('data-theme', 'light')
  } else {
    document.documentElement.removeAttribute('data-theme')
  }
}

export function useTheme() {
  const [theme, setTheme] = useState<'dark' | 'light'>(getStored)

  useEffect(() => {
    apply(theme)
    try { localStorage.setItem(KEY, theme) } catch { /* ignore */ }
  }, [theme])

  const toggle = () => setTheme((t) => (t === 'dark' ? 'light' : 'dark'))

  return { theme, toggle }
}

// Apply stored theme immediately on module load (before React renders)
apply(getStored())
