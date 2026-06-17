// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useState } from 'react'
import { useAuth } from '../AuthContext'
import { useLocale } from '../useLocale'
import * as api from '../api/client'
import type { WorkflowGraph } from '../types'

interface Props {
  onClose: () => void
  onImport: (graph: WorkflowGraph, name: string, description: string) => void
  onCreated?: (workflowId: string) => void
}

const EXAMPLE_PROMPTS_EN = [
  'Fetch data from a REST API, filter results, and post a summary to Slack',
  'When triggered, read a Jira issue, summarize it with Claude, and update a Notion page',
  'Poll a database every hour, aggregate results, and email a report if anomalies are found',
  'Accept a GitHub webhook, analyze the PR diff with Claude, and post a review comment',
  'Fan out to 3 parallel API calls, join results, validate schema, and store to database',
]

const EXAMPLE_PROMPTS_ZH = [
  '从 REST API 获取数据，过滤结果，将摘要发送到企业微信',
  '触发时读取飞书工单，用 DeepSeek 生成摘要，并更新飞书文档',
  '每小时轮询数据库，聚合结果，发现异常时发送邮件报告',
  '接收 GitHub Webhook，用 Claude 分析 PR 差异，并发布评审评论',
  '并行调用 3 个 API，合并结果，校验 Schema，存入数据库',
]

// Generation LLM providers. `anthropic` uses the Anthropic Messages API; the
// rest are OpenAI-compatible (the backend resolves their /chat/completions URL).
const GEN_PROVIDERS: { value: string; label: string; models: string[]; keyHint: string; needsBaseUrl?: boolean }[] = [
  { value: 'anthropic', label: 'Anthropic Claude', models: ['claude-sonnet-4-6', 'claude-opus-4-8', 'claude-haiku-4-5-20251001'], keyHint: 'sk-ant-...' },
  { value: 'openai', label: 'OpenAI', models: ['gpt-5.4-mini', 'gpt-5.5', 'gpt-4.1'], keyHint: 'sk-...' },
  { value: 'deepseek', label: 'DeepSeek', models: ['deepseek-v4-flash', 'deepseek-v4-pro'], keyHint: 'sk-...' },
  { value: 'qwen', label: 'Qwen 通义千问', models: ['qwen-max', 'qwen3-max', 'qwen3.5-plus'], keyHint: 'sk-...' },
  { value: 'zhipu', label: 'Zhipu GLM 智谱', models: ['glm-4.6', 'glm-4.7'], keyHint: 'API Key' },
  { value: 'moonshot', label: 'Moonshot (Kimi)', models: ['kimi-latest', 'kimi-k2.6'], keyHint: 'sk-...' },
  { value: 'grok', label: 'xAI Grok', models: ['grok-4.3'], keyHint: 'xai-...' },
  { value: 'custom', label: 'OpenAI-compatible (custom)', models: [], keyHint: 'API Key', needsBaseUrl: true },
]

// Node modules the generated workflow may use (empty selection = unrestricted).
const GEN_MODULES = [
  'http', 'condition', 'transform', 'filter', 'aggregate', 'delay', 'code', 'loop',
  'extract', 'merge', 'assert', 'validate', 'fan_out', 'fan_in', 'catch', 'note',
  'claude', 'openai', 'gemini', 'deepseek', 'qwen', 'zhipu', 'moonshot', 'grok',
  'slack', 'github', 'jira', 'notion', 'database', 'email', 'webhook',
]

