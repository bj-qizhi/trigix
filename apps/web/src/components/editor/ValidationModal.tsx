// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

// Workflow validation results modal: lists the pre-publish warnings (or a
// ready-to-publish confirmation). Node-scoped warnings are clickable and select
// + center the offending node on the canvas.

import type { PublishWarning } from './publishWarnings'

export interface ValidationModalProps {
  warnings: PublishWarning[]
  zh: boolean
  onClose: () => void
  onJump?: (nodeId: string) => void
}

export function ValidationModal({ warnings, zh, onClose, onJump }: ValidationModalProps) {
  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" style={{ width: 480 }} onClick={(e) => e.stopPropagation()}>
        <h2 style={{ marginBottom: 8 }}>
          {zh ? '工作流校验' : 'Workflow Validation'}
          {warnings.length === 0
            ? <span style={{ color: 'var(--success-text)', fontSize: 14, fontWeight: 400, marginLeft: 8 }}>✓ {zh ? '无问题' : 'No issues'}</span>
            : <span style={{ color: 'var(--warning-text)', fontSize: 14, fontWeight: 400, marginLeft: 8 }}>{zh ? `${warnings.length} 个问题` : `${warnings.length} issue${warnings.length !== 1 ? 's' : ''}`}</span>
          }
        </h2>
        {warnings.length === 0 ? (
          <p style={{ color: 'var(--muted)', fontSize: 13 }}>
            {zh ? '所有节点均已连接并配置完成。工作流已准备好发布。' : 'All nodes are connected and configured. The workflow is ready to publish.'}
          </p>
        ) : (
          <ul style={{ margin: '8px 0 16px', padding: 0, listStyle: 'none', fontSize: 13, lineHeight: 1.6, display: 'flex', flexDirection: 'column', gap: 4 }}>
            {warnings.map((w, i) => {
              const jumpable = w.nodeId && onJump
              return (
                <li
                  key={i}
                  onClick={jumpable ? () => { onJump!(w.nodeId!); onClose() } : undefined}
                  title={jumpable ? (zh ? '点击定位到该节点' : 'Click to jump to this node') : undefined}
                  style={{
                    color: 'var(--warning-text)',
                    display: 'flex', alignItems: 'baseline', gap: 6,
                    padding: '4px 8px', borderRadius: 'var(--radius)',
                    cursor: jumpable ? 'pointer' : 'default',
                  }}
                  onMouseEnter={jumpable ? (e) => { (e.currentTarget as HTMLElement).style.background = 'var(--faint)' } : undefined}
                  onMouseLeave={jumpable ? (e) => { (e.currentTarget as HTMLElement).style.background = 'transparent' } : undefined}
                >
                  <span aria-hidden>•</span>
                  <span style={{ flex: 1 }}>{w.message}</span>
                  {jumpable && <span style={{ fontSize: 12, opacity: 0.8, whiteSpace: 'nowrap' }}>{zh ? '定位 →' : 'Jump →'}</span>}
                </li>
              )
            })}
          </ul>
        )}
        <div className="modal-actions">
          <button className="btn btn-primary" onClick={onClose}>{zh ? '关闭' : 'Close'}</button>
        </div>
      </div>
    </div>
  )
}
