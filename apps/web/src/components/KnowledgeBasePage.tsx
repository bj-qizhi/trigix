// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useEffect, useState } from 'react'
import { ThemeToggleIcon } from './uiIcons'
import { useLocale } from '../useLocale'
import { useTheme } from '../useTheme'
import { friendlyError } from '../errorMessage'
import * as api from '../api/client'
import type { KnowledgeBase, RagDocument } from '../api/client'
import logoWordmark from '../assets/logo-wordmark.svg'

interface Props {
  onBack: () => void
}

function formatTs(secs: number): string {
  return new Date(secs * 1000).toLocaleString()
}

export function KnowledgeBasePage({ onBack }: Props) {
  const { theme, toggle: toggleTheme } = useTheme()
  const { locale } = useLocale()
  const zh = locale === 'zh'

  const [kbs, setKbs] = useState<KnowledgeBase[]>([])
  const [selected, setSelected] = useState<string | null>(null)
  const [docs, setDocs] = useState<RagDocument[]>([])
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  // Ingest form
  const [kb, setKb] = useState('')
  const [docId, setDocId] = useState('')
  const [text, setText] = useState('')

  const loadKbs = () => {
    api.listKnowledgeBases()
      .then((r) => setKbs(r.knowledge_bases))
      .catch((e: unknown) => setError(friendlyError(e, zh)))
  }

  const loadDocs = (name: string) => {
    setSelected(name)
    api.listRagDocuments(name)
      .then((r) => setDocs(r.documents))
      .catch((e: unknown) => setError(friendlyError(e, zh)))
  }

  useEffect(loadKbs, [])

  const handleIngest = async () => {
    if (!kb.trim() || !docId.trim() || !text.trim()) return
    setBusy(true)
    setError(null)
    try {
      await api.ingestRagDocument({ kb: kb.trim(), doc_id: docId.trim(), text: text.trim() })
      setDocId(''); setText('')
      loadKbs()
      loadDocs(kb.trim())
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setBusy(false)
    }
  }

  const handleDelete = async (docId: string) => {
    if (!selected) return
    if (!window.confirm(zh ? `删除文档 "${docId}"？` : `Delete document "${docId}"?`)) return
    try {
      await api.deleteRagDocument(selected, docId)
      loadDocs(selected)
      loadKbs()
    } catch (e: unknown) {
      setError(friendlyError(e, zh))
    }
  }

  const fieldStyle: React.CSSProperties = { display: 'flex', flexDirection: 'column', gap: 4 }
  const labelStyle: React.CSSProperties = { fontSize: 12, color: 'var(--muted)' }

  return (
    <div className="app">
      <header className="topbar">
        <button className="btn btn-sm btn-icon" onClick={onBack} title={zh ? '返回' : 'Back'}>←</button>
        <span className="topbar-sep">|</span>
        <img src={logoWordmark} alt="Trigix" style={{ height: '28px' }} />
        <span className="topbar-sep">|</span>
        <span className="topbar-title">{zh ? '知识库' : 'Knowledge Bases'}</span>
        <div className="topbar-actions">
          <button className="btn btn-sm" onClick={toggleTheme} title={zh ? '切换主题' : 'Toggle theme'}>
            {theme === 'dark' ? <ThemeToggleIcon dark /> : <ThemeToggleIcon dark={false} />}
          </button>
        </div>
      </header>

      <div className="list-page">
        <div className="list-header">
          <h1>{zh ? '知识库（RAG）' : 'Knowledge Bases (RAG)'}</h1>
        </div>

        <p style={{ marginBottom: 16, fontSize: 13, color: 'var(--muted)' }}>
          {zh
            ? '将文档灌入知识库后，工作流中的 RAG 节点即可检索相关内容增强提示词。需要 AI Runtime 已配置（AI_RUNTIME_BASE_URL）且连接到带 pgvector 的数据库。'
            : 'Ingest documents into a knowledge base so the RAG node can retrieve relevant context in workflows. Requires the AI Runtime (AI_RUNTIME_BASE_URL) connected to a pgvector database.'}
        </p>

        {error && <p style={{ color: '#ef4444', fontSize: 13, marginBottom: 12 }}>{error}</p>}

        {/* Ingest form */}
        <div style={{
          display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 10, marginBottom: 20,
          background: 'var(--surface)', border: '1px solid var(--border)',
          borderRadius: 'var(--radius)', padding: '14px 16px',
        }}>
          <div style={fieldStyle}>
            <label style={labelStyle}>{zh ? '知识库名称' : 'Knowledge base'}</label>
            <input className="input" placeholder="handbook" value={kb} onChange={(e) => setKb(e.target.value)} />
          </div>
          <div style={fieldStyle}>
            <label style={labelStyle}>{zh ? '文档 ID' : 'Document ID'}</label>
            <input className="input" placeholder="pto-policy" value={docId} onChange={(e) => setDocId(e.target.value)} />
          </div>
          <div style={{ ...fieldStyle, gridColumn: '1 / 3' }}>
            <label style={labelStyle}>{zh ? '文档内容' : 'Document text'}</label>
            <textarea className="input" rows={6} value={text} onChange={(e) => setText(e.target.value)} />
          </div>
          <div style={{ gridColumn: '1 / 3', display: 'flex', justifyContent: 'flex-end' }}>
            <button className="btn btn-primary" onClick={handleIngest} disabled={busy || !kb.trim() || !docId.trim() || !text.trim()}>
              {busy ? (zh ? '灌入中…' : 'Ingesting…') : (zh ? '+ 灌入文档' : '+ Ingest Document')}
            </button>
          </div>
        </div>

        <div style={{ display: 'flex', gap: 20, alignItems: 'flex-start' }}>
          {/* KB list */}
          <div style={{ flex: '0 0 240px' }}>
            <h3 style={{ fontSize: 14, marginBottom: 8 }}>{zh ? '知识库' : 'Knowledge bases'}</h3>
            {kbs.length === 0 ? (
              <p style={{ color: 'var(--muted)', fontSize: 13 }}>{zh ? '暂无知识库' : 'No knowledge bases yet'}</p>
            ) : (
              <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
                {kbs.map((k) => (
                  <button
                    key={k.kb}
                    className={`btn btn-sm${selected === k.kb ? ' btn-primary' : ''}`}
                    style={{ justifyContent: 'space-between', display: 'flex', width: '100%' }}
                    onClick={() => loadDocs(k.kb)}
                  >
                    <span>{k.kb}</span>
                    <span style={{ fontSize: 11, opacity: 0.7 }}>{k.docs} docs · {k.chunks} chunks</span>
                  </button>
                ))}
              </div>
            )}
          </div>

          {/* Documents in selected KB */}
          <div style={{ flex: 1 }}>
            <h3 style={{ fontSize: 14, marginBottom: 8 }}>
              {selected ? (zh ? `「${selected}」中的文档` : `Documents in "${selected}"`) : (zh ? '选择一个知识库' : 'Select a knowledge base')}
            </h3>
            {selected && docs.length === 0 && (
              <p style={{ color: 'var(--muted)', fontSize: 13 }}>{zh ? '该知识库暂无文档' : 'No documents in this knowledge base'}</p>
            )}
            {selected && docs.length > 0 && (
              <table className="data-table" style={{ width: '100%' }}>
                <thead>
                  <tr>
                    <th>{zh ? '文档 ID' : 'Document ID'}</th>
                    <th>{zh ? '分块数' : 'Chunks'}</th>
                    <th>{zh ? '创建时间' : 'Created'}</th>
                    <th></th>
                  </tr>
                </thead>
                <tbody>
                  {docs.map((d) => (
                    <tr key={d.doc_id}>
                      <td><code>{d.doc_id}</code></td>
                      <td>{d.chunks}</td>
                      <td style={{ color: 'var(--muted)', fontSize: 12 }}>{formatTs(d.created_at)}</td>
                      <td>
                        <button className="btn btn-sm btn-danger" onClick={() => handleDelete(d.doc_id)}>
                          {zh ? '删除' : 'Delete'}
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </div>
        </div>
      </div>
    </div>
  )
}
