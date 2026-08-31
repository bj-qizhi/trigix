import { useEffect, useMemo, useRef, useState } from 'react'
import * as api from '../../api/client'
import type {
  DesktopCapability,
  DesktopCommandRecord,
  DesktopDevice,
  DesktopElementSelector,
  DesktopInspectionResult,
  DesktopVisualSelectorSuggestion,
  DesktopWindowSelector,
  ExecutionRecord,
  ExecutionSummary,
} from '../../types'
import type { ConfigProps } from './types'
import { useAuth } from '../../AuthContext'
import { useLocale } from '../../useLocale'

export type DesktopActionKind =
  | 'read_system_information'
  | 'focus_window'
  | 'click_element'
  | 'type_text'
  | 'press_key'
  | 'pointer_click'
  | 'launch_application'

export const DESKTOP_ACTION_SCHEMA: Record<DesktopActionKind, {
  capability: DesktopCapability
  risk: 'low' | 'medium' | 'high'
  approval: boolean
  selector: 'none' | 'window' | 'element'
}> = {
  read_system_information: { capability: 'system_information', risk: 'low', approval: false, selector: 'none' },
  focus_window: { capability: 'window_management', risk: 'medium', approval: true, selector: 'window' },
  click_element: { capability: 'ui_automation', risk: 'medium', approval: true, selector: 'element' },
  type_text: { capability: 'ui_automation', risk: 'high', approval: true, selector: 'element' },
  press_key: { capability: 'keyboard_input', risk: 'high', approval: true, selector: 'window' },
  pointer_click: { capability: 'pointer_input', risk: 'high', approval: true, selector: 'element' },
  launch_application: { capability: 'window_management', risk: 'high', approval: true, selector: 'none' },
}

const TERMINAL_COMMAND_STATES = new Set(['succeeded', 'failed', 'rejected', 'cancelled', 'timed_out'])
export const INSPECTION_BOUNDS = {
  max_depth: 8,
  max_windows: 16,
  max_elements: 256,
  max_duration_ms: 5_000,
  max_payload_bytes: 49_152,
} as const
export const VISUAL_SUGGESTION_POLICY = {
  minimum_confidence_basis_points: 9_000,
  maximum_age_ms: 30_000,
  candidate_count: 1,
} as const

export function buildVisualConfirmationAction(
  value: unknown,
  nowUnixMs: number,
): Record<string, unknown> | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null
  const suggestion = value as Partial<DesktopVisualSelectorSuggestion> & Record<string, unknown>
  const selector = suggestion.selector
  if (!selector || typeof selector !== 'object' || Array.isArray(selector)) return null
  const windowSelector = selector.window
  if (!windowSelector || typeof windowSelector !== 'object' || Array.isArray(windowSelector)) return null
  const forbidden = /^(x|y|left|top|right|bottom|width|height|bounds|coordinates)$/i
  if ([suggestion, selector as unknown as Record<string, unknown>, windowSelector as Record<string, unknown>]
    .some((item) => Object.keys(item).some((key) => forbidden.test(key)))) return null
  if (typeof suggestion.snapshot_id !== 'string'
    || !suggestion.snapshot_id
    || windowSelector.snapshot_id !== suggestion.snapshot_id
    || !Number.isInteger(suggestion.confidence_basis_points)
    || Number(suggestion.confidence_basis_points) < VISUAL_SUGGESTION_POLICY.minimum_confidence_basis_points
    || Number(suggestion.confidence_basis_points) > 10_000
    || suggestion.candidate_count !== VISUAL_SUGGESTION_POLICY.candidate_count
    || !Number.isInteger(suggestion.observed_at_unix_ms)
    || Number(suggestion.observed_at_unix_ms) > nowUnixMs
    || nowUnixMs - Number(suggestion.observed_at_unix_ms) > VISUAL_SUGGESTION_POLICY.maximum_age_ms
  ) return null
  const semanticSelector: DesktopElementSelector = {
    window: compact({
      executable: windowSelector.executable,
      title: windowSelector.title,
      automation_id: windowSelector.automation_id,
      snapshot_id: windowSelector.snapshot_id,
    }),
    ...compact({
      automation_id: selector.automation_id,
      name: selector.name,
      control_type: selector.control_type,
    }),
  }
  if (!semanticSelector.window.executable && !semanticSelector.window.title && !semanticSelector.window.automation_id) return null
  if (!semanticSelector.automation_id && !semanticSelector.name && !semanticSelector.control_type) return null
  return {
    kind: 'inspect_targets',
    request: {
      ...INSPECTION_BOUNDS,
      expected_snapshot_id: suggestion.snapshot_id,
      visual_suggestion: {
        selector: semanticSelector,
        snapshot_id: suggestion.snapshot_id,
        confidence_basis_points: suggestion.confidence_basis_points,
        candidate_count: suggestion.candidate_count,
        observed_at_unix_ms: suggestion.observed_at_unix_ms,
      },
    },
  }
}

