// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useId } from 'react'
import type { ConfigProps } from '../types'
import { fl } from '../i18nLabels'

// Editable model field: type any model name, or pick a current suggestion.
// `options` are [value, label] pairs surfaced via a native datalist.
export function ModelField({ str, set, fallback, options }: Pick<ConfigProps, 'str' | 'set'> & { fallback: string; options: Array<[string, string]> }) {
  const listId = useId()
  return (
    <div className="field">
      <label>{fl("Model")} <span style={{ color: 'var(--muted)' }}>{fl("(输入或选择)")}</span></label>
      <input list={listId} placeholder={fallback} value={str('model', fallback)} onChange={(e) => set('model', e.target.value)} />
      <datalist id={listId}>
        {options.map(([value, label]) => <option key={value} value={value}>{label}</option>)}
      </datalist>
    </div>
  )
}

export function llmEndpointFields(str: ConfigProps['str'], set: ConfigProps['set'], defModel: string) {
  return (
    <>
      <div className="field">
        <label>{fl("API Key")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div style={{ display: 'flex', gap: 8 }}>
        <div className="field" style={{ flex: 2 }}>
          <label>{fl("Base URL")}</label>
          <input placeholder="(OpenAI-compatible default)" value={str('base_url', '')} onChange={(e) => set('base_url', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
        <div className="field" style={{ flex: 1 }}>
          <label>{fl("Model")}</label>
          <input placeholder={defModel} value={str('model', '')} onChange={(e) => set('model', e.target.value)} />
        </div>
      </div>
    </>
  )
}

export function hostAuthFields(str: ConfigProps['str'], set: ConfigProps['set'], defPort: number, userLabel = 'Username') {
  return (
    <>
      <div style={{ display: 'flex', gap: 8 }}>
        <div className="field" style={{ flex: 2 }}>
          <label>{fl("Host")} <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input value={str('host', '')} onChange={(e) => set('host', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
        <div className="field" style={{ flex: 1 }}>
          <label>{fl("Port")}</label>
          <input placeholder={String(defPort)} value={str('port', '')} onChange={(e) => set('port', e.target.value ? parseInt(e.target.value) : undefined)} />
        </div>
      </div>
      <div style={{ display: 'flex', gap: 8 }}>
        <div className="field" style={{ flex: 1 }}>
          <label>{userLabel}</label>
          <input value={str('username', '')} onChange={(e) => set('username', e.target.value)} />
        </div>
        <div className="field" style={{ flex: 1 }}>
          <label>{fl("Password")}</label>
          <input type="password" value={str('password', '')} onChange={(e) => set('password', e.target.value)} />
        </div>
      </div>
    </>
  )
}

export function fileOpFields(operation: string, str: ConfigProps['str'], set: ConfigProps['set']) {
  return (
    <>
      <div className="field">
        <label>{fl("Operation")}</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {['list', 'download', 'upload', 'delete'].map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Path {operation !== 'list' && <span style={{ color: 'var(--danger)' }}>*</span>}</label>
        <input placeholder={operation === 'list' ? '(directory, optional)' : '/path/to/file'} value={str('path', '')} onChange={(e) => set('path', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      {operation === 'upload' && (
        <div className="field">
          <label>{fl("Content (base64)")} <span style={{ color: 'var(--danger)' }}>*</span></label>
          <textarea rows={3} value={str('content', '')} onChange={(e) => set('content', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
    </>
  )
}

export function sshKeyFields(str: ConfigProps['str'], set: ConfigProps['set']) {
  return (
    <>
      <div className="field">
        <label>{fl("Private Key")} <span style={{ color: 'var(--muted)' }}>{fl("(PEM — overrides password)")}</span></label>
        <textarea rows={3} placeholder="-----BEGIN OPENSSH PRIVATE KEY-----…" value={str('private_key', '')} onChange={(e) => set('private_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 11 }} />
      </div>
      <div className="field">
        <label>{fl("Key Passphrase")}</label>
        <input type="password" value={str('passphrase', '')} onChange={(e) => set('passphrase', e.target.value)} />
      </div>
    </>
  )
}

// ── 国内大模型通用字段组件 ────────────────────────────────────────────────────
export function CnLlmCommonFields({ str, set, num }: Pick<ConfigProps, 'str' | 'set' | 'num'>) {
  return (
    <>
      <div className="field">
        <label>{fl("System Prompt")} <span style={{ color: 'var(--muted)' }}>{fl("(optional)")}</span></label>
        <textarea rows={2} placeholder="You are a helpful assistant." value={str('system_prompt', '')} onChange={(e) => set('system_prompt', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Prompt Template *")}</label>
        <textarea rows={4} placeholder="{{input.text}}" value={str('prompt_template', '')} onChange={(e) => set('prompt_template', e.target.value)} />
      </div>
      <div style={{ display: 'flex', gap: 8 }}>
        <div className="field" style={{ flex: 1 }}>
          <label>{fl("Max Tokens")}</label>
          <input type="number" min={64} max={32768} value={num('max_tokens', 1024)} onChange={(e) => set('max_tokens', Number(e.target.value))} />
        </div>
        <div className="field" style={{ flex: 1 }}>
          <label>{fl("Temperature")}</label>
          <input type="number" step={0.1} min={0} max={2} value={num('temperature', 0.7)} onChange={(e) => set('temperature', Number(e.target.value))} />
        </div>
      </div>
      <div className="field">
        <label>{fl("API Base URL")} <span style={{ color: 'var(--muted)' }}>{fl("(可选，覆盖默认端点)")}</span></label>
        <input placeholder="留空用默认；OpenAI 兼容 /chat/completions 地址" value={str('base_url', '')} onChange={(e) => set('base_url', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>{fl("新模型（MiniMax M 系列、ERNIE 4.5/5.0/X1 等）填对应新接口地址即可调用。")}</span>
      </div>
    </>
  )
}
