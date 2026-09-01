// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import type { NodeType } from '../../types'
import type { ConfigProps } from './types'
import { fl } from './i18nLabels'

interface Props extends ConfigProps { nodeType: NodeType }

const SessionField = ({ str, set }: Pick<ConfigProps, 'str' | 'set'>) => (
  <div className="field">
    <label>{fl('Session ID')} <span className="req">*</span></label>
    <input value={str('session_id')} onChange={(event) => set('session_id', event.target.value)} placeholder="{{BrowserStart.browser.session_id}}" />
  </div>
)

const TimeoutField = ({ num, set }: Pick<ConfigProps, 'num' | 'set'>) => (
  <div className="field">
    <label>{fl('Timeout (ms)')}</label>
    <input type="number" min={1} max={60000} value={num('timeout_ms', 10000)} onChange={(event) => set('timeout_ms', Number(event.target.value))} />
  </div>
)

export function BrowserNodeConfig({ nodeType, config, set, str, num }: Props) {
  if (nodeType === 'browser_start') {
    return <div className="config-section"><p className="config-hint">Creates a new tenant-isolated Browser Context. Pass its session ID to every following Browser node and close it explicitly.</p></div>
  }
  return (
    <div className="config-section">
      <SessionField str={str} set={set} />
      {nodeType === 'browser_navigate' && <>
        <div className="field"><label>{fl('URL')} <span className="req">*</span></label><input value={str('url')} onChange={(event) => set('url', event.target.value)} placeholder="https://example.com" /></div>
        <div className="field"><label>{fl('Wait until')}</label><select value={str('wait_until', 'domcontentloaded')} onChange={(event) => set('wait_until', event.target.value)}><option value="commit">Commit</option><option value="domcontentloaded">DOM content loaded</option><option value="load">Load</option><option value="networkidle">Network idle</option></select></div>
      </>}
      {nodeType === 'browser_click' && <div className="field"><label>{fl('Selector')} <span className="req">*</span></label><input value={str('selector')} onChange={(event) => set('selector', event.target.value)} placeholder="button[type=submit]" /></div>}
      {nodeType === 'browser_input' && <>
        <div className="field"><label>{fl('Selector')} <span className="req">*</span></label><input value={str('selector')} onChange={(event) => set('selector', event.target.value)} placeholder="#username" /></div>
        <div className="field"><label>{fl('Value')} <span className="req">*</span></label><input type="password" value={str('value')} onChange={(event) => set('value', event.target.value)} autoComplete="off" /></div>
        <label className="check-row"><input type="checkbox" checked={config.clear_first !== false} onChange={(event) => set('clear_first', event.target.checked)} /> Clear existing value first</label>
      </>}
      {nodeType === 'browser_wait' && <>
        <div className="field"><label>{fl('Wait mode')}</label><select value={str('wait_mode', 'selector')} onChange={(event) => {
          const mode = event.target.value
          set('wait_mode', mode)
          if (mode !== 'selector') { set('selector', undefined); set('state', undefined) }
          if (mode !== 'milliseconds') set('milliseconds', undefined)
          if (mode !== 'url') set('url', undefined)
          if (mode !== 'load_state') set('load_state', undefined)
        }}><option value="selector">Selector</option><option value="milliseconds">Milliseconds</option><option value="url">URL</option><option value="load_state">Load state</option></select></div>
        {str('wait_mode', 'selector') === 'selector' && <><div className="field"><label>{fl('Selector')}</label><input value={str('selector')} onChange={(event) => set('selector', event.target.value)} /></div><div className="field"><label>{fl('State')}</label><select value={str('state', 'visible')} onChange={(event) => set('state', event.target.value)}><option value="visible">Visible</option><option value="hidden">Hidden</option><option value="attached">Attached</option><option value="detached">Detached</option></select></div></>}
        {str('wait_mode', 'selector') === 'milliseconds' && <div className="field"><label>{fl('Milliseconds')}</label><input type="number" min={0} max={60000} value={num('milliseconds', 1000)} onChange={(event) => set('milliseconds', Number(event.target.value))} /></div>}
        {str('wait_mode', 'selector') === 'url' && <div className="field"><label>{fl('URL pattern')}</label><input value={str('url')} onChange={(event) => set('url', event.target.value)} /></div>}
        {str('wait_mode', 'selector') === 'load_state' && <div className="field"><label>{fl('Load state')}</label><select value={str('load_state', 'domcontentloaded')} onChange={(event) => set('load_state', event.target.value)}><option value="domcontentloaded">DOM content loaded</option><option value="load">Load</option><option value="networkidle">Network idle</option></select></div>}
      </>}
      {nodeType === 'browser_extract' && <>
        <div className="field"><label>{fl('Selector')} <span className="req">*</span></label><input value={str('selector')} onChange={(event) => set('selector', event.target.value)} /></div>
        <div className="field"><label>{fl('Mode')}</label><select value={str('mode', 'text')} onChange={(event) => set('mode', event.target.value)}><option value="text">Text</option><option value="html">HTML</option><option value="attribute">Attribute</option><option value="json">JSON</option><option value="list">List</option><option value="table">Table</option></select></div>
        {str('mode', 'text') === 'attribute' && <div className="field"><label>{fl('Attribute')}</label><input value={str('attribute')} onChange={(event) => set('attribute', event.target.value)} /></div>}
      </>}
      {nodeType === 'browser_screenshot' && <label className="check-row"><input type="checkbox" checked={config.full_page !== false} onChange={(event) => set('full_page', event.target.checked)} /> Capture full page</label>}
      {nodeType === 'browser_close' ? <p className="config-hint">Closes every Page and releases the Browser Context. Repeating close is safe.</p> : <TimeoutField num={num} set={set} />}
    </div>
  )
}
