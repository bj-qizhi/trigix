// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useEffect, useState, useCallback } from 'react'
import { useAuth } from '../AuthContext'
import * as api from '../api/client'
import { useTheme } from '../useTheme'
import { useLocale } from '../useLocale'
import logoWordmark from '../assets/logo-wordmark.svg'

interface Props {
  onBack: () => void
}

type Section = 'profile' | 'billing'

const PLANS = [
  {
    tier: 'free' as const,
    labelEn: 'Free',
    labelZh: '免费版',
    priceEn: 'Free',
    priceZh: '免费',
    execsEn: '1,000 execs/mo',
    execsZh: '1,000 次/月',
    concurrent: '10',
    workflows: '50',
  },
  {
    tier: 'pro' as const,
    labelEn: 'Pro',
    labelZh: '专业版',
    priceEn: '$29/mo',
    priceZh: '$29/月',
    execsEn: '50,000 execs/mo',
    execsZh: '50,000 次/月',
    concurrent: '50',
    workflows: '500',
  },
  {
    tier: 'business' as const,
    labelEn: 'Business',
    labelZh: '商业版',
    priceEn: '$99/mo',
    priceZh: '$99/月',
    execsEn: '500,000 execs/mo',
    execsZh: '500,000 次/月',
    concurrent: '200',
    workflows: '5,000',
  },
  {
    tier: 'enterprise' as const,
    labelEn: 'Enterprise',
    labelZh: '企业版',
    priceEn: 'Contact us',
    priceZh: '联系我们',
    execsEn: 'Unlimited',
    execsZh: '无限',
    concurrent: '∞',
    workflows: '∞',
  },
]

