// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useEffect, useState } from 'react'
import { useLocale } from '../useLocale'
import * as api from '../api/client'

export function AdminPayoutsPage({ onBack }: { onBack: () => void }) {
  const { locale } = useLocale()
  const zh = locale === 'zh'
  const [requests, setRequests] = useState<api.PayoutRequest[] | null>(null)
  const [busy, setBusy] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  const refresh = () => api.listPendingPayouts().then(setRequests).catch(() => setRequests([]))
  useEffect(() => {
    refresh()
  }, [])

  const act = async (id: string, approve: boolean) => {
    setError(null)
    setBusy(id)
    try {
      await api.processPayout(id, approve)
      await refresh()
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed')
    } finally {
      setBusy(null)
    }
  }

  const fmtCur = (cents: number, cur: string) =>
    `${(cents / 100).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })} ${cur.toUpperCase()}`

  return (
    <div style={{ maxWidth: 860, margin: '0 auto', padding: '24px 20px' }}>
      <button type="button" className="btn btn-sm" onClick={onBack} style={{ marginBottom: 16 }}>
        ← {zh ? '返回' : 'Back'}
      </button>
      <h1 style={{ marginBottom: 4 }}>{zh ? '提现审批' : 'Payout Approvals'}</h1>
      <p style={{ color: 'var(--muted)', fontSize: 13, marginTop: 0 }}>
        {zh
          ? '批准将记入账本(借应付联盟 / 贷现金)并扣减联盟余额;实际转账请在链上完成。'
          : 'Approving books the payout (Dr affiliate-payable, Cr cash) and reduces the balance. Send the transfer out-of-band.'}
      </p>
      {error && <p style={{ color: '#ef4444', fontSize: 13 }}>{error}</p>}

      {!requests ? (
        <p style={{ color: 'var(--muted)' }}>{zh ? '加载中…' : 'Loading…'}</p>
      ) : requests.length === 0 ? (
        <p style={{ color: 'var(--muted)', fontSize: 13 }}>{zh ? '暂无待处理的提现申请。' : 'No pending payout requests.'}</p>
      ) : (
        <table className="workflow-table" style={{ width: '100%' }}>
          <thead>
            <tr>
              <th>{zh ? '联盟租户' : 'Affiliate'}</th>
              <th>{zh ? '金额' : 'Amount'}</th>
              <th>{zh ? '方式/地址' : 'Method / Address'}</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            {requests.map((r) => (
              <tr key={r.id}>
                <td style={{ fontSize: 12, fontFamily: 'monospace' }}>{r.tenant_id}</td>
                <td style={{ fontSize: 12, fontWeight: 600 }}>{fmtCur(r.amount_cents, r.currency)}</td>
                <td style={{ fontSize: 11, color: 'var(--muted)', fontFamily: 'monospace', maxWidth: 240, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }} title={r.address}>
                  {r.method.toUpperCase()} · {r.address}
                </td>
                <td style={{ whiteSpace: 'nowrap' }}>
                  <button
                    type="button"
                    className="btn btn-primary btn-sm"
                    disabled={busy === r.id}
                    onClick={() => act(r.id, true)}
                    style={{ marginRight: 6 }}
                  >
                    {zh ? '批准' : 'Approve'}
                  </button>
                  <button
                    type="button"
                    className="btn btn-sm"
                    disabled={busy === r.id}
                    onClick={() => act(r.id, false)}
                  >
                    {zh ? '拒绝' : 'Reject'}
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  )
}
