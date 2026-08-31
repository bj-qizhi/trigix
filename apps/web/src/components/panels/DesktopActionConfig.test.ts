import { describe, expect, it } from 'vitest'
import { buildDesktopAction, buildVisualConfirmationAction, desktopTargetLabel, DESKTOP_ACTION_SCHEMA, INSPECTION_BOUNDS, desktopErrorMessage, VISUAL_SUGGESTION_POLICY } from './DesktopActionConfig'

describe('Desktop action authoring contract', () => {
  it('keeps capabilities, risk, approval, and selector shape explicit', () => {
    expect(DESKTOP_ACTION_SCHEMA.read_system_information).toEqual({
      capability: 'system_information', risk: 'low', approval: false, selector: 'none',
    })
    expect(DESKTOP_ACTION_SCHEMA.click_element).toEqual({
      capability: 'ui_automation', risk: 'medium', approval: true, selector: 'element',
    })
    expect(DESKTOP_ACTION_SCHEMA.type_text.risk).toBe('high')
    expect(DESKTOP_ACTION_SCHEMA.press_key.capability).toBe('keyboard_input')
    expect(DESKTOP_ACTION_SCHEMA.pointer_click).toEqual({
      capability: 'pointer_input', risk: 'high', approval: true, selector: 'element',
    })
    expect(DESKTOP_ACTION_SCHEMA.launch_application.capability).toBe('window_management')
    expect(INSPECTION_BOUNDS).toEqual({
      max_depth: 8, max_windows: 16, max_elements: 256, max_duration_ms: 5_000, max_payload_bytes: 49_152,
    })
  })

  it('builds bounded input actions without browser-supplied coordinates', () => {
    const selector = { window: { automation_id: 'Fixture.Main' }, automation_id: '1002', control_type: 'button' }
    expect(buildDesktopAction('press_key', {
      selector: selector.window, key: 'a', modifiers: ['control', 'control', 'invalid'],
    })).toEqual({ kind: 'press_key', selector: selector.window, key: 'a', modifiers: ['control'] })
    const pointer = buildDesktopAction('pointer_click', {
      selector, pointer_button: 'right', click_count: 2, x: 123, y: 456,
    })
    expect(pointer).toEqual({ kind: 'pointer_click', selector, button: 'right', click_count: 2 })
    expect(JSON.stringify(pointer)).not.toMatch(/\"[xy]\"/)
    expect(desktopTargetLabel('pointer_click', { selector })).toBe('1002')
    expect(buildDesktopAction('press_key', { selector, key: 'enter' })).toBeNull()
    expect(buildDesktopAction('pointer_click', { selector: selector.window })).toBeNull()
  })

  it('turns transport details into actionable, sanitized author messages', () => {
    const source = new Error('409 Conflict: internal stack Workflow Execution is not active secret=abc')
    expect(desktopErrorMessage(source, false)).toBe('The Workflow Execution ended. Start or select an active execution.')
    expect(desktopErrorMessage(new Error('target_ambiguous: private window title'), false)).not.toContain('private window title')
    expect(desktopErrorMessage(new Error('403 forbidden: jwt claims'), true)).toBe('当前角色无权使用此设备或操作。')
  })

  it('confirms only fresh unique high-confidence visual suggestions as semantic selectors', () => {
    const now = 100_000
    const suggestion = {
      selector: {
        window: { executable: 'fixture.exe', automation_id: 'Fixture.Main', snapshot_id: 'snapshot-1' },
        automation_id: 'missing-primary', name: 'Submit', control_type: 'button',
      },
      snapshot_id: 'snapshot-1',
      confidence_basis_points: VISUAL_SUGGESTION_POLICY.minimum_confidence_basis_points,
      candidate_count: 1,
      observed_at_unix_ms: now - 1,
    }
    const action = buildVisualConfirmationAction(suggestion, now)
    expect(action).toEqual({
      kind: 'inspect_targets',
      request: {
        ...INSPECTION_BOUNDS,
        expected_snapshot_id: 'snapshot-1',
        visual_suggestion: suggestion,
      },
    })
    expect(JSON.stringify(action)).not.toMatch(/coordinates|"x"|"y"/)
    expect(buildVisualConfirmationAction({ ...suggestion, candidate_count: 2 }, now)).toBeNull()
    expect(buildVisualConfirmationAction({ ...suggestion, confidence_basis_points: 8_999 }, now)).toBeNull()
    expect(buildVisualConfirmationAction({ ...suggestion, observed_at_unix_ms: now - 30_001 }, now)).toBeNull()
    expect(buildVisualConfirmationAction({ ...suggestion, x: 1, y: 2 }, now)).toBeNull()
    expect(buildVisualConfirmationAction({
      ...suggestion,
      selector: { ...suggestion.selector, coordinates: { x: 1, y: 2 } },
    }, now)).toBeNull()
  })
})
