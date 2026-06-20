// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

// Workflow validation results modal: lists the pre-publish warnings (or a
// ready-to-publish confirmation). Extracted verbatim from WorkflowEditor.

export interface ValidationModalProps {
  warnings: string[]
  zh: boolean
  onClose: () => void
}

export function ValidationModal({ warnings, zh, onClose }: ValidationModalProps) {
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
          <ul style={{ margin: '8px 0 16px', padding: '0 0 0 18px', fontSize: 13, lineHeight: 1.8 }}>
            {warnings.map((w, i) => (
              <li key={i} style={{ color: 'var(--warning-text)' }}>{w}</li>
            ))}
          </ul>
        )}
        <div className="modal-actions">
          <button className="btn btn-primary" onClick={onClose}>{zh ? '关闭' : 'Close'}</button>
        </div>
      </div>
    </div>
  )
}
