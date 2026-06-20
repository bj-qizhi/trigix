// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useState } from 'react'
import type * as api from '../../api/client'

// Self-contained modals extracted from WorkflowList: the create-workflow form,
// the platform info dialog and the keyboard-shortcuts cheat sheet. Verbatim
// moves; the create form owns its own input state and reports the new name via
// onCreate.

export interface CreateWorkflowModalProps {
  onCreate: (name: string, description?: string) => Promise<void>
  onClose: () => void
  zh: boolean
}

export function CreateWorkflowModal({ onCreate, onClose, zh }: CreateWorkflowModalProps) {
  const [name, setName] = useState('')
  const [description, setDescription] = useState('')
  const [saving, setSaving] = useState(false)

  const submit = async () => {
    if (!name.trim()) return
    setSaving(true)
    // On success the parent navigates to the new workflow (this unmounts);
    // on error we surface it and re-enable the form.
    try { await onCreate(name.trim(), description.trim() || undefined) }
    catch (e) { alert(String(e)); setSaving(false) }
  }

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h2>{zh ? '新建工作流' : 'New Workflow'}</h2>
        <div className="field">
          <label>{zh ? '名称' : 'Name'}</label>
          <input
            autoFocus
            placeholder={zh ? '如：线索富化' : 'e.g. Lead Enrichment'}
            value={name}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && submit()}
          />
        </div>
        <div className="field">
          <label>{zh ? '描述' : 'Description'} <span style={{ color: 'var(--muted)', fontWeight: 400 }}>{zh ? '（可选）' : '(optional)'}</span></label>
          <input
            placeholder={zh ? '此工作流的用途？' : 'What does this workflow do?'}
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && submit()}
          />
        </div>
        <div className="modal-actions">
          <button className="btn" onClick={onClose}>{zh ? '取消' : 'Cancel'}</button>
          <button className="btn btn-primary" disabled={!name.trim() || saving} onClick={submit}>
            {saving ? (zh ? '创建中…' : 'Creating…') : (zh ? '创建' : 'Create')}
          </button>
        </div>
      </div>
    </div>
  )
}

export interface SystemInfoModalProps {
  info: api.SystemInfo | null
  onClose: () => void
  zh: boolean
}

export function SystemInfoModal({ info, onClose, zh }: SystemInfoModalProps) {
  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" style={{ width: 400 }} onClick={(e) => e.stopPropagation()}>
        <h2>{zh ? '平台信息' : 'Platform Info'}</h2>
        {!info ? (
          <p style={{ color: 'var(--muted)' }}>{zh ? '加载中…' : 'Loading…'}</p>
        ) : (
          <>
            <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 13, marginBottom: 16 }}>
              <tbody>
                {[
                  [zh ? '版本' : 'Version', info.version],
                  [zh ? '节点类型数' : 'Node types', String(info.node_types)],
                  [zh ? 'Rust 版本' : 'Rust edition', info.rust_edition],
                  [zh ? '需要鉴权' : 'Auth required', info.auth_required ? (zh ? '是' : 'Yes') : (zh ? '否（开发模式）' : 'No (dev mode)')],
                ].map(([k, v]) => (
                  <tr key={k} style={{ borderBottom: '1px solid var(--border)' }}>
                    <td style={{ padding: '6px 8px', color: 'var(--muted)', width: 130 }}>{k}</td>
                    <td style={{ padding: '6px 8px', fontWeight: 600 }}>{v}</td>
                  </tr>
                ))}
              </tbody>
            </table>
            <div style={{ fontSize: 12, color: 'var(--muted)', marginBottom: 16 }}>
              <strong>{zh ? '功能特性：' : 'Features:'}</strong>
              <div style={{ display: 'flex', flexWrap: 'wrap', gap: 4, marginTop: 6 }}>
                {info.features.map((f) => (
                  <span key={f} style={{
                    background: 'var(--panel)', border: '1px solid var(--border)',
                    borderRadius: 4, padding: '2px 6px', fontSize: 11, fontFamily: 'monospace',
                  }}>{f}</span>
                ))}
              </div>
            </div>
          </>
        )}
        <div className="modal-footer">
          <button className="btn btn-sm" onClick={onClose}>{zh ? '关闭' : 'Close'}</button>
        </div>
      </div>
    </div>
  )
}

export interface ShortcutsModalProps {
  onClose: () => void
  zh: boolean
}

export function ShortcutsModal({ onClose, zh }: ShortcutsModalProps) {
  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" style={{ maxWidth: 480 }} onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h3>⌨ {zh ? '键盘快捷键' : 'Keyboard Shortcuts'}</h3>
          <button className="btn btn-sm" onClick={onClose}>✕</button>
        </div>
        <div style={{ padding: '0 24px 20px' }}>
          {[
            { section: zh ? '导航' : 'Navigation', items: [
              { key: '/', desc: zh ? '聚焦搜索框' : 'Focus search' },
              { key: 'j / ↓', desc: zh ? '下移光标' : 'Move cursor down' },
              { key: 'k / ↑', desc: zh ? '上移光标' : 'Move cursor up' },
              { key: 'Enter', desc: zh ? '打开选中工作流' : 'Open focused workflow' },
            ]},
            { section: zh ? '操作' : 'Actions', items: [
              { key: 'n', desc: zh ? '新建工作流' : 'New workflow' },
              { key: 'Ctrl+R', desc: zh ? '快速运行' : 'Quick Run modal' },
              { key: 'Ctrl+Shift+F', desc: zh ? '全局搜索' : 'Global search' },
            ]},
            { section: zh ? '界面' : 'UI', items: [
              { key: '? / h', desc: zh ? '显示此帮助' : 'Show this help' },
            ]},
          ].map(({ section, items }) => (
            <div key={section} style={{ marginTop: 16 }}>
              <div style={{ fontSize: 11, fontWeight: 700, color: 'var(--muted)', letterSpacing: 1, textTransform: 'uppercase', marginBottom: 8 }}>{section}</div>
              {items.map(({ key, desc }) => (
                <div key={key} style={{ display: 'flex', alignItems: 'center', gap: 12, padding: '4px 0', borderBottom: '1px solid var(--border)', fontSize: 13 }}>
                  <code style={{ background: 'var(--panel)', border: '1px solid var(--border)', borderRadius: 4, padding: '2px 7px', fontSize: 12, fontFamily: 'monospace', minWidth: 110, textAlign: 'center' }}>{key}</code>
                  <span style={{ color: 'var(--muted)' }}>{desc}</span>
                </div>
              ))}
            </div>
          ))}
        </div>
      </div>
    </div>
  )
}
