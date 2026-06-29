// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

// Auxiliary UI pieces for the workflow editor — the input-schema / variables /
// readme / schedule / forms modals, the recent-runs sidebar widget and the AI
// copilot side panel. Moved verbatim out of WorkflowEditor.tsx so that file
// holds only the editor component itself; each piece is already self-contained
// with explicit props.

import { useEffect, useRef, useState } from 'react'
import * as api from '../../api/client'
import type { ExecutionSummary, InputField } from '../../types'
import { useLocale } from '../../useLocale'
import { friendlyError } from '../../errorMessage'
import { IconKey, IconX} from '../uiIcons'
import type { FlowNode } from '../Canvas'

export function InputSchemaModal({
  schema, onChange, onClose,
}: {
  schema: InputField[]
  onChange: (s: InputField[]) => void
  onClose: () => void
}) {
  const { locale } = useLocale()
  const zh = locale === 'zh'
  const [fields, setFields] = useState<InputField[]>(schema)

  const addField = () => setFields((f) => [...f, { key: '', field_type: 'string', required: false, description: '', default_value: '' }])

  const updateField = (i: number, patch: Partial<InputField>) =>
    setFields((f) => f.map((x, j) => (j === i ? { ...x, ...patch } : x)))

  const removeField = (i: number) => setFields((f) => f.filter((_, j) => j !== i))

  const handleSave = () => {
    onChange(fields.filter((f) => f.key.trim()))
    onClose()
  }

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" style={{ width: 580, maxHeight: '80vh', overflow: 'auto' }} onClick={(e) => e.stopPropagation()}>
        <h2>{zh ? '输入模式' : 'Input Schema'}</h2>
        <p style={{ color: 'var(--muted)', fontSize: 12, marginBottom: 16 }}>
          {zh ? <>定义预期输入字段。运行面板将根据此模式显示表单。在节点配置中使用 <code>{'{{input.FIELD}}'}</code> 引用这些值。</> : <>Define the expected input fields. The run panel will show a form based on this schema. Use <code>{'{{input.FIELD}}'}</code> in node configs to reference these values.</>}
        </p>

        {fields.length === 0 && (
          <p style={{ color: 'var(--muted)', fontSize: 13, textAlign: 'center', padding: '12px 0' }}>
            {zh ? '暂无字段。在下方添加一个。' : 'No fields yet. Add one below.'}
          </p>
        )}

        {fields.map((f, i) => (
          <div key={i} style={{ display: 'grid', gridTemplateColumns: '1fr 100px 80px auto', gap: 8, marginBottom: 8, alignItems: 'center' }}>
            <input
              placeholder={zh ? '键名（如 lead_id）' : 'key (e.g. lead_id)'}
              value={f.key}
              onChange={(e) => updateField(i, { key: e.target.value })}
              style={{ fontFamily: 'monospace', fontSize: 12 }}
            />
            <select
              value={f.field_type}
              onChange={(e) => updateField(i, { field_type: e.target.value as InputField['field_type'] })}
              style={{ fontSize: 12 }}
            >
              <option value="string">string</option>
              <option value="number">number</option>
              <option value="boolean">boolean</option>
              <option value="json">json</option>
            </select>
            <label style={{ fontSize: 12, display: 'flex', alignItems: 'center', gap: 4, color: 'var(--muted)' }}>
              <input type="checkbox" checked={f.required} onChange={(e) => updateField(i, { required: e.target.checked })} />
              {zh ? '必填' : 'required'}
            </label>
            <button className="btn btn-sm btn-danger" onClick={() => removeField(i)}><IconX aria-hidden /></button>
            <input
              placeholder={zh ? '描述（可选）' : 'description (optional)'}
              value={f.description}
              onChange={(e) => updateField(i, { description: e.target.value })}
              style={{ fontSize: 12, gridColumn: '1 / 3' }}
            />
            <input
              placeholder={zh ? '默认值（可选）' : 'default value (optional)'}
              value={f.default_value ?? ''}
              onChange={(e) => updateField(i, { default_value: e.target.value })}
              style={{ fontSize: 12, gridColumn: '3 / 5' }}
            />
          </div>
        ))}

        <button className="btn btn-sm" onClick={addField} style={{ marginTop: 8, marginBottom: 16 }}>
          {zh ? '+ 添加字段' : '+ Add Field'}
        </button>

        <div className="modal-actions">
          <button className="btn" onClick={onClose}>{zh ? '取消' : 'Cancel'}</button>
          <button className="btn btn-primary" onClick={handleSave}>{zh ? '保存模式' : 'Save Schema'}</button>
        </div>
      </div>
    </div>
  )
}