export function desktopErrorMessage(error: unknown, zh: boolean): string {
  const raw = error instanceof Error ? error.message : String(error)
  if (/401|Authentication required/i.test(raw)) return zh ? '登录已失效，请重新登录。' : 'Your session expired. Sign in again.'
  if (/403|forbidden|role required|cannot perform/i.test(raw)) return zh ? '当前角色无权使用此设备或操作。' : 'Your role cannot use this Device or action.'
  if (/not active|409.*Execution/i.test(raw)) return zh ? '工作流执行已结束，请启动或选择一个活动执行。' : 'The Workflow Execution ended. Start or select an active execution.'
  if (/not eligible|offline|stale/i.test(raw)) return zh ? '设备已离线或心跳过期，请恢复连接后重试。' : 'The Device is offline or stale. Reconnect it and retry.'
  if (/capability|incompatible/i.test(raw)) return zh ? '设备版本或能力与此操作不兼容。' : 'The Device version or capabilities are incompatible with this action.'
  if (/ambiguous/i.test(raw)) return zh ? '目标不唯一，请重新检查并选择更具体的控件。' : 'The target is ambiguous. Inspect again and choose a more specific control.'
  if (/not found/i.test(raw)) return zh ? '未找到目标，请确认窗口已打开并重新检查。' : 'Target not found. Open the window and inspect again.'
  if (/stale/i.test(raw)) return zh ? '选择器快照已过期，请重新检查目标。' : 'The selector snapshot is stale. Inspect the target again.'
  return zh ? '桌面命令失败。请检查活动执行、设备状态和所需能力。' : 'Desktop command failed. Check the active execution, Device state, and required capability.'
}

function compact<T extends Record<string, unknown>>(value: T): T {
  return Object.fromEntries(Object.entries(value).filter(([, item]) => item !== '' && item !== undefined)) as T
}

export function buildDesktopAction(kind: DesktopActionKind, config: Record<string, unknown>): Record<string, unknown> | null {
  const selector = config.selector as DesktopElementSelector | DesktopWindowSelector | undefined
  const elementSelector = selector && 'window' in selector ? selector : undefined
  const windowSelector = selector && !('window' in selector) ? selector : undefined
  switch (kind) {
    case 'read_system_information': return { kind }
    case 'focus_window': return windowSelector ? { kind, selector: windowSelector } : null
    case 'click_element': return elementSelector ? { kind, selector: elementSelector } : null
    case 'type_text': return elementSelector && typeof config.text === 'string' ? { kind, selector: elementSelector, text: config.text.slice(0, 16_384) } : null
    case 'press_key': {
      const key = typeof config.key === 'string' ? config.key : 'enter'
      const modifiers = Array.isArray(config.modifiers)
        ? config.modifiers.filter((value): value is string => ['control', 'alt', 'shift', 'meta'].includes(String(value)))
        : []
      return windowSelector && key ? { kind, selector: windowSelector, key, modifiers: [...new Set(modifiers)] } : null
    }
    case 'pointer_click': {
      const button = ['left', 'right', 'middle'].includes(String(config.pointer_button)) ? config.pointer_button : 'left'
      const clickCount = Number(config.click_count) === 2 ? 2 : 1
      return elementSelector ? { kind, selector: elementSelector, button, click_count: clickCount } : null
    }
    case 'launch_application': {
      const applicationId = typeof config.application_id === 'string' ? config.application_id.trim() : ''
      return applicationId ? { kind, application_id: applicationId } : null
    }
  }
}

