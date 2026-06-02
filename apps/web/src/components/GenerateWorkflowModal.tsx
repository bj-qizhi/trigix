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

export function GenerateWorkflowModal({ onClose, onImport, onCreated }: Props) {
  const { auth } = useAuth()
  const { locale } = useLocale()
  const zh = locale === 'zh'
  const [prompt, setPrompt] = useState('')
  const [apiKey, setApiKey] = useState('')
  const [model, setModel] = useState('claude-sonnet-4-6')
  const [loading, setLoading] = useState(false)
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
        apiKey: apiKey || undefined,
        model,
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
        apiKey: apiKey || undefined,
        model,
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
              <div style={{ marginTop: '0.5rem', display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
                <div className="field">
                  <label>
                    Claude API Key{' '}
                    <span style={{ color: 'var(--muted)', fontWeight: 400 }}>
                      {zh ? '（留空使用 ANTHROPIC_API_KEY 环境变量）' : '(uses ANTHROPIC_API_KEY env if blank)'}
                    </span>
                  </label>
                  <input
                    type="password"
                    value={apiKey}
                    onChange={(e) => setApiKey(e.target.value)}
                    placeholder="sk-ant-..."
                  />
                </div>
                <div className="field">
                  <label>{zh ? '模型' : 'Model'}</label>
                  <select value={model} onChange={(e) => setModel(e.target.value)}>
                    <option value="claude-sonnet-4-6">claude-sonnet-4-6 ({zh ? '推荐' : 'recommended'})</option>
                    <option value="claude-opus-4-7">claude-opus-4-7 ({zh ? '最强' : 'most capable'})</option>
                    <option value="claude-haiku-4-5-20251001">claude-haiku-4-5 ({zh ? '最快' : 'fastest'})</option>
                  </select>
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