export function VariablesModal({
  workflowId, tenantId, variables, onChanged, onClose,
}: {
  workflowId: string
  tenantId: string
  variables: api.Variable[]
  onChanged: (v: api.Variable[]) => void
  onClose: () => void
}) {
  const { locale } = useLocale()
  const zh = locale === 'zh'
  const [newKey, setNewKey] = useState('')
  const [newVal, setNewVal] = useState('')
  const [saving, setSaving] = useState(false)
  const [error, setError]   = useState<string | null>(null)

  const handleSet = async () => {
    if (!newKey.trim()) return
    let parsed: unknown
    try { parsed = JSON.parse(newVal) } catch { parsed = newVal }
    setSaving(true)
    setError(null)
    try {
      await api.setVariable(tenantId, workflowId, newKey.trim(), parsed)
      const updated = await api.listVariables(tenantId, workflowId)
      onChanged(updated)
      setNewKey('')
      setNewVal('')
    } catch (e) {
      setError(friendlyError(e, zh))
    } finally {
      setSaving(false)
    }
  }

  const handleDelete = async (key: string) => {
    try {
      await api.deleteVariable(tenantId, workflowId, key)
      onChanged(variables.filter((v) => v.key !== key))
    } catch (e) {
      setError(friendlyError(e, zh))
    }
  }

  const handleIncrement = async (key: string) => {
    try {
      const updated = await api.incrementVariable(tenantId, workflowId, key)
      onChanged(variables.map((v) => v.key === key ? updated : v))
    } catch (e) {
      setError(friendlyError(e, zh))
    }
  }

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" style={{ width: 520, maxHeight: '75vh', overflow: 'auto' }} onClick={(e) => e.stopPropagation()}>
        <h2>{zh ? '工作流变量' : 'Workflow Variables'}</h2>
        <p style={{ color: 'var(--muted)', fontSize: 12, marginBottom: 14 }}>
          {zh ? <>此工作流的持久化键值存储。在节点配置中通过 <code>{'{{variable.KEY}}'}</code> 访问。值在多次执行间保留。</> : <>Persistent key-value store for this workflow. Access via <code>{'{{variable.KEY}}'}</code> in node configs. Values survive across executions.</>}
        </p>
        {error && <p style={{ color: 'var(--danger-text)', fontSize: 12, marginBottom: 8 }}>{error}</p>}

        {variables.length === 0 ? (
          <p style={{ color: 'var(--muted)', fontSize: 13, textAlign: 'center', padding: '12px 0' }}>
            {zh ? '暂无变量。' : 'No variables yet.'}
          </p>
        ) : (
          <table style={{ width: '100%', borderCollapse: 'collapse', marginBottom: 16, fontSize: 12 }}>
            <thead>
              <tr style={{ borderBottom: '1px solid var(--border)' }}>
                <th style={{ textAlign: 'left', padding: '4px 8px', color: 'var(--muted)', fontWeight: 600 }}>{zh ? '键' : 'Key'}</th>
                <th style={{ textAlign: 'left', padding: '4px 8px', color: 'var(--muted)', fontWeight: 600 }}>{zh ? '值' : 'Value'}</th>
                <th style={{ width: 80 }}></th>
              </tr>
            </thead>
            <tbody>
              {variables.map((v) => (
                <tr key={v.key} style={{ borderBottom: '1px solid var(--border)' }}>
                  <td style={{ padding: '6px 8px', fontFamily: 'monospace', fontWeight: 600 }}>{v.key}</td>
                  <td style={{ padding: '6px 8px', fontFamily: 'monospace', color: 'var(--muted)', maxWidth: 240, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                    {JSON.stringify(v.value)}
                  </td>
                  <td style={{ padding: '4px 8px', display: 'flex', gap: 4 }}>
                    {typeof v.value === 'number' && (
                      <button className="btn btn-sm btn-icon" onClick={() => handleIncrement(v.key)} title={zh ? '加 1' : 'Increment by 1'} style={{ fontSize: 11 }}>+1</button>
                    )}
                    <button className="btn btn-sm btn-danger" onClick={() => handleDelete(v.key)} title={zh ? '删除变量' : 'Delete variable'} style={{ fontSize: 11 }}><IconX aria-hidden /></button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}

        <div style={{ display: 'flex', gap: 8, alignItems: 'center', marginBottom: 16 }}>
          <input
            placeholder={zh ? '键名' : 'key'}
            value={newKey}
            onChange={(e) => setNewKey(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && handleSet()}
            style={{ fontFamily: 'monospace', fontSize: 12, flex: '0 0 140px' }}
          />
          <input
            placeholder={zh ? '值（JSON 或字符串）' : 'value (JSON or string)'}
            value={newVal}
            onChange={(e) => setNewVal(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && handleSet()}
            style={{ fontFamily: 'monospace', fontSize: 12, flex: 1 }}
          />
          <button className="btn btn-sm btn-primary" disabled={!newKey.trim() || saving} onClick={handleSet}>
            {zh ? '设置' : 'Set'}
          </button>
        </div>

        <div className="modal-actions">
          <button className="btn" onClick={onClose}>{zh ? '关闭' : 'Close'}</button>
        </div>
      </div>
    </div>
  )
}

export function ReadmeModal({
  readme,
  onSave,
  onClose,
}: {
  readme: string
  onSave: (text: string) => Promise<void>
  onClose: () => void
}) {
  const { locale } = useLocale()
  const zh = locale === 'zh'
  const [text, setText] = useState(readme)
  const [saving, setSaving] = useState(false)

  const handleSave = async () => {
    setSaving(true)
    try { await onSave(text) } finally { setSaving(false) }
  }

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" style={{ width: 640, maxHeight: '85vh', display: 'flex', flexDirection: 'column', gap: 0, padding: 0 }} onClick={(e) => e.stopPropagation()}>
        <div style={{ padding: '16px 20px 12px', borderBottom: '1px solid var(--border)', display: 'flex', alignItems: 'center', justifyContent: 'space-between', flexShrink: 0 }}>
          <div>
            <h2 style={{ margin: 0, fontSize: 15 }}>{zh ? '工作流文档' : 'Workflow Documentation'}</h2>
            <p style={{ margin: '3px 0 0', fontSize: 12, color: 'var(--muted)' }}>
              {zh ? '支持 Markdown。描述输入、输出、依赖项和使用说明。' : 'Markdown supported. Describe inputs, outputs, dependencies, and usage notes.'}
            </p>
          </div>
          <button className="btn btn-sm" onClick={onClose}><IconX aria-hidden /></button>
        </div>
        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: 0, overflow: 'hidden' }}>
          <textarea
            autoFocus
            value={text}
            onChange={(e) => setText(e.target.value)}
            placeholder={zh ? `# 我的工作流\n\n描述此工作流的功能、期望的输入以及输出内容。\n\n## 输入\n- \`lead_id\` — 要处理的线索\n\n## 输出\n返回丰富后的线索记录。` : `# My Workflow\n\nDescribe what this workflow does, what inputs it expects, and what it outputs.\n\n## Inputs\n- \`lead_id\` — The lead to process\n\n## Outputs\nReturns the enriched lead record.`}
            style={{
              flex: 1, width: '100%', minHeight: 340, resize: 'none',
              fontFamily: 'monospace', fontSize: 13, padding: '14px 20px',
              border: 'none', outline: 'none', background: 'var(--bg)',
              color: 'var(--fg)', boxSizing: 'border-box', lineHeight: 1.6,
            }}
          />
        </div>
        <div style={{ padding: '10px 20px', borderTop: '1px solid var(--border)', display: 'flex', justifyContent: 'flex-end', gap: 8, flexShrink: 0 }}>
          <span style={{ fontSize: 11, color: 'var(--muted)', alignSelf: 'center', marginRight: 'auto' }}>
            {zh ? `${text.length} 个字符` : `${text.length} chars`}
          </span>
          {text && (
            <button className="btn btn-sm btn-danger" onClick={() => setText('')} disabled={saving}>{zh ? '清除' : 'Clear'}</button>
          )}
          <button className="btn" onClick={onClose}>{zh ? '取消' : 'Cancel'}</button>
          <button className="btn btn-primary" onClick={handleSave} disabled={saving}>
            {saving ? (zh ? '保存中…' : 'Saving…') : (zh ? '保存' : 'Save')}
          </button>
        </div>
      </div>
    </div>
  )
}

export function ScheduleModal({
  triggerNode,
  onSave,
  onClose,
}: {
  triggerNode: FlowNode | null
  onSave: (config: Record<string, unknown>) => void
  onClose: () => void
}) {
  const cfg = (triggerNode?.data.config ?? {}) as Record<string, unknown>
  const initCron = (cfg.cron_expression as string) ?? ''
  const initInterval = (cfg.interval_secs as number) ?? 0
  const initMode = initCron ? 'cron' : initInterval > 0 ? 'interval' : 'none'

  const { locale } = useLocale()
  const zh = locale === 'zh'
  const [mode, setMode] = useState<'none' | 'interval' | 'cron'>(initMode)
  const [cronExpr, setCronExpr] = useState(initCron || '0 9 * * 1-5')
  const [intervalSecs, setIntervalSecs] = useState(initInterval > 0 ? initInterval : 3600)
  const [nextTimes, setNextTimes] = useState<string[]>([])
  const [cronError, setCronError] = useState<string | null>(null)
  const [previewing, setPreviewing] = useState(false)

  useEffect(() => {
    if (mode !== 'cron') { setNextTimes([]); setCronError(null); return }
    const id = setTimeout(async () => {
      if (!cronExpr.trim()) return
      setPreviewing(true)
      try {
        const res = await api.previewCron(cronExpr.trim(), 5)
        if (res.error) { setCronError(res.error); setNextTimes([]) }
        else { setCronError(null); setNextTimes(res.next_times) }
      } catch { setCronError('Preview failed') }
      finally { setPreviewing(false) }
    }, 500)
    return () => clearTimeout(id)
  }, [cronExpr, mode])

  const handleSave = () => {
    if (mode === 'none') {
      onSave({ cron_expression: undefined, interval_secs: undefined })
    } else if (mode === 'interval') {
      onSave({ cron_expression: undefined, interval_secs: intervalSecs })
    } else {
      if (cronError) return
      onSave({ interval_secs: undefined, cron_expression: cronExpr.trim() })
    }
  }

  const PRESETS = zh ? [
    { label: '工作日 9 点',   expr: '0 9 * * 1-5' },
    { label: '每小时',        expr: '0 * * * *' },
    { label: '每天午夜',      expr: '0 0 * * *' },
    { label: '周一 8 点',     expr: '0 8 * * 1' },
    { label: '每 15 分钟',    expr: '*/15 * * * *' },
  ] : [
    { label: 'Weekdays 9am',     expr: '0 9 * * 1-5' },
    { label: 'Every hour',       expr: '0 * * * *' },
    { label: 'Daily midnight',   expr: '0 0 * * *' },
    { label: 'Mon 8am',          expr: '0 8 * * 1' },
    { label: 'Every 15 min',     expr: '*/15 * * * *' },
  ]

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" style={{ width: 480 }} onClick={(e) => e.stopPropagation()}>
        <h2 style={{ marginBottom: 4 }}>{zh ? '自动运行计划' : 'Auto-Run Schedule'}</h2>
        <p style={{ color: 'var(--muted)', fontSize: 12, marginBottom: 16 }}>
          {zh ? '配置触发节点以自动运行。计划在发布后生效。' : 'Configure the trigger node to run automatically. Schedule activates after publishing.'}
        </p>

        {!triggerNode && (
          <p style={{ color: 'var(--danger-text)', fontSize: 12, marginBottom: 12 }}>
            {zh ? '未找到触发节点，请先在画布中添加一个。' : 'No trigger node found. Add one to the canvas first.'}
          </p>
        )}

        <div className="field">
          <label>{zh ? '计划类型' : 'Schedule type'}</label>
          <select value={mode} onChange={(e) => setMode(e.target.value as typeof mode)} disabled={!triggerNode}>
            <option value="none">{zh ? '无（手动 / Webhook）' : 'None (manual / webhook only)'}</option>
            <option value="interval">{zh ? '固定间隔' : 'Fixed interval'}</option>
            <option value="cron">{zh ? 'Cron 表达式' : 'Cron expression'}</option>
          </select>
        </div>

        {mode === 'interval' && (
          <div className="field">
            <label>{zh ? '间隔' : 'Interval'}</label>
            <select value={intervalSecs} onChange={(e) => setIntervalSecs(Number(e.target.value))}>
              <option value={60}>{zh ? '每分钟' : 'Every minute'}</option>
              <option value={300}>{zh ? '每 5 分钟' : 'Every 5 minutes'}</option>
              <option value={900}>{zh ? '每 15 分钟' : 'Every 15 minutes'}</option>
              <option value={1800}>{zh ? '每 30 分钟' : 'Every 30 minutes'}</option>
              <option value={3600}>{zh ? '每小时' : 'Every hour'}</option>
              <option value={21600}>{zh ? '每 6 小时' : 'Every 6 hours'}</option>
              <option value={86400}>{zh ? '每天' : 'Every day'}</option>
            </select>
          </div>
        )}

        {mode === 'cron' && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
            <div className="field">
              <label>{zh ? 'Cron 表达式' : 'Cron expression'} <span style={{ color: 'var(--muted)', fontWeight: 400 }}>{zh ? '（5 字段 UTC）' : '(5-field UTC)'}</span></label>
              <input
                value={cronExpr}
                onChange={(e) => setCronExpr(e.target.value)}
                placeholder="0 9 * * 1-5"
                style={{ fontFamily: 'monospace' }}
              />
              {cronError && <p style={{ color: 'var(--danger-text)', fontSize: 11, marginTop: 4 }}>{cronError}</p>}
            </div>
            <div>
              <div style={{ fontSize: 11, color: 'var(--muted)', marginBottom: 4 }}>{zh ? '预设：' : 'Presets:'}</div>
              <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6 }}>
                {PRESETS.map((p) => (
                  <button
                    key={p.expr}
                    className={`btn btn-sm${cronExpr === p.expr ? ' btn-primary' : ''}`}
                    onClick={() => setCronExpr(p.expr)}
                    style={{ fontSize: 11 }}
                  >
                    {p.label}
                  </button>
                ))}
              </div>
            </div>
            <div>
              <div style={{ fontSize: 11, color: 'var(--muted)', marginBottom: 4 }}>
                {zh ? '接下来 5 次运行（UTC）：' : 'Next 5 runs (UTC):'} {previewing && '…'}
              </div>
              {nextTimes.length > 0 && (
                <ul style={{ margin: 0, padding: 0, listStyle: 'none', fontSize: 12, fontFamily: 'monospace', color: 'var(--fg)' }}>
                  {nextTimes.map((t, i) => (
                    <li key={i} style={{ padding: '2px 0', color: i === 0 ? 'var(--link)' : undefined }}>{t}</li>
                  ))}
                </ul>
              )}
              {!previewing && !cronError && nextTimes.length === 0 && cronExpr.trim() && (
                <div style={{ fontSize: 11, color: 'var(--muted)' }}>{zh ? '请输入有效的 Cron 表达式以预览。' : 'Enter a valid cron expression to preview.'}</div>
              )}
            </div>
          </div>
        )}

        <div className="modal-actions" style={{ marginTop: 20 }}>
          <button className="btn" onClick={onClose}>{zh ? '取消' : 'Cancel'}</button>
          <button
            className="btn btn-primary"
            onClick={handleSave}
            disabled={!triggerNode || (mode === 'cron' && !!cronError)}
          >
            {zh ? '应用到触发节点' : 'Apply to Trigger Node'}
          </button>
        </div>
        <p style={{ fontSize: 11, color: 'var(--muted)', marginTop: 8 }}>
          {zh ? <>修改在 <strong>保存版本</strong> + <strong>发布</strong> 后生效。</> : <>Changes take effect after <strong>Save Version</strong> + <strong>Publish</strong>.</>}
        </p>
      </div>
    </div>
  )
}

