// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { createContext, useContext, useState, useCallback, useMemo, useRef, type ReactNode } from 'react'
import { PiCheckCircle, PiXCircle, PiWarningCircle, PiInfo } from 'react-icons/pi'
import type { IconType } from 'react-icons'

// A single global, app-wide notification surface. Before this, feedback was
// split three ways — native alert() (blocking, unstyled), a couple of bespoke
// per-page toasts, and inline error text — so most successful actions gave the
// user no confirmation at all. useToast() unifies all of it: success/error/
// warning/info, auto-dismissing, stacked, click-to-dismiss.

export type ToastVariant = 'success' | 'error' | 'warning' | 'info'

export interface ToastApi {
  show: (message: string, variant?: ToastVariant) => void
  success: (message: string) => void
  error: (message: string) => void
  warning: (message: string) => void
  info: (message: string) => void
}

interface ToastItem { id: number; message: string; variant: ToastVariant }

const ICON: Record<ToastVariant, IconType> = {
  success: PiCheckCircle,
  error: PiXCircle,
  warning: PiWarningCircle,
  info: PiInfo,
}

const ToastContext = createContext<ToastApi | null>(null)

export function useToast(): ToastApi {
  const ctx = useContext(ToastContext)
  if (!ctx) throw new Error('useToast must be used within a ToastProvider')
  return ctx
}

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<ToastItem[]>([])
  const nextId = useRef(0)

  const remove = useCallback((id: number) => {
    setToasts((ts) => ts.filter((t) => t.id !== id))
  }, [])

  const show = useCallback((message: string, variant: ToastVariant = 'info') => {
    const id = nextId.current++
    setToasts((ts) => [...ts, { id, message, variant }])
    // Errors linger longer — they usually need to be read, not just glanced at.
    const ttl = variant === 'error' ? 6000 : 3500
    setTimeout(() => remove(id), ttl)
  }, [remove])

  const api = useMemo<ToastApi>(() => ({
    show,
    success: (m) => show(m, 'success'),
    error: (m) => show(m, 'error'),
    warning: (m) => show(m, 'warning'),
    info: (m) => show(m, 'info'),
  }), [show])

  return (
    <ToastContext.Provider value={api}>
      {children}
      <div className="toast-stack" role="status" aria-live="polite">
        {toasts.map((t) => {
          const Icon = ICON[t.variant]
          return (
            <div key={t.id} className={`toast toast-${t.variant}`} onClick={() => remove(t.id)} title="Dismiss">
              <Icon aria-hidden size={16} style={{ flexShrink: 0 }} />
              <span>{t.message}</span>
            </div>
          )
        })}
      </div>
    </ToastContext.Provider>
  )
}
