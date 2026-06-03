// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import type { ReactNode } from 'react'
import type { ConfigProps } from './types'

function TemplateHint() {
  return (
    <p style={{ fontSize: 11, color: 'var(--muted)', marginTop: -6, lineHeight: 1.6 }}>
      Templates:{' '}
      <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{{input.field}}'}</code>
      {' · '}
      <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{{node_id.field}}'}</code>
      {' · '}
      <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{{credential.name}}'}</code>
    </p>
  )
}

/** Inline highlight of {{...}} tokens in a template string */
function TemplatePreview({ text }: { text: string }) {
  if (!text || !text.includes('{{')) return null
  const parts: ReactNode[] = []
  const re = /\{\{([^}]+)\}\}/g
  let last = 0, m: RegExpExecArray | null
  while ((m = re.exec(text)) !== null) {
    if (m.index > last) parts.push(<span key={last}>{text.slice(last, m.index)}</span>)
    parts.push(
      <code key={m.index} style={{ background: 'rgba(37,99,235,0.10)', color: 'var(--link)', padding: '0 3px', borderRadius: 3, fontSize: 10 }}>
        {'{{'}{m[1]}{'}}'}
      </code>
    )
    last = m.index + m[0].length
  }
  if (last < text.length) parts.push(<span key={last}>{text.slice(last)}</span>)
  return (
    <div style={{
      marginTop: 4, padding: '5px 8px', fontSize: 11, lineHeight: 1.6,
      background: 'var(--canvas-bg)', border: '1px solid var(--border)',
      borderRadius: 4, color: 'var(--muted)', wordBreak: 'break-word',
    }}>
      {parts}
    </div>
  )
}

export function OpenAIConfig({ set, str, num }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Model</label>
        <select value={str('model', 'gpt-4o-mini')} onChange={(e) => set('model', e.target.value)}>
          <option value="gpt-4o-mini">gpt-4o-mini (fast)</option>
          <option value="gpt-4o">gpt-4o (balanced)</option>
          <option value="o1-mini">o1-mini (reasoning)</option>
          <option value="o1">o1 (powerful)</option>
        </select>
      </div>
      <div className="field">
        <label>API Key *</label>
        <input
          placeholder="{{credential.openai_key}}"
          value={str('api_key')}
          onChange={(e) => set('api_key', e.target.value)}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>
          Use <code>{'{{credential.openai_key}}'}</code> to reference a stored credential.
        </span>
      </div>
      <div className="field">
        <label>System Prompt <span style={{ color: 'var(--muted)' }}>(optional)</span></label>
        <textarea
          rows={3}
          placeholder="You are a helpful assistant."
          value={str('system_prompt')}
          onChange={(e) => set('system_prompt', e.target.value)}
          style={{ minHeight: 60 }}
        />
      </div>
      <div className="field">
        <label>Prompt Template *</label>
        <textarea
          rows={4}
          placeholder={'Summarize: {{input.text}}'}
          value={str('prompt_template')}
          onChange={(e) => set('prompt_template', e.target.value)}
          style={{ minHeight: 72 }}
        />
        <TemplatePreview text={str('prompt_template')} />
        {str('prompt_template') && (
          <span style={{ fontSize: 10, color: 'var(--muted)', marginTop: 2 }}>
            ~{Math.ceil(str('prompt_template').length / 4)} tokens (rough estimate)
          </span>
        )}
      </div>
      <TemplateHint />
      <div style={{ display: 'flex', gap: 8 }}>
        <div className="field" style={{ flex: 1 }}>
          <label>Max Tokens</label>
          <input
            type="number" min={64} max={16384}
            value={num('max_tokens', 1024)}
            onChange={(e) => set('max_tokens', Number(e.target.value))}
          />
        </div>
        <div className="field" style={{ flex: 1 }}>
          <label>Temperature</label>
          <input
            type="number" min={0} max={2} step={0.1}
            value={num('temperature', 0.7)}
            onChange={(e) => set('temperature', Number(e.target.value))}
          />
        </div>
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>
          {'{ "content": "...", "model": "...", "usage": {...} }'}
        </code>
      </p>
    </>
  )
}

