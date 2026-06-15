// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

// First-touch referral capture: a visitor arriving with ?ref=CODE has that
// affiliate code stored and sent on signup, linking them to the referrer.

const KEY = 'trigix_referral_v1'

/** Stores the ?ref code once (first-touch); a no-op if already captured. */
export function captureReferral(): void {
  try {
    if (localStorage.getItem(KEY)) return
    const ref = new URLSearchParams(window.location.search).get('ref')
    if (ref && ref.trim()) localStorage.setItem(KEY, ref.trim())
  } catch {
    /* localStorage unavailable — referral is best-effort */
  }
}

/** The captured referral code, if any. */
export function getReferralCode(): string | undefined {
  try {
    return localStorage.getItem(KEY) || undefined
  } catch {
    return undefined
  }
}
