// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useState } from 'react'
import { useAuth } from '../../AuthContext'
import * as api from '../../api/client'
import type { WorkflowRecord } from '../../types'

// Toolbar workflow-tag editor: renders the tag chips and an inline "+ tag"
// add control. Owns its add-mode UI state and persists tag changes itself,
// reporting the refreshed workflow back via onUpdate. Extracted verbatim from
// WorkflowEditor's toolbar.

export interface TagEditorProps {
  workflow: WorkflowRecord
  workflowId: string
  zh: boolean
  toast: (message: string, kind?: 'success' | 'error') => void
  onUpdate: (wf: WorkflowRecord) => void
}

export function TagEditor({ workflow, workflowId, zh, toast, onUpdate }: TagEditorProps) {
  const { auth } = useAuth()
  const [addingTag, setAddingTag] = useState(false)
  const [newTagInput, setNewTagInput] = useState('')

  const handleAddTag = async (tag: string) => {
    const trimmed = tag.trim().toLowerCase().replace(/\s+/g, '-').slice(0, 40)
    if (!trimmed || workflow.tags?.includes(trimmed)) { setAddingTag(false); setNewTagInput(''); return }
    const newTags = [...(workflow.tags ?? []), trimmed]
    try {
      const wf = await api.updateWorkflowTags(auth!.tenantId, workflowId, workflow.name, newTags)
      onUpdate(wf)
    } catch (e) { toast(String(e), 'error') }
    setAddingTag(false); setNewTagInput('')
  }

  const handleRemoveTag = async (tag: string) => {
    const newTags = (workflow.tags ?? []).filter(t => t !== tag)
    try {
      const wf = await api.updateWorkflowTags(auth!.tenantId, workflowId, workflow.name, newTags)
      onUpdate(wf)
    } catch (e) { toast(String(e), 'error') }
  }

  return (
    <>
      {(workflow.tags ?? []).map(tag => (
        <span key={tag} style={{ display: 'inline-flex', alignItems: 'center', gap: 2, background: 'var(--border)', borderRadius: 4, padding: '1px 5px', fontSize: 11, color: 'var(--fg)' }}>
          #{tag}
          <button
            onClick={() => handleRemoveTag(tag)}
            style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--muted)', fontSize: 11, padding: '0 1px', lineHeight: 1 }}
            title={zh ? `移除标签 #${tag}` : `Remove tag #${tag}`}
          >×</button>
        </span>
      ))}
      {addingTag ? (
        <input
          autoFocus
          value={newTagInput}
          onChange={(e) => setNewTagInput(e.target.value)}
          onBlur={() => { if (newTagInput.trim()) handleAddTag(newTagInput); else { setAddingTag(false); setNewTagInput('') } }}
          onKeyDown={(e) => { if (e.key === 'Enter') handleAddTag(newTagInput); if (e.key === 'Escape') { setAddingTag(false); setNewTagInput('') } }}
          placeholder={zh ? '标签名…' : 'tag name…'}
          style={{ width: 100, fontSize: 11 }}
        />
      ) : (
        <span
          style={{ fontSize: 11, color: 'var(--muted)', cursor: 'pointer' }}
          onClick={() => setAddingTag(true)}
          title={zh ? '添加标签' : 'Add tag'}
        >+ tag</span>
      )}
    </>
  )
}
