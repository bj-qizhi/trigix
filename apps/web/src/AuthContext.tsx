// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { createContext, useContext, useState, type ReactNode } from 'react'
import { type AuthInfo, getStoredAuth, storeAuth, clearAuth } from './auth'

interface AuthContextValue {
  auth: AuthInfo | null
  login: (info: AuthInfo) => void
  logout: () => void
}

const AuthContext = createContext<AuthContextValue | null>(null)

export function AuthProvider({ children }: { children: ReactNode }) {
  const [auth, setAuth] = useState<AuthInfo | null>(getStoredAuth)

  function login(info: AuthInfo) {
    storeAuth(info)
    setAuth(info)
  }

  function logout() {
    clearAuth()
    setAuth(null)
  }

  return <AuthContext.Provider value={{ auth, login, logout }}>{children}</AuthContext.Provider>
}

export function useAuth(): AuthContextValue {
  const ctx = useContext(AuthContext)
  if (!ctx) throw new Error('useAuth must be used inside AuthProvider')
  return ctx
}
