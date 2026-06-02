// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { Fragment, useEffect, useState } from 'react'
import { useAuth } from '../AuthContext'
import { useLocale } from '../useLocale'
import * as api from '../api/client'
import type { WebhookRecord } from '../types'
import { useTheme } from '../useTheme'
import logoWordmark from '../assets/logo-wordmark.svg'

interface Props {
  onBack: () => void
  onOpenWorkflow?: (workflowId: string) => void
}

export function WebhookPage({ onBack, onOpenWorkflow }: Props) {
  const { auth } = useAuth()
  const { theme, toggle: toggleTheme } = useTheme()
  const { t, locale } = useLocale()
  const zh = locale === 'zh'
  const [webhooks, setWebhooks] = useState<WebhookRecord[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [copiedToken, setCopiedToken] = useState<string | null>(null)
  const [expandedToken, setExpandedToken] = useState<string | null>(null)
  const [deliveries, setDeliveries] = useState<Record<string, api.WebhookDelivery[]>>({})
  const [testingToken, setTestingToken] = useState<string | null>(null)
  const [testPayload, setTestPayload] = useState('{\n  "event": "test"\n}')
  const [testResult, setTestResult] = useState<{ status: number; ok: boolean; body: string } | null>(null)
  const [testSending, setTestSending] = useState(false)
  const [editingConditionToken, setEditingConditionToken] = useState<string | null>(null)
  const [conditionInput, setConditionInput] = useState('')
  const [savingCondition, setSavingCondition] = useState(false)
  const [editingRateLimitToken, setEditingRateLimitToken] = useState<string | null>(null)
  const [rateLimitInput, setRateLimitInput] = useState('')
  const [savingRateLimit, setSavingRateLimit] = useState(false)
  const [rotatedSecret, setRotatedSecret] = useState<{ token: string; secret: string } | null>(null)
  const [rotatingToken, setRotatingToken] = useState<string | null>(null)
  const [editingTransformToken, setEditingTransformToken] = useState<string | null>(null)
  const [transformInput, setTransformInput] = useState('')
  const [savingTransform, setSavingTransform] = useState(false)

  const handleSendTest = async (token: string) => {
    let parsed: unknown
    try { parsed = JSON.parse(testPayload) } catch { alert(zh ? '无效的 JSON 格式' : 'Invalid JSON payload'); return }
    setTestSending(true)
    setTestResult(null)
    try {
      const res = await fetch(`/v1/webhooks/${token}`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(parsed),
      })
      const body = await res.text().catch(() => '')
      setTestResult({ status: res.status, ok: res.ok, body })
      if (res.ok) {
        const list = await api.listWebhookDeliveries(token, 20).catch(() => [])
        setDeliveries((prev) => ({ ...prev, [token]: list }))
      }
    } catch (e) {
      setTestResult({ status: 0, ok: false, body: String(e) })
    } finally {
      setTestSending(false)
    }
  }

  const handleSaveRateLimit = async (token: string) => {
    setSavingRateLimit(true)
    try {
      const parsed = rateLimitInput.trim() ? parseInt(rateLimitInput, 10) : null
      if (parsed !== null && (isNaN(parsed) || parsed < 1)) { alert(zh ? '请输入正整数' : 'Enter a positive integer'); return }
      const updated = await api.setWebhookRateLimit(token, parsed)
      setWebhooks((prev) => prev.map((w) => w.token === token ? { ...w, max_calls_per_minute: updated.max_calls_per_minute } : w))
      setEditingRateLimitToken(null)
    } catch (e) {
      setError(String(e))
    } finally {
      setSavingRateLimit(false)
    }
  }

  const handleSaveCondition = async (token: string) => {
    setSavingCondition(true)
    try {
      const updated = await api.setWebhookCondition(token, conditionInput.trim() || null)
      setWebhooks((prev) => prev.map((w) => w.token === token ? { ...w, condition_expr: updated.condition_expr } : w))
      setEditingConditionToken(null)
    } catch (e) {
      setError(String(e))
    } finally {
      setSavingCondition(false)
    }
  }

  const handleSaveTransform = async (token: string) => {
    setSavingTransform(true)
    try {
      const script = transformInput.trim() || null
      const updated = await api.setWebhookPayloadTransform(token, script)
      setWebhooks((prev) => prev.map((w) => w.token === token ? { ...w, payload_transform_script: updated.payload_transform_script } : w))
      setEditingTransformToken(null)
    } catch (e) {
      setError(String(e))
    } finally {
      setSavingTransform(false)
    }
  }

  const handleRotateSecret = async (token: string) => {
    if (!confirm(zh ? '确认轮换密钥？此操作不可撤销，旧密钥将立即失效。' : 'Rotate the webhook secret? The old secret will stop working immediately.')) return
    setRotatingToken(token)
    try {
      const result = await api.rotateWebhookSecret(token)
      setRotatedSecret({ token, secret: result.secret })
    } catch (e) {
      setError(String(e))
    } finally {
      setRotatingToken(null)
    }
  }

  const handleTogglePause = async (wh: WebhookRecord) => {
    try {
      const updated = wh.paused ? await api.resumeWebhook(wh.token) : await api.pauseWebhook(wh.token)
      setWebhooks((prev) => prev.map((w) => w.token === wh.token ? { ...w, paused: updated.paused } : w))
    } catch (e) {
      setError(String(e))
    }
  }

  const load = () => {
    setLoading(true)
    setError(null)
    api.listWebhooks(auth!.tenantId)
      .then(setWebhooks)
      .catch((e: unknown) => setError(String(e)))
      .finally(() => setLoading(false))
  }

  useEffect(load, [])

  const handleDelete = async (token: string) => {
    try {
      await api.deleteWebhook(auth!.tenantId, token)
      setWebhooks((prev) => prev.filter((w) => w.token !== token))
    } catch (e) {
      setError(String(e))
    }
  }

  const handleCopy = (text: string, token: string) => {
    navigator.clipboard.writeText(text).then(() => {
      setCopiedToken(token)
      setTimeout(() => setCopiedToken(null), 1500)
    })
  }

  const toggleDeliveries = async (token: string) => {
    if (expandedToken === token) {
      setExpandedToken(null)
      return
    }
    setExpandedToken(token)
    if (!deliveries[token]) {
      try {
        const list = await api.listWebhookDeliveries(token, 20)
        setDeliveries((prev) => ({ ...prev, [token]: list }))
      } catch {
        setDeliveries((prev) => ({ ...prev, [token]: [] }))
      }
    }
  }

  return (
    <div className="app">
      <header className="topbar">
        <button className="btn btn-sm btn-icon" onClick={onBack} title="Back">←</button>
        <span className="topbar-sep">|</span>
        <img src={logoWordmark} alt="Trigix" style={{ height: '28px' }} />
        <span className="topbar-sep">|</span>
        <span className="topbar-title">{t('webhook.title')}</span>
        <div className="topbar-actions">
          <button className="btn btn-sm" onClick={toggleTheme} title="Toggle dark/light theme">
            {theme === 'dark' ? '☀' : '◑'}
          </button>
        </div>
      </header>

      <div className="list-page">
        <div className="list-header">
          <h1>{t('webhook.title')}</h1>
        </div>

        <p style={{ marginBottom: 16, fontSize: 13, color: 'var(--muted)' }}>
          {zh
            ? 'Webhook 在编辑器中已发布版本的触发节点下创建。POST 到该 URL 启动执行；'
            : 'Webhooks are created from the editor under a published version\'s trigger node. POST to the URL to start an execution; include an'}{' '}
          <code style={{ background: 'var(--panel)', padding: '2px 5px', borderRadius: 4, fontSize: 12 }}>
            x-webhook-signature: sha256=&lt;hmac&gt;
          </code>
          {zh ? ' 请求头（如果配置了密钥）。' : ' header if a secret is configured.'}
        </p>

        {error && (
          <div style={{ color: 'var(--danger-text)', marginBottom: 12, fontSize: 13 }}>{error}</div>
        )}

        {loading ? (
          <div style={{ color: 'var(--muted)' }}>{zh ? '加载中…' : 'Loading…'}</div>
        ) : webhooks.length === 0 ? (
          <div className="empty-state">{zh ? '暂无 Webhook。发布版本后在触发节点下创建。' : 'No webhooks yet. Publish a version and create a webhook from the Trigger node.'}</div>
        ) : (
          <table className="workflow-table">
            <thead>
              <tr>
                <th>{t('webhook.col.url')}</th>
                <th>{t('webhook.col.workflow')}</th>
                <th>{t('webhook.col.version')}</th>
                <th>{t('webhook.col.secret')}</th>
                <th style={{ width: 120 }}></th>
              </tr>
            </thead>
            <tbody>
              {webhooks.map((wh) => (
                <Fragment key={wh.token}>
                <tr>
                  <td style={{ fontFamily: 'monospace', fontSize: 12 }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                      <span style={{ color: wh.paused ? 'var(--muted)' : 'var(--link)', textDecoration: wh.paused ? 'line-through' : undefined }}>/v1/webhooks/{wh.token.slice(0, 12)}…</span>
                      {wh.paused && <span className="badge badge-archived" style={{ fontSize: 10 }}>{zh ? '已暂停' : 'PAUSED'}</span>}
                      <button
                        className="btn btn-sm"
                        style={{ fontSize: 10, padding: '1px 6px' }}
                        onClick={() => handleCopy(`${window.location.origin}/v1/webhooks/${wh.token}`, wh.token)}
                        title={zh ? '复制完整 webhook URL' : 'Copy full webhook URL (with domain)'}
                      >
                        {copiedToken === wh.token ? '✓ Copied' : 'Copy'}
                      </button>
                    </div>
                  </td>
                  <td>
                    {onOpenWorkflow ? (
                      <button
                        className="btn btn-sm"
                        style={{ background: 'none', border: 'none', padding: 0, color: 'var(--link)', cursor: 'pointer', fontSize: 13 }}
                        onClick={() => onOpenWorkflow(wh.workflow_id)}
                        title={wh.workflow_id}
                      >
                        {wh.workflow_id.slice(0, 16)}…
                      </button>
                    ) : (
                      <code style={{ fontSize: 12 }}>{wh.workflow_id.slice(0, 16)}…</code>
                    )}
                  </td>
                  <td>
                    <code style={{ fontSize: 12, color: 'var(--muted)' }}>{wh.workflow_version_id.slice(0, 16)}…</code>
                  </td>
                  <td>
                    {wh.secret ? (
                      <span className="badge badge-running" style={{ fontSize: 11 }}>{zh ? '已加密' : 'Secured'}</span>
                    ) : (
                      <span style={{ color: 'var(--muted)', fontSize: 12 }}>{zh ? '无' : 'None'}</span>
                    )}
                  </td>
                  <td style={{ display: 'flex', gap: 6, flexWrap: 'wrap', alignItems: 'flex-start', flexDirection: 'column' }}>
                    <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap' }}>
                      <button
                        className="btn btn-sm btn-primary"
                        style={{ fontSize: 11 }}
                        onClick={() => { setTestingToken(wh.token); setTestResult(null) }}
                        title={zh ? '发送测试请求' : 'Send test request'}
                      >
                        ▶ {zh ? '测试' : 'Test'}
                      </button>
                      <button
                        className="btn btn-sm"
                        style={{ fontSize: 11 }}
                        onClick={() => void toggleDeliveries(wh.token)}
                        title="Show delivery history"
                      >
                        {expandedToken === wh.token ? (zh ? '▲ 历史' : '▲ History') : (zh ? '▼ 历史' : '▼ History')}
                      </button>
                      <button
                        className="btn btn-sm"
                        style={{ fontSize: 11 }}
                        onClick={() => { setEditingConditionToken(wh.token); setConditionInput(wh.condition_expr ?? '') }}
                        title={zh ? '设置触发条件' : 'Set payload condition filter'}
                      >
                        ⚙ {zh ? '条件' : 'Condition'}
                      </button>
                      <button
                        className={`btn btn-sm${wh.payload_transform_script ? ' btn-primary' : ''}`}
                        style={{ fontSize: 11 }}
                        onClick={() => { setEditingTransformToken(wh.token); setTransformInput(wh.payload_transform_script ?? '') }}
                        title={zh ? '设置 Rhai 脚本转换 payload' : 'Set Rhai script to transform the incoming payload'}
                      >
                        ⚡ {zh ? '转换' : 'Transform'}
                      </button>
                      <button
                        className="btn btn-sm"
                        style={{ fontSize: 11 }}
                        onClick={() => { setEditingRateLimitToken(wh.token); setRateLimitInput(String(wh.max_calls_per_minute ?? '')) }}
                        title={zh ? '设置每分钟最大调用次数' : 'Set max calls per minute'}
                      >
                        ⏱ {zh ? '频率' : 'Rate'}
                      </button>
                      <button
                        className="btn btn-sm"
                        style={{ fontSize: 11 }}
                        onClick={() => void handleTogglePause(wh)}
                        title={wh.paused ? (zh ? '已暂停 — 点击恢复' : 'Paused — click to resume') : (zh ? '点击暂停 webhook' : 'Click to pause webhook')}
                      >
                        {wh.paused ? (zh ? '▶ 恢复' : '▶ Resume') : (zh ? '⏸ 暂停' : '⏸ Pause')}
                      </button>
                      <button
                        className="btn btn-sm"
                        style={{ fontSize: 11 }}
                        disabled={rotatingToken === wh.token}
                        onClick={() => void handleRotateSecret(wh.token)}
                        title={zh ? '生成新密钥（旧密钥立即失效）' : 'Generate new secret (old secret immediately revoked)'}
                      >
                        {rotatingToken === wh.token ? '…' : (zh ? '🔑 轮换密钥' : '🔑 Rotate Secret')}
                      </button>
                      <button
                        className="btn btn-sm btn-danger"
                        onClick={() => handleDelete(wh.token)}
                      >
                        {zh ? '删除' : 'Delete'}
                      </button>
                    </div>
                    {wh.paused && (
                      <div style={{ fontSize: 10, color: 'var(--warning-text, #b45309)', marginTop: 2, fontWeight: 600 }}>
                        ⏸ {zh ? '已暂停 — 此 webhook 当前不接受请求' : 'PAUSED — this webhook is not accepting requests'}
                      </div>
                    )}
                    {wh.condition_expr && (
                      <div style={{ fontSize: 10, color: 'var(--muted)', fontFamily: 'monospace', marginTop: 2 }}>
                        🔍 {wh.condition_expr}
                      </div>
                    )}
                    {wh.max_calls_per_minute && (
                      <div style={{ fontSize: 10, color: 'var(--muted)', marginTop: 2 }}>
                        ⏱ {zh ? `限速: ${wh.max_calls_per_minute} 次/分钟` : `Rate limit: ${wh.max_calls_per_minute}/min`}
                      </div>
                    )}
                  </td>
                </tr>
                {expandedToken === wh.token && (
                  <tr>
                    <td colSpan={5} style={{ background: 'var(--panel)', padding: '0.75rem 1rem' }}>
                      <div style={{ fontSize: 12, fontWeight: 600, color: 'var(--muted)', marginBottom: 8 }}>
                        {zh ? '投递历史（最近 20 条）' : 'DELIVERY HISTORY (last 20)'}
                      </div>
                      {!deliveries[wh.token] ? (
                        <div style={{ color: 'var(--muted)', fontSize: 12 }}>{zh ? '加载中…' : 'Loading…'}</div>
                      ) : deliveries[wh.token].length === 0 ? (
                        <div style={{ color: 'var(--muted)', fontSize: 12 }}>{zh ? '暂无投递记录。' : 'No deliveries yet.'}</div>
                      ) : (
                        <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 12 }}>
                          <thead>
                            <tr style={{ color: 'var(--muted)', borderBottom: '1px solid var(--border)' }}>
                              <th style={{ textAlign: 'left', padding: '3px 6px' }}>{zh ? '时间' : 'Time'}</th>
                              <th style={{ textAlign: 'left', padding: '3px 6px' }}>{zh ? '状态' : 'Status'}</th>
                              <th style={{ textAlign: 'left', padding: '3px 6px' }}>{zh ? '执行' : 'Execution'}</th>
                              <th style={{ textAlign: 'left', padding: '3px 6px' }}>{zh ? '错误' : 'Error'}</th>
                            </tr>
                          </thead>
                          <tbody>
                            {deliveries[wh.token].map((d) => (
                              <tr key={d.id} style={{ borderBottom: '1px solid var(--border)' }}>
                                <td style={{ padding: '3px 6px', color: 'var(--muted)' }}>
                                  {new Date(d.delivered_at * 1000).toLocaleString()}
                                </td>
                                <td style={{ padding: '3px 6px' }}>
                                  <span style={{ color: d.success ? '#3fb950' : '#f85149', fontWeight: 600 }}>
                                    {d.status_code ?? '—'} {d.success ? '✓' : '✗'}
                                  </span>
                                </td>
                                <td style={{ padding: '3px 6px', fontFamily: 'monospace', fontSize: 11 }}>
                                  {d.execution_id ? d.execution_id.slice(0, 16) + '…' : '—'}
                                </td>
                                <td style={{ padding: '3px 6px', color: '#f85149', fontSize: 11 }}>
                                  {d.error_message ?? ''}
                                </td>
                              </tr>
                            ))}
                          </tbody>
                        </table>
                      )}
                    </td>
                  </tr>
                )}
                </Fragment>
              ))}
            </tbody>
          </table>
        )}
      </div>

      {testingToken && (
        <div className="modal-backdrop" onClick={() => setTestingToken(null)}>
          <div className="modal" style={{ maxWidth: 500 }} onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h3>▶ {zh ? '发送测试 Webhook 请求' : 'Send Test Webhook Request'}</h3>
              <button className="btn btn-sm" onClick={() => setTestingToken(null)}>✕</button>
            </div>
            <div style={{ padding: '16px 20px 20px', display: 'flex', flexDirection: 'column', gap: 12 }}>
              <div>
                <label style={{ fontSize: 12, fontWeight: 600, marginBottom: 4, display: 'block' }}>
                  {zh ? 'URL (POST)' : 'URL (POST)'}
                </label>
                <code style={{ fontSize: 11, color: 'var(--link)', background: 'var(--panel)', padding: '4px 8px', borderRadius: 4, display: 'block' }}>
                  /v1/webhooks/{testingToken.slice(0, 20)}…
                </code>
              </div>
              <div>
                <label style={{ fontSize: 12, fontWeight: 600, marginBottom: 4, display: 'block' }}>
                  {zh ? 'JSON 请求体' : 'JSON Payload'}
                </label>
                <textarea
                  value={testPayload}
                  onChange={(e) => setTestPayload(e.target.value)}
                  rows={6}
                  style={{ width: '100%', fontFamily: 'monospace', fontSize: 12, resize: 'vertical', padding: '6px 8px', boxSizing: 'border-box' }}
                />
              </div>
              {testResult && (
                <div style={{
                  padding: '10px 12px', borderRadius: 6,
                  background: testResult.ok ? 'rgba(22,163,74,0.07)' : 'rgba(220,38,38,0.07)',
                  border: `1px solid ${testResult.ok ? 'var(--success-text)' : 'var(--danger-text)'}`,
                }}>
                  <div style={{ fontWeight: 600, fontSize: 13, marginBottom: 6, color: testResult.ok ? 'var(--success-text)' : 'var(--danger-text)' }}>
                    {testResult.ok ? '✓' : '✗'} HTTP {testResult.status}
                  </div>
                  {testResult.body && (
                    <pre style={{ margin: 0, fontSize: 11, fontFamily: 'monospace', whiteSpace: 'pre-wrap', wordBreak: 'break-all', maxHeight: 120, overflowY: 'auto', color: 'var(--muted)' }}>
                      {testResult.body}
                    </pre>
                  )}
                </div>
              )}
              <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end' }}>
                <button className="btn" onClick={() => setTestingToken(null)}>{zh ? '关闭' : 'Close'}</button>
                <button
                  className="btn btn-primary"
                  disabled={testSending}
                  onClick={() => handleSendTest(testingToken)}
                >
                  {testSending ? (zh ? '发送中…' : 'Sending…') : (zh ? '发送请求' : 'Send Request')}
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {editingConditionToken && (
        <div className="modal-backdrop" onClick={() => setEditingConditionToken(null)}>
          <div className="modal" style={{ maxWidth: 460 }} onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h3>⚙ {zh ? '设置触发条件' : 'Set Payload Condition Filter'}</h3>
              <button className="btn btn-sm" onClick={() => setEditingConditionToken(null)}>✕</button>
            </div>
            <div style={{ padding: '16px 20px 20px', display: 'flex', flexDirection: 'column', gap: 12 }}>
              <p style={{ fontSize: 13, color: 'var(--muted)', margin: 0 }}>
                {zh
                  ? '仅当 JSON 载荷满足条件时才触发执行。语法：field.path == "value" 或 field.amount > 100'
                  : 'Only trigger an execution if the JSON payload matches this condition. Syntax: field.path == "value" or field.amount > 100'}
              </p>
              <input
                autoFocus
                value={conditionInput}
                onChange={(e) => setConditionInput(e.target.value)}
                placeholder={zh ? '如：event == "purchase" 或置空表示无条件' : 'e.g. event == "purchase"  (leave blank for no filter)'}
                onKeyDown={(e) => { if (e.key === 'Enter') void handleSaveCondition(editingConditionToken); if (e.key === 'Escape') setEditingConditionToken(null) }}
              />
              <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end' }}>
                <button className="btn" onClick={() => setEditingConditionToken(null)}>{zh ? '取消' : 'Cancel'}</button>
                <button
                  className="btn btn-primary"
                  disabled={savingCondition}
                  onClick={() => void handleSaveCondition(editingConditionToken)}
                >
                  {savingCondition ? (zh ? '保存中…' : 'Saving…') : (zh ? '保存' : 'Save')}
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
      {editingRateLimitToken && (
        <div className="modal-backdrop" onClick={() => setEditingRateLimitToken(null)}>
          <div className="modal" style={{ maxWidth: 400 }} onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h3>⏱ {zh ? '设置频率限制' : 'Set Rate Limit'}</h3>
              <button className="btn btn-sm" onClick={() => setEditingRateLimitToken(null)}>✕</button>
            </div>
            <div style={{ padding: '16px 20px 20px', display: 'flex', flexDirection: 'column', gap: 12 }}>
              <p style={{ fontSize: 13, color: 'var(--muted)', margin: 0 }}>
                {zh
                  ? '设置该 Webhook 每分钟最多可触发的执行次数（内存滑动窗口）。留空表示无限制。'
                  : 'Maximum executions this webhook can trigger per minute (in-memory sliding window). Leave blank for unlimited.'}
              </p>
              <input
                autoFocus
                type="number"
                min={1}
                value={rateLimitInput}
                onChange={(e) => setRateLimitInput(e.target.value)}
                placeholder={zh ? '如：60（留空表示无限制）' : 'e.g. 60  (blank = unlimited)'}
                onKeyDown={(e) => { if (e.key === 'Enter') void handleSaveRateLimit(editingRateLimitToken); if (e.key === 'Escape') setEditingRateLimitToken(null) }}
              />
              <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end' }}>
                <button className="btn" onClick={() => setEditingRateLimitToken(null)}>{zh ? '取消' : 'Cancel'}</button>
                <button
                  className="btn btn-primary"
                  disabled={savingRateLimit}
                  onClick={() => void handleSaveRateLimit(editingRateLimitToken)}
                >
                  {savingRateLimit ? (zh ? '保存中…' : 'Saving…') : (zh ? '保存' : 'Save')}
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
      {editingTransformToken && (
        <div className="modal-backdrop" onClick={() => setEditingTransformToken(null)}>
          <div className="modal" style={{ maxWidth: 560 }} onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h3>⚡ {zh ? 'Payload 转换脚本 (Rhai)' : 'Payload Transform Script (Rhai)'}</h3>
              <button className="btn btn-sm" onClick={() => setEditingTransformToken(null)}>✕</button>
            </div>
            <div style={{ padding: '1rem', display: 'flex', flexDirection: 'column', gap: 12 }}>
              <p style={{ fontSize: 12, color: 'var(--muted)' }}>
                {zh
                  ? '编写 Rhai 脚本转换传入的 payload。脚本可访问 `payload` 变量（JSON 对象），返回值将作为执行输入。留空则使用原始 payload。'
                  : 'Write a Rhai script to transform the incoming payload. The `payload` variable holds the JSON object; the return value becomes the execution input. Leave empty to use the raw payload.'}
              </p>
              <textarea
                value={transformInput}
                onChange={(e) => setTransformInput(e.target.value)}
                rows={8}
                style={{ fontFamily: 'monospace', fontSize: 12, padding: '8px', borderRadius: 4, border: '1px solid var(--border)', background: 'var(--surface)', resize: 'vertical' }}
                placeholder={`// Example: extract nested data\nlet data = payload["data"];\n#{"event": data["type"], "id": data["id"]}`}
              />
              <div style={{ display: 'flex', justifyContent: 'flex-end', gap: 8 }}>
                <button className="btn btn-sm" onClick={() => setEditingTransformToken(null)}>{zh ? '取消' : 'Cancel'}</button>
                <button
                  className="btn btn-sm btn-primary"
                  disabled={savingTransform}
                  onClick={() => void handleSaveTransform(editingTransformToken)}
                >
                  {savingTransform ? '…' : (zh ? '保存' : 'Save')}
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
      {rotatedSecret && (
        <div className="modal-backdrop" onClick={() => setRotatedSecret(null)}>
          <div className="modal" style={{ maxWidth: 480 }} onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h3>🔑 {zh ? '新密钥已生成' : 'New Secret Generated'}</h3>
              <button className="btn btn-sm" onClick={() => setRotatedSecret(null)}>✕</button>
            </div>
            <div style={{ padding: '1rem', display: 'flex', flexDirection: 'column', gap: 12 }}>
              <p style={{ fontSize: 13, color: 'var(--danger-text)' }}>
                ⚠ {zh ? '请立即复制此密钥！关闭此对话框后将无法再次查看。' : 'Copy this secret now — it will not be shown again after closing this dialog.'}
              </p>
              <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
                <code style={{ flex: 1, background: 'var(--panel)', padding: '8px 12px', borderRadius: 6, fontSize: 12, wordBreak: 'break-all', border: '1px solid var(--border)' }}>
                  {rotatedSecret.secret}
                </code>
                <button
                  className="btn btn-sm"
                  onClick={() => navigator.clipboard.writeText(rotatedSecret!.secret)}
                  title="Copy to clipboard"
                >
                  ⎘ {zh ? '复制' : 'Copy'}
                </button>
              </div>
              <p style={{ fontSize: 12, color: 'var(--muted)' }}>
                {zh ? '使用此密钥签名 webhook 请求：' : 'Use this secret to sign webhook requests:'}
                <code style={{ display: 'block', marginTop: 4, fontSize: 11, background: 'var(--panel)', padding: '4px 8px', borderRadius: 4 }}>
                  X-Webhook-Signature: sha256=&lt;HMAC-SHA256(secret, body)&gt;
                </code>
              </p>
              <div style={{ display: 'flex', justifyContent: 'flex-end' }}>
                <button className="btn btn-primary" onClick={() => setRotatedSecret(null)}>{zh ? '已复制，关闭' : 'Done'}</button>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