export function AccountPage({ onBack }: Props) {
  const { auth } = useAuth()
  const { theme, toggle: toggleTheme } = useTheme()
  const { locale, toggle: toggleLocale, t } = useLocale()
  const zh = locale === 'zh'

  const [section, setSection] = useState<Section>('profile')

  const [user, setUser] = useState<api.User | null>(null)
  const [loadError, setLoadError] = useState<string | null>(null)

  const [name, setName] = useState('')
  const [nameMsg, setNameMsg] = useState<string | null>(null)
  const [nameSaving, setNameSaving] = useState(false)

  const [currentPw, setCurrentPw] = useState('')
  const [newPw, setNewPw] = useState('')
  const [confirmPw, setConfirmPw] = useState('')
  const [pwMsg, setPwMsg] = useState<string | null>(null)
  const [pwSaving, setPwSaving] = useState(false)

  const [notifPrefs, setNotifPrefs] = useState<api.NotificationPrefs | null>(null)
  const [notifMsg, setNotifMsg] = useState<string | null>(null)
  const [notifSaving, setNotifSaving] = useState(false)

  const [billingStatus, setBillingStatus] = useState<api.BillingStatus | null>(null)
  const [usageHistory, setUsageHistory] = useState<api.UsageSummary[]>([])
  const [checkoutLoading, setCheckoutLoading] = useState<string | null>(null)
  const [portalLoading, setPortalLoading] = useState(false)
  const [billingMsg, setBillingMsg] = useState<string | null>(null)
  const [billingSuccess, setBillingSuccess] = useState(
    new URLSearchParams(window.location.search).get('billing') === 'success'
  )
  const [resetCountdown, setResetCountdown] = useState('')

  useEffect(() => {
    api.getCurrentUser()
      .then((u) => { setUser(u); setName(u.name ?? '') })
      .catch((e: unknown) => {
        const msg = String(e)
        if (msg.includes('401') || msg.toLowerCase().includes('unauthorized') || msg.toLowerCase().includes('authenticated')) {
          setLoadError(zh ? '此页面需要邮箱账号登录。' : 'This page requires email login.')
        } else {
          setLoadError(zh ? '加载用户信息失败' : 'Failed to load profile')
        }
      })
    api.getNotificationPrefs().then(setNotifPrefs).catch(() => {})
    api.getBillingStatus().then(setBillingStatus).catch(() => {})
    api.getBillingHistory(6).then(setUsageHistory).catch(() => {})
  }, [])

  // Update reset countdown every minute from the server-provided reset_in_secs
  useEffect(() => {
    if (!billingStatus) return
    let remaining = billingStatus.reset_in_secs
    const fmt = (secs: number) => {
      const d = Math.floor(secs / 86400)
      const h = Math.floor((secs % 86400) / 3600)
      const m = Math.floor((secs % 3600) / 60)
      if (d > 0) return zh ? `${d} 天 ${h} 小时后重置` : `resets in ${d}d ${h}h`
      if (h > 0) return zh ? `${h} 小时 ${m} 分后重置` : `resets in ${h}h ${m}m`
      return zh ? `${m} 分钟后重置` : `resets in ${m}m`
    }
    setResetCountdown(fmt(remaining))
    const id = setInterval(() => {
      remaining = Math.max(0, remaining - 60)
      setResetCountdown(fmt(remaining))
    }, 60_000)
    return () => clearInterval(id)
  }, [billingStatus, zh])

  // Auto-switch to billing section on return from Stripe
  useEffect(() => {
    if (billingSuccess) setSection('billing')
  }, [billingSuccess])

  const toggleNotifPref = useCallback(async (key: 'email_on_failure' | 'email_on_success') => {
    if (!notifPrefs) return
    const updated = { ...notifPrefs, [key]: !notifPrefs[key] }
    setNotifPrefs(updated)
    setNotifSaving(true)
    setNotifMsg(null)
    try {
      const saved = await api.updateNotificationPrefs({ email_on_failure: updated.email_on_failure, email_on_success: updated.email_on_success })
      setNotifPrefs(saved)
      setNotifMsg(zh ? '偏好已保存。' : 'Preferences saved.')
    } catch {
      setNotifMsg(zh ? '保存偏好失败。' : 'Failed to save preferences.')
    } finally {
      setNotifSaving(false)
    }
  }, [notifPrefs, zh])

  async function saveName() {
    if (!name.trim()) return
    setNameSaving(true)
    setNameMsg(null)
    try {
      const updated = await api.updateProfile({ name: name.trim() })
      setUser(updated)
      setNameMsg(zh ? '名称已更新。' : 'Name updated.')
    } catch {
      setNameMsg(zh ? '更新名称失败。' : 'Failed to update name.')
    } finally {
      setNameSaving(false)
    }
  }

  async function changePassword() {
    if (!currentPw || !newPw) { setPwMsg(zh ? '所有密码字段均为必填。' : 'All password fields are required.'); return }
    if (newPw !== confirmPw) { setPwMsg(zh ? '两次输入的新密码不一致。' : 'New passwords do not match.'); return }
    if (newPw.length < 6) { setPwMsg(zh ? '密码至少需要 6 个字符。' : 'Password must be at least 6 characters.'); return }
    setPwSaving(true)
    setPwMsg(null)
    try {
      await api.updateProfile({ current_password: currentPw, new_password: newPw })
      setCurrentPw(''); setNewPw(''); setConfirmPw('')
      setPwMsg(zh ? '密码修改成功。' : 'Password changed successfully.')
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e)
      setPwMsg(msg.toLowerCase().includes('401') || msg.toLowerCase().includes('incorrect')
        ? (zh ? '当前密码不正确。' : 'Current password is incorrect.')
        : (zh ? '修改密码失败。' : 'Failed to change password.'))
    } finally {
      setPwSaving(false)
    }
  }

  async function handleUpgrade(tier: string) {
    setCheckoutLoading(tier)
    setBillingMsg(null)
    try {
      const { url } = await api.createCheckoutSession(tier)
      window.location.href = url
    } catch {
      setBillingMsg(zh ? '创建支付会话失败，请稍后重试。' : 'Failed to start checkout. Please try again.')
      setCheckoutLoading(null)
    }
  }

  async function handlePortal() {
    setPortalLoading(true)
    setBillingMsg(null)
    try {
      const { url } = await api.createPortalSession()
      window.location.href = url
    } catch {
      setBillingMsg(zh ? '无法打开订阅管理页面。' : 'Failed to open customer portal.')
      setPortalLoading(false)
    }
  }

  const isError = (msg: string) =>
    msg.includes('Failed') || msg.includes('failed') || msg.includes('incorrect') || msg.includes('match') ||
    msg.includes('required') || msg.includes('characters') || msg.includes('失败') || msg.includes('不正确') ||
    msg.includes('不一致') || msg.includes('必填') || msg.includes('字符')

  const currentTier = billingStatus?.quota.tier ?? 'free'

  const navItems: { id: Section; labelEn: string; labelZh: string }[] = [
    { id: 'profile', labelEn: 'Profile', labelZh: '个人资料' },
    { id: 'billing', labelEn: 'Plan & Billing', labelZh: '套餐 & 账单' },
  ]

  return (
    <div className="app" data-theme={theme}>
      <header className="topbar">
        <img src={logoWordmark} alt="Trigix" style={{ height: '28px' }} />
        <div className="topbar-actions">
          <button className="btn btn-sm" onClick={onBack}>{t('nav.back')}</button>
          <button className="btn btn-sm" onClick={toggleTheme} title="Toggle theme">
            {theme === 'dark' ? '☀' : '◑'}
          </button>
          <button className="btn btn-sm" onClick={toggleLocale} title="切换语言 / Switch language">
            {locale === 'zh' ? 'EN' : '中'}
          </button>
        </div>
      </header>

      <div style={{ display: 'flex', height: 'calc(100vh - 52px)', overflow: 'hidden' }}>
        {/* Sidebar */}
        <nav style={{
          width: 200,
          flexShrink: 0,
          borderRight: '1px solid var(--border)',
          padding: '1.5rem 0',
          display: 'flex',
          flexDirection: 'column',
          gap: '0.25rem',
          background: 'var(--bg)',
        }}>
          {/* User identity */}
          {user && (
            <div style={{ padding: '0 1rem 1rem', borderBottom: '1px solid var(--border)', marginBottom: '0.5rem' }}>
              <div style={{
                width: 40, height: 40, borderRadius: '50%',
                background: 'var(--accent)',
                color: '#fff', display: 'flex', alignItems: 'center', justifyContent: 'center',
                fontWeight: 700, fontSize: '1.1rem', marginBottom: '0.5rem',
              }}>
                {(user.name || user.email).charAt(0).toUpperCase()}
              </div>
              <div style={{ fontSize: '0.8rem', fontWeight: 600, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                {user.name || user.email}
              </div>
              {user.name && (
                <div style={{ fontSize: '0.72rem', color: 'var(--fg-muted)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                  {user.email}
                </div>
              )}
              {auth?.role && (
                <div style={{ fontSize: '0.65rem', color: 'var(--accent)', fontWeight: 600, textTransform: 'uppercase', marginTop: '0.2rem' }}>
                  {auth.role}
                </div>
              )}
            </div>
          )}

          {navItems.map(item => (
            <button
              key={item.id}
              onClick={() => setSection(item.id)}
              style={{
                background: section === item.id ? 'var(--primary-faint, rgba(31,111,235,0.1))' : 'none',
                border: 'none',
                color: section === item.id ? 'var(--accent)' : 'var(--fg)',
                fontWeight: section === item.id ? 600 : 400,
                textAlign: 'left',
                padding: '0.5rem 1rem',
                cursor: 'pointer',
                fontSize: '0.875rem',
                borderRadius: '0 6px 6px 0',
                marginRight: '0.5rem',
              }}
            >
              {zh ? item.labelZh : item.labelEn}
            </button>
          ))}
        </nav>

        {/* Main content */}
        <main style={{ flex: 1, overflow: 'auto', padding: '2rem 2.5rem', maxWidth: 720 }}>
          {loadError && (
            <div style={{ background: 'var(--error-bg, #fef2f2)', border: '1px solid var(--error-border, #fecaca)', borderRadius: 8, padding: '0.75rem 1rem', color: 'var(--error)', marginBottom: '1.5rem', fontSize: '0.875rem' }}>
              {loadError}
            </div>
          )}

          {/* ─── Profile Section ─────────────────────────────── */}
          {section === 'profile' && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: '2rem' }}>
              <h2 style={{ margin: 0 }}>{zh ? '个人资料' : 'Profile'}</h2>

              <Card title={zh ? '显示名称' : 'Display Name'}>
                <input
                  className="input"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder={zh ? '您的名字' : 'Your name'}
                  onKeyDown={(e) => e.key === 'Enter' && saveName()}
                />
                <RowActions>
                  <button className="btn btn-primary" onClick={saveName} disabled={nameSaving}>
                    {nameSaving ? (zh ? '保存中…' : 'Saving…') : t('account.update.name')}
                  </button>
                  {nameMsg && <Msg text={nameMsg} error={isError(nameMsg)} />}
                </RowActions>
              </Card>

              <Card title={zh ? '修改密码' : 'Change Password'}>
                <input className="input" type="password" value={currentPw} onChange={(e) => setCurrentPw(e.target.value)} placeholder={t('account.current.pw')} />
                <input className="input" type="password" value={newPw} onChange={(e) => setNewPw(e.target.value)} placeholder={t('account.new.pw')} />
                <input className="input" type="password" value={confirmPw} onChange={(e) => setConfirmPw(e.target.value)} placeholder={t('account.confirm.pw')} onKeyDown={(e) => e.key === 'Enter' && changePassword()} />
                <RowActions>
                  <button className="btn btn-primary" onClick={changePassword} disabled={pwSaving}>
                    {pwSaving ? (zh ? '保存中…' : 'Saving…') : t('account.change.pw.btn')}
                  </button>
                  {pwMsg && <Msg text={pwMsg} error={isError(pwMsg)} />}
                </RowActions>
              </Card>

              {notifPrefs && (
                <Card title={zh ? '邮件通知' : 'Email Notifications'} subtitle={zh ? '工作流执行完成时发送通知邮件' : 'Receive email when workflow executions finish'}>
                  {(['email_on_failure', 'email_on_success'] as const).map((key) => (
                    <label key={key} style={{ display: 'flex', alignItems: 'center', gap: '0.75rem', cursor: 'pointer', padding: '0.25rem 0' }}>
                      <input type="checkbox" checked={notifPrefs[key]} onChange={() => toggleNotifPref(key)} disabled={notifSaving} />
                      <span style={{ fontSize: '0.875rem' }}>
                        {key === 'email_on_failure' ? t('account.notif.failure') : t('account.notif.success')}
                      </span>
                    </label>
                  ))}
                  {notifMsg && <Msg text={notifMsg} error={isError(notifMsg)} />}
                </Card>
              )}
            </div>
          )}

          {/* ─── Billing Section ─────────────────────────────── */}
          {section === 'billing' && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: '2rem' }}>
              <h2 style={{ margin: 0 }}>{zh ? '套餐 & 账单' : 'Plan & Billing'}</h2>

              {billingSuccess && (
                <div style={{ background: '#f0fdf4', border: '1px solid #bbf7d0', borderRadius: 8, padding: '0.75rem 1rem', display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                  <span style={{ color: '#15803d', fontSize: '0.875rem', fontWeight: 500 }}>
                    {zh ? '🎉 订阅成功！套餐已升级，立即生效。' : '🎉 Subscription activated! Your plan has been upgraded.'}
                  </span>
                  <button onClick={() => setBillingSuccess(false)} style={{ background: 'none', border: 'none', color: '#15803d', cursor: 'pointer', fontSize: '1.1rem', lineHeight: 1, padding: '0 0.25rem' }}>×</button>
                </div>
              )}

              {/* Current usage card */}
              {billingStatus && (
                <Card title={zh ? '当前用量' : 'Current Usage'}>
                  {/* Stat row */}
                  <div style={{ display: 'flex', gap: '1.5rem', flexWrap: 'wrap' }}>
                    <Stat label={zh ? '当前套餐' : 'Current Plan'} value={
                      <span style={{ display: 'flex', alignItems: 'center', gap: '0.4rem' }}>
                        <span style={{ textTransform: 'capitalize', fontWeight: 600 }}>{PLANS.find(p => p.tier === currentTier)?.[zh ? 'labelZh' : 'labelEn'] ?? currentTier}</span>
                        <TierBadge tier={currentTier} />
                      </span>
                    } />
                    <Stat
                      label={zh ? '本月执行次数' : 'Executions this month'}
                      value={`${billingStatus.usage.executions_used.toLocaleString()} / ${billingStatus.quota.max_executions_per_month >= 9007199254740991 ? '∞' : billingStatus.quota.max_executions_per_month.toLocaleString()}`}
                    />
                    <Stat
                      label={zh ? '并发上限' : 'Max concurrent'}
                      value={billingStatus.quota.max_concurrent_executions >= 9007199254740991 ? '∞' : String(billingStatus.quota.max_concurrent_executions)}
                    />
                    <Stat
                      label={zh ? '工作流上限' : 'Max workflows'}
                      value={billingStatus.quota.max_workflows >= 9007199254740991 ? '∞' : String(billingStatus.quota.max_workflows)}
                    />
                  </div>

                  {/* Usage bar */}
                  {billingStatus.quota.max_executions_per_month < 9007199254740991 && (
                    <div>
                      <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '0.75rem', color: 'var(--fg-muted)', marginBottom: '0.3rem' }}>
                        <span style={{ display: 'flex', alignItems: 'center', gap: '0.5rem' }}>
                          <span>{zh ? '本月用量' : 'This month'}</span>
                          {resetCountdown && (
                            <span style={{ color: 'var(--fg-muted)', fontStyle: 'italic' }}>({resetCountdown})</span>
                          )}
                        </span>
                        <span style={{ color: billingStatus.usage_pct >= 100 ? '#dc2626' : billingStatus.usage_pct >= 80 ? '#d97706' : 'var(--fg-muted)' }}>
                          {billingStatus.usage_pct.toFixed(1)}%
                        </span>
                      </div>
                      <div style={{ height: 6, background: 'var(--border)', borderRadius: 3, overflow: 'hidden' }}>
                        <div style={{
                          height: '100%',
                          width: `${Math.min(billingStatus.usage_pct, 100)}%`,
                          background: billingStatus.usage_pct >= 100 ? '#dc2626' : billingStatus.usage_pct >= 80 ? '#f59e0b' : '#22c55e',
                          borderRadius: 3,
                          transition: 'width 0.4s',
                        }} />
                      </div>
                    </div>
                  )}

                  {/* 6-month history chart */}
                  {usageHistory.length > 0 && (
                    <UsageHistoryChart
                      history={usageHistory}
                      quota={billingStatus.quota.max_executions_per_month}
                      zh={zh}
                    />
                  )}

                  {billingStatus.has_subscription && (
                    <div style={{ paddingTop: '0.75rem', borderTop: '1px solid var(--border)' }}>
                      <button className="btn btn-sm" onClick={handlePortal} disabled={portalLoading}>
                        {portalLoading ? (zh ? '跳转中…' : 'Redirecting…') : (zh ? '管理订阅 / 查看发票' : 'Manage Subscription & Invoices')}
                      </button>
                    </div>
                  )}
                </Card>
              )}

              {/* Pricing table — always show, gate upgrade button on stripe_enabled */}
              <div>
                <h3 style={{ margin: '0 0 1rem', fontSize: '1rem' }}>{zh ? '套餐对比' : 'Plans'}</h3>
                <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(160px, 1fr))', gap: '0.75rem' }}>
                  {PLANS.map((plan) => {
                    const isCurrent = currentTier === plan.tier
                    const isEnterprise = plan.tier === 'enterprise'
                    return (
                      <div
                        key={plan.tier}
                        style={{
                          border: `1.5px solid ${isCurrent ? 'var(--accent)' : 'var(--border)'}`,
                          borderRadius: 10,
                          padding: '1rem',
                          display: 'flex',
                          flexDirection: 'column',
                          gap: '0.5rem',
                          background: isCurrent ? 'var(--primary-faint, rgba(31,111,235,0.05))' : 'var(--bg)',
                          position: 'relative',
                          transition: 'border-color 0.2s',
                        }}
                      >
                        {isCurrent && (
                          <span style={{
                            position: 'absolute', top: -10, left: '50%', transform: 'translateX(-50%)',
                            background: 'var(--accent)', color: '#fff',
                            fontSize: '0.65rem', fontWeight: 700, padding: '2px 8px', borderRadius: 10,
                            whiteSpace: 'nowrap',
                          }}>
                            {zh ? '当前套餐' : 'Current Plan'}
                          </span>
                        )}
                        <div style={{ fontWeight: 700, fontSize: '0.95rem' }}>{zh ? plan.labelZh : plan.labelEn}</div>
                        <div style={{ fontSize: '1.1rem', fontWeight: 600, color: 'var(--accent)' }}>
                          {zh ? plan.priceZh : plan.priceEn}
                        </div>
                        <ul style={{ margin: 0, padding: '0 0 0 1rem', fontSize: '0.75rem', color: 'var(--fg-muted)', display: 'flex', flexDirection: 'column', gap: '0.2rem', flex: 1 }}>
                          <li>{zh ? plan.execsZh : plan.execsEn}</li>
                          <li>{zh ? `${plan.concurrent} 并发` : `${plan.concurrent} concurrent`}</li>
                          <li>{zh ? `${plan.workflows} 工作流` : `${plan.workflows} workflows`}</li>
                        </ul>
                        <div style={{ marginTop: '0.25rem' }}>
                          {isEnterprise ? (
                            <a
                              href="mailto:managecode@gmail.com"
                              className="btn btn-sm"
                              style={{ display: 'block', textAlign: 'center', textDecoration: 'none' }}
                            >
                              {zh ? '联系销售' : 'Contact Sales'}
                            </a>
                          ) : isCurrent ? (
                            <button className="btn btn-sm" disabled style={{ width: '100%', opacity: 0.5, cursor: 'default' }}>
                              {zh ? '当前套餐' : 'Current Plan'}
                            </button>
                          ) : billingStatus?.stripe_enabled ? (
                            <button
                              className="btn btn-sm btn-primary"
                              style={{ width: '100%' }}
                              disabled={checkoutLoading === plan.tier}
                              onClick={() => handleUpgrade(plan.tier)}
                            >
                              {checkoutLoading === plan.tier ? (zh ? '跳转中…' : '…') : (zh ? '立即升级' : 'Upgrade')}
                            </button>
                          ) : (
                            <div style={{ fontSize: '0.72rem', color: 'var(--fg-muted)', textAlign: 'center', padding: '0.25rem 0' }}>
                              {zh ? '配置 Stripe 后可用' : 'Configure Stripe to enable'}
                            </div>
                          )}
                        </div>
                      </div>
                    )
                  })}
                </div>
                {billingMsg && <p style={{ marginTop: '0.75rem', fontSize: '0.85rem', color: 'var(--error)' }}>{billingMsg}</p>}
              </div>
            </div>
          )}
        </main>
      </div>
    </div>
  )
}

