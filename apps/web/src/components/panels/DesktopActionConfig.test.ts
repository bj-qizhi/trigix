import { describe, expect, it } from 'vitest'
import { DESKTOP_ACTION_SCHEMA, INSPECTION_BOUNDS, desktopErrorMessage } from './DesktopActionConfig'

describe('Desktop action authoring contract', () => {
  it('keeps capabilities, risk, approval, and selector shape explicit', () => {
    expect(DESKTOP_ACTION_SCHEMA.read_system_information).toEqual({
      capability: 'system_information', risk: 'low', approval: false, selector: 'none',
    })
    expect(DESKTOP_ACTION_SCHEMA.click_element).toEqual({
      capability: 'ui_automation', risk: 'medium', approval: true, selector: 'element',
    })
    expect(DESKTOP_ACTION_SCHEMA.type_text.risk).toBe('high')
    expect(DESKTOP_ACTION_SCHEMA.launch_application.capability).toBe('window_management')
    expect(INSPECTION_BOUNDS).toEqual({
      max_depth: 8, max_windows: 16, max_elements: 256, max_duration_ms: 5_000, max_payload_bytes: 49_152,
    })
  })

  it('turns transport details into actionable, sanitized author messages', () => {
    const source = new Error('409 Conflict: internal stack Workflow Execution is not active secret=abc')
    expect(desktopErrorMessage(source, false)).toBe('The Workflow Execution ended. Start or select an active execution.')
    expect(desktopErrorMessage(new Error('target_ambiguous: private window title'), false)).not.toContain('private window title')
    expect(desktopErrorMessage(new Error('403 forbidden: jwt claims'), true)).toBe('当前角色无权使用此设备或操作。')
  })
})