export function GenerateWorkflowModal({ onClose, onImport, onCreated }: Props) {
  const { auth } = useAuth()
  const { locale } = useLocale()
  const zh = locale === 'zh'
  const [prompt, setPrompt] = useState('')
  const [apiKey, setApiKey] = useState('')
  const [model, setModel] = useState('claude-sonnet-4-6')
  const [provider, setProvider] = useState('anthropic')
  const [baseUrl, setBaseUrl] = useState('')
  const [temperature, setTemperature] = useState(0.4)
  const [maxNodes, setMaxNodes] = useState<number | ''>('')
  const [errorHandling, setErrorHandling] = useState<'auto' | 'yes' | 'no'>('auto')
  const [allowedModules, setAllowedModules] = useState<string[]>([])
  const [loading, setLoading] = useState(false)

  const providerDef = GEN_PROVIDERS.find((p) => p.value === provider) ?? GEN_PROVIDERS[0]
  function changeProvider(p: string) {
    setProvider(p)
    const def = GEN_PROVIDERS.find((x) => x.value === p)
    if (def && def.models.length) setModel(def.models[0])
  }
  function toggleModule(m: string) {
    setAllowedModules((prev) => (prev.includes(m) ? prev.filter((x) => x !== m) : [...prev, m]))
  }
  // Shared advanced-option payload for both preview and create requests.
  const genOpts = () => ({
    apiKey: apiKey || undefined,
    model,
    provider,
    baseUrl: baseUrl || undefined,
    temperature,
    maxNodes: typeof maxNodes === 'number' ? maxNodes : undefined,
    allowedModules,
    errorHandling: errorHandling === 'auto' ? undefined : errorHandling === 'yes',
    language: zh ? 'zh' : 'en',
  })
  const [error, setError] = useState<string | null>(null)
  const [preview, setPreview] = useState<api.GenerateWorkflowResult | null>(null)
  const [mode, setMode] = useState<'generate' | 'preview'>('generate')

  async function handleGenerate() {
    if (!prompt.trim()) return
    setLoading(true)
    setError(null)
    try {
      const result = await api.generateWorkflow(prompt.trim(), {
        tenantId: auth?.tenantId,
        workspaceId: auth?.workspaceId,
        projectId: auth?.projectId,
        ...genOpts(),
        create: false,
      })
      setPreview(result)
      setMode('preview')
    } catch (err) {
      setError(err instanceof Error ? err.message : (zh ? '生成失败' : 'Generation failed'))
    } finally {
      setLoading(false)
    }
  }

  async function handleCreateAndOpen() {
    if (!preview) return
    setLoading(true)
    setError(null)
    try {
      const result = await api.generateWorkflow(prompt.trim(), {
        tenantId: auth?.tenantId,
        workspaceId: auth?.workspaceId,
        projectId: auth?.projectId,
        ...genOpts(),
        create: true,
      })
      if (result.workflow && onCreated) {
        onCreated(result.workflow.id)
      }
      onClose()
    } catch (err) {
      setError(err instanceof Error ? err.message : (zh ? '创建失败' : 'Creation failed'))
    } finally {
      setLoading(false)
    }
  }

  function handleLoadIntoEditor() {
    if (!preview) return
    onImport(preview.graph, preview.name, preview.description)
    onClose()
  }

  const nodeCount = preview?.graph?.nodes?.length ?? 0
  const edgeCount = preview?.graph?.edges?.length ?? 0
  const examples = zh ? EXAMPLE_PROMPTS_ZH : EXAMPLE_PROMPTS_EN

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div
        className="modal"
        style={{ maxWidth: 640, width: '95vw' }}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="modal-header">
          <h2 style={{ margin: 0, fontSize: '1rem' }}>✦ {zh ? 'AI 生成工作流' : 'Generate Workflow with AI'}</h2>
          <button className="btn btn-sm" onClick={onClose}>✕</button>
        </div>

        {mode === 'generate' ? (
          <div style={{ padding: '1rem', display: 'flex', flexDirection: 'column', gap: '1rem' }}>
            <div className="field">
              <label>{zh ? '描述你的工作流' : 'Describe your workflow'}</label>
              <textarea
                rows={4}
                autoFocus
                value={prompt}
                onChange={(e) => setPrompt(e.target.value)}
                placeholder={zh
                  ? '例如：每天早晨从 GitHub 获取开放 PR，用 Claude 生成摘要，发送到飞书群'
                  : 'e.g. Fetch data from GitHub, summarize open PRs with Claude, post a digest to Slack every morning'}
                style={{ resize: 'vertical', fontSize: '0.875rem' }}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) handleGenerate()
                }}
              />
              <span style={{ fontSize: 11, color: 'var(--muted)' }}>{zh ? 'Ctrl+Enter 生成' : 'Ctrl+Enter to generate'}</span>
            </div>

            <div style={{ display: 'flex', flexDirection: 'column', gap: '0.4rem' }}>
              <span style={{ fontSize: 12, color: 'var(--muted)' }}>{zh ? '示例（点击使用）：' : 'Examples (click to use):'}</span>
              <div style={{ display: 'flex', flexDirection: 'column', gap: '0.3rem' }}>
                {examples.map(p => (
                  <button
                    key={p}
                    className="btn"
                    style={{ textAlign: 'left', fontSize: 12, padding: '0.3rem 0.6rem' }}
                    onClick={() => setPrompt(p)}
                  >
                    {p}
                  </button>
                ))}
              </div>
            </div>

            <details style={{ fontSize: 12 }}>
              <summary style={{ cursor: 'pointer', color: 'var(--muted)', userSelect: 'none' }}>
                {zh ? '高级选项' : 'Advanced options'}
              </summary>
              <div style={{ marginTop: '0.5rem', display: 'flex', flexDirection: 'column', gap: '0.6rem' }}>
                {/* ── Generation model ── */}
                <div style={{ display: 'flex', gap: 8 }}>
                  <div className="field" style={{ flex: 1 }}>
                    <label>{zh ? '生成提供商' : 'Provider'}</label>
                    <select value={provider} onChange={(e) => changeProvider(e.target.value)}>
                      {GEN_PROVIDERS.map((p) => <option key={p.value} value={p.value}>{p.label}</option>)}
                    </select>
                  </div>
                  <div className="field" style={{ flex: 1 }}>
                    <label>{zh ? '模型' : 'Model'}</label>
                    <input
                      list="gen-models"
                      value={model}
                      onChange={(e) => setModel(e.target.value)}
                      placeholder={providerDef.models[0] ?? 'model'}
                      style={{ fontFamily: 'monospace', fontSize: 12 }}
                    />
                    <datalist id="gen-models">
                      {providerDef.models.map((m) => <option key={m} value={m}>{m}</option>)}
                    </datalist>
                  </div>
                </div>
                <div className="field">
                  <label>
                    API Key{' '}
                    <span style={{ color: 'var(--muted)', fontWeight: 400 }}>
                      {provider === 'anthropic'
                        ? (zh ? '（留空用 ANTHROPIC_API_KEY 环境变量）' : '(uses ANTHROPIC_API_KEY env if blank)')
                        : (zh ? '（留空用 OPENAI_API_KEY 环境变量）' : '(uses OPENAI_API_KEY env if blank)')}
                    </span>
                  </label>
                  <input type="password" value={apiKey} onChange={(e) => setApiKey(e.target.value)} placeholder={providerDef.keyHint} />
                </div>
                {provider !== 'anthropic' && (
                  <div className="field">
                    <label>
                      Base URL{' '}
                      <span style={{ color: 'var(--muted)', fontWeight: 400 }}>
                        {providerDef.needsBaseUrl ? (zh ? '（必填，OpenAI 兼容 /chat/completions）' : '(required, OpenAI-compatible /chat/completions)') : (zh ? '（可选，覆盖默认端点）' : '(optional override)')}
                      </span>
                    </label>
                    <input value={baseUrl} onChange={(e) => setBaseUrl(e.target.value)} placeholder="https://api.example.com/v1/chat/completions" style={{ fontFamily: 'monospace', fontSize: 12 }} />
                  </div>
                )}

                {/* ── Generation parameters ── */}
                <div style={{ display: 'flex', gap: 8, alignItems: 'flex-end' }}>
                  <div className="field" style={{ flex: 1 }}>
                    <label>{zh ? '创造性 (temperature)' : 'Creativity (temperature)'}: {temperature.toFixed(1)}</label>
                    <input type="range" min={0} max={1} step={0.1} value={temperature} onChange={(e) => setTemperature(Number(e.target.value))} />
                  </div>
                  <div className="field" style={{ width: 110 }}>
                    <label>{zh ? '最大节点数' : 'Max nodes'}</label>
                    <input type="number" min={2} max={30} value={maxNodes} placeholder={zh ? '不限' : 'auto'} onChange={(e) => setMaxNodes(e.target.value ? Number(e.target.value) : '')} />
                  </div>
                  <div className="field" style={{ width: 130 }}>
                    <label>{zh ? '错误处理' : 'Error handling'}</label>
                    <select value={errorHandling} onChange={(e) => setErrorHandling(e.target.value as 'auto' | 'yes' | 'no')}>
                      <option value="auto">{zh ? '默认' : 'Auto'}</option>
                      <option value="yes">{zh ? '包含' : 'Include'}</option>
                      <option value="no">{zh ? '不含' : 'Omit'}</option>
                    </select>
                  </div>
                </div>

                {/* ── Allowed node modules ── */}
                <div className="field">
                  <label>
                    {zh ? '可用节点模块' : 'Allowed node modules'}{' '}
                    <span style={{ color: 'var(--muted)', fontWeight: 400 }}>
                      {allowedModules.length === 0 ? (zh ? '（不限）' : '(unrestricted)') : `(${allowedModules.length})`}
                    </span>
                  </label>
                  <div style={{ display: 'flex', flexWrap: 'wrap', gap: 4 }}>
                    {GEN_MODULES.map((m) => {
                      const on = allowedModules.includes(m)
                      return (
                        <button
                          key={m}
                          type="button"
                          onClick={() => toggleModule(m)}
                          className="btn btn-sm"
                          style={{
                            fontSize: 11, padding: '2px 7px', fontFamily: 'monospace',
                            background: on ? 'var(--accent)' : 'var(--panel)',
                            color: on ? '#fff' : 'var(--muted)',
                            borderColor: on ? 'var(--accent)' : 'var(--border)',
                          }}
                        >
                          {m}
                        </button>
                      )
                    })}
                  </div>
                  {allowedModules.length > 0 && (
                    <button type="button" className="btn btn-sm" style={{ fontSize: 11, marginTop: 4, alignSelf: 'flex-start' }} onClick={() => setAllowedModules([])}>
                      {zh ? '清空（不限）' : 'Clear (unrestricted)'}
                    </button>
                  )}
                </div>
              </div>
            </details>

            {error && <p style={{ color: 'var(--danger)', margin: 0, fontSize: 13 }}>{error}</p>}

            <div style={{ display: 'flex', gap: '0.5rem', justifyContent: 'flex-end' }}>
              <button className="btn" onClick={onClose}>{zh ? '取消' : 'Cancel'}</button>
              <button
                className="btn btn-primary"
                disabled={loading || !prompt.trim()}
                onClick={handleGenerate}
              >
                {loading ? (zh ? '生成中…' : 'Generating…') : `✦ ${zh ? '生成' : 'Generate'}`}
              </button>
            </div>
          </div>
        ) : (
          <div style={{ padding: '1rem', display: 'flex', flexDirection: 'column', gap: '1rem' }}>
            <div style={{
              background: 'var(--panel)',
              border: '1px solid var(--border)',
              borderRadius: 6,
              padding: '0.75rem 1rem',
            }}>
              <div style={{ fontWeight: 600, marginBottom: 4 }}>{preview?.name}</div>
              {preview?.description && (
                <div style={{ fontSize: 13, color: 'var(--muted)', marginBottom: 8 }}>{preview.description}</div>
              )}
              <div style={{ display: 'flex', gap: '0.75rem', fontSize: 12 }}>
                <span style={{ background: 'var(--surface)', padding: '2px 8px', borderRadius: 4, border: '1px solid var(--border)' }}>
                  {nodeCount} {zh ? '个节点' : 'nodes'}
                </span>
                <span style={{ background: 'var(--surface)', padding: '2px 8px', borderRadius: 4, border: '1px solid var(--border)' }}>
                  {edgeCount} {zh ? '条边' : 'edges'}
                </span>
              </div>
            </div>

            <div>
              <div style={{ fontSize: 12, color: 'var(--muted)', marginBottom: 6 }}>{zh ? '生成的节点：' : 'Generated nodes:'}</div>
              <div style={{ display: 'flex', flexWrap: 'wrap', gap: '0.4rem' }}>
                {(preview?.graph?.nodes ?? []).map((n: { id: string; type: string }) => (
                  <span
                    key={n.id}
                    style={{
                      padding: '2px 8px',
                      borderRadius: 4,
                      fontSize: 12,
                      background: 'var(--surface)',
                      border: '1px solid var(--border)',
                      fontFamily: 'monospace',
                    }}
                  >
                    {n.id} <span style={{ color: 'var(--muted)' }}>({n.type})</span>
                  </span>
                ))}
              </div>
            </div>

            {error && <p style={{ color: 'var(--danger)', margin: 0, fontSize: 13 }}>{error}</p>}

            <div style={{ display: 'flex', gap: '0.5rem', justifyContent: 'flex-end', flexWrap: 'wrap' }}>
              <button className="btn" onClick={() => { setMode('generate'); setPreview(null) }}>
                ← {zh ? '返回' : 'Back'}
              </button>
              <button className="btn" onClick={handleLoadIntoEditor} disabled={loading}>
                {zh ? '载入编辑器' : 'Load into Editor'}
              </button>
              <button
                className="btn btn-primary"
                onClick={handleCreateAndOpen}
                disabled={loading}
              >
                {loading ? (zh ? '创建中…' : 'Creating…') : `+ ${zh ? '创建工作流' : 'Create Workflow'}`}
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  )
}
