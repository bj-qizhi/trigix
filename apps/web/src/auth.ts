export interface AuthInfo {
  token: string
  tenantId: string
  workspaceId: string
  projectId: string
}

const KEY = 'af_auth'

export function getStoredAuth(): AuthInfo | null {
  try {
    const raw = localStorage.getItem(KEY)
    return raw ? (JSON.parse(raw) as AuthInfo) : null
  } catch {
    return null
  }
}

export function storeAuth(info: AuthInfo): void {
  localStorage.setItem(KEY, JSON.stringify(info))
}

export function clearAuth(): void {
  localStorage.removeItem(KEY)
}