export function RecentRunsMini({
  executions,
  onLoad,
}: {
  executions: ExecutionSummary[]
  onLoad: (id: string) => Promise<void>
}) {
  const { locale } = useLocale()
  const zh = locale === 'zh'
  const [open, setOpen] = useState(true)
  const [loading, setLoading] = useState<string | null>(null)

  const recent = executions.slice(0, 5)

  const statusColor = (s: string) => {
    if (s === 'succeeded') return 'var(--success-text)'
    if (s === 'failed')    return 'var(--danger-text)'
    if (s === 'running')   return 'var(--link)'
    return 'var(--muted)'
  }

  const age = (ts: number) => {
    const secs = Math.floor(Date.now() / 1000) - ts
    if (zh) {
      if (secs < 60)    return `${secs}秒前`
      if (secs < 3600)  return `${Math.floor(secs / 60)}分钟前`
      if (secs < 86400) return `${Math.floor(secs / 3600)}小时前`
      return `${Math.floor(secs / 86400)}天前`
    }
    if (secs < 60)    return `${secs}s ago`
    if (secs < 3600)  return `${Math.floor(secs / 60)}m ago`
    if (secs < 86400) return `${Math.floor(secs / 3600)}h ago`
    return `${Math.floor(secs / 86400)}d ago`
  }

  const handleLoad = async (id: string) => {
    setLoading(id)
    try { await onLoad(id) } finally { setLoading(null) }
  }

  return (
    <div style={{ marginTop: 12, borderTop: '1px solid var(--border)', paddingTop: 8 }}>
      <button
        onClick={() => setOpen((o) => !o)}
        style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--muted)', fontSize: 11, fontWeight: 600, padding: '2px 0', display: 'flex', alignItems: 'center', gap: 4 }}
      >
        <span>{open ? '▾' : '▸'}</span> {zh ? '最近运行' : 'Recent Runs'}
      </button>
      {open && (
        <table style={{ width: '100%', borderCollapse: 'collapse', marginTop: 6, fontSize: 11 }}>
          <tbody>
            {recent.map((ex) => (
              <tr key={ex.id} style={{ borderBottom: '1px solid var(--border)' }}>
                <td style={{ padding: '4px 4px', color: statusColor(ex.status), fontWeight: 600, width: 72 }}>
                  {ex.status}
                </td>
                <td style={{ padding: '4px 4px', color: 'var(--muted)', fontFamily: 'monospace', fontSize: 10 }}>
                  {ex.label || ex.id.slice(-8)}
                </td>
                <td style={{ padding: '4px 4px', color: 'var(--muted)', textAlign: 'right' }}>
                  {age(ex.started_at)}
                </td>
                <td style={{ padding: '4px 4px', textAlign: 'right', width: 48 }}>
                  <button
                    className="btn btn-sm"
                    disabled={loading === ex.id}
                    onClick={() => handleLoad(ex.id)}
                    style={{ fontSize: 10, padding: '1px 6px' }}
                  >
                    {loading === ex.id ? '…' : (zh ? '加载' : 'Load')}
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

// ── FormsModal ──────────────────────────────────────────────────────────────

export function FormsModal({
  tenantId,
  workflowId,
  onClose,
}: {
  tenantId: string
  workflowId: string
  onClose: () => void
}) {
  const { locale } = useLocale()
  const zh = locale === 'zh'
  const [forms, setForms] = useState<api.FormTokenRecord[]>([])
  const [title, setTitle] = useState('')
  const [desc, setDesc] = useState('')
  const [publishing, setPublishing] = useState(false)
  const [copied, setCopied] = useState<string | null>(null)

  useEffect(() => {
    api.listForms(tenantId, workflowId).then(setForms).catch(() => {})
  }, [tenantId, workflowId])

  const handlePublish = async () => {
    if (!title.trim()) return
    setPublishing(true)
    try {
      await api.publishForm(tenantId, workflowId, title.trim(), desc.trim() || undefined)
      const updated = await api.listForms(tenantId, workflowId)
      setForms(updated)
      setTitle('')
      setDesc('')
    } catch {
      // ignore
    } finally {
      setPublishing(false)
    }
  }

  const handleDelete = async (token: string) => {
    await api.deleteForm(token).catch(() => {})
    setForms((prev) => prev.filter((f) => f.token !== token))
  }

  const formUrl = (token: string) => `${window.location.origin}/forms/${token}`

  const copyLink = (token: string) => {
    navigator.clipboard.writeText(formUrl(token)).catch(() => {})
    setCopied(token)
    setTimeout(() => setCopied(null), 2000)
  }

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" style={{ width: 520 }} onClick={(e) => e.stopPropagation()}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 16 }}>
          <h2 style={{ margin: 0, fontSize: 16 }}>{zh ? '表单发布器' : 'Form Publisher'}</h2>
          <button className="btn btn-sm" onClick={onClose}><IconX aria-hidden /></button>
        </div>

        <div style={{ display: 'flex', flexDirection: 'column', gap: 8, marginBottom: 16, padding: '12px', background: 'var(--bg)', borderRadius: 6, border: '1px solid var(--border)' }}>
          <input
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder={zh ? '表单标题 *' : 'Form title *'}
            style={{ fontSize: 13, padding: '6px 8px' }}
          />
          <input
            value={desc}
            onChange={(e) => setDesc(e.target.value)}
            placeholder={zh ? '描述（可选）' : 'Description (optional)'}
            style={{ fontSize: 13, padding: '6px 8px' }}
          />
          <button
            className="btn btn-primary btn-sm"
            disabled={publishing || !title.trim()}
            onClick={handlePublish}
            style={{ alignSelf: 'flex-end' }}
          >
            {publishing ? (zh ? '发布中…' : 'Publishing…') : (zh ? '发布表单' : 'Publish Form')}
          </button>
        </div>

        {forms.length === 0 ? (
          <div style={{ color: 'var(--muted)', fontSize: 13, textAlign: 'center', padding: '12px 0' }}>
            {zh ? '暂无已发布表单。' : 'No published forms yet.'}
          </div>
        ) : (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
            {forms.map((f) => (
              <div key={f.token} style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '8px 10px', background: 'var(--bg)', borderRadius: 6, border: '1px solid var(--border)' }}>
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ fontSize: 13, fontWeight: 500 }}>{f.title}</div>
                  <div style={{ fontSize: 11, color: 'var(--muted)', fontFamily: 'monospace', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                    {formUrl(f.token)}
                  </div>
                </div>
                <button
                  className="btn btn-sm"
                  onClick={() => copyLink(f.token)}
                  title={zh ? '复制表单链接' : 'Copy form URL'}
                >
                  {copied === f.token ? (zh ? '✓ 已复制' : '✓ Copied') : (zh ? '复制' : 'Copy')}
                </button>
                <a
                  href={formUrl(f.token)}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="btn btn-sm"
                  title="Open form"
                >
                  ↗
                </a>
                <button
                  className="btn btn-sm btn-danger"
                  onClick={() => handleDelete(f.token)}
                  title={zh ? '删除表单' : 'Delete form'}
                  style={{ fontSize: 11 }}
                >
                  <IconX aria-hidden />
                </button>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  )
}

interface CopilotPanelProps {
  onClose: () => void
  graphJson: string
  tenantId: string
  zh: boolean
}

interface CopilotMessage {
  role: 'user' | 'assistant'
  content: string
}

export function CopilotPanel({ onClose, graphJson, tenantId, zh }: CopilotPanelProps) {
  const [messages, setCopMessages] = useState<CopilotMessage[]>([])
  const [copInput, setCopInput] = useState('')
  const [copLoading, setCopLoading] = useState(false)
  const [copApiKey, setCopApiKey] = useState(() => localStorage.getItem('af:claude_key') ?? '')
  const [showKeyInput, setShowKeyInput] = useState(false)
  const bottomRef = useRef<HTMLDivElement>(null)

  const QUICK_ACTIONS = zh
    ? ['解释这个工作流', '找出潜在问题', '如何添加错误处理？', '建议性能优化']
    : ['Explain this workflow', 'Find potential issues', 'How to add error handling?', 'Suggest improvements']

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [messages])

  const sendMsg = async (msg: string) => {
    if (!msg.trim()) return
    const key = copApiKey.trim() || undefined
    setCopMessages((prev) => [...prev, { role: 'user', content: msg }])
    setCopInput('')
    setCopLoading(true)
    try {
      const res = await api.copilotQuery(msg, { tenantId, graphJson: graphJson || undefined, apiKey: key })
      setCopMessages((prev) => [...prev, { role: 'assistant', content: res.reply }])
    } catch (e: unknown) {
      setCopMessages((prev) => [...prev, { role: 'assistant', content: `⚠ ${String(e)}` }])
    } finally {
      setCopLoading(false)
    }
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); sendMsg(copInput) }
  }

  return (
    <div style={{
      position: 'absolute', top: 0, right: 0, bottom: 0, width: 340,
      background: 'var(--surface)', borderLeft: '1px solid var(--border)',
      display: 'flex', flexDirection: 'column', zIndex: 20,
      boxShadow: '-4px 0 16px rgba(0,0,0,0.1)',
    }}>
      <div style={{ padding: '10px 12px', borderBottom: '1px solid var(--border)', display: 'flex', alignItems: 'center', gap: 8 }}>
        <span style={{ fontWeight: 700, fontSize: 14, color: 'var(--node-claude)' }}>✦ {zh ? 'AI 助手' : 'Copilot'}</span>
        <span style={{ fontSize: 11, color: 'var(--muted)', flex: 1 }}>{zh ? '询问关于此工作流的任何问题' : 'Ask anything about this workflow'}</span>
        <button onClick={() => setShowKeyInput((v) => !v)} title={zh ? 'API Key' : 'API Key'} style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--muted)', fontSize: 14 }}><IconKey size={14} /></button>
        <button onClick={onClose} style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--muted)', fontSize: 18, lineHeight: 1 }}><IconX aria-hidden /></button>
      </div>

      {showKeyInput && (
        <div style={{ padding: '8px 12px', borderBottom: '1px solid var(--border)', background: 'var(--code-bg, rgba(0,0,0,0.04))' }}>
          <div style={{ fontSize: 11, color: 'var(--muted)', marginBottom: 4 }}>{zh ? 'Anthropic API Key（本地存储）：' : 'Anthropic API Key (stored locally):'}</div>
          <input type="password" placeholder="sk-ant-..." value={copApiKey}
            onChange={(e) => { setCopApiKey(e.target.value); localStorage.setItem('af:claude_key', e.target.value) }}
            style={{ width: '100%', fontSize: 12, padding: '4px 6px', boxSizing: 'border-box' }} />
        </div>
      )}

      {messages.length === 0 && (
        <div style={{ padding: '12px', display: 'flex', flexWrap: 'wrap', gap: 6 }}>
          {QUICK_ACTIONS.map((action) => (
            <button key={action} onClick={() => sendMsg(action)} style={{
              background: 'var(--bg)', border: '1px solid var(--border)', borderRadius: 12,
              padding: '4px 10px', fontSize: 11, cursor: 'pointer', color: 'var(--text)',
            }}
            onMouseEnter={(e) => { e.currentTarget.style.borderColor = 'var(--accent)'; e.currentTarget.style.color = 'var(--accent)' }}
            onMouseLeave={(e) => { e.currentTarget.style.borderColor = 'var(--border)'; e.currentTarget.style.color = 'var(--text)' }}>
              {action}
            </button>
          ))}
        </div>
      )}

      <div style={{ flex: 1, overflowY: 'auto', padding: '8px 12px', display: 'flex', flexDirection: 'column', gap: 10 }}>
        {messages.map((m, i) => (
          <div key={i} style={{ display: 'flex', flexDirection: 'column', alignItems: m.role === 'user' ? 'flex-end' : 'flex-start' }}>
            <div style={{
              maxWidth: '90%', padding: '8px 12px', fontSize: 13, lineHeight: 1.5,
              whiteSpace: 'pre-wrap', wordBreak: 'break-word',
              borderRadius: m.role === 'user' ? '12px 12px 2px 12px' : '12px 12px 12px 2px',
              background: m.role === 'user' ? 'var(--accent)' : 'var(--bg)',
              color: m.role === 'user' ? '#fff' : 'var(--text)',
              border: m.role === 'user' ? 'none' : '1px solid var(--border)',
            }}>{m.content}</div>
          </div>
        ))}
        {copLoading && (
          <div style={{ display: 'flex', alignItems: 'flex-start' }}>
            <div style={{ padding: '8px 12px', borderRadius: '12px 12px 12px 2px', background: 'var(--bg)', border: '1px solid var(--border)', fontSize: 13, color: 'var(--muted)' }}>
              {zh ? '思考中…' : 'Thinking…'}
            </div>
          </div>
        )}
        <div ref={bottomRef} />
      </div>

      <div style={{ padding: '8px 12px', borderTop: '1px solid var(--border)', display: 'flex', gap: 6 }}>
        <textarea value={copInput} onChange={(e) => setCopInput(e.target.value)} onKeyDown={handleKeyDown}
          placeholder={zh ? '输入消息… (Enter 发送)' : 'Ask a question… (Enter to send)'}
          rows={2} style={{
            flex: 1, resize: 'none', fontSize: 13, padding: '6px 8px', borderRadius: 6,
            border: '1px solid var(--border)', background: 'var(--bg)', color: 'var(--text)', fontFamily: 'inherit',
          }} />
        <button onClick={() => sendMsg(copInput)} disabled={copLoading || !copInput.trim()} style={{
          background: 'var(--node-claude)', color: '#fff', border: 'none', borderRadius: 6,
          padding: '0 12px', cursor: 'pointer', fontWeight: 600, fontSize: 16,
          opacity: copLoading || !copInput.trim() ? 0.5 : 1,
        }}>↑</button>
      </div>
    </div>
  )
}
