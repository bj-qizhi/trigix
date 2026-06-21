// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useEffect, useState } from 'react'
import type { Dispatch, SetStateAction } from 'react'
import { useAuth } from '../../AuthContext'
import * as api from '../../api/client'

// Global-search state for the workflow list: the modal open flag, the query and
// the debounced results. Extracted from WorkflowList so the search box and modal
// don't carry the debounce machinery inline.
export interface GlobalSearch {
  showGlobalSearch: boolean
  setShowGlobalSearch: Dispatch<SetStateAction<boolean>>
  globalQuery: string
  setGlobalQuery: Dispatch<SetStateAction<string>>
  globalResults: api.SearchResult | null
  globalSearching: boolean
}

export function useGlobalSearch(): GlobalSearch {
  const { auth } = useAuth()
  const [showGlobalSearch, setShowGlobalSearch] = useState(false)
  const [globalQuery, setGlobalQuery] = useState('')
  const [globalResults, setGlobalResults] = useState<api.SearchResult | null>(null)
  const [globalSearching, setGlobalSearching] = useState(false)

  useEffect(() => {
    if (!showGlobalSearch || globalQuery.trim().length < 2) { setGlobalResults(null); return }
    setGlobalSearching(true)
    const timer = setTimeout(() => {
      api.search(auth!.tenantId, globalQuery.trim())
        .then(setGlobalResults)
        .catch(() => {})
        .finally(() => setGlobalSearching(false))
    }, 300)
    return () => clearTimeout(timer)
  }, [globalQuery, showGlobalSearch])

  return { showGlobalSearch, setShowGlobalSearch, globalQuery, setGlobalQuery, globalResults, globalSearching }
}
