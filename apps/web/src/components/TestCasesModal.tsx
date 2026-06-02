// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useEffect, useState } from 'react'
import * as api from '../api/client'
import type { TestCase, TestCaseRunResult } from '../api/client'
import { useLocale } from '../useLocale'

interface Props {
  tenantId: string
  workflowId: string
  onClose: () => void
}

export function TestCasesModal({ tenantId, workflowId, onClose }: Props) {
  const { locale } = useLocale()
  const zh = locale === 'zh'
  const [cases, setCases] = useState<TestCase[]>([])
  const [editing, setEditing] = useState<string | null>(null)
  const [adding, setAdding] = useState(false)
  const [results, setResults] = useState<Record<string, TestCaseRunResult>>({})
  const [running, setRunning] = useState<Record<string, boolean>>({})

  useEffect(() => {
    api.listTestCases(tenantId, workflowId).then(setCases).catch(() => {})
  }, [tenantId, workflowId])

  const refresh = () =>
    api.listTestCases(tenantId, workflowId).then(setCases).catch(() => {})

  const handleRunOne = async (id: string) => {
    setRunning((p) => ({ ...p, [id]: true }))
    try {
      const result = await api.runTestCase(id)
      setResults((p) => ({ ...p, [id]: result }))
    } catch {
      // ignore
    } finally {
      setRunning((p) => ({ ...p, [id]: false }))
    }
  }

  const handleRunAll = async () => {
    for (const tc of cases) {
      handleRunOne(tc.id)
    }
  }

  const handleDelete = async (id: string) => {
    await api.deleteTestCase(id).catch(() => {})
    setCases((prev) => prev.filter((tc) => tc.id !== id))
    setResults((p) => { const n = { ...p }; delete n[id]; return n })
  }

  const passCount = Object.values(results).filter((r) => r.passed).length
  const failCount = Object.values(results).filter((r) => !r.passed).length

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div
        className="modal"
        style={{ width: 620, maxHeight: '80vh', display: 'flex', flexDirection: 'column', padding: 0 }}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '14px 18px', borderBottom: '1px solid var(--border)' }}>
          <h2 style={{ margin: 0, fontSize: 15, flex: 1 }}>{zh ? '测试用例' : 'Test Cases'}</h2>
          {Object.keys(results).length > 0 && (
            <span style={{ fontSize: 12, color: 'var(--muted)' }}>
              {passCount > 0 && <span style={{ color: 'var(--success-text)' }}>{passCount} {zh ? '通过' : 'passed'}</span>}
              {failCount > 0 && passCount > 0 && ' · '}
              {failCount > 0 && <span style={{ color: 'var(--danger-text)' }}>{failCount} {zh ? '失败' : 'failed'}</span>}
            </span>
          )}
          {cases.length > 0 && (
            <button
              className="btn btn-sm btn-primary"
              onClick={handleRunAll}
              title={zh ? '运行全部测试用例' : 'Run all test cases'}
            >
              {zh ? '全部运行' : 'Run All'}
            </button>
          )}
          <button
            className="btn btn-sm"
            onClick={() => { setAdding(true); setEditing(null) }}
            title={zh ? '添加测试用例' : 'Add test case'}
          >
            {zh ? '+ 添加' : '+ Add'}
          </button>
          <button className="btn btn-sm" onClick={onClose}>✕</button>
        </div>

        {/* Body */}
        <div style={{ flex: 1, overflowY: 'auto', padding: '12px 18px', display: 'flex', flexDirection: 'column', gap: 10 }}>
          {adding && (
            <TestCaseForm
              tenantId={tenantId}
              workflowId={workflowId}
              onSave={async (tc) => {
                await refresh()
                setAdding(false)
                void tc
              }}
              onCancel={() => setAdding(false)}
            />
          )}

          {cases.length === 0 && !adding && (
            <div style={{ color: 'var(--muted)', fontSize: 13, textAlign: 'center', padding: '20px 0' }}>
              {zh ? '暂无测试用例，点击「+ 添加」创建。' : 'No test cases yet. Click "+ Add" to create one.'}
            </div>
          )}

          {cases.map((tc) => {
            const result = results[tc.id]
            const isRunning = running[tc.id]
            return editing === tc.id ? (
              <TestCaseForm
                key={tc.id}
                tenantId={tenantId}
                workflowId={workflowId}
                existing={tc}
                onSave={async () => { await refresh(); setEditing(null) }}
                onCancel={() => setEditing(null)}
              />
            ) : (
              <div
                key={tc.id}
                style={{
                  padding: '10px 12px',
                  background: 'var(--bg)',
                  border: `1px solid ${result ? (result.passed ? 'var(--success-text)' : 'var(--danger-text)') : 'var(--border)'}`,
                  borderRadius: 6,
                }}
              >
                <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                  <span style={{ flex: 1, fontSize: 13, fontWeight: 500 }}>{tc.name}</span>
                  {result && (
                    <span style={{ fontSize: 11, padding: '1px 6px', borderRadius: 3, background: result.passed ? 'rgba(16,185,129,0.15)' : 'rgba(220,38,38,0.15)', color: result.passed ? 'var(--success-text)' : 'var(--danger-text)', fontWeight: 600 }}>
                      {result.passed ? 'PASS' : 'FAIL'}
                    </span>
                  )}
                  <button
                    className="btn btn-sm btn-success"
                    disabled={isRunning}
                    onClick={() => handleRunOne(tc.id)}
                    style={{ fontSize: 11, padding: '2px 8px' }}
                  >
                    {isRunning ? '…' : zh ? '▶ 运行' : '▶ Run'}
                  </button>
                  <button
                    className="btn btn-sm"
                    onClick={() => { setEditing(tc.id); setAdding(false) }}
                    style={{ fontSize: 11, padding: '2px 8px' }}
                  >
                    {zh ? '编辑' : 'Edit'}
                  </button>
                  <button
                    className="btn btn-sm btn-danger"
                    onClick={() => handleDelete(tc.id)}
                    style={{ fontSize: 11, padding: '2px 6px' }}
                  >
                    ✕
                  </button>
                </div>

                <div style={{ marginTop: 6, fontSize: 11, fontFamily: 'monospace', color: 'var(--muted)', display: 'flex', gap: 16 }}>
                  <span>{zh ? '输入：' : 'Input: '}{tc.input_json.slice(0, 60)}{tc.input_json.length > 60 ? '…' : ''}</span>
                  {tc.expected_output && (
                    <span>{zh ? '期望：' : 'Expected: '}{tc.expected_output.slice(0, 40)}{tc.expected_output.length > 40 ? '…' : ''}</span>
                  )}
                </div>

                {result && !result.passed && result.output_json && (
                  <div style={{ marginTop: 6, fontSize: 11, color: 'var(--danger-text)', fontFamily: 'monospace' }}>
                    Got: {result.output_json.slice(0, 100)}
                  </div>
                )}

                {result && (
                  <div style={{ marginTop: 4, fontSize: 10, color: 'var(--muted)' }}>
                    {zh ? '执行ID：' : 'Run ID: '}{result.execution_id}
                  </div>
                )}
              </div>
            )
          })}
        </div>
      </div>
    </div>
  )
}

