// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useState } from 'react'
import type { WorkflowRecord } from '../../types'

// Toolbar title block: the click-to-rename workflow name, status badge and the
// click-to-edit description, plus the rename modal. Owns its own edit state
// (rename / description), persisting via the onRename / onSaveDescription
// callbacks. Extracted from WorkflowEditor's toolbar.

export interface WorkflowTitleBarProps {
  workflow: WorkflowRecord | null
  zh: boolean
  onRename: (name: string) => Promise<void>
  onSaveDescription: (description: string) => Promise<void>
}

export function WorkflowTitleBar({ workflow, zh, onRename, onSaveDescription }: WorkflowTitleBarProps) {
  const [renaming, setRenaming] = useState(false)
  const [newName, setNewName] = useState('')
  const [editingDescription, setEditingDescription] = useState(false)
  const [newDescription, setNewDescription] = useState('')

  const startRename = () => { setNewName(workflow?.name ?? ''); setRenaming(true) }
  const submitRename = async () => {
    if (!newName.trim() || newName === workflow?.name) { setRenaming(false); return }
    try { await onRename(newName.trim()) } finally { setRenaming(false) }
  }
  const submitDescription = async () => {
    try { await onSaveDescription(newDescription.trim()) } finally { setEditingDescription(false) }
  }

  return (
    <>
      {renaming ? (
        <input
          autoFocus
          value={newName}
          onChange={(e) => setNewName(e.target.value)}
          onBlur={submitRename}
          onKeyDown={(e) => { if (e.key === 'Enter') submitRename(); if (e.key === 'Escape') setRenaming(false) }}
          style={{ width: 200, fontSize: 14, fontWeight: 600 }}
        />
      ) : (
        <span
          className="topbar-title"
          style={{ cursor: 'pointer' }}
          onClick={startRename}
          title={zh ? '点击重命名' : 'Click to rename'}
        >
          {workflow?.name ?? '…'}
        </span>
      )}

      {workflow && (
        <span className={`badge badge-${workflow.status}`}>{workflow.status}</span>
      )}
      {workflow && (
        editingDescription ? (
          <input
            autoFocus
            value={newDescription}
            onChange={(e) => setNewDescription(e.target.value)}
            onBlur={submitDescription}
            onKeyDown={(e) => { if (e.key === 'Enter') submitDescription(); if (e.key === 'Escape') setEditingDescription(false) }}
            placeholder={zh ? '添加描述…' : 'Add a description…'}
            style={{ width: 260, fontSize: 12, color: 'var(--muted)', fontStyle: 'normal' }}
          />
        ) : (
          <span
            style={{ fontSize: 12, color: 'var(--muted)', cursor: 'pointer', fontStyle: workflow.description ? 'normal' : 'italic', maxWidth: 260, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}
            onClick={() => { setEditingDescription(true); setNewDescription(workflow.description ?? '') }}
            title={workflow.description ? (zh ? '点击编辑描述' : 'Click to edit description') : (zh ? '点击添加描述' : 'Click to add description')}
          >
            {workflow.description ?? (zh ? '添加描述…' : 'Add description…')}
          </span>
        )
      )}
    </>
  )
}