// ── Small helper components ───────────────────────────────────────────────────

function Card({ title, subtitle, children }: { title: string; subtitle?: string; children: React.ReactNode }) {
  return (
    <div style={{ border: '1px solid var(--border)', borderRadius: 10, padding: '1.25rem 1.5rem', display: 'flex', flexDirection: 'column', gap: '0.75rem', background: 'var(--bg)' }}>
      <div>
        <div style={{ fontWeight: 600, fontSize: '0.95rem', marginBottom: subtitle ? '0.2rem' : 0 }}>{title}</div>
        {subtitle && <div style={{ fontSize: '0.8rem', color: 'var(--fg-muted)' }}>{subtitle}</div>}
      </div>
      {children}
    </div>
  )
}

function RowActions({ children }: { children: React.ReactNode }) {
  return <div style={{ display: 'flex', alignItems: 'center', gap: '0.75rem', flexWrap: 'wrap' }}>{children}</div>
}

function Msg({ text, error }: { text: string; error: boolean }) {
  return <span style={{ fontSize: '0.82rem', color: error ? 'var(--error)' : 'var(--success, #16a34a)' }}>{text}</span>
}

function Stat({ label, value }: { label: string; value: React.ReactNode }) {
  return (
    <div style={{ minWidth: 100 }}>
      <div style={{ fontSize: '0.72rem', color: 'var(--fg-muted)', marginBottom: '0.2rem', textTransform: 'uppercase', letterSpacing: '0.04em' }}>{label}</div>
      <div style={{ fontSize: '0.9rem', fontWeight: 600 }}>{value}</div>
    </div>
  )
}

