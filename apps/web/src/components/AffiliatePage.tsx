// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useEffect, useState } from 'react'
import { useLocale } from '../useLocale'
import * as api from '../api/client'

export function AffiliatePage({ onBack }: { onBack: () => void }) {
  const { locale } = useLocale()
  const zh = locale === 'zh'
  const [info, setInfo] = useState<api.AffiliateInfo | null>(null)
  const [copied, setCopied] = useState(false)
  const [address, setAddress] = useState('')
  const [amount, setAmount] = useState('')
  const [currency, setCurrency] = useState('')
  const [payoutError, setPayoutError] = useState<string | null>(null)
  const [submitting, setSubmitting] = useState(false)

  const refresh = () => api.getAffiliate().then(setInfo).catch(() => {})
  useEffect(() => {
    refresh()
  }, [])

  const submitPayout = async (e: React.FormEvent) => {
    e.preventDefault()
    setPayoutError(null)
    const cur = currency || info?.balances[0]?.currency || 'usd'
    const cents = Math.round(parseFloat(amount) * 100)
    if (!address.trim() || !Number.isFinite(cents) || cents <= 0) {
      setPayoutError(zh ? '请填写地址和有效金额' : 'Enter an address and a valid amount')
      return
    }
    setSubmitting(true)
    try {
      await api.requestPayout(address.trim(), cur, cents)
      setAddress('')
      setAmount('')
      await refresh()
    } catch (err) {
      setPayoutError(err instanceof Error ? err.message : 'Request failed')
    } finally {
      setSubmitting(false)
    }
  }

  const fmtCur = (cents: number, cur: string) =>
    `${(cents / 100).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })} ${cur.toUpperCase()}`
  const link = info ? `${window.location.origin}/?ref=${info.code}` : ''
  const copy = () => {
    navigator.clipboard
      ?.writeText(link)
      .then(() => {
        setCopied(true)
        setTimeout(() => setCopied(false), 1500)
      })
      .catch(() => {})
  }

  const kindLabel = (k: string) =>
    zh
      ? { commission: '佣金', clawback: '退款冲回', payout: '已结算' }[k] ?? k
      : k

  const card: React.CSSProperties = {
    background: 'var(--surface)',
    border: '1px solid var(--border)',
    borderRadius: 8,
    padding: '16px 20px',
  }

  return (
    <div style={{ maxWidth: 760, margin: '0 auto', padding: '24px 20px' }}>
      <button type="button" className="btn btn-sm" onClick={onBack} style={{ marginBottom: 16 }}>
        ← {zh ? '返回' : 'Back'}
      </button>
      <h1 style={{ marginBottom: 4 }}>{zh ? '推荐返佣' : 'Affiliate'}</h1>
      <p style={{ color: 'var(--muted)', fontSize: 13, marginTop: 0 }}>
        {zh
          ? '分享你的专属链接,被推荐用户付费后你将获得佣金。'
          : 'Share your link — earn commission when referred users pay.'}
      </p>

      {!info ? (
        <p style={{ color: 'var(--muted)' }}>{zh ? '加载中…' : 'Loading…'}</p>
      ) : (
        <>
          {/* Referral link */}
          <div style={{ ...card, marginBottom: 16 }}>
            <div style={{ fontSize: 11, color: 'var(--muted)', marginBottom: 6 }}>
              {zh ? '你的推荐链接' : 'Your referral link'}
            </div>
            <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
              <input
                readOnly
                value={link}
                style={{
                  flex: 1,
                  padding: '8px 10px',
                  borderRadius: 4,
                  border: '1px solid var(--border)',
                  background: 'var(--panel)',
                  color: 'var(--fg)',
                  fontFamily: 'monospace',
                  fontSize: 13,
                }}
              />
              <button type="button" className="btn btn-primary btn-sm" onClick={copy}>
                {copied ? (zh ? '已复制' : 'Copied') : zh ? '复制' : 'Copy'}
              </button>
            </div>
            <div style={{ fontSize: 11, color: 'var(--muted)', marginTop: 8 }}>
              {zh ? '推荐码' : 'Code'}:{' '}
              <span style={{ fontFamily: 'monospace', color: 'var(--fg)' }}>{info.code}</span>
            </div>
          </div>

          {/* Stats */}
          <div style={{ display: 'flex', gap: 12, flexWrap: 'wrap', marginBottom: 20 }}>
            <div style={{ ...card, minWidth: 150 }}>
              <div style={{ fontSize: 11, color: 'var(--muted)', marginBottom: 4 }}>{zh ? '推荐用户' : 'Referrals'}</div>
              <div style={{ fontSize: 22, fontWeight: 700 }}>{info.referral_count.toLocaleString()}</div>
            </div>
            <div style={{ ...card, minWidth: 150 }}>
              <div style={{ fontSize: 11, color: 'var(--muted)', marginBottom: 4 }}>{zh ? '可结算余额' : 'Balance'}</div>
              <div style={{ fontSize: 18, fontWeight: 700 }}>
                {info.balances.length === 0
                  ? fmtCur(0, 'usd')
                  : info.balances.map((b) => <div key={b.currency}>{fmtCur(b.cents, b.currency)}</div>)}
              </div>
            </div>
            <div style={{ ...card, minWidth: 150 }}>
              <div style={{ fontSize: 11, color: 'var(--muted)', marginBottom: 4 }}>{zh ? '佣金比例' : 'Commission'}</div>
              <div style={{ fontSize: 22, fontWeight: 700 }}>{`${info.commission_pct}%`}</div>
            </div>
          </div>

          {/* Request payout (USDT) */}
          {info.balances.length > 0 && (
            <div style={{ ...card, marginBottom: 16 }}>
              <div style={{ fontSize: 11, color: 'var(--muted)', marginBottom: 8 }}>
                {zh ? '申请提现(USDT)' : 'Request payout (USDT)'}
              </div>
              <form onSubmit={submitPayout} style={{ display: 'flex', gap: 8, flexWrap: 'wrap', alignItems: 'center' }}>
                <input
                  placeholder={zh ? 'USDT 地址' : 'USDT address'}
                  value={address}
                  onChange={(e) => setAddress(e.target.value)}
                  style={{ flex: 2, minWidth: 200, padding: '8px 10px', borderRadius: 4, border: '1px solid var(--border)', background: 'var(--panel)', color: 'var(--fg)', fontSize: 13 }}
                />
                <select
                  value={currency || info.balances[0]?.currency || 'usd'}
                  onChange={(e) => setCurrency(e.target.value)}
                  style={{ padding: '8px 10px', borderRadius: 4, border: '1px solid var(--border)', background: 'var(--panel)', color: 'var(--fg)', fontSize: 13 }}
                >
                  {info.balances.map((b) => (
                    <option key={b.currency} value={b.currency}>{b.currency.toUpperCase()}</option>
                  ))}
                </select>
                <input
                  type="number"
                  step="0.01"
                  min="0"
                  placeholder={zh ? '金额' : 'Amount'}
                  value={amount}
                  onChange={(e) => setAmount(e.target.value)}
                  style={{ width: 120, padding: '8px 10px', borderRadius: 4, border: '1px solid var(--border)', background: 'var(--panel)', color: 'var(--fg)', fontSize: 13 }}
                />
                <button type="submit" className="btn btn-primary btn-sm" disabled={submitting}>
                  {submitting ? (zh ? '提交中…' : 'Submitting…') : zh ? '申请' : 'Request'}
                </button>
              </form>
              {payoutError && (
                <p style={{ color: '#ef4444', margin: '8px 0 0', fontSize: 12 }}>{payoutError}</p>
              )}
            </div>
          )}

          {/* Payout requests */}
          {info.payout_requests.length > 0 && (
            <div style={{ marginBottom: 20 }}>
              <h2 style={{ marginBottom: 8 }}>{zh ? '提现申请' : 'Payout requests'}</h2>
              <table className="workflow-table" style={{ maxWidth: 600 }}>
                <thead>
                  <tr>
                    <th>{zh ? '金额' : 'Amount'}</th>
                    <th>{zh ? '地址' : 'Address'}</th>
                    <th>{zh ? '状态' : 'Status'}</th>
                  </tr>
                </thead>
                <tbody>
                  {info.payout_requests.map((p) => (
                    <tr key={p.id}>
                      <td style={{ fontSize: 12, fontWeight: 600 }}>{fmtCur(p.amount_cents, p.currency)}</td>
                      <td style={{ fontSize: 11, color: 'var(--muted)', fontFamily: 'monospace', maxWidth: 200, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }} title={p.address}>{p.address}</td>
                      <td style={{ fontSize: 12 }}>
                        {zh
                          ? { requested: '待处理', paid: '已支付', rejected: '已拒绝' }[p.status] ?? p.status
                          : p.status}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}

          {/* Ledger */}
          <h2 style={{ marginBottom: 8 }}>{zh ? '账单明细' : 'Ledger'}</h2>
          {info.entries.length === 0 ? (
            <p style={{ color: 'var(--muted)', fontSize: 13 }}>
              {zh ? '暂无记录。' : 'No entries yet.'}
            </p>
          ) : (
            <table className="workflow-table" style={{ maxWidth: 600 }}>
              <thead>
                <tr>
                  <th>{zh ? '类型' : 'Type'}</th>
                  <th>{zh ? '金额' : 'Amount'}</th>
                  <th>{zh ? '来源' : 'Ref'}</th>
                </tr>
              </thead>
              <tbody>
                {info.entries.map((e) => (
                  <tr key={e.id}>
                    <td style={{ fontSize: 12 }}>{kindLabel(e.kind)}</td>
                    <td
                      style={{
                        fontSize: 12,
                        fontWeight: 600,
                        color: e.amount_cents >= 0 ? 'var(--success-text, #16a34a)' : 'var(--danger-text, #dc2626)',
                      }}
                    >
                      {e.amount_cents >= 0 ? '+' : ''}
                      {fmtCur(e.amount_cents, e.currency)}
                    </td>
                    <td style={{ fontSize: 11, color: 'var(--muted)', fontFamily: 'monospace' }}>
                      {e.referee_tenant ?? e.source_ref ?? '—'}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </>
      )}
    </div>
  )
}
