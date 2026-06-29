// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useMemo, useState } from 'react'
import { IconSave, IconCheck, IconX } from './uiIcons'
import type { ExecutionRecord, NodeExecutionRecord, InputField, EnvSetSummary } from '../types'
import { useLocale } from '../useLocale'

function validateInput(json: string, schema: InputField[]): string[] {
  if (!schema.length) return []
  let parsed: unknown
  try {
    parsed = JSON.parse(json)
  } catch {
    return json.trim() && json.trim() !== '{}' ? ['Invalid JSON'] : []
  }
  if (typeof parsed !== 'object' || Array.isArray(parsed) || parsed === null) {
    return ['Input must be a JSON object']
  }
  const obj = parsed as Record<string, unknown>
  const errors: string[] = []
  for (const field of schema) {
    const val = obj[field.key]
    if (field.required && (val === undefined || val === null || val === '')) {
      errors.push(`Required: "${field.key}"`)
    } else if (val !== undefined && val !== null) {
      if (field.field_type === 'number' && typeof val !== 'number') errors.push(`"${field.key}" must be a number`)
      if (field.field_type === 'boolean' && typeof val !== 'boolean') errors.push(`"${field.key}" must be a boolean`)
    }
  }
  return errors
}

interface Props {
  execution: ExecutionRecord | null
  running: boolean
  inputJson: string
  onInputChange: (v: string) => void
  onRun: () => void
  canRun: boolean
  onApprove?: (comment?: string) => void
  onReject?: (comment?: string) => void
  inputSchema?: InputField[]
  envSets?: EnvSetSummary[]
  envSet?: string
  onEnvSetChange?: (s: string) => void
  label?: string
  onLabelChange?: (v: string) => void
  callbackUrl?: string
  onCallbackUrlChange?: (v: string) => void
  workflowId?: string
  dryRun?: boolean
  onDryRunChange?: (v: boolean) => void
  lastRunInput?: string
}

function useInputHistory(workflowId?: string) {
  const key = workflowId ? `af:input-history:${workflowId}` : null
  const getHistory = (): string[] => {
    if (!key) return []
    try { return JSON.parse(localStorage.getItem(key) ?? '[]') } catch { return [] }
  }
  const pushHistory = (input: string) => {
    if (!key || !input || input === '{}') return
    const existing = getHistory().filter((h) => h !== input)
    const updated = [input, ...existing].slice(0, 10)
    localStorage.setItem(key, JSON.stringify(updated))
  }
  return { getHistory, pushHistory }
}

function usePresets(workflowId?: string) {
  const key = workflowId ? `af:presets:${workflowId}` : null
  const load = (): Record<string, string> => {
    if (!key) return {}
    try { return JSON.parse(localStorage.getItem(key) ?? '{}') } catch { return {} }
  }
  const save = (presets: Record<string, string>) => {
    if (!key) return
    localStorage.setItem(key, JSON.stringify(presets))
  }
  return { load, save }
}

