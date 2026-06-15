// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useEffect, useState } from 'react'
import { useAuth } from '../AuthContext'
import { loginUser, registerUser, acceptInvite, getInvitation, forgotPassword, resetPassword, verifyEmail, resendVerification, listPublicSso, getSystemInfo, type PublicSsoConnection } from '../api/client'
import { CaptchaWidget, type CaptchaProvider } from './CaptchaWidget'
import { useLocale } from '../useLocale'
import logoWordmark from '../assets/logo-wordmark.svg'

const API_BASE = import.meta.env.VITE_API_BASE ?? ''

const ROLE_OPTIONS = [
  { value: 'editor', label: 'Editor', desc: 'Create and edit workflows' },
  { value: 'admin',  label: 'Admin',  desc: 'Full access including API key management' },
  { value: 'viewer', label: 'Viewer', desc: 'Read-only access' },
]

function parseJwtPayload(token: string): Record<string, unknown> {
  try {
    const payload = token.split('.')[1]
    return JSON.parse(atob(payload.replace(/-/g, '+').replace(/_/g, '/')))
  } catch {
    return {}
  }
}

export function LoginPage() {
  const { login } = useAuth()
  const { locale, toggle: toggleLocale, t } = useLocale()
  const resetTokenParam = new URLSearchParams(window.location.search).get('reset') ?? ''
  const verifyTokenParam = new URLSearchParams(window.location.search).get('verify') ?? ''
  const [mode, setMode] = useState<'apikey' | 'email' | 'invite' | 'forgot' | 'reset' | 'verify'>(() => {
    const params = new URLSearchParams(window.location.search)
    if (params.has('reset')) return 'reset'
    if (params.has('invite')) return 'invite'
    if (params.has('verify')) return 'verify'
    return 'apikey'
  })
  const [apiKey, setApiKey] = useState('')
  const [role, setRole] = useState('editor')
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [isRegister, setIsRegister] = useState(false)
  const [name, setName] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)

  // Captcha (Turnstile / hCaptcha) — config comes from the backend at runtime.
  const [captchaProvider, setCaptchaProvider] = useState<CaptchaProvider | null>(null)
  const [captchaSiteKey, setCaptchaSiteKey] = useState<string | null>(null)
  const [captchaToken, setCaptchaToken] = useState<string | null>(null)

  useEffect(() => {
    getSystemInfo()
      .then((info) => {
        if (info.captcha_provider === 'turnstile' || info.captcha_provider === 'hcaptcha') {
          setCaptchaProvider(info.captcha_provider)
          setCaptchaSiteKey(info.captcha_site_key ?? null)
        }
      })
      .catch(() => {})
  }, [])

  // Enterprise SSO state
  const [ssoConns, setSsoConns] = useState<PublicSsoConnection[]>([])

  // Handle SSO callback redirect (?sso_token / ?sso_error) and load SSO buttons.
  useEffect(() => {
    const params = new URLSearchParams(window.location.search)
    const ssoToken = params.get('sso_token')
    const ssoError = params.get('sso_error')
    if (ssoError) {
      setError(ssoError)
      window.history.replaceState({}, '', window.location.pathname)
    }
    if (ssoToken) {
      const claims = parseJwtPayload(ssoToken)
      login({
        token: ssoToken,
        tenantId: (claims.tenant_id as string) || 'tenant-1',
        workspaceId: (claims.workspace_id as string) || 'workspace-1',
        projectId: (claims.project_id as string) || 'project-1',
        role: (claims.role as string) || 'editor',
        email: claims.email as string | undefined,
        emailVerified: true,
      })
      window.history.replaceState({}, '', window.location.pathname)
      return
    }
    listPublicSso().then(setSsoConns).catch(() => {})
  }, [login])

  // Forgot password state
  const [forgotEmail, setForgotEmail] = useState('')
  const [forgotSent, setForgotSent] = useState(false)

  // Reset password state
  const [resetNewPw, setResetNewPw] = useState('')
  const [resetConfirm, setResetConfirm] = useState('')
  const [resetDone, setResetDone] = useState(false)

  // Email verification state
  const [verifyDone, setVerifyDone] = useState(false)
  const [verifyError, setVerifyError] = useState<string | null>(null)
  const [verifyLoading, setVerifyLoading] = useState(false)
  const [resendEmail, setResendEmail] = useState('')
  const [, setResendSent] = useState(false)

  // After registration, nudge user to verify
  const [, setJustRegistered] = useState(false)

  async function handleForgotSubmit(e: React.FormEvent) {
    e.preventDefault()
    setError(null)
    setLoading(true)
    try {
      await forgotPassword(forgotEmail.trim())
      setForgotSent(true)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Request failed')
    } finally {
      setLoading(false)
    }
  }

  async function handleResetSubmit(e: React.FormEvent) {
    e.preventDefault()
    if (resetNewPw !== resetConfirm) { setError('Passwords do not match'); return }
    if (resetNewPw.length < 6) { setError('Password must be at least 6 characters'); return }
    setError(null)
    setLoading(true)
    try {
      await resetPassword(resetTokenParam, resetNewPw)
      setResetDone(true)
      window.history.replaceState({}, '', window.location.pathname)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Reset failed')
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    if (!verifyTokenParam || mode !== 'verify') return
    setVerifyLoading(true)
    verifyEmail(verifyTokenParam)
      .then(() => { setVerifyDone(true); window.history.replaceState({}, '', window.location.pathname) })
      .catch((err) => setVerifyError(err instanceof Error ? err.message : 'Verification failed'))
      .finally(() => setVerifyLoading(false))
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  async function handleResendVerification(e: React.FormEvent) {
    e.preventDefault()
    setError(null)
    setLoading(true)
    try {
      await resendVerification(resendEmail.trim())
      setResendSent(true)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Request failed')
    } finally {
      setLoading(false)
    }
  }

  // Invite-specific state
  const inviteToken = new URLSearchParams(window.location.search).get('invite') ?? ''
  const [inviteEmail, setInviteEmail] = useState<string | null>(null)
  const [invitePassword, setInvitePassword] = useState('')
  const [inviteName, setInviteName] = useState('')
  const [inviteConfirm, setInviteConfirm] = useState('')
  const [inviteInvalid, setInviteInvalid] = useState(false)

  useEffect(() => {
    if (!inviteToken) return
    getInvitation(inviteToken)
      .then((info) => setInviteEmail(info.email))
      .catch(() => setInviteInvalid(true))
  }, [inviteToken])

  async function handleInviteSubmit(e: React.FormEvent) {
    e.preventDefault()
    if (invitePassword !== inviteConfirm) { setError('Passwords do not match'); return }
    if (invitePassword.length < 6) { setError('Password must be at least 6 characters'); return }
    setError(null)
    setLoading(true)
    try {
      const data = await acceptInvite(inviteToken, invitePassword, inviteName || undefined)
      const claims = parseJwtPayload(data.token)
      login({
        token: data.token,
        tenantId: (claims.tenant_id as string) || data.user.tenant_id,
        workspaceId: (claims.workspace_id as string) || 'workspace-1',
        projectId: (claims.project_id as string) || 'project-1',
        role: (claims.role as string) || 'editor',
        email: data.user.email,
        emailVerified: data.user.email_verified,
      })
      window.history.replaceState({}, '', window.location.pathname)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to accept invitation')
    } finally {
      setLoading(false)
    }
  }

  async function handleApiKeySubmit(e: React.FormEvent) {
    e.preventDefault()
    setError(null)
    setLoading(true)
    try {
      const res = await fetch(`${API_BASE}/v1/auth/token`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ api_key: apiKey, role }),
      })
      if (!res.ok) {
        const body = await res.text().catch(() => '')
        throw new Error(body || `${res.status} ${res.statusText}`)
      }
      const data = await res.json() as {
        token: string
        tenant_id: string
        workspace_id: string
        project_id: string
        role: string
      }
      login({
        token: data.token,
        tenantId: data.tenant_id,
        workspaceId: data.workspace_id,
        projectId: data.project_id,
        role: data.role,
      })
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Login failed')
    } finally {
      setLoading(false)
    }
  }

  async function handleEmailSubmit(e: React.FormEvent) {
    e.preventDefault()
    setError(null)
    if (captchaProvider && captchaSiteKey && !captchaToken) {
      setError(locale === 'zh' ? '请先完成人机验证' : 'Please complete the captcha')
      return
    }
    setLoading(true)
    try {
      const data = isRegister
        ? await registerUser(email, password, name || undefined, undefined, captchaToken ?? undefined)
        : await loginUser(email, password, captchaToken ?? undefined)

      if (isRegister) setJustRegistered(true)
      const claims = parseJwtPayload(data.token)
      login({
        token: data.token,
        tenantId: (claims.tenant_id as string) || data.user.tenant_id,
        workspaceId: (claims.workspace_id as string) || 'workspace-1',
        projectId: (claims.project_id as string) || 'project-1',
        role: (claims.role as string) || 'editor',
        email: data.user.email,
        emailVerified: data.user.email_verified,
      })
    } catch (err) {
      setError(err instanceof Error ? err.message : isRegister ? 'Registration failed' : 'Login failed')
      // The captcha token is single-use; force a fresh solve before retrying.
      setCaptchaToken(null)
    } finally {
      setLoading(false)
    }
  }

  const inputStyle: React.CSSProperties = {
    padding: '0.5rem 0.75rem',
    borderRadius: '4px',
    border: '1px solid #334155',
    background: '#0f172a',
    color: '#f1f5f9',
    fontSize: '0.875rem',
    width: '100%',
    boxSizing: 'border-box',
  }

  const tabStyle = (active: boolean): React.CSSProperties => ({
    flex: 1,
    padding: '0.4rem 0',
    borderRadius: '4px',
    border: `1px solid ${active ? '#6366f1' : '#334155'}`,
    background: active ? '#312e81' : 'transparent',
    color: active ? '#c7d2fe' : '#94a3b8',
    fontSize: '0.8rem',
    cursor: 'pointer',
  })

  return (
    <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100vh', background: '#0f172a' }}>
      <div style={{ background: '#1e293b', padding: '2rem', borderRadius: '8px', minWidth: '340px', display: 'flex', flexDirection: 'column', gap: '1rem' }}>
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
          <img src={logoWordmark} alt="Trigix" style={{ height: '38px' }} />
          <button type="button" onClick={toggleLocale} style={{ background: 'rgba(255,255,255,0.1)', border: '1px solid rgba(255,255,255,0.2)', color: '#94a3b8', padding: '0.2rem 0.5rem', borderRadius: '4px', fontSize: '0.75rem', cursor: 'pointer' }}>
            {locale === 'zh' ? 'EN' : '中'}
          </button>
        </div>

        {/* Mode tabs — hidden in invite/verify mode */}
        {mode !== 'invite' && mode !== 'verify' && (
          <div style={{ display: 'flex', gap: '0.5rem' }}>
            <button type="button" style={tabStyle(mode === 'apikey')} onClick={() => { setMode('apikey'); setError(null) }}>
              {t('login.tab.apikey')}
            </button>
            <button type="button" style={tabStyle(mode === 'email')} onClick={() => { setMode('email'); setError(null) }}>
              {t('login.tab.email')}
            </button>
          </div>
        )}

        {mode === 'apikey' && (
          <form onSubmit={handleApiKeySubmit} style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }}>
            <p style={{ color: '#94a3b8', margin: 0, fontSize: '0.875rem' }}>{t('login.apikey.placeholder')}</p>
            <input
              type="password"
              placeholder={t('login.apikey.label')}
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              required
              style={inputStyle}
            />
            <p style={{ color: '#64748b', margin: '-0.4rem 0 0', fontSize: '0.75rem' }}>
              {locale === 'zh'
                ? <>首次使用?默认开发密钥为 <code>dev</code>(可通过 <code>DEV_API_KEY</code> 修改)。</>
                : <>First run? The default development key is <code>dev</code> (change it via <code>DEV_API_KEY</code>).</>}
            </p>
            <div>
              <label style={{ color: '#94a3b8', fontSize: '0.75rem', display: 'block', marginBottom: '0.4rem' }}>{t('login.role.label')}</label>
              <div style={{ display: 'flex', gap: '0.5rem' }}>
                {ROLE_OPTIONS.map(opt => (
                  <button
                    key={opt.value}
                    type="button"
                    title={opt.desc}
                    onClick={() => setRole(opt.value)}
                    style={tabStyle(role === opt.value)}
                  >
                    {opt.label}
                  </button>
                ))}
              </div>
            </div>
            {error && <p style={{ color: '#ef4444', margin: 0, fontSize: '0.8rem' }}>{error}</p>}
            <button type="submit" disabled={loading} className="btn btn-primary">
              {loading ? (locale === 'zh' ? '登录中…' : 'Signing in…') : t('login.signin')}
            </button>
          </form>
        )}

        {mode === 'invite' && (
          <form onSubmit={handleInviteSubmit} style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }}>
            {inviteInvalid ? (
              <p style={{ color: '#ef4444', margin: 0, fontSize: '0.875rem' }}>
                This invitation is invalid, expired, or already used.
              </p>
            ) : inviteEmail === null ? (
              <p style={{ color: '#94a3b8', margin: 0, fontSize: '0.875rem' }}>Checking invitation…</p>
            ) : (
              <>
                <p style={{ color: '#94a3b8', margin: 0, fontSize: '0.875rem' }}>
                  You've been invited as <strong style={{ color: '#f1f5f9' }}>{inviteEmail}</strong>. Create your password to get started.
                </p>
                <input type="text" placeholder="Your name (optional)" value={inviteName}
                  onChange={(e) => setInviteName(e.target.value)} style={inputStyle} />
                <input type="password" placeholder="Password" value={invitePassword} required
                  onChange={(e) => setInvitePassword(e.target.value)} style={inputStyle} />
                <input type="password" placeholder="Confirm password" value={inviteConfirm} required
                  onChange={(e) => setInviteConfirm(e.target.value)} style={inputStyle} />
                {error && <p style={{ color: '#ef4444', margin: 0, fontSize: '0.8rem' }}>{error}</p>}
                <button type="submit" disabled={loading || !invitePassword} className="btn btn-primary">
                  {loading ? 'Creating account…' : 'Accept invitation'}
                </button>
              </>
            )}
          </form>
        )}

        {mode === 'email' && (
          <form onSubmit={handleEmailSubmit} style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }}>
            <div style={{ display: 'flex', gap: '0.5rem' }}>
              <button type="button" style={tabStyle(!isRegister)} onClick={() => { setIsRegister(false); setError(null) }}>{t('login.signin')}</button>
              <button type="button" style={tabStyle(isRegister)} onClick={() => { setIsRegister(true); setError(null) }}>{t('login.register')}</button>
            </div>
            {isRegister && (
              <input
                type="text"
                placeholder={t('login.name.placeholder')}
                value={name}
                onChange={(e) => setName(e.target.value)}
                style={inputStyle}
              />
            )}
            <input
              type="email"
              placeholder={t('login.email.placeholder')}
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              required
              style={inputStyle}
            />
            <input
              type="password"
              placeholder={t('login.password.placeholder')}
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              required
              style={inputStyle}
            />
            {captchaProvider && captchaSiteKey && (
              <CaptchaWidget provider={captchaProvider} siteKey={captchaSiteKey} onToken={setCaptchaToken} />
            )}
            {error && <p style={{ color: '#ef4444', margin: 0, fontSize: '0.8rem' }}>{error}</p>}
            <button type="submit" disabled={loading} className="btn btn-primary">
              {loading ? (isRegister ? (locale === 'zh' ? '注册中…' : 'Registering…') : (locale === 'zh' ? '登录中…' : 'Signing in…')) : (isRegister ? t('login.register') : t('login.signin'))}
            </button>
            {!isRegister && (
              <button type="button" style={{ background: 'none', border: 'none', color: '#6366f1', fontSize: '0.8rem', cursor: 'pointer', padding: 0, textAlign: 'left' }}
                onClick={() => { setMode('forgot'); setError(null) }}>
                {t('login.forgot')}
              </button>
            )}
          </form>
        )}

        {(mode === 'apikey' || mode === 'email') && ssoConns.length > 0 && (
          <div style={{ marginTop: '1.25rem' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: '0.75rem', margin: '0.5rem 0 1rem' }}>
              <div style={{ flex: 1, height: 1, background: 'var(--border)' }} />
              <span style={{ color: '#94a3b8', fontSize: '0.75rem' }}>
                {locale === 'zh' ? '或使用企业账号' : 'or continue with'}
              </span>
              <div style={{ flex: 1, height: 1, background: 'var(--border)' }} />
            </div>
            <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
              {ssoConns.map((c) => (
                <button
                  key={c.slug}
                  type="button"
                  className="btn"
                  style={{ width: '100%' }}
                  onClick={() => { window.location.href = `${API_BASE}/v1/sso/${c.slug}/login` }}
                >
                  {locale === 'zh' ? `使用 ${c.provider} 登录` : `Sign in with ${c.provider}`}
                </button>
              ))}
            </div>
          </div>
        )}

        {mode === 'forgot' && (
          <form onSubmit={handleForgotSubmit} style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }}>
            {forgotSent ? (
              <>
                <p style={{ color: '#94a3b8', margin: 0, fontSize: '0.875rem' }}>
                  {t('login.forgot.sent')}
                </p>
                <button type="button" className="btn btn-primary" onClick={() => { setMode('email'); setForgotSent(false) }}>
                  {t('login.have.account')}
                </button>
              </>
            ) : (
              <>
                <p style={{ color: '#94a3b8', margin: 0, fontSize: '0.875rem' }}>{t('login.forgot.desc')}</p>
                <input type="email" placeholder={t('login.email.placeholder')} value={forgotEmail} required
                  onChange={(e) => setForgotEmail(e.target.value)} style={inputStyle} />
                {error && <p style={{ color: '#ef4444', margin: 0, fontSize: '0.8rem' }}>{error}</p>}
                <button type="submit" disabled={loading || !forgotEmail.trim()} className="btn btn-primary">
                  {loading ? (locale === 'zh' ? '发送中…' : 'Sending…') : t('login.forgot.send')}
                </button>
                <button type="button" style={{ background: 'none', border: 'none', color: '#6366f1', fontSize: '0.8rem', cursor: 'pointer', padding: 0 }}
                  onClick={() => { setMode('email'); setError(null) }}>
                  {t('nav.back')}
                </button>
              </>
            )}
          </form>
        )}

        {mode === 'verify' && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }}>
            {verifyLoading ? (
              <p style={{ color: '#94a3b8', margin: 0, fontSize: '0.875rem' }}>{t('login.verify.loading')}</p>
            ) : verifyDone ? (
              <>
                <p style={{ color: '#4ade80', margin: 0, fontSize: '0.875rem' }}>{t('login.verify.done')}</p>
                <button type="button" className="btn btn-primary" onClick={() => setMode('email')}>{t('login.signin')}</button>
              </>
            ) : (
              <>
                <p style={{ color: '#ef4444', margin: 0, fontSize: '0.875rem' }}>{verifyError ?? t('login.verify.error')}</p>
                <form onSubmit={handleResendVerification} style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
                  <p style={{ color: '#94a3b8', margin: 0, fontSize: '0.8rem' }}>{t('login.verify.resend.email')}:</p>
                  <input type="email" placeholder={t('login.email.placeholder')} value={resendEmail} required
                    onChange={(e) => setResendEmail(e.target.value)} style={inputStyle} />
                  {error && <p style={{ color: '#ef4444', margin: 0, fontSize: '0.8rem' }}>{error}</p>}
                  <button type="submit" disabled={loading || !resendEmail.trim()} className="btn btn-primary">
                    {loading ? (locale === 'zh' ? '发送中…' : 'Sending…') : t('login.verify.resend.btn')}
                  </button>
                </form>
                <button type="button" style={{ background: 'none', border: 'none', color: '#6366f1', fontSize: '0.8rem', cursor: 'pointer', padding: 0 }}
                  onClick={() => setMode('email')}>
                  {t('nav.back')}
                </button>
              </>
            )}
          </div>
        )}

        {mode === 'reset' && (
          <form onSubmit={handleResetSubmit} style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }}>
            {resetDone ? (
              <>
                <p style={{ color: '#94a3b8', margin: 0, fontSize: '0.875rem' }}>{t('login.reset.done')}</p>
                <button type="button" className="btn btn-primary" onClick={() => setMode('email')}>{t('login.signin')}</button>
              </>
            ) : (
              <>
                <p style={{ color: '#94a3b8', margin: 0, fontSize: '0.875rem' }}>{t('login.reset.title')}</p>
                <input type="password" placeholder={t('login.reset.newpw')} value={resetNewPw} required
                  onChange={(e) => setResetNewPw(e.target.value)} style={inputStyle} />
                <input type="password" placeholder={t('login.reset.confirm')} value={resetConfirm} required
                  onChange={(e) => setResetConfirm(e.target.value)} style={inputStyle} />
                {error && <p style={{ color: '#ef4444', margin: 0, fontSize: '0.8rem' }}>{error}</p>}
                <button type="submit" disabled={loading || !resetNewPw} className="btn btn-primary">
                  {loading ? (locale === 'zh' ? '更新中…' : 'Updating…') : t('login.reset.submit')}
                </button>
              </>
            )}
          </form>
        )}
      </div>
    </div>
  )
}
