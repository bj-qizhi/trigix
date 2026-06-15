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

  useEffect(() => {
    api.getAffiliate().then(setInfo).catch(() => {})
  }, [])

  const money = (c: number) =>
    `$${(c / 100).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`
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
            {[
              { label: zh ? '推荐用户' : 'Referrals', value: info.referral_count.toLocaleString() },
              { label: zh ? '可结算余额' : 'Balance', value: money(info.balance_cents) },
              { label: zh ? '佣金比例' : 'Commission', value: `${info.commission_pct}%` },
            ].map((s) => (
              <div key={s.label} style={{ ...card, minWidth: 150 }}>
                <div style={{ fontSize: 11, color: 'var(--muted)', marginBottom: 4 }}>{s.label}</div>
                <div style={{ fontSize: 22, fontWeight: 700 }}>{s.value}</div>
              </div>
            ))}
          </div>

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
                      {money(e.amount_cents)}
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
