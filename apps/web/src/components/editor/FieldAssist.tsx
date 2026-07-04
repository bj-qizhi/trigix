// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

// Node-level AI assist: a small "✦ AI" button that sits next to a config field.
// Click it, describe what you want in plain language, and an LLM writes the
// value (a regex, a prompt, a code snippet, …) straight into the field. Reuses
// the copilot's stored LLM settings (provider / credential / key) so there's no
// extra setup once the copilot is configured.

import { useState } from 'react'
import { useAuth } from '../../AuthContext'
import { useLocale } from '../../useLocale'
import * as api from '../../api/client'

const LABEL: Record<string, [string, string]> = {
  regex: ['正则', 'regex'],
  prompt: ['提示词', 'prompt'],
  code: ['代码', 'code'],
  jsonpath: ['JSONPath', 'JSONPath'],
  template: ['模板', 'template'],
  sql: ['SQL', 'SQL'],
  jq: ['jq 过滤器', 'jq filter'],
  text: ['内容', 'value'],
}

const PLACEHOLDER: Record<string, [string, string]> = {
  regex: ['如：匹配中国大陆手机号', 'e.g. match a US phone number'],
  prompt: ['如：把输入总结成三点要点', 'e.g. summarize the input into 3 bullets'],
  code: ['如：过滤出 amount>100 的记录', 'e.g. keep records where amount > 100'],
  jsonpath: ['如：取所有订单的 id', 'e.g. all order ids'],
  template: ['如：拼出问候语', 'e.g. build a greeting line'],
  sql: ['如：查最近 7 天的订单数', 'e.g. count orders in the last 7 days'],
  jq: ['如：提取 .items[].name', 'e.g. extract .items[].name'],
  text: ['描述你想要的内容', 'describe what you want'],
}

export function FieldAssist({ kind, onInsert, context }: {
  kind: string
  onInsert: (value: string) => void
  context?: string
}) {
  const { auth } = useAuth()
  const { locale } = useLocale()
  const zh = locale === 'zh'
  const [open, setOpen] = useState(false)
  const [instruction, setInstruction] = useState('')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const label = (LABEL[kind] ?? LABEL.text)[zh ? 0 : 1]
  const placeholder = (PLACEHOLDER[kind] ?? PLACEHOLDER.text)[zh ? 0 : 1]

  const run = async () => {
    if (!instruction.trim()) return
    setLoading(true)
    setError(null)
    try {
      const provider = localStorage.getItem('af:cop_provider') ?? 'anthropic'
      const credential = localStorage.getItem('af:cop_credential') ?? ''
      const res = await api.assistField(kind, instruction.trim(), {
        context,
        tenantId: auth?.tenantId,
        provider: provider === 'custom' ? undefined : provider,
        baseUrl: provider === 'custom' ? (localStorage.getItem('af:cop_base_url') || undefined) : undefined,
        credentialName: credential || undefined,
        apiKey: credential ? undefined : (localStorage.getItem('af:claude_key') || undefined),
      })
      onInsert(res.value)
      setOpen(false)
      setInstruction('')
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setLoading(false)
    }
  }

  return (
    <span style={{ position: 'relative', display: 'inline-block' }}>
      <button
        type="button"
        className="btn btn-sm"
        title={zh ? `AI 帮我写${label}` : `AI: write this ${label} for me`}
        onClick={() => setOpen((o) => !o)}
        style={{ fontSize: 11, padding: '2px 6px', color: 'var(--node-claude)' }}
      >
        ✦ AI
      </button>
      {open && (
        <div style={{
          position: 'absolute', zIndex: 50, top: '100%', right: 0, marginTop: 4, width: 300,
          background: 'var(--surface)', border: '1px solid var(--border)', borderRadius: 6,
          padding: 8, boxShadow: '0 4px 16px rgba(0,0,0,0.18)',
        }}>
          <div style={{ fontSize: 11, color: 'var(--muted)', marginBottom: 4 }}>
            {zh ? `描述你想要的${label}：` : `Describe the ${label} you want:`}
          </div>
          <textarea
            value={instruction}
            onChange={(e) => setInstruction(e.target.value)}
            rows={2}
            placeholder={placeholder}
            autoFocus
            onKeyDown={(e) => { if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) run() }}
            style={{ width: '100%', fontSize: 12, boxSizing: 'border-box', resize: 'vertical' }}
          />
          {error && <div style={{ color: 'var(--danger-text)', fontSize: 11, marginTop: 4 }}>{error}</div>}
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginTop: 6 }}>
            <span style={{ fontSize: 10, color: 'var(--muted)' }}>{zh ? '用 Copilot 的模型设置' : 'uses Copilot LLM settings'}</span>
            <div style={{ display: 'flex', gap: 6 }}>
              <button type="button" className="btn btn-sm" onClick={() => setOpen(false)}>{zh ? '取消' : 'Cancel'}</button>
              <button type="button" className="btn btn-sm btn-primary" onClick={run} disabled={loading || !instruction.trim()}>
                {loading ? (zh ? '生成中…' : 'Generating…') : (zh ? '生成并插入' : 'Generate')}
              </button>
            </div>
          </div>
        </div>
      )}
    </span>
  )
}