export function GeminiConfig({ set, str, num }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Model</label>
        <select value={str('model', 'gemini-2.0-flash')} onChange={(e) => set('model', e.target.value)}>
          <option value="gemini-2.0-flash">gemini-2.0-flash (fast)</option>
          <option value="gemini-1.5-pro">gemini-1.5-pro (balanced)</option>
          <option value="gemini-1.5-flash">gemini-1.5-flash (efficient)</option>
          <option value="gemini-2.0-flash-thinking-exp">gemini-2.0-flash-thinking (reasoning)</option>
        </select>
      </div>
      <div className="field">
        <label>API Key *</label>
        <input
          placeholder="{{credential.gemini_key}}"
          value={str('api_key')}
          onChange={(e) => set('api_key', e.target.value)}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>
          Use <code>{'{{credential.gemini_key}}'}</code> to reference a stored credential.
        </span>
      </div>
      <div className="field">
        <label>System Instruction <span style={{ color: 'var(--muted)' }}>(optional)</span></label>
        <textarea
          rows={3}
          placeholder="You are a helpful assistant."
          value={str('system_prompt')}
          onChange={(e) => set('system_prompt', e.target.value)}
          style={{ minHeight: 60 }}
        />
      </div>
      <div className="field">
        <label>Prompt Template *</label>
        <textarea
          rows={4}
          placeholder={'Summarize: {{input.text}}'}
          value={str('prompt_template')}
          onChange={(e) => set('prompt_template', e.target.value)}
          style={{ minHeight: 72 }}
        />
        <TemplatePreview text={str('prompt_template')} />
        {str('prompt_template') && (
          <span style={{ fontSize: 10, color: 'var(--muted)', marginTop: 2 }}>
            ~{Math.ceil(str('prompt_template').length / 4)} tokens (rough estimate)
          </span>
        )}
      </div>
      <TemplateHint />
      <div style={{ display: 'flex', gap: 8 }}>
        <div className="field" style={{ flex: 1 }}>
          <label>Max Tokens</label>
          <input
            type="number" min={64} max={32768}
            value={num('max_tokens', 1024)}
            onChange={(e) => set('max_tokens', Number(e.target.value))}
          />
        </div>
        <div className="field" style={{ flex: 1 }}>
          <label>Temperature</label>
          <input
            type="number" min={0} max={2} step={0.1}
            value={num('temperature', 0.7)}
            onChange={(e) => set('temperature', Number(e.target.value))}
          />
        </div>
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>
          {'{ "content": "...", "model": "...", "usage": {...} }'}
        </code>
      </p>
    </>
  )
}