function TestCaseForm({
  tenantId,
  workflowId,
  existing,
  onSave,
  onCancel,
}: {
  tenantId: string
  workflowId: string
  existing?: TestCase
  onSave: (tc: TestCase) => void
  onCancel: () => void
}) {
  const { locale } = useLocale()
  const zh = locale === 'zh'
  const [name, setName] = useState(existing?.name ?? '')
  const [inputJson, setInputJson] = useState(existing?.input_json ?? '{}')
  const [expected, setExpected] = useState(existing?.expected_output ?? '')
  const [saving, setSaving] = useState(false)

  const handleSave = async () => {
    if (!name.trim()) return
    setSaving(true)
    try {
      let tc: TestCase
      if (existing) {
        tc = await api.updateTestCase(existing.id, {
          name: name.trim(),
          input_json: inputJson,
          expected_output: expected.trim() || undefined,
        })
      } else {
        tc = await api.createTestCase(
          tenantId, workflowId, name.trim(), inputJson, expected.trim() || undefined,
        )
      }
      onSave(tc)
    } catch {
      // ignore
    } finally {
      setSaving(false)
    }
  }

  return (
    <div style={{ padding: '12px', background: 'var(--surface)', border: '1px solid var(--link)', borderRadius: 6, display: 'flex', flexDirection: 'column', gap: 8 }}>
      <input
        value={name}
        onChange={(e) => setName(e.target.value)}
        placeholder={zh ? '测试用例名称 *' : 'Test case name *'}
        style={{ fontSize: 13, padding: '5px 8px' }}
        autoFocus
      />
      <label style={{ fontSize: 12, display: 'flex', flexDirection: 'column', gap: 3 }}>
        <span style={{ color: 'var(--muted)' }}>{zh ? '输入 JSON' : 'Input JSON'}</span>
        <textarea
          value={inputJson}
          onChange={(e) => setInputJson(e.target.value)}
          rows={3}
          style={{ fontFamily: 'monospace', fontSize: 12, padding: '5px 8px', resize: 'vertical' }}
        />
      </label>
      <label style={{ fontSize: 12, display: 'flex', flexDirection: 'column', gap: 3 }}>
        <span style={{ color: 'var(--muted)' }}>{zh ? '期望输出 JSON（可选，留空则仅验证运行成功）' : 'Expected output JSON (optional — leave blank to just check it runs)'}</span>
        <textarea
          value={expected}
          onChange={(e) => setExpected(e.target.value)}
          rows={2}
          placeholder='{"result": "..."}'
          style={{ fontFamily: 'monospace', fontSize: 12, padding: '5px 8px', resize: 'vertical' }}
        />
      </label>
      <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end' }}>
        <button className="btn btn-sm" onClick={onCancel}>{zh ? '取消' : 'Cancel'}</button>
        <button
          className="btn btn-sm btn-primary"
          disabled={saving || !name.trim()}
          onClick={handleSave}
        >
          {saving ? (zh ? '保存中…' : 'Saving…') : existing ? (zh ? '更新' : 'Update') : (zh ? '创建' : 'Create')}
        </button>
      </div>
    </div>
  )
}