export function ExecutionPanel({ execution, running, inputJson, onInputChange, onRun, canRun, onApprove, onReject, inputSchema = [], envSets = [], envSet = 'default', onEnvSetChange, label = '', onLabelChange, callbackUrl = '', onCallbackUrlChange, workflowId, dryRun = false, onDryRunChange, lastRunInput }: Props) {
  const { locale } = useLocale()
  const zh = locale === 'zh'
  const [approvalComment, setApprovalComment] = useState('')
  const hasSchema = inputSchema.length > 0
  const hasEnvSets = envSets.length > 1
  const { load: loadPresets, save: savePresets } = usePresets(workflowId)
  const [presets, setPresets] = useState<Record<string, string>>(loadPresets)
  const { getHistory, pushHistory } = useInputHistory(workflowId)
  const [inputHistory, setInputHistory] = useState<string[]>(getHistory)

  const handleSavePreset = () => {
    const name = window.prompt('Preset name:', '')?.trim()
    if (!name) return
    const updated = { ...presets, [name]: inputJson }
    setPresets(updated)
    savePresets(updated)
  }

  const handleDeletePreset = (name: string) => {
    const updated = { ...presets }
    delete updated[name]
    setPresets(updated)
    savePresets(updated)
  }

  const validationErrors = useMemo(() => validateInput(inputJson, inputSchema), [inputJson, inputSchema])

  return (
    <div className="exec-panel">
      <div className="exec-panel-header">
        <span style={{ fontWeight: 600, fontSize: 13 }}>{zh ? '执行' : 'Execution'}</span>
        {execution && (
          <>
            <span className={`badge badge-${execution.status}`}>{execution.status}</span>
            {execution.dry_run && (
              <span style={{ fontSize: 10, padding: '1px 5px', background: 'var(--link)', color: '#fff', borderRadius: 3, fontWeight: 600 }}>
                DRY
              </span>
            )}
            {execution.status === 'running' && (execution.node_count ?? 0) > 0 && (() => {
              // Live progress for the run in flight. The data already streams in
              // via SSE/poll (completed_node_count grows); this surfaces it in the
              // editor so the run isn't a black box, mirroring the canvas's
              // per-node highlight. The currently-running node is named too.
              const nc = execution.node_count!
              const cc = execution.completed_node_count ?? 0
              const pct = Math.round((cc / nc) * 100)
              const runningNode = execution.node_results.find((r) => r.status === 'running')
              return (
                <div data-testid="exec-progress" style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                  <div style={{ width: 90, height: 5, borderRadius: 3, background: 'var(--border)', overflow: 'hidden' }}>
                    <div style={{ height: '100%', width: `${pct}%`, background: 'var(--link)', borderRadius: 3, transition: 'width 0.4s ease' }} />
                  </div>
                  <span style={{ fontSize: 10, color: 'var(--muted)' }}>{cc}/{nc}</span>
                  {runningNode && <span style={{ fontSize: 11, color: 'var(--link)' }}>{zh ? '运行中：' : 'running: '}{runningNode.node_id}</span>}
                </div>
              )
            })()}
          </>
        )}
        {!execution && !running && (
          <span style={{ color: 'var(--muted)', fontSize: 12 }}>
            {zh ? '运行工作流以查看结果' : 'Run the workflow to see results'}
          </span>
        )}
        {running && <span style={{ color: 'var(--link)', fontSize: 12 }}>{zh ? '运行中…' : 'Running…'}</span>}

        {execution?.status === 'waiting_approval' && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 5 }}>
            <div style={{ display: 'flex', gap: 6, alignItems: 'center' }}>
              <span style={{ fontSize: 12, color: 'var(--muted)' }}>{zh ? '等待审批' : 'Waiting for approval'}</span>
              <button className="btn btn-success btn-sm" onClick={() => onApprove?.(approvalComment || undefined)}><IconCheck aria-hidden style={{ verticalAlign: '-2px', marginRight: 3 }} />{zh ? '批准' : 'Approve'}</button>
              <button className="btn btn-danger btn-sm" onClick={() => onReject?.(approvalComment || undefined)}><IconX aria-hidden style={{ verticalAlign: '-2px', marginRight: 3 }} />{zh ? '拒绝' : 'Reject'}</button>
            </div>
            <input
              value={approvalComment}
              onChange={(e) => setApprovalComment(e.target.value)}
              placeholder={zh ? '备注（可选）' : 'Comment (optional)'}
              style={{ fontSize: 11, padding: '2px 6px', width: 240 }}
            />
          </div>
        )}

        {hasEnvSets && (
          <div style={{ display: 'flex', alignItems: 'center', gap: 4, fontSize: 12 }}>
            <span style={{ color: 'var(--muted)', whiteSpace: 'nowrap' }}>{zh ? '环境集：' : 'Env set:'}</span>
            <select
              value={envSet}
              onChange={(e) => onEnvSetChange?.(e.target.value)}
              style={{ fontSize: 12, padding: '1px 4px' }}
            >
              {envSets.map((s) => (
                <option key={s.name} value={s.name}>{s.name}</option>
              ))}
            </select>
          </div>
        )}
        {onLabelChange && (
          <div style={{ display: 'flex', alignItems: 'center', gap: 4, fontSize: 12 }}>
            <span style={{ color: 'var(--muted)', whiteSpace: 'nowrap' }}>{zh ? '标签：' : 'Label:'}</span>
            <input
              value={label}
              onChange={(e) => onLabelChange(e.target.value)}
              placeholder={zh ? '如 production' : 'e.g. production'}
              style={{ width: 110, fontSize: 12, padding: '1px 4px' }}
            />
          </div>
        )}
        {onCallbackUrlChange && (
          <div style={{ display: 'flex', alignItems: 'center', gap: 4, fontSize: 12 }}>
            <span style={{ color: 'var(--muted)', whiteSpace: 'nowrap' }}>{zh ? '回调：' : 'Callback:'}</span>
            <input
              value={callbackUrl}
              onChange={(e) => onCallbackUrlChange(e.target.value)}
              placeholder="https://…/webhook"
              title={zh ? '执行完成后将结果 JSON POST 到此 URL' : 'URL to POST the execution result JSON to when complete'}
              style={{ width: 160, fontSize: 12, padding: '1px 4px', fontFamily: 'monospace' }}
            />
          </div>
        )}
        {onDryRunChange && (
          <label style={{ display: 'flex', alignItems: 'center', gap: 4, fontSize: 12, cursor: 'pointer', userSelect: 'none' }}
            title={zh ? '跳过外部 API 调用，返回模拟数据' : 'Skip external API calls and return mock data'}>
            <input
              type="checkbox"
              checked={dryRun}
              onChange={(e) => onDryRunChange(e.target.checked)}
              style={{ margin: 0 }}
            />
            <span style={{ color: dryRun ? 'var(--link)' : 'var(--muted)', whiteSpace: 'nowrap' }}>
              {zh ? '演练模式' : 'Dry Run'}
            </span>
          </label>
        )}
        {hasSchema ? (
          <SchemaForm
            schema={inputSchema}
            onChange={(json) => onInputChange(json)}
            onRun={onRun}
            canRun={canRun}
            running={running}
            zh={zh}
          />
        ) : (
          <div className="exec-input-row">
            <label style={{ fontSize: 12, color: 'var(--muted)', margin: 0, whiteSpace: 'nowrap' }}>
              {zh ? '输入 JSON' : 'Input JSON'}
            </label>
            <input
              value={inputJson}
              onChange={(e) => onInputChange(e.target.value)}
              placeholder="{}"
              style={{ width: 180, fontFamily: 'monospace', fontSize: 12 }}
            />
            {workflowId && (
              <>
                <button
                  className="btn btn-sm btn-icon"
                  onClick={handleSavePreset}
                  title={zh ? '保存当前输入为预设' : 'Save current input as a preset'}
                  style={{ fontSize: 11, padding: '2px 6px' }}
                >
                  <IconSave size={15} />
                </button>
                {Object.keys(presets).length > 0 && (
                  <select
                    defaultValue=""
                    onChange={(e) => {
                      if (e.target.value === '') return
                      if (e.target.value.startsWith('__del__')) {
                        handleDeletePreset(e.target.value.slice(7))
                      } else {
                        onInputChange(presets[e.target.value])
                      }
                      e.target.value = ''
                    }}
                    style={{ fontSize: 11, padding: '2px 4px', maxWidth: 100 }}
                    title={zh ? '加载已保存的预设' : 'Load a saved preset'}
                  >
                    <option value="">{zh ? '预设…' : 'Presets…'}</option>
                    {Object.keys(presets).map((name) => (
                      <option key={name} value={name}>▶ {name}</option>
                    ))}
                    <option disabled>──</option>
                    {Object.keys(presets).map((name) => (
                      <option key={`del-${name}`} value={`__del__${name}`}>✕ {name}</option>
                    ))}
                  </select>
                )}
              </>
            )}
            {lastRunInput && lastRunInput !== '{}' && (
              <button
                className="btn btn-sm"
                onClick={() => onInputChange(lastRunInput)}
                title={zh ? '使用上次执行的输入 JSON' : 'Fill with last run\'s input JSON'}
                style={{ fontSize: 11, padding: '2px 6px' }}
              >
                ↑ {zh ? '上次' : 'Last'}
              </button>
            )}
            {inputHistory.length > 0 && (
              <select
                style={{ fontSize: 11, padding: '2px 4px', borderRadius: 4, border: '1px solid var(--border)', background: 'var(--surface)', maxWidth: 90 }}
                value=""
                onChange={(e) => { if (e.target.value) onInputChange(e.target.value) }}
                title={zh ? '输入历史记录' : 'Input history'}
              >
                <option value="">{zh ? '历史 ▾' : 'History ▾'}</option>
                {inputHistory.map((h, i) => (
                  <option key={i} value={h}>{i === 0 ? (zh ? '最近' : 'Latest') : `#${i + 1}`}: {h.slice(0, 30)}{h.length > 30 ? '…' : ''}</option>
                ))}
              </select>
            )}
            <button
              className="btn btn-success btn-sm"
              disabled={!canRun || running}
              onClick={() => { pushHistory(inputJson); setInputHistory(getHistory()); onRun() }}
              title={!canRun ? (zh ? '请先发布版本再运行' : 'Publish a version first to run') : (zh ? '运行最新发布版本（Ctrl+Enter）' : 'Run the latest published version (Ctrl+Enter)')}
            >
              {running ? '…' : (zh ? '▶ 运行' : '▶ Run')}
            </button>
          </div>
        )}
        {!hasSchema && inputSchema.length > 0 && (() => {
          let parsed: Record<string, unknown> = {}
          try { parsed = JSON.parse(inputJson) as Record<string, unknown> } catch { /* ignore */ }
          return (
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: '3px 6px', padding: '4px 0 0', fontSize: 11 }}>
              {inputSchema.map(field => {
                const present = field.key in parsed
                const hasError = validationErrors.some(e => e.includes(`"${field.key}"`))
                const color = hasError ? 'var(--danger-text)' : present ? 'var(--success-text)' : 'var(--muted)'
                const handleFill = () => {
                  try {
                    const obj = (typeof parsed === 'object' && parsed !== null) ? { ...parsed } : {}
                    if (!(field.key in obj)) {
                      const def = field.default_value != null ? JSON.parse(field.default_value) : (field.field_type === 'number' ? 0 : field.field_type === 'boolean' ? false : '')
                      obj[field.key] = def
                      onInputChange(JSON.stringify(obj, null, 2))
                    }
                  } catch { /* ignore */ }
                }
                return (
                  <span key={field.key}
                    style={{ color, cursor: present ? 'default' : 'pointer', fontFamily: 'monospace', borderBottom: `1px dashed ${color}` }}
                    title={present ? `${field.key}: ${field.field_type}${field.required ? ' (required)' : ''}` : `Click to fill "${field.key}" with default value`}
                    onClick={present ? undefined : handleFill}
                  >
                    {present ? '✓' : field.required ? '✗' : '○'} {field.key}
                  </span>
                )
              })}
            </div>
          )
        })()}
        {!hasSchema && validationErrors.length > 0 && (
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: '2px 8px', padding: '3px 0 0', fontSize: 11 }}>
            {validationErrors.map((e, i) => (
              <span key={i} style={{ color: 'var(--danger-text)' }}>⚠ {e}</span>
            ))}
          </div>
        )}
      </div>

      {execution && execution.status === 'succeeded' && execution.output_json && (
        <div style={{
          margin: '8px 0 4px',
          padding: '8px 10px',
          background: 'rgba(16,185,129,0.06)',
          border: '1px solid var(--success-text)',
          borderRadius: 'var(--radius)',
        }}>
          <div style={{ fontSize: 11, fontWeight: 600, color: 'var(--success-text)', marginBottom: 4 }}>
            {zh ? '工作流输出' : 'Workflow Output'}
          </div>
          <pre style={{
            margin: 0, fontSize: 11, fontFamily: 'monospace',
            whiteSpace: 'pre-wrap', wordBreak: 'break-all',
            maxHeight: 120, overflowY: 'auto', lineHeight: 1.4,
            color: 'var(--fg)',
          }}>
            {(() => { try { return JSON.stringify(JSON.parse(execution.output_json!), null, 2) } catch { return execution.output_json } })()}
          </pre>
        </div>
      )}

      {execution?.dry_run && (
        <div style={{
          margin: '6px 0 2px',
          padding: '5px 10px',
          background: 'rgba(59,130,246,0.08)',
          border: '1px solid var(--link)',
          borderRadius: 'var(--radius)',
          fontSize: 11,
          color: 'var(--link)',
        }}>
          {zh ? '演练模式 — 已跳过外部调用，节点输出为模拟数据。' : 'Dry run — external calls were skipped. Node outputs show mock data.'}
        </div>
      )}

      {execution && (
        <div className="exec-panel-body">
          {execution.node_results.length === 0 && (
            <span style={{ color: 'var(--muted)', fontSize: 12 }}>{zh ? '暂无节点结果。' : 'No node results.'}</span>
          )}
          {execution.node_results.map((nr) => (
            <NodeResultRow key={nr.node_id} nr={nr} />
          ))}
        </div>
      )}
    </div>
  )
}