function UsageHistoryChart({ history, quota, zh }: {
  history: api.UsageSummary[]
  quota: number
  zh: boolean
}) {
  const max = Math.max(...history.map(h => h.executions_used), 1)
  const isUnlimited = quota >= 9007199254740991

  const fmtMonth = (ym: string) => {
    const y = ym.slice(0, 4)
    const m = parseInt(ym.slice(4), 10)
    return zh
      ? `${m}月`
      : ['Jan','Feb','Mar','Apr','May','Jun','Jul','Aug','Sep','Oct','Nov','Dec'][m - 1] ?? ym
  }

  return (
    <div>
      <div style={{ fontSize: '0.72rem', color: 'var(--fg-muted)', textTransform: 'uppercase', letterSpacing: '0.04em', marginBottom: '0.5rem' }}>
        {zh ? '近 6 个月执行次数' : '6-Month Execution History'}
      </div>
      <div style={{ display: 'flex', alignItems: 'flex-end', gap: '4px', height: 56 }}>
        {[...history].reverse().map((h) => {
          const barPct = h.executions_used / max
          const quotaPct = isUnlimited ? 0 : h.executions_used / quota
          const barColor = quotaPct >= 1 ? '#dc2626' : quotaPct >= 0.8 ? '#f59e0b' : 'var(--accent)'
          const isCurrent = h.year_month === history[0].year_month
          return (
            <div key={h.year_month} style={{ flex: 1, display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 2 }}>
              <div
                title={`${fmtMonth(h.year_month)}: ${h.executions_used.toLocaleString()} execs`}
                style={{
                  width: '100%',
                  height: `${Math.max(barPct * 44, h.executions_used > 0 ? 3 : 1)}px`,
                  background: isCurrent ? barColor : `${barColor}88`,
                  borderRadius: '3px 3px 0 0',
                  transition: 'height 0.3s',
                  cursor: 'default',
                }}
              />
              <div style={{ fontSize: '0.6rem', color: isCurrent ? 'var(--fg)' : 'var(--fg-muted)', fontWeight: isCurrent ? 600 : 400, whiteSpace: 'nowrap' }}>
                {fmtMonth(h.year_month)}
              </div>
            </div>
          )
        })}
      </div>
      {!isUnlimited && (
        <div style={{ fontSize: '0.72rem', color: 'var(--fg-muted)', marginTop: '0.3rem', textAlign: 'right' }}>
          {zh ? `配额上限: ${quota.toLocaleString()}` : `Quota: ${quota.toLocaleString()}/mo`}
        </div>
      )}
    </div>
  )
}

function TierBadge({ tier }: { tier: string }) {
  const colors: Record<string, { bg: string; text: string }> = {
    free:       { bg: '#f3f4f6', text: '#6b7280' },
    pro:        { bg: '#ede9fe', text: 'var(--accent)' },
    business:   { bg: '#dbeafe', text: '#1d4ed8' },
    enterprise: { bg: '#fef3c7', text: '#b45309' },
  }
  const c = colors[tier] ?? colors.free
  return (
    <span style={{ fontSize: '0.65rem', fontWeight: 700, padding: '1px 7px', borderRadius: 10, background: c.bg, color: c.text, textTransform: 'uppercase', letterSpacing: '0.05em' }}>
      {tier}
    </span>
  )
}