export function ClaudeConfig({ set, str, num }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Model</label>
        <select value={str('model', 'claude-sonnet-4-6')} onChange={(e) => set('model', e.target.value)}>
          <option value="claude-sonnet-4-6">claude-sonnet-4-6 (balanced)</option>
          <option value="claude-opus-4-7">claude-opus-4-7 (powerful)</option>
          <option value="claude-haiku-4-5-20251001">claude-haiku-4-5 (fast)</option>
        </select>
      </div>
      <div className="field">
        <label>API Key *</label>
        <input
          placeholder="{{credential.anthropic_key}}"
          value={str('api_key')}
          onChange={(e) => set('api_key', e.target.value)}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>
          Use <code>{'{{credential.anthropic_key}}'}</code> to reference a stored credential.
        </span>
      </div>
      <div className="field">
        <label>System Prompt <span style={{ color: 'var(--muted)' }}>(optional)</span></label>
        <textarea
          rows={3}
          placeholder="You are a helpful assistant."
          value={str('system_prompt')}
          onChange={(e) => set('system_prompt', e.target.value)}
          style={{ minHeight: 60 }}
        />
      </div>
      <div className="field">
        <label>Prompt Template *</label>
        <textarea
          rows={4}
          placeholder={'Analyze: {{input.text}}'}
          value={str('prompt_template')}
          onChange={(e) => set('prompt_template', e.target.value)}
          style={{ minHeight: 72 }}
        />
        <TemplatePreview text={str('prompt_template')} />
        {str('prompt_template') && (
          <span style={{ fontSize: 10, color: 'var(--muted)', marginTop: 2 }}>
            ~{Math.ceil(str('prompt_template').length / 4)} tokens (rough estimate)
          </span>
        )}
      </div>
      <TemplateHint />
      <div className="field">
        <label>Max Tokens</label>
        <input
          type="number" min={64} max={8192}
          value={num('max_tokens', 1024)}
          onChange={(e) => set('max_tokens', Number(e.target.value))}
        />
      </div>
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>
          {'{ "content": "...", "model": "...", "usage": {...} }'}
        </code>
      </p>
    </>
  )
}

export function DatabaseConfig({ set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Connection URL *</label>
        <input
          placeholder="{{credential.db_url}}"
          value={str('url')}
          onChange={(e) => set('url', e.target.value)}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>
          PostgreSQL URL, e.g. <code>postgresql://user:pass@host/db</code>.
          Use <code>{'{{credential.db_url}}'}</code> for secure storage.
        </span>
      </div>
      <div className="field">
        <label>SQL Query *</label>
        <textarea
          rows={5}
          placeholder={'SELECT id, name, score\nFROM leads\nWHERE status = \'active\'\nLIMIT 100'}
          value={str('query')}
          onChange={(e) => set('query', e.target.value)}
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>
          Use <code>{'{{input.field}}'}</code> to inject values. SELECT returns <code>rows[]</code>; INSERT/UPDATE/DELETE returns <code>rows_affected</code>.
        </span>
      </div>
      <TemplateHint />
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        SELECT → <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ "rows": [...], "count": N }'}</code>
        <br />
        DML → <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ "rows_affected": N }'}</code>
      </p>
    </>
  )
}

export function GraphQLConfig({ set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>Endpoint URL *</label>
        <input
          placeholder="https://api.example.com/graphql"
          value={str('url')}
          onChange={(e) => set('url', e.target.value)}
        />
      </div>
      <div className="field">
        <label>Query / Mutation *</label>
        <textarea
          rows={6}
          placeholder={'query GetUser($id: ID!) {\n  user(id: $id) {\n    name\n    email\n  }\n}'}
          value={str('query')}
          onChange={(e) => set('query', e.target.value)}
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <div className="field">
        <label>Variables <span style={{ color: 'var(--muted)' }}>(JSON, supports {'{{}}'} templates)</span></label>
        <textarea
          rows={3}
          placeholder={'{ "id": "{{input.user_id}}" }'}
          value={str('variables')}
          onChange={(e) => set('variables', e.target.value)}
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <div className="field">
        <label>Bearer token <span style={{ color: 'var(--muted)' }}>(optional)</span></label>
        <input
          placeholder="{{credential.graphql_token}}"
          value={str('bearer_token')}
          onChange={(e) => set('bearer_token', e.target.value)}
        />
      </div>
      <TemplateHint />
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>
          {'{ "data": {...} }'}
        </code> or fails on GraphQL errors.
      </p>
    </>
  )
}