function NodeResultRow({ nr }: { nr: NodeExecutionRecord }) {
  const output = (() => {
    if (nr.error) return `Error: ${nr.error}`
    if (!nr.output_json) return ''
    try {
      const parsed = JSON.parse(nr.output_json)
      return JSON.stringify(parsed, null, 0)
    } catch {
      return nr.output_json
    }
  })()

  return (
    <div className="exec-node-row">
      <span className={`dot dot-${nr.status}`} style={{ marginTop: 4 }} />
      <span className="exec-node-id">{nr.node_id}</span>
      <span
        className={`badge badge-${nr.status}`}
        style={{ fontSize: 10, padding: '1px 6px', flexShrink: 0 }}
      >
        {nr.status}
      </span>
      {output && (
        <span className="exec-node-output" style={{ flex: 1, maxHeight: 60, overflow: 'hidden' }}>
          {output}
        </span>
      )}
    </div>
  )
}

function SchemaForm({
  schema, onChange, onRun, canRun, running, zh = false,
}: {
  schema: InputField[]
  onChange: (json: string) => void
  onRun: () => void
  canRun: boolean
  running: boolean
  zh?: boolean
}) {
  const init: Record<string, string> = {}
  for (const f of schema) init[f.key] = f.default_value ?? ''
  const [values, setValues] = useState<Record<string, string>>(init)

  const set = (key: string, val: string) => {
    const next = { ...values, [key]: val }
    setValues(next)
    const obj: Record<string, unknown> = {}
    for (const f of schema) {
      const raw = next[f.key] ?? ''
      if (f.field_type === 'number') obj[f.key] = raw === '' ? null : Number(raw)
      else if (f.field_type === 'boolean') obj[f.key] = raw === 'true'
      else if (f.field_type === 'json') { try { obj[f.key] = JSON.parse(raw) } catch { obj[f.key] = raw } }
      else obj[f.key] = raw
    }
    onChange(JSON.stringify(obj))
  }

  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: 8, flexWrap: 'wrap' }}>
      {schema.map((f) => (
        <label key={f.key} style={{ display: 'flex', alignItems: 'center', gap: 4, fontSize: 12 }}>
          <span style={{ color: 'var(--muted)', whiteSpace: 'nowrap' }}>
            {f.key}{f.required ? <span style={{ color: 'var(--danger-text)' }}>*</span> : ''}
          </span>
          {f.field_type === 'boolean' ? (
            <select
              value={values[f.key] ?? 'false'}
              onChange={(e) => set(f.key, e.target.value)}
              style={{ fontSize: 11, padding: '1px 4px' }}
            >
              <option value="true">true</option>
              <option value="false">false</option>
            </select>
          ) : (
            <input
              value={values[f.key] ?? ''}
              onChange={(e) => set(f.key, e.target.value)}
              placeholder={f.description || f.field_type}
              title={f.description}
              style={{ width: f.field_type === 'json' ? 120 : 90, fontFamily: 'monospace', fontSize: 11 }}
            />
          )}
        </label>
      ))}
      <button
        className="btn btn-success btn-sm"
        disabled={!canRun || running}
        onClick={onRun}
        title={!canRun ? (zh ? '请先发布版本再运行' : 'Publish a version first to run') : (zh ? '运行最新发布版本' : 'Run the latest published version')}
      >
        {running ? '…' : (zh ? '▶ 运行' : '▶ Run')}
      </button>
    </div>
  )
}