export function desktopTargetLabel(kind: DesktopActionKind, config: Record<string, unknown>): string {
  if (kind === 'read_system_information') return 'system_information'
  if (kind === 'launch_application') return String(config.application_id || 'application')
  const selector = config.selector as DesktopElementSelector | DesktopWindowSelector | undefined
  if (!selector) return 'unselected'
  if ('window' in selector) {
    return selector.automation_id || [selector.control_type, selector.name].filter(Boolean).join(':') || 'selected_element'
  }
  return selector.automation_id || selector.executable || 'selected_window'
}

async function waitForCommand(
  tenantId: string,
  initial: DesktopCommandRecord,
  onUpdate?: (record: DesktopCommandRecord) => void,
): Promise<DesktopCommandRecord> {
  let record = initial
  onUpdate?.(record)
  for (let attempt = 0; attempt < 80 && !TERMINAL_COMMAND_STATES.has(record.status); attempt += 1) {
    await new Promise((resolve) => window.setTimeout(resolve, 750))
    record = await api.getDesktopCommand(tenantId, record.command.command_id)
    onUpdate?.(record)
  }
  if (!TERMINAL_COMMAND_STATES.has(record.status)) throw new Error('desktop command timed out')
  return record
}

interface Props extends ConfigProps {
  workflowProjectId?: string
  activeExecution?: ExecutionRecord | null
  recentExecutions?: ExecutionSummary[]
}

