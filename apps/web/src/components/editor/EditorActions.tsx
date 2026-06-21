// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import type { Dispatch, SetStateAction } from 'react'
import type { TranslationKey } from '../../i18n'

// Toolbar save / publish / publish-and-run controls extracted from
// WorkflowEditor — the cluster carrying the dirty/publishable/in-flight
// conditionals. The save-message popover state lives in the persistence hook
// (handleSave reads it), so it is passed in.

export interface EditorActionsProps {
  isDirty: boolean
  saving: boolean
  publishing: boolean
  publishingAndRunning: boolean
  canPublish: boolean
  saveMessage: string
  setSaveMessage: Dispatch<SetStateAction<string>>
  showSaveMessage: boolean
  setShowSaveMessage: Dispatch<SetStateAction<boolean>>
  onSave: () => void
  onPublish: () => void
  onPublishAndRun: () => void
  zh: boolean
  t: (key: TranslationKey) => string
}

export function EditorActions({
  isDirty, saving, publishing, publishingAndRunning, canPublish,
  saveMessage, setSaveMessage, showSaveMessage, setShowSaveMessage,
  onSave, onPublish, onPublishAndRun, zh, t,
}: EditorActionsProps) {
  return (
    <>
      <div style={{ position: 'relative', display: 'inline-flex', alignItems: 'center' }}>
        <button
          className={`btn btn-sm${isDirty ? ' btn-primary' : ''}`}
          disabled={saving}
          onClick={showSaveMessage ? onSave : () => setShowSaveMessage(true)}
          title={isDirty ? 'Unsaved changes — save a new version' : 'Save current graph as a new version'}
        >
          {saving ? (zh ? '保存中…' : 'Saving…') : isDirty ? t('we.save.dirty') : t('we.save')}
        </button>
        {showSaveMessage && (
          <div style={{ position: 'absolute', top: '100%', right: 0, zIndex: 200, background: 'var(--panel)', border: '1px solid var(--border)', borderRadius: 6, padding: '8px 10px', width: 240, marginTop: 4, boxShadow: '0 4px 12px rgba(0,0,0,0.3)' }}>
            <input
              autoFocus
              placeholder={zh ? '保存备注（可选）' : 'Save message (optional)'}
              value={saveMessage}
              onChange={(e) => setSaveMessage(e.target.value)}
              onKeyDown={(e) => { if (e.key === 'Enter') onSave(); if (e.key === 'Escape') { setShowSaveMessage(false); setSaveMessage('') } }}
              style={{ width: '100%', fontSize: 12, marginBottom: 6, boxSizing: 'border-box' }}
            />
            <div style={{ display: 'flex', gap: 6, justifyContent: 'flex-end' }}>
              <button className="btn btn-sm" onClick={() => { setShowSaveMessage(false); setSaveMessage('') }}>{zh ? '取消' : 'Cancel'}</button>
              <button className="btn btn-sm btn-primary" disabled={saving} onClick={onSave}>{zh ? '保存' : 'Save'}</button>
            </div>
          </div>
        )}
      </div>
      <button
        className="btn btn-sm btn-primary"
        disabled={!canPublish || publishing}
        onClick={onPublish}
        title={canPublish ? 'Publish this draft version' : 'No draft version to publish'}
      >
        {publishing ? (zh ? '发布中…' : 'Publishing…') : t('we.publish')}
      </button>
      {canPublish && (
        <button
          className="btn btn-sm"
          disabled={publishingAndRunning || publishing}
          onClick={onPublishAndRun}
          title={zh ? '发布版本并立即运行' : 'Publish and immediately run'}
          style={{ fontSize: 11 }}
        >
          {publishingAndRunning ? '…' : (zh ? '▶ 发布并运行' : '▶ Publish & Run')}
        </button>
      )}
    </>
  )
}
