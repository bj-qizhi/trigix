// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useEffect, useRef, useState } from 'react'
import * as api from '../api/client'
import type { WorkflowComment } from '../types'
import { useLocale } from '../useLocale'

interface Props {
  tenantId: string
  workflowId: string
  author: string
  onClose: () => void
}

function formatTime(ts: number): string {
  return new Date(ts * 1000).toLocaleString()
}

export function CommentsModal({ tenantId, workflowId, author, onClose }: Props) {
  const { locale } = useLocale()
  const zh = locale === 'zh'
  const [comments, setComments] = useState<WorkflowComment[]>([])
  const [newBody, setNewBody] = useState('')
  const [submitting, setSubmitting] = useState(false)
  const [editingId, setEditingId] = useState<string | null>(null)
  const [editBody, setEditBody] = useState('')
  const [error, setError] = useState<string | null>(null)
  const bottomRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    api.listComments(tenantId, workflowId).then(setComments).catch(() => {})
  }, [tenantId, workflowId])

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [comments])

  const handlePost = async () => {
    if (!newBody.trim()) return
    setSubmitting(true)
    setError(null)
    try {
      const comment = await api.createComment(tenantId, workflowId, author, newBody.trim())
      setComments((prev) => [...prev, comment])
      setNewBody('')
    } catch (e) {
      setError(e instanceof Error ? e.message : (zh ? '发布评论失败' : 'Failed to post comment'))
    } finally {
      setSubmitting(false)
    }
  }

  const handleEdit = async (id: string) => {
    if (!editBody.trim()) return
    try {
      const updated = await api.editComment(tenantId, id, editBody.trim())
      setComments((prev) => prev.map((c) => (c.id === id ? updated : c)))
      setEditingId(null)
    } catch (e) {
      setError(e instanceof Error ? e.message : (zh ? '编辑评论失败' : 'Failed to edit comment'))
    }
  }

  const handleDelete = async (id: string) => {
    if (!confirm(zh ? '删除此评论？' : 'Delete this comment?')) return
    try {
      await api.deleteComment(tenantId, id)
      setComments((prev) => prev.filter((c) => c.id !== id))
    } catch (e) {
      setError(e instanceof Error ? e.message : (zh ? '删除评论失败' : 'Failed to delete comment'))
    }
  }

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div
        className="modal"
        style={{ width: 560, maxHeight: '80vh', display: 'flex', flexDirection: 'column' }}
        onClick={(e) => e.stopPropagation()}
      >
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 12 }}>
          <h2 style={{ margin: 0 }}>{zh ? '评论' : 'Comments'}</h2>
          <button className="btn btn-sm" onClick={onClose}>✕</button>
        </div>

        {/* Comment list */}
        <div style={{ flex: 1, overflowY: 'auto', display: 'flex', flexDirection: 'column', gap: 12, marginBottom: 12 }}>
          {comments.length === 0 && (
            <p style={{ color: 'var(--muted)', textAlign: 'center', margin: '24px 0' }}>
              {zh ? '暂无评论，来发表第一条吧！' : 'No comments yet. Be the first!'}
            </p>
          )}
          {comments.map((c) => (
            <div key={c.id} style={{ background: 'var(--bg)', border: '1px solid var(--border)', borderRadius: 8, padding: '10px 12px' }}>
              <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 6 }}>
                <span style={{ fontWeight: 600, fontSize: 13 }}>{c.author}</span>
                <span style={{ fontSize: 11, color: 'var(--muted)' }}>
                  {formatTime(c.created_at)}
                  {c.edited_at ? (zh ? '（已编辑）' : ' (edited)') : ''}
                </span>
              </div>
              {editingId === c.id ? (
                <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
                  <textarea
                    autoFocus
                    value={editBody}
                    onChange={(e) => setEditBody(e.target.value)}
                    rows={3}
                    style={{ width: '100%', resize: 'vertical', fontFamily: 'inherit', fontSize: 13 }}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) handleEdit(c.id)
                      if (e.key === 'Escape') setEditingId(null)
                    }}
                  />
                  <div style={{ display: 'flex', gap: 6, justifyContent: 'flex-end' }}>
                    <button className="btn btn-sm" onClick={() => setEditingId(null)}>{zh ? '取消' : 'Cancel'}</button>
                    <button className="btn btn-sm btn-primary" onClick={() => handleEdit(c.id)}>{zh ? '保存' : 'Save'}</button>
                  </div>
                </div>
              ) : (
                <div style={{ display: 'flex', alignItems: 'flex-start', justifyContent: 'space-between', gap: 8 }}>
                  <p style={{ margin: 0, whiteSpace: 'pre-wrap', fontSize: 13, flex: 1 }}>{c.body}</p>
                  <div style={{ display: 'flex', gap: 4, flexShrink: 0 }}>
                    <button
                      className="btn btn-sm"
                      style={{ fontSize: 11 }}
                      onClick={() => { setEditingId(c.id); setEditBody(c.body) }}
                      title={zh ? '编辑评论' : 'Edit comment'}
                    >✎</button>
                    <button
                      className="btn btn-sm"
                      style={{ fontSize: 11, color: 'var(--error, #e55)' }}
                      onClick={() => handleDelete(c.id)}
                      title={zh ? '删除评论' : 'Delete comment'}
                    >✕</button>
                  </div>
                </div>
              )}
            </div>
          ))}
          <div ref={bottomRef} />
        </div>

        {error && <p style={{ color: 'var(--error, #e55)', margin: '0 0 8px', fontSize: 12 }}>{error}</p>}

        {/* New comment input */}
        <div style={{ borderTop: '1px solid var(--border)', paddingTop: 12 }}>
          <textarea
            placeholder={zh ? '添加评论…（Ctrl+Enter 提交）' : 'Add a comment… (Ctrl+Enter to submit)'}
            value={newBody}
            onChange={(e) => setNewBody(e.target.value)}
            rows={3}
            style={{ width: '100%', resize: 'vertical', fontFamily: 'inherit', fontSize: 13, marginBottom: 8 }}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) handlePost()
            }}
          />
          <div style={{ display: 'flex', justifyContent: 'flex-end' }}>
            <button
              className="btn btn-primary"
              disabled={submitting || !newBody.trim()}
              onClick={handlePost}
            >
              {submitting ? (zh ? '发布中…' : 'Posting…') : (zh ? '发布评论' : 'Post Comment')}
            </button>
          </div>
        </div>
      </div>
    </div>
  )
}
