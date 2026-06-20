// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

// Editor help modal: keyboard shortcuts, canvas tips and the template-variable
// cheat sheet. Pure presentational content extracted verbatim from
// WorkflowEditor.

export interface HelpModalProps {
  zh: boolean
  onClose: () => void
}

export function HelpModal({ zh, onClose }: HelpModalProps) {
  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" style={{ width: 480, maxHeight: '80vh', overflowY: 'auto' }} onClick={(e) => e.stopPropagation()}>
        <h2>{zh ? '键盘快捷键与提示' : 'Keyboard Shortcuts & Tips'}</h2>
        <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 12, marginBottom: 16 }}>
          <tbody>
            {(zh ? [
              ['Ctrl+S', '将当前图保存为新版本'],
              ['Ctrl+Enter', '运行工作流'],
              ['Ctrl+K', '打开节点命令面板（搜索/跳转到节点）'],
              ['Escape', '取消选中节点'],
              ['Ctrl+D', '复制选中节点'],
              ['Ctrl+Z', '撤销最近的节点添加/删除/布局'],
              ['Ctrl+Shift+Z / Ctrl+Y', '重做'],
              ['Delete / Backspace', '删除选中节点及其关联边'],
              ['f', '将所有节点适配到视图'],
              ['点击边标签', '切换条件分支（true ↔ false）'],
            ] : [
              ['Ctrl+S', 'Save current graph as a new version'],
              ['Ctrl+Enter', 'Run the workflow'],
              ['Ctrl+K', 'Open node command palette (search/jump to node)'],
              ['Escape', 'Deselect selected node'],
              ['Ctrl+D', 'Duplicate selected node'],
              ['Ctrl+Z', 'Undo last node add/delete/layout'],
              ['Ctrl+Shift+Z / Ctrl+Y', 'Redo'],
              ['Delete / Backspace', 'Delete selected node and its edges'],
              ['f', 'Fit all nodes into view'],
              ['Click edge label', 'Toggle condition branch (true ↔ false)'],
            ]).map(([k, v]) => (
              <tr key={k} style={{ borderBottom: '1px solid var(--border)' }}>
                <td style={{ padding: '6px 8px', fontFamily: 'monospace', fontWeight: 700, whiteSpace: 'nowrap', color: 'var(--link)' }}>{k}</td>
                <td style={{ padding: '6px 8px', color: 'var(--fg)' }}>{v}</td>
              </tr>
            ))}
          </tbody>
        </table>
        <h3 style={{ fontSize: 13, marginBottom: 6 }}>{zh ? '画布提示' : 'Canvas tips'}</h3>
        <ul style={{ fontSize: 12, color: 'var(--fg)', lineHeight: 1.7, paddingLeft: 18, marginBottom: 14 }}>
          <li>{zh ? '从节点连接点拖出以连线' : 'Drag from a node handle to connect nodes'}</li>
          <li>{zh ? <>使用 <strong>⊞ 布局</strong> 自动拓扑排列节点</> : <>Use <strong>⊞ Layout</strong> to auto-arrange nodes topologically</>}</li>
          <li>{zh ? <>启用 <strong>⊹ 对齐</strong> 以在拖拽时 16px 网格对齐</> : <>Enable <strong>⊹ Snap</strong> for 16px grid alignment while dragging</>}</li>
          <li>{zh ? <>切换 <strong>▣ 地图</strong> 以显示/隐藏小地图</> : <>Toggle <strong>▣ Map</strong> to hide/show the minimap</>}</li>
          <li>{zh ? <>按 <strong>f</strong> 或点击 <strong>⊡ 适配</strong> 将所有节点适配到视图</> : <>Press <strong>f</strong> or click <strong>⊡ Fit</strong> to fit all nodes into view</>}</li>
          <li>{zh ? '点击节点以在右侧面板中打开其配置' : 'Click a node to open its config in the right panel'}</li>
          <li>{zh ? <>在配置面板标题栏中使用 <strong>⧉</strong> 复制节点</> : <>Use <strong>⧉</strong> in the config panel header to duplicate a node</>}</li>
        </ul>
        <h3 style={{ fontSize: 13, marginBottom: 6 }}>{zh ? '模板变量' : 'Template variables'}</h3>
        <ul style={{ fontSize: 12, color: 'var(--fg)', lineHeight: 1.7, paddingLeft: 18, marginBottom: 14 }}>
          <li><code>{'{{input.field}}'}</code> — {zh ? '工作流输入 JSON 字段' : 'workflow input JSON field'}</li>
          <li><code>{'{{node_id.field}}'}</code> — {zh ? '前置节点的输出字段' : 'output field from a previous node'}</li>
          <li><code>{'{{credential.name}}'}</code> — {zh ? '已存储的凭证值' : 'stored credential value'}</li>
          <li><code>{'{{env.KEY}}'}</code> — {zh ? '当前环境集中的环境变量' : 'environment variable from active env set'}</li>
          <li><code>{'{{variable.KEY}}'}</code> — {zh ? '持久化工作流变量' : 'persistent workflow variable'}</li>
        </ul>
        <div className="modal-actions">
          <button className="btn btn-primary" onClick={onClose}>{zh ? '关闭' : 'Close'}</button>
        </div>
      </div>
    </div>
  )
}
