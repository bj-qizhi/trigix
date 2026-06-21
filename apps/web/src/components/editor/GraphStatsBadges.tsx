// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import type { FlowNode, FlowEdge } from '../Canvas'

// Graph statistics for the editor stats bar: node/edge counts, an AI/HTTP/
// integration node-type breakdown and a derived complexity score. Pure from
// nodes/edges — extracted from WorkflowEditor to take the two derivation IIFEs
// out of its return.

export interface GraphStatsBadgesProps {
  nodes: FlowNode[]
  edges: FlowEdge[]
  zh: boolean
}

export function GraphStatsBadges({ nodes, edges, zh }: GraphStatsBadgesProps) {
  const breakdown = (() => {
    const execNodes = nodes.filter(n => n.data.nodeType !== 'note' && n.data.nodeType !== 'trigger')
    const aiNodes = execNodes.filter(n => ['openai', 'gemini', 'claude', 'agent'].includes(n.data.nodeType ?? ''))
    const httpNodes = execNodes.filter(n => ['http', 'graphql', 'github', 'webhook', 'jira', 'notion', 'linear', 'airtable'].includes(n.data.nodeType ?? ''))
    const integNodes = execNodes.filter(n => ['slack', 'email', 'database'].includes(n.data.nodeType ?? ''))
    const parts: string[] = []
    if (aiNodes.length > 0) parts.push(`${aiNodes.length} AI`)
    if (httpNodes.length > 0) parts.push(`${httpNodes.length} HTTP`)
    if (integNodes.length > 0) parts.push(zh ? `${integNodes.length} 集成` : `${integNodes.length} integration`)
    return parts.length > 0
      ? <span style={{ color: 'var(--muted)' }} title={zh ? '节点类型分布' : 'Node type breakdown'}>{parts.join(' · ')}</span>
      : null
  })()

  const complexity = (() => {
    const execNodes = nodes.filter(n => n.data.nodeType !== 'note' && n.data.nodeType !== 'trigger')
    const aiNodes = execNodes.filter(n => ['openai', 'gemini', 'claude', 'agent'].includes(n.data.nodeType ?? ''))
    const score = Math.min(10, Math.floor(execNodes.length * 0.5 + edges.length * 0.3 + aiNodes.length * 1.5))
    if (score < 2) return null
    const label = zh ? (score <= 3 ? '简单' : score <= 6 ? '中等' : '复杂') : (score <= 3 ? 'simple' : score <= 6 ? 'moderate' : 'complex')
    const color = score <= 3 ? 'var(--success-text)' : score <= 6 ? '#d97706' : 'var(--danger-text)'
    return <span title={zh ? `复杂度分数：${score}/10（基于节点/边数量和 AI 节点）` : `Complexity score: ${score}/10 (based on node/edge count and AI nodes)`} style={{ color }}>{zh ? '复杂度：' : 'complexity: '}<strong>{label}</strong></span>
  })()

  return (
    <>
      <span title={zh ? `${nodes.filter(n => n.data.nodeType !== 'note').length} 个可执行节点 + ${nodes.filter(n => n.data.nodeType === 'note').length} 个注释` : `${nodes.filter(n => n.data.nodeType !== 'note').length} executable nodes + ${nodes.filter(n => n.data.nodeType === 'note').length} note(s)`}>
        <strong style={{ color: 'var(--fg)' }}>{nodes.length}</strong> {zh ? '个节点' : `node${nodes.length !== 1 ? 's' : ''}`}
      </span>
      <span><strong style={{ color: 'var(--fg)' }}>{edges.length}</strong> {zh ? '条边' : `edge${edges.length !== 1 ? 's' : ''}`}</span>
      {breakdown}
      {complexity}
    </>
  )
}