export function MistralConfig({ config, set, str }: ConfigProps) {
  const operation = str('operation', 'chat')
  const OPERATIONS = ['chat', 'embeddings', 'list_models']
  const MODELS = ['mistral-small-latest', 'mistral-medium-latest', 'mistral-large-latest', 'mistral-embed', 'open-mistral-7b', 'open-mixtral-8x7b']
  return (
    <>
      <div className="field">
        <label>API Key <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation === 'chat' && (
        <>
          <div className="field">
            <label>Model</label>
            <select value={str('model', 'mistral-small-latest')} onChange={(e) => set('model', e.target.value)}>
              {MODELS.filter(m => m !== 'mistral-embed').map((m) => <option key={m} value={m}>{m}</option>)}
            </select>
          </div>
          <div className="field">
            <label>Prompt (or use Messages JSON)</label>
            <textarea rows={3} placeholder="{{input.prompt}}" value={str('prompt', '')} onChange={(e) => set('prompt', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>Temperature</label>
            <input type="number" min={0} max={2} step={0.1} placeholder="0.7" value={(config['temperature'] as number | undefined) ?? ''} onChange={(e) => set('temperature', e.target.value ? parseFloat(e.target.value) : undefined)} />
          </div>
          <div className="field">
            <label>Max Tokens</label>
            <input type="number" min={1} placeholder="1024" value={(config['max_tokens'] as number | undefined) ?? ''} onChange={(e) => set('max_tokens', e.target.value ? parseInt(e.target.value) : undefined)} />
          </div>
        </>
      )}
      {operation === 'embeddings' && (
        <>
          <div className="field">
            <label>Model</label>
            <select value={str('model', 'mistral-embed')} onChange={(e) => set('model', e.target.value)}>
              <option value="mistral-embed">mistral-embed</option>
            </select>
          </div>
          <div className="field">
            <label>Input (string or array)</label>
            <textarea rows={2} placeholder='"{{input.text}}"' value={str('input', '')} onChange={(e) => set('input', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
        </>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function PerplexityConfig({ config, set, str }: ConfigProps) {
  const MODELS = [
    'llama-3.1-sonar-small-128k-online',
    'llama-3.1-sonar-large-128k-online',
    'llama-3.1-sonar-huge-128k-online',
    'llama-3.1-sonar-small-128k-chat',
    'llama-3.1-sonar-large-128k-chat',
  ]
  return (
    <>
      <div className="field">
        <label>API Key <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="pplx-…" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Model</label>
        <select value={str('model', 'llama-3.1-sonar-small-128k-online')} onChange={(e) => set('model', e.target.value)}>
          {MODELS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Prompt <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={3} placeholder="{{input.question}}" value={str('prompt', '')} onChange={(e) => set('prompt', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Temperature</label>
        <input type="number" min={0} max={2} step={0.1} placeholder="0.2" value={(config['temperature'] as number | undefined) ?? ''} onChange={(e) => set('temperature', e.target.value ? parseFloat(e.target.value) : undefined)} />
      </div>
      <div className="field">
        <label>Max Tokens</label>
        <input type="number" min={1} placeholder="1024" value={(config['max_tokens'] as number | undefined) ?? ''} onChange={(e) => set('max_tokens', e.target.value ? parseInt(e.target.value) : undefined)} />
      </div>
      <div className="field" style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
        <input type="checkbox" id="pplx-citations" checked={!!config.return_citations} onChange={(e) => set('return_citations', e.target.checked)} />
        <label htmlFor="pplx-citations" style={{ margin: 0 }}>Return Citations</label>
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Online models perform real-time web search. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function CohereConfig({ config, set, str }: ConfigProps) {
  const operation = str('operation', 'chat')
  const OPERATIONS = ['chat', 'embed', 'classify', 'rerank']
  const EMBED_MODELS = ['embed-english-v3.0', 'embed-multilingual-v3.0', 'embed-english-light-v3.0']
  const RERANK_MODELS = ['rerank-english-v3.0', 'rerank-multilingual-v3.0']
  const CHAT_MODELS = ['command-r-plus', 'command-r', 'command', 'command-light']
  return (
    <>
      <div className="field">
        <label>API Key <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation === 'chat' && (
        <>
          <div className="field">
            <label>Model</label>
            <select value={str('model', 'command-r-plus')} onChange={(e) => set('model', e.target.value)}>
              {CHAT_MODELS.map((m) => <option key={m} value={m}>{m}</option>)}
            </select>
          </div>
          <div className="field">
            <label>Message <span style={{ color: 'var(--danger)' }}>*</span></label>
            <textarea rows={3} placeholder="{{input.message}}" value={str('message', '')} onChange={(e) => set('message', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>Temperature</label>
            <input type="number" min={0} max={1} step={0.1} value={(config['temperature'] as number | undefined) ?? ''} onChange={(e) => set('temperature', e.target.value ? parseFloat(e.target.value) : undefined)} />
          </div>
        </>
      )}
      {operation === 'embed' && (
        <>
          <div className="field">
            <label>Model</label>
            <select value={str('model', 'embed-english-v3.0')} onChange={(e) => set('model', e.target.value)}>
              {EMBED_MODELS.map((m) => <option key={m} value={m}>{m}</option>)}
            </select>
          </div>
          <div className="field">
            <label>Texts (JSON array) <span style={{ color: 'var(--danger)' }}>*</span></label>
            <textarea rows={2} placeholder='["{{input.text}}"]' value={typeof config.texts === 'object' ? JSON.stringify(config.texts) : str('texts', '')} onChange={(e) => { try { set('texts', JSON.parse(e.target.value)) } catch { set('texts', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>Input Type</label>
            <select value={str('input_type', 'search_document')} onChange={(e) => set('input_type', e.target.value)}>
              {['search_document','search_query','classification','clustering'].map((t) => <option key={t} value={t}>{t}</option>)}
            </select>
          </div>
        </>
      )}
      {operation === 'rerank' && (
        <>
          <div className="field">
            <label>Model</label>
            <select value={str('model', 'rerank-english-v3.0')} onChange={(e) => set('model', e.target.value)}>
              {RERANK_MODELS.map((m) => <option key={m} value={m}>{m}</option>)}
            </select>
          </div>
          <div className="field">
            <label>Query <span style={{ color: 'var(--danger)' }}>*</span></label>
            <input placeholder="{{input.query}}" value={str('query', '')} onChange={(e) => set('query', e.target.value)} />
          </div>
          <div className="field">
            <label>Documents (JSON array) <span style={{ color: 'var(--danger)' }}>*</span></label>
            <textarea rows={3} placeholder='["doc1","doc2"]' value={typeof config.documents === 'object' ? JSON.stringify(config.documents) : str('documents', '')} onChange={(e) => { try { set('documents', JSON.parse(e.target.value)) } catch { set('documents', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
        </>
      )}
      {operation === 'classify' && (
        <>
          <div className="field">
            <label>Inputs (JSON array) <span style={{ color: 'var(--danger)' }}>*</span></label>
            <textarea rows={2} placeholder='["text to classify"]' value={typeof config.inputs === 'object' ? JSON.stringify(config.inputs) : str('inputs', '')} onChange={(e) => { try { set('inputs', JSON.parse(e.target.value)) } catch { set('inputs', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>Examples (JSON array) <span style={{ color: 'var(--danger)' }}>*</span></label>
            <textarea rows={3} placeholder='[{"text":"pos example","label":"positive"}]' value={typeof config.examples === 'object' ? JSON.stringify(config.examples) : str('examples', '')} onChange={(e) => { try { set('examples', JSON.parse(e.target.value)) } catch { set('examples', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
        </>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function ReplicateConfig({ config, set, str }: ConfigProps) {
  const operation = str('operation', 'run')
  const OPERATIONS = ['run', 'create_prediction', 'get_prediction', 'list_models']
  return (
    <>
      <div className="field">
        <label>API Token <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="r8_…" value={str('api_token', '')} onChange={(e) => set('api_token', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {['run', 'create_prediction'].includes(operation) && (
        <>
          <div className="field">
            <label>Model Version <span style={{ color: 'var(--danger)' }}>*</span></label>
            <input placeholder="abc123… (version hash)" value={str('version', '')} onChange={(e) => set('version', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>Input (JSON)</label>
            <textarea rows={3} placeholder='{"prompt": "{{input.prompt}}"}' value={typeof config.input === 'object' ? JSON.stringify(config.input, null, 2) : str('input', '')} onChange={(e) => { try { set('input', JSON.parse(e.target.value)) } catch { set('input', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
        </>
      )}
      {operation === 'get_prediction' && (
        <div className="field">
          <label>Prediction ID <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input placeholder="{{input.prediction_id}}" value={str('prediction_id', '')} onChange={(e) => set('prediction_id', e.target.value)} />
        </div>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Calls the Replicate REST API. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function GroqConfig({ config, set, str }: ConfigProps) {
  const operation = str('operation', 'chat')
  const OPERATIONS = ['chat', 'models']
  return (
    <>
      <div className="field">
        <label>API Key <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="gsk_…" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation === 'chat' && (
        <>
          <div className="field">
            <label>Model</label>
            <input placeholder="llama3-8b-8192" value={str('model', '')} onChange={(e) => set('model', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>Messages (JSON array)</label>
            <textarea rows={4} placeholder='[{"role":"user","content":"Hello"}]' value={typeof config.messages === 'object' ? JSON.stringify(config.messages, null, 2) : str('messages', '')} onChange={(e) => { try { set('messages', JSON.parse(e.target.value)) } catch { set('messages', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>Temperature</label>
            <input type="number" min={0} max={2} step={0.1} placeholder="1.0" value={(config['temperature'] as number | undefined) ?? ''} onChange={(e) => set('temperature', e.target.value ? parseFloat(e.target.value) : undefined)} />
          </div>
          <div className="field">
            <label>Max Tokens</label>
            <input type="number" min={1} placeholder="1024" value={(config['max_tokens'] as number | undefined) ?? ''} onChange={(e) => set('max_tokens', e.target.value ? parseInt(e.target.value) : undefined)} />
          </div>
        </>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Ultra-fast LLM inference. Models: llama3-8b-8192, llama3-70b-8192, mixtral-8x7b-32768, gemma-7b-it. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function OpenrouterConfig({ config, set, str }: ConfigProps) {
  const operation = str('operation', 'chat')
  const OPERATIONS = ['chat', 'models']
  return (
    <>
      <div className="field">
        <label>API Key <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="sk-or-…" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation === 'chat' && (
        <>
          <div className="field">
            <label>Model</label>
            <input placeholder="openai/gpt-4o" value={str('model', '')} onChange={(e) => set('model', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>Messages (JSON array)</label>
            <textarea rows={4} placeholder='[{"role":"user","content":"Hello"}]' value={typeof config.messages === 'object' ? JSON.stringify(config.messages, null, 2) : str('messages', '')} onChange={(e) => { try { set('messages', JSON.parse(e.target.value)) } catch { set('messages', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>Temperature</label>
            <input type="number" min={0} max={2} step={0.1} placeholder="1.0" value={(config['temperature'] as number | undefined) ?? ''} onChange={(e) => set('temperature', e.target.value ? parseFloat(e.target.value) : undefined)} />
          </div>
          <div className="field">
            <label>Max Tokens</label>
            <input type="number" min={1} placeholder="1024" value={(config['max_tokens'] as number | undefined) ?? ''} onChange={(e) => set('max_tokens', e.target.value ? parseInt(e.target.value) : undefined)} />
          </div>
        </>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Access 100+ models from OpenAI, Anthropic, Meta, Mistral, and more. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function TogetheraiConfig({ config, set, str }: ConfigProps) {
  const operation = str('operation', 'chat')
  const OPERATIONS = ['chat', 'completions', 'embeddings']
  return (
    <>
      <div className="field">
        <label>API Key <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Model</label>
        <input placeholder="meta-llama/Llama-3.2-11B-Vision-Instruct-Turbo" value={str('model', '')} onChange={(e) => set('model', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      {['chat', 'completions'].includes(operation) && (
        <>
          <div className="field">
            <label>Prompt {operation === 'chat' ? '(or Messages JSON)' : ''}</label>
            <textarea rows={3} placeholder="{{input.prompt}}" value={str('prompt', '')} onChange={(e) => set('prompt', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>Temperature</label>
            <input type="number" min={0} max={2} step={0.1} placeholder="0.7" value={(config['temperature'] as number | undefined) ?? ''} onChange={(e) => set('temperature', e.target.value ? parseFloat(e.target.value) : undefined)} />
          </div>
          <div className="field">
            <label>Max Tokens</label>
            <input type="number" min={1} placeholder="512" value={(config['max_tokens'] as number | undefined) ?? ''} onChange={(e) => set('max_tokens', e.target.value ? parseInt(e.target.value) : undefined)} />
          </div>
        </>
      )}
      {operation === 'embeddings' && (
        <div className="field">
          <label>Input</label>
          <input placeholder="{{input.text}}" value={str('input', '')} onChange={(e) => set('input', e.target.value)} />
        </div>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Runs open-source LLMs. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function HuggingfaceConfig({ config, set, str }: ConfigProps) {
  const operation = str('operation', 'inference')
  const OPERATIONS = ['inference', 'model_info', 'list_models']
  return (
    <>
      <div className="field">
        <label>API Token <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="hf_…" value={str('api_token', '')} onChange={(e) => set('api_token', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Model <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="facebook/bart-large-cnn" value={str('model', '')} onChange={(e) => set('model', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation === 'inference' && (
        <>
          <div className="field">
            <label>Inputs <span style={{ color: 'var(--danger)' }}>*</span></label>
            <textarea rows={3} placeholder='"{{input.text}}" or {"question":"…","context":"…"}' value={str('inputs', '')} onChange={(e) => { try { set('inputs', JSON.parse(e.target.value)) } catch { set('inputs', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>Parameters (JSON)</label>
            <textarea rows={2} placeholder='{"max_length":100}' value={typeof config.parameters === 'object' ? JSON.stringify(config.parameters) : str('parameters', '')} onChange={(e) => { try { set('parameters', JSON.parse(e.target.value)) } catch { set('parameters', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
        </>
      )}
      {operation === 'list_models' && (
        <>
          <div className="field">
            <label>Search</label>
            <input placeholder="text-classification" value={str('search', '')} onChange={(e) => set('search', e.target.value)} />
          </div>
          <div className="field">
            <label>Limit</label>
            <input type="number" min={1} max={100} placeholder="20" value={(config['limit'] as number | undefined) ?? ''} onChange={(e) => set('limit', e.target.value ? parseInt(e.target.value) : undefined)} />
          </div>
        </>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Supports any model on the Hub: summarization, classification, Q&amp;A, image, etc. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function PineconeConfig({ config, set, str }: ConfigProps) {
  const operation = str('operation', 'query')
  const OPERATIONS = ['query', 'upsert', 'delete', 'fetch']
  return (
    <>
      <div className="field">
        <label>API Key <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Index Host <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="https://my-index-abc.svc.pinecone.io" value={str('index_host', '')} onChange={(e) => set('index_host', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      <div className="field">
        <label>Namespace</label>
        <input placeholder="(optional)" value={str('namespace', '')} onChange={(e) => set('namespace', e.target.value)} />
      </div>
      {operation === 'query' && (
        <>
          <div className="field">
            <label>Vector (JSON float array) <span style={{ color: 'var(--danger)' }}>*</span></label>
            <textarea rows={2} placeholder="[0.1, 0.2, 0.3, …]" value={typeof config.vector === 'object' ? JSON.stringify(config.vector) : str('vector', '')} onChange={(e) => { try { set('vector', JSON.parse(e.target.value)) } catch { set('vector', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>Top K</label>
            <input type="number" min={1} max={10000} placeholder="10" value={(config['top_k'] as number | undefined) ?? ''} onChange={(e) => set('top_k', e.target.value ? parseInt(e.target.value) : undefined)} />
          </div>
          <div className="field" style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
            <input type="checkbox" id="pc-meta" checked={!!config.include_metadata} onChange={(e) => set('include_metadata', e.target.checked)} />
            <label htmlFor="pc-meta" style={{ margin: 0 }}>Include Metadata</label>
          </div>
        </>
      )}
      {operation === 'upsert' && (
        <div className="field">
          <label>Vectors (JSON array) <span style={{ color: 'var(--danger)' }}>*</span></label>
          <textarea rows={4} placeholder='[{"id":"v1","values":[0.1,0.2],"metadata":{"text":"hello"}}]' value={typeof config.vectors === 'object' ? JSON.stringify(config.vectors, null, 2) : str('vectors', '')} onChange={(e) => { try { set('vectors', JSON.parse(e.target.value)) } catch { set('vectors', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {['delete', 'fetch'].includes(operation) && (
        <div className="field">
          <label>IDs (JSON array) <span style={{ color: 'var(--danger)' }}>*</span></label>
          <textarea rows={2} placeholder='["id1","id2"]' value={typeof config.ids === 'object' ? JSON.stringify(config.ids) : str('ids', '')} onChange={(e) => { try { set('ids', JSON.parse(e.target.value)) } catch { set('ids', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function QdrantConfig({ config, set, str }: ConfigProps) {
  const operation = str('operation', 'search')
  const OPERATIONS = ['search', 'upsert', 'delete', 'get_collection', 'create_collection']
  return (
    <>
      <div className="field">
        <label>Server URL <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="https://your-cluster.qdrant.io" value={str('url', '')} onChange={(e) => set('url', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>API Key</label>
        <input type="password" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>Collection <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="my_collection" value={str('collection', '')} onChange={(e) => set('collection', e.target.value)} />
      </div>
      <div className="field">
        <label>Operation</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation === 'search' && (
        <>
          <div className="field">
            <label>Query Vector (JSON array)</label>
            <textarea rows={2} placeholder="[0.1, 0.2, 0.3, …]" value={typeof config.vector === 'object' ? JSON.stringify(config.vector) : str('vector', '')} onChange={(e) => { try { set('vector', JSON.parse(e.target.value)) } catch { set('vector', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>Top K</label>
            <input type="number" min={1} max={100} placeholder="10" value={(config['top'] as number | undefined) ?? ''} onChange={(e) => set('top', e.target.value ? parseInt(e.target.value) : undefined)} />
          </div>
        </>
      )}
      {operation === 'upsert' && (
        <div className="field">
          <label>Points (JSON array)</label>
          <textarea rows={4} placeholder='[{"id":1,"vector":[0.1,0.2],"payload":{"text":"…"}}]' value={typeof config.points === 'object' ? JSON.stringify(config.points, null, 2) : str('points', '')} onChange={(e) => { try { set('points', JSON.parse(e.target.value)) } catch { set('points', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {operation === 'delete' && (
        <div className="field">
          <label>Point IDs (JSON array)</label>
          <textarea rows={2} placeholder='[1, 2, 3]' value={typeof config.ids === 'object' ? JSON.stringify(config.ids) : str('ids', '')} onChange={(e) => { try { set('ids', JSON.parse(e.target.value)) } catch { set('ids', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {operation === 'create_collection' && (
        <div className="field">
          <label>Vector Size</label>
          <input type="number" min={1} placeholder="1536" value={(config['vector_size'] as number | undefined) ?? ''} onChange={(e) => set('vector_size', e.target.value ? parseInt(e.target.value) : undefined)} />
        </div>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        High-performance vector similarity search. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}