export function DesktopActionConfig({ config, set, str, workflowProjectId, activeExecution, recentExecutions = [] }: Props) {
  const { auth } = useAuth()
  const { locale } = useLocale()
  const zh = locale === 'zh'
  const mounted = useRef(true)
  const [devices, setDevices] = useState<DesktopDevice[]>([])
  const [loadingDevices, setLoadingDevices] = useState(true)
  const [busy, setBusy] = useState<'inspect' | 'visual' | 'test' | null>(null)
  const [error, setError] = useState('')
  const [notice, setNotice] = useState('')
  const [inspection, setInspection] = useState<DesktopInspectionResult | null>(null)
  const [testCommand, setTestCommand] = useState<DesktopCommandRecord | null>(null)
  const [stopping, setStopping] = useState(false)
  const kind = (str('action_kind', 'click_element') as DesktopActionKind)
  const schema = DESKTOP_ACTION_SCHEMA[kind] ?? DESKTOP_ACTION_SCHEMA.click_element
  const targetLabel = desktopTargetLabel(kind, config)
  const executionId = activeExecution && ['running', 'waiting_approval'].includes(activeExecution.status)
    ? activeExecution.id
    : recentExecutions.find((item) => ['running', 'waiting_approval'].includes(item.status))?.id

  useEffect(() => {
    if (kind === 'press_key' && typeof config.key !== 'string') {
      set('key', 'enter')
    } else if (kind === 'pointer_click' && !['left', 'right', 'middle'].includes(String(config.pointer_button))) {
      set('pointer_button', 'left')
    } else if (kind === 'pointer_click' && ![1, 2].includes(Number(config.click_count))) {
      set('click_count', 1)
    }
  }, [kind, config.key, config.pointer_button, config.click_count])

  useEffect(() => {
    mounted.current = true
    setLoadingDevices(true)
    api.listDesktopDevices()
      .then((result) => { if (mounted.current) setDevices(result.items) })
      .catch((cause) => { if (mounted.current) setError(desktopErrorMessage(cause, zh)) })
      .finally(() => { if (mounted.current) setLoadingDevices(false) })
    return () => { mounted.current = false }
  }, [zh])

  const eligibleDevices = useMemo(() => devices.filter((device) =>
    !device.stale
    && device.compatible
    && ['online', 'busy', 'awaiting_approval'].includes(device.state)
    && device.capabilities.includes(schema.capability)
    && (schema.selector === 'none' || device.capabilities.includes('ui_automation'))
  ), [devices, schema.capability, schema.selector])
  const selectedDevice = eligibleDevices.find((device) => device.id === str('device_id'))
  const inspectorEligible = selectedDevice?.capabilities.includes('ui_automation') ?? false
  const selectorCounts = useMemo(() => {
    const windows = new Map<string, number>()
    const elements = new Map<string, number>()
    for (const inspectedWindow of inspection?.windows ?? []) {
      const windowKey = JSON.stringify(inspectedWindow.selector)
      windows.set(windowKey, (windows.get(windowKey) ?? 0) + 1)
      for (const element of inspectedWindow.elements) {
        const elementKey = JSON.stringify(element.selector)
        elements.set(elementKey, (elements.get(elementKey) ?? 0) + 1)
      }
    }
    return { windows, elements }
  }, [inspection])

  const dispatch = async (action: Record<string, unknown>, onUpdate?: (record: DesktopCommandRecord) => void) => {
    if (!auth || !executionId || !selectedDevice) throw new Error('Workflow Execution is not active')
    const initial = await api.dispatchDesktopCommand({
      tenant_id: auth.tenantId,
      project_id: workflowProjectId || auth.projectId,
      execution_id: executionId,
      device_id: selectedDevice.id,
      action,
      lease_seconds: 60,
    })
    return waitForCommand(auth.tenantId, initial, onUpdate)
  }

  const inspect = async () => {
    setError('')
    setNotice('')
    setBusy('inspect')
    try {
      const record = await dispatch({
        kind: 'inspect_targets',
        request: {
          ...INSPECTION_BOUNDS,
        },
      })
      if (record.status !== 'succeeded' || !record.result?.output) {
        throw new Error(record.result?.error_code || record.result?.error_message || record.status)
      }
      if (mounted.current) {
        setInspection(record.result.output as DesktopInspectionResult)
        setNotice(zh ? '检查完成，请选择唯一目标。' : 'Inspection complete. Select a unique target.')
      }
    } catch (cause) {
      if (mounted.current) setError(desktopErrorMessage(cause, zh))
    } finally {
      if (mounted.current) setBusy(null)
    }
  }

  const testAction = async () => {
    setError('')
    setNotice('')
    const action = buildDesktopAction(kind, config)
    if (!action) {
      setError(zh ? '请先完成此操作的必填字段。' : 'Complete the required fields for this action first.')
      return
    }
    setBusy('test')
    setTestCommand(null)
    try {
      const record = await dispatch(action, (next) => { if (mounted.current) setTestCommand(next) })
      if (record.status !== 'succeeded') throw new Error(record.result?.error_code || record.result?.error_message || record.status)
      if (mounted.current) setNotice(zh ? '测试操作已成功完成。' : 'Test action completed successfully.')
    } catch (cause) {
      if (mounted.current) setError(desktopErrorMessage(cause, zh))
    } finally {
      if (mounted.current) setBusy(null)
    }
  }

  const stopTest = async () => {
    if (!auth || !testCommand || TERMINAL_COMMAND_STATES.has(testCommand.status)) return
    setStopping(true)
    setError('')
    try {
      const cancelled = await api.cancelDesktopCommand(auth.tenantId, testCommand.command.command_id)
      if (mounted.current) {
        setTestCommand(cancelled)
        setNotice(zh ? '停止请求已接受。' : 'Stop request accepted.')
      }
    } catch (cause) {
      try {
        const current = await api.getDesktopCommand(auth.tenantId, testCommand.command.command_id)
        if (TERMINAL_COMMAND_STATES.has(current.status)) {
          if (mounted.current) setTestCommand(current)
          return
        }
      } catch { /* preserve the sanitized cancellation error */ }
      if (mounted.current) setError(desktopErrorMessage(cause, zh))
    } finally {
      if (mounted.current) setStopping(false)
    }
  }

  const toggleModifier = (modifier: string, enabled: boolean) => {
    const current = Array.isArray(config.modifiers) ? config.modifiers.map(String) : []
    set('modifiers', enabled ? [...new Set([...current, modifier])] : current.filter((value) => value !== modifier))
  }

  const saveWindow = (windowSelector: DesktopWindowSelector, snapshotId: string) => {
    set('selector', compact({ ...windowSelector, snapshot_id: snapshotId }))
  }
  const saveElement = (selector: DesktopElementSelector, snapshotId: string) => {
    set('selector', { ...selector, window: compact({ ...selector.window, snapshot_id: snapshotId }) })
  }

  const confirmVisualSuggestion = async () => {
    setError('')
    setNotice('')
    const action = buildVisualConfirmationAction(config.visual_suggestion, Date.now())
    if (!action) {
      setError(zh ? '视觉建议已过期、不唯一、置信度不足或包含坐标。' : 'The visual suggestion is stale, ambiguous, below confidence, or contains coordinates.')
      return
    }
    setBusy('visual')
    try {
      const record = await dispatch(action)
      const output = record.result?.output as DesktopInspectionResult | undefined
      const target = output?.windows[0]?.elements[0]
      if (record.status !== 'succeeded'
        || !output?.selector_resolution
        || output.windows.length !== 1
        || output.windows[0].elements.length !== 1
        || !target
      ) throw new Error(record.result?.error_code || record.result?.error_message || 'visual suggestion was not uniquely resolved')
      if (mounted.current) {
        setInspection(output)
        saveElement(target.selector, output.snapshot_id)
        setNotice(zh
          ? `视觉建议已确认并保存为语义选择器（${output.selector_resolution.strategy}）。`
          : `Visual suggestion confirmed and saved as a semantic selector (${output.selector_resolution.strategy}).`)
      }
    } catch (cause) {
      if (mounted.current) setError(desktopErrorMessage(cause, zh))
    } finally {
      if (mounted.current) setBusy(null)
    }
  }

  return <>
    <div className="field">
      <label>{zh ? '操作类型' : 'Action type'}</label>
      <select value={kind} onChange={(event) => { set('action_kind', event.target.value); setInspection(null); setTestCommand(null) }}>
        <option value="read_system_information">{zh ? '读取系统信息' : 'Read system information'}</option>
        <option value="focus_window">{zh ? '聚焦窗口' : 'Focus window'}</option>
        <option value="click_element">{zh ? '点击控件' : 'Click element'}</option>
        <option value="type_text">{zh ? '输入文本' : 'Type text'}</option>
        <option value="press_key">{zh ? '按键或快捷键' : 'Press key or shortcut'}</option>
        <option value="pointer_click">{zh ? '指针点击控件' : 'Pointer click element'}</option>
        <option value="launch_application">{zh ? '启动应用' : 'Launch application'}</option>
      </select>
    </div>
    <div style={{ border: '1px solid var(--border)', borderRadius: 4, padding: '7px 8px', marginBottom: 8, fontSize: 11 }}>
      <strong>{zh ? '风险' : 'Risk'}: {schema.risk.toUpperCase()}</strong>
      <span style={{ color: 'var(--muted)', marginLeft: 8 }}>
        {schema.approval ? (zh ? '测试前需要管理员授权，命令审批会写入审计记录。' : 'Admin authorization is required before test execution; command approval is audited.') : (zh ? '低风险，只读操作。' : 'Low-risk read-only action.')}
      </span>
    </div>
    <div className="field">
      <label>{zh ? '设备' : 'Device'}</label>
      <select value={str('device_id')} disabled={loadingDevices} onChange={(event) => { set('device_id', event.target.value); setInspection(null); setTestCommand(null) }}>
        <option value="">{loadingDevices ? (zh ? '加载设备…' : 'Loading Devices…') : (zh ? '选择符合条件的设备' : 'Select an eligible Device')}</option>
        {eligibleDevices.map((device) => <option key={device.id} value={device.id}>{device.display_name} · {device.operating_system} · {device.agent_version}</option>)}
      </select>
      {!loadingDevices && eligibleDevices.length === 0 && <small style={{ color: 'var(--muted)' }}>{zh ? '没有在线、版本兼容且具备所需能力的设备。' : 'No online, current, compatible Device advertises the required capability.'}</small>}
    </div>
    {kind === 'launch_application' && <div className="field">
      <label>{zh ? '应用标识' : 'Application ID'}</label>
      <input value={str('application_id')} maxLength={128} pattern="[A-Za-z0-9._-]+" placeholder="com.example.application" onChange={(event) => set('application_id', event.target.value.replace(/[^A-Za-z0-9._-]/g, '').slice(0, 128))} />
    </div>}
    {kind === 'type_text' && <div className="field">
      <label>{zh ? '文本' : 'Text'}</label>
      <textarea value={str('text')} maxLength={16_384} rows={3} onChange={(event) => set('text', event.target.value)} />
      <small style={{ color: 'var(--muted)' }}>{str('text').length}/16384</small>
    </div>}
    {kind === 'press_key' && <>
      <div className="field">
        <label>{zh ? '按键' : 'Key'}</label>
        <select value={str('key', 'enter')} onChange={(event) => set('key', event.target.value)}>
          {['enter', 'escape', 'tab', 'backspace', 'delete', 'space', 'home', 'end', 'page_up', 'page_down', 'arrow_up', 'arrow_down', 'arrow_left', 'arrow_right', 'f1', 'f2', 'f3', 'f4', 'f5', 'f6', 'f7', 'f8', 'f9', 'f10', 'f11', 'f12', ...'abcdefghijklmnopqrstuvwxyz0123456789'].map((key) => <option key={key} value={key}>{key}</option>)}
        </select>
      </div>
      <div className="field">
        <label>{zh ? '修饰键' : 'Modifiers'}</label>
        <div style={{ display: 'flex', gap: 10, flexWrap: 'wrap' }}>
          {['control', 'alt', 'shift', 'meta'].map((modifier) => <label key={modifier} style={{ display: 'inline-flex', gap: 4, alignItems: 'center' }}>
            <input type="checkbox" checked={Array.isArray(config.modifiers) && config.modifiers.includes(modifier)} onChange={(event) => toggleModifier(modifier, event.target.checked)} /> {modifier}
          </label>)}
        </div>
      </div>
    </>}
    {kind === 'pointer_click' && <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 8 }}>
      <div className="field">
        <label>{zh ? '指针按钮' : 'Pointer button'}</label>
        <select value={str('pointer_button', 'left')} onChange={(event) => set('pointer_button', event.target.value)}>
          <option value="left">{zh ? '左键' : 'Left'}</option><option value="right">{zh ? '右键' : 'Right'}</option><option value="middle">{zh ? '中键' : 'Middle'}</option>
        </select>
      </div>
      <div className="field">
        <label>{zh ? '点击次数' : 'Click count'}</label>
        <select value={String(config.click_count ?? 1)} onChange={(event) => set('click_count', Number(event.target.value))}>
          <option value="1">1</option><option value="2">2</option>
        </select>
      </div>
    </div>}
    {schema.selector !== 'none' && <div style={{ marginBottom: 8 }}>
      <button className="btn btn-sm" disabled={busy !== null || !executionId || !selectedDevice || !inspectorEligible} onClick={() => void inspect()}>
        {busy === 'inspect' ? (zh ? '正在检查…' : 'Inspecting…') : (zh ? '检查桌面目标' : 'Inspect desktop targets')}
      </button>
      {!executionId && <div style={{ color: 'var(--warning, #b45309)', fontSize: 11, marginTop: 4 }}>{zh ? '先启动或选择一个活动工作流执行。' : 'Start or select an active Workflow Execution first.'}</div>}
      {selectedDevice && !inspectorEligible && <div style={{ color: 'var(--warning, #b45309)', fontSize: 11, marginTop: 4 }}>{zh ? '所选设备不支持 UI Automation 检查。' : 'The selected Device cannot inspect UI Automation targets.'}</div>}
      {schema.selector === 'element' && Boolean(config.visual_suggestion) && <div style={{ marginTop: 6 }}>
        <button className="btn btn-sm" disabled={busy !== null || !executionId || !selectedDevice || !inspectorEligible} onClick={() => void confirmVisualSuggestion()}>
          {busy === 'visual' ? (zh ? '正在确认视觉建议…' : 'Confirming visual suggestion…') : (zh ? '确认视觉建议为语义目标' : 'Confirm visual suggestion as semantic target')}
        </button>
        <div style={{ color: 'var(--muted)', fontSize: 11, marginTop: 4 }}>{zh ? '仅确认唯一、最新的语义控件；不会保存或执行屏幕坐标。' : 'Only one fresh semantic control can be confirmed; screen coordinates are never saved or executed.'}</div>
      </div>}
    </div>}
    {inspection && <div style={{ borderTop: '1px solid var(--border)', paddingTop: 8, marginBottom: 8 }}>
      <div style={{ fontSize: 11, marginBottom: 6 }}><strong>{zh ? '稳定目标' : 'Stable targets'}</strong>{inspection.truncated ? ` · ${zh ? '结果已截断' : 'truncated'}` : ''}</div>
      {inspection.windows.map((windowItem, windowIndex) => <div key={`${windowItem.process_id}-${windowIndex}`} style={{ border: '1px solid var(--border)', borderRadius: 4, padding: 6, marginBottom: 6 }}>
        <div style={{ fontSize: 11, overflowWrap: 'anywhere' }}>
          {windowItem.selector.executable || (zh ? '未知应用' : 'Unknown application')} · {windowItem.title_policy === 'redacted' ? (zh ? '标题已隐藏' : 'title redacted') : (windowItem.selector.title || windowItem.selector.automation_id)}
        </div>
        {schema.selector === 'window' && <button className="btn btn-sm" style={{ marginTop: 5 }} disabled={selectorCounts.windows.get(JSON.stringify(windowItem.selector)) !== 1} onClick={() => saveWindow(windowItem.selector, inspection.snapshot_id)}>{selectorCounts.windows.get(JSON.stringify(windowItem.selector)) === 1 ? (zh ? '选择窗口' : 'Select window') : (zh ? '选择器不唯一' : 'Selector is ambiguous')}</button>}
        {schema.selector === 'element' && windowItem.elements.map((element, elementIndex) => <div key={`${element.selector.automation_id || element.selector.name}-${elementIndex}`} style={{ borderTop: '1px solid var(--border)', paddingTop: 5, marginTop: 5, fontSize: 11 }}>
          <div>{element.selector.control_type || 'Control'} · {element.selector.name || element.selector.automation_id || (zh ? '未命名' : 'Unnamed')}</div>
          <div style={{ color: 'var(--muted)' }}>{element.supported_patterns.join(', ')}{element.redaction ? ` · ${zh ? '受保护值' : 'protected value'} (${element.redaction})` : ''}</div>
          <button className="btn btn-sm" style={{ marginTop: 4 }} disabled={selectorCounts.elements.get(JSON.stringify(element.selector)) !== 1} onClick={() => saveElement(element.selector, inspection.snapshot_id)}>{selectorCounts.elements.get(JSON.stringify(element.selector)) === 1 ? (zh ? '选择控件' : 'Select control') : (zh ? '选择器不唯一' : 'Selector is ambiguous')}</button>
        </div>)}
      </div>)}
    </div>}
    {config.selector && <div style={{ fontSize: 11, color: 'var(--muted)', overflowWrap: 'anywhere', marginBottom: 8 }}>
      {zh ? '已保存选择器：' : 'Saved selector: '}{JSON.stringify(config.selector)}
    </div>}
    <button className="btn btn-sm btn-primary" disabled={busy !== null || !executionId || !selectedDevice} onClick={() => void testAction()}>
      {busy === 'test' ? (zh ? '正在测试…' : 'Testing…') : (zh ? '测试操作' : 'Test action')}
    </button>
    {testCommand && <div aria-live="polite" style={{ border: '1px solid var(--border)', borderRadius: 4, padding: 8, marginTop: 8, fontSize: 11 }}>
      <div><strong>{zh ? '测试目标' : 'Test target'}:</strong> {selectedDevice?.display_name || testCommand.device_id} · {kind} · {targetLabel}</div>
      <div style={{ color: 'var(--muted)', marginTop: 3 }}><strong>{zh ? '命令状态' : 'Command state'}:</strong> {testCommand.status}</div>
      {!TERMINAL_COMMAND_STATES.has(testCommand.status) && <button className="btn btn-sm" style={{ marginTop: 6, color: 'var(--danger-text, #dc2626)' }} disabled={stopping} onClick={() => void stopTest()}>
        {stopping ? (zh ? '正在停止…' : 'Stopping…') : (zh ? '立即停止测试' : 'Stop test now')}
      </button>}
    </div>}
    {notice && <div role="status" style={{ color: 'var(--success-text, #15803d)', fontSize: 11, marginTop: 6 }}>{notice}</div>}
    {error && <div role="alert" style={{ color: 'var(--danger-text, #dc2626)', fontSize: 11, marginTop: 6 }}>{error}</div>}
  </>
}
