// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import type { ConfigProps } from '../types'
import { fl } from '../i18nLabels'
import { ModelField, llmEndpointFields, CnLlmCommonFields } from './_helpers'

export function ReplicateConfig({ config, set, str }: ConfigProps) {
  const operation = str('operation', 'run')
  const OPERATIONS = ['run', 'create_prediction', 'get_prediction', 'list_models']
  return (
    <>
      <div className="field">
        <label>{fl("API Token")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="r8_…" value={str('api_token', '')} onChange={(e) => set('api_token', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Operation")}</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {['run', 'create_prediction'].includes(operation) && (
        <>
          <div className="field">
            <label>{fl("Model Version")} <span style={{ color: 'var(--danger)' }}>*</span></label>
            <input placeholder="abc123… (version hash)" value={str('version', '')} onChange={(e) => set('version', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>{fl("Input (JSON)")}</label>
            <textarea rows={3} placeholder='{"prompt": "{{input.prompt}}"}' value={typeof config.input === 'object' ? JSON.stringify(config.input, null, 2) : str('input', '')} onChange={(e) => { try { set('input', JSON.parse(e.target.value)) } catch { set('input', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
        </>
      )}
      {operation === 'get_prediction' && (
        <div className="field">
          <label>{fl("Prediction ID")} <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input placeholder="{{input.prediction_id}}" value={str('prediction_id', '')} onChange={(e) => set('prediction_id', e.target.value)} />
        </div>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Calls the Replicate REST API. Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
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
        <label>{fl("API Key")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Operation")}</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation === 'chat' && (
        <>
          <div className="field">
            <label>{fl("Model")}</label>
            <select value={str('model', 'mistral-small-latest')} onChange={(e) => set('model', e.target.value)}>
              {MODELS.filter(m => m !== 'mistral-embed').map((m) => <option key={m} value={m}>{m}</option>)}
            </select>
          </div>
          <div className="field">
            <label>{fl("Prompt (or use Messages JSON)")}</label>
            <textarea rows={3} placeholder="{{input.prompt}}" value={str('prompt', '')} onChange={(e) => set('prompt', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>{fl("Temperature")}</label>
            <input type="number" min={0} max={2} step={0.1} placeholder="0.7" value={(config['temperature'] as number | undefined) ?? ''} onChange={(e) => set('temperature', e.target.value ? parseFloat(e.target.value) : undefined)} />
          </div>
          <div className="field">
            <label>{fl("Max Tokens")}</label>
            <input type="number" min={1} placeholder="1024" value={(config['max_tokens'] as number | undefined) ?? ''} onChange={(e) => set('max_tokens', e.target.value ? parseInt(e.target.value) : undefined)} />
          </div>
        </>
      )}
      {operation === 'embeddings' && (
        <>
          <div className="field">
            <label>{fl("Model")}</label>
            <select value={str('model', 'mistral-embed')} onChange={(e) => set('model', e.target.value)}>
              <option value="mistral-embed">{fl("mistral-embed")}</option>
            </select>
          </div>
          <div className="field">
            <label>{fl("Input (string or array)")}</label>
            <textarea rows={2} placeholder='"{{input.text}}"' value={str('input', '')} onChange={(e) => set('input', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
        </>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
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
        <label>{fl("API Key")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="pplx-…" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Model")}</label>
        <select value={str('model', 'llama-3.1-sonar-small-128k-online')} onChange={(e) => set('model', e.target.value)}>
          {MODELS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>{fl("Prompt")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={3} placeholder="{{input.question}}" value={str('prompt', '')} onChange={(e) => set('prompt', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Temperature")}</label>
        <input type="number" min={0} max={2} step={0.1} placeholder="0.2" value={(config['temperature'] as number | undefined) ?? ''} onChange={(e) => set('temperature', e.target.value ? parseFloat(e.target.value) : undefined)} />
      </div>
      <div className="field">
        <label>{fl("Max Tokens")}</label>
        <input type="number" min={1} placeholder="1024" value={(config['max_tokens'] as number | undefined) ?? ''} onChange={(e) => set('max_tokens', e.target.value ? parseInt(e.target.value) : undefined)} />
      </div>
      <div className="field" style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
        <input type="checkbox" id="pplx-citations" checked={!!config.return_citations} onChange={(e) => set('return_citations', e.target.checked)} />
        <label htmlFor="pplx-citations" style={{ margin: 0 }}>{fl("Return Citations")}</label>
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Online models perform real-time web search. Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
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
        <label>{fl("API Key")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Operation")}</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation === 'chat' && (
        <>
          <div className="field">
            <label>{fl("Model")}</label>
            <select value={str('model', 'command-r-plus')} onChange={(e) => set('model', e.target.value)}>
              {CHAT_MODELS.map((m) => <option key={m} value={m}>{m}</option>)}
            </select>
          </div>
          <div className="field">
            <label>{fl("Message")} <span style={{ color: 'var(--danger)' }}>*</span></label>
            <textarea rows={3} placeholder="{{input.message}}" value={str('message', '')} onChange={(e) => set('message', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>{fl("Temperature")}</label>
            <input type="number" min={0} max={1} step={0.1} value={(config['temperature'] as number | undefined) ?? ''} onChange={(e) => set('temperature', e.target.value ? parseFloat(e.target.value) : undefined)} />
          </div>
        </>
      )}
      {operation === 'embed' && (
        <>
          <div className="field">
            <label>{fl("Model")}</label>
            <select value={str('model', 'embed-english-v3.0')} onChange={(e) => set('model', e.target.value)}>
              {EMBED_MODELS.map((m) => <option key={m} value={m}>{m}</option>)}
            </select>
          </div>
          <div className="field">
            <label>{fl("Texts (JSON array)")} <span style={{ color: 'var(--danger)' }}>*</span></label>
            <textarea rows={2} placeholder='["{{input.text}}"]' value={typeof config.texts === 'object' ? JSON.stringify(config.texts) : str('texts', '')} onChange={(e) => { try { set('texts', JSON.parse(e.target.value)) } catch { set('texts', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>{fl("Input Type")}</label>
            <select value={str('input_type', 'search_document')} onChange={(e) => set('input_type', e.target.value)}>
              {['search_document','search_query','classification','clustering'].map((t) => <option key={t} value={t}>{t}</option>)}
            </select>
          </div>
        </>
      )}
      {operation === 'rerank' && (
        <>
          <div className="field">
            <label>{fl("Model")}</label>
            <select value={str('model', 'rerank-english-v3.0')} onChange={(e) => set('model', e.target.value)}>
              {RERANK_MODELS.map((m) => <option key={m} value={m}>{m}</option>)}
            </select>
          </div>
          <div className="field">
            <label>{fl("Query")} <span style={{ color: 'var(--danger)' }}>*</span></label>
            <input placeholder="{{input.query}}" value={str('query', '')} onChange={(e) => set('query', e.target.value)} />
          </div>
          <div className="field">
            <label>{fl("Documents (JSON array)")} <span style={{ color: 'var(--danger)' }}>*</span></label>
            <textarea rows={3} placeholder='["doc1","doc2"]' value={typeof config.documents === 'object' ? JSON.stringify(config.documents) : str('documents', '')} onChange={(e) => { try { set('documents', JSON.parse(e.target.value)) } catch { set('documents', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
        </>
      )}
      {operation === 'classify' && (
        <>
          <div className="field">
            <label>{fl("Inputs (JSON array)")} <span style={{ color: 'var(--danger)' }}>*</span></label>
            <textarea rows={2} placeholder='["text to classify"]' value={typeof config.inputs === 'object' ? JSON.stringify(config.inputs) : str('inputs', '')} onChange={(e) => { try { set('inputs', JSON.parse(e.target.value)) } catch { set('inputs', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>{fl("Examples (JSON array)")} <span style={{ color: 'var(--danger)' }}>*</span></label>
            <textarea rows={3} placeholder='[{"text":"pos example","label":"positive"}]' value={typeof config.examples === 'object' ? JSON.stringify(config.examples) : str('examples', '')} onChange={(e) => { try { set('examples', JSON.parse(e.target.value)) } catch { set('examples', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
        </>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
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
        <label>{fl("API Key")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Operation")}</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      <div className="field">
        <label>{fl("Model")}</label>
        <input placeholder="meta-llama/Llama-3.2-11B-Vision-Instruct-Turbo" value={str('model', '')} onChange={(e) => set('model', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      {['chat', 'completions'].includes(operation) && (
        <>
          <div className="field">
            <label>Prompt {operation === 'chat' ? '(or Messages JSON)' : ''}</label>
            <textarea rows={3} placeholder="{{input.prompt}}" value={str('prompt', '')} onChange={(e) => set('prompt', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>{fl("Temperature")}</label>
            <input type="number" min={0} max={2} step={0.1} placeholder="0.7" value={(config['temperature'] as number | undefined) ?? ''} onChange={(e) => set('temperature', e.target.value ? parseFloat(e.target.value) : undefined)} />
          </div>
          <div className="field">
            <label>{fl("Max Tokens")}</label>
            <input type="number" min={1} placeholder="512" value={(config['max_tokens'] as number | undefined) ?? ''} onChange={(e) => set('max_tokens', e.target.value ? parseInt(e.target.value) : undefined)} />
          </div>
        </>
      )}
      {operation === 'embeddings' && (
        <div className="field">
          <label>{fl("Input")}</label>
          <input placeholder="{{input.text}}" value={str('input', '')} onChange={(e) => set('input', e.target.value)} />
        </div>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Runs open-source LLMs. Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
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
        <label>{fl("API Token")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="hf_…" value={str('api_token', '')} onChange={(e) => set('api_token', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Model")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="facebook/bart-large-cnn" value={str('model', '')} onChange={(e) => set('model', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Operation")}</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation === 'inference' && (
        <>
          <div className="field">
            <label>{fl("Inputs")} <span style={{ color: 'var(--danger)' }}>*</span></label>
            <textarea rows={3} placeholder='"{{input.text}}" or {"question":"…","context":"…"}' value={str('inputs', '')} onChange={(e) => { try { set('inputs', JSON.parse(e.target.value)) } catch { set('inputs', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>{fl("Parameters (JSON)")}</label>
            <textarea rows={2} placeholder='{"max_length":100}' value={typeof config.parameters === 'object' ? JSON.stringify(config.parameters) : str('parameters', '')} onChange={(e) => { try { set('parameters', JSON.parse(e.target.value)) } catch { set('parameters', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
        </>
      )}
      {operation === 'list_models' && (
        <>
          <div className="field">
            <label>{fl("Search")}</label>
            <input placeholder="text-classification" value={str('search', '')} onChange={(e) => set('search', e.target.value)} />
          </div>
          <div className="field">
            <label>{fl("Limit")}</label>
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

export function GroqConfig({ config, set, str }: ConfigProps) {
  const operation = str('operation', 'chat')
  const OPERATIONS = ['chat', 'models']
  return (
    <>
      <div className="field">
        <label>{fl("API Key")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="gsk_…" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Operation")}</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation === 'chat' && (
        <>
          <div className="field">
            <label>{fl("Model")}</label>
            <input placeholder="llama3-8b-8192" value={str('model', '')} onChange={(e) => set('model', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>{fl("Messages (JSON array)")}</label>
            <textarea rows={4} placeholder='[{"role":"user","content":"Hello"}]' value={typeof config.messages === 'object' ? JSON.stringify(config.messages, null, 2) : str('messages', '')} onChange={(e) => { try { set('messages', JSON.parse(e.target.value)) } catch { set('messages', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>{fl("Temperature")}</label>
            <input type="number" min={0} max={2} step={0.1} placeholder="1.0" value={(config['temperature'] as number | undefined) ?? ''} onChange={(e) => set('temperature', e.target.value ? parseFloat(e.target.value) : undefined)} />
          </div>
          <div className="field">
            <label>{fl("Max Tokens")}</label>
            <input type="number" min={1} placeholder="1024" value={(config['max_tokens'] as number | undefined) ?? ''} onChange={(e) => set('max_tokens', e.target.value ? parseInt(e.target.value) : undefined)} />
          </div>
        </>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Ultra-fast LLM inference. Models: llama3-8b-8192, llama3-70b-8192, mixtral-8x7b-32768, gemma-7b-it. Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
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
        <label>{fl("API Key")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="sk-or-…" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Operation")}</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation === 'chat' && (
        <>
          <div className="field">
            <label>{fl("Model")}</label>
            <input placeholder="openai/gpt-4o" value={str('model', '')} onChange={(e) => set('model', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>{fl("Messages (JSON array)")}</label>
            <textarea rows={4} placeholder='[{"role":"user","content":"Hello"}]' value={typeof config.messages === 'object' ? JSON.stringify(config.messages, null, 2) : str('messages', '')} onChange={(e) => { try { set('messages', JSON.parse(e.target.value)) } catch { set('messages', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>{fl("Temperature")}</label>
            <input type="number" min={0} max={2} step={0.1} placeholder="1.0" value={(config['temperature'] as number | undefined) ?? ''} onChange={(e) => set('temperature', e.target.value ? parseFloat(e.target.value) : undefined)} />
          </div>
          <div className="field">
            <label>{fl("Max Tokens")}</label>
            <input type="number" min={1} placeholder="1024" value={(config['max_tokens'] as number | undefined) ?? ''} onChange={(e) => set('max_tokens', e.target.value ? parseInt(e.target.value) : undefined)} />
          </div>
        </>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Access 100+ models from OpenAI, Anthropic, Meta, Mistral, and more. Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function BedrockConfig({ config, set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>{fl("Access Key ID")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input value={str('access_key_id', '')} onChange={(e) => set('access_key_id', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Secret Access Key")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" value={str('secret_access_key', '')} onChange={(e) => set('secret_access_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Region")}</label>
        <input placeholder="us-east-1" value={str('region', '')} onChange={(e) => set('region', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Model ID")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="anthropic.claude-3-5-sonnet-20240620-v1:0" value={str('model_id', '')} onChange={(e) => set('model_id', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Body (model-native JSON)")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={6} placeholder={'{\n  "anthropic_version": "bedrock-2023-05-31",\n  "max_tokens": 1024,\n  "messages": [{"role":"user","content":"Hi"}]\n}'} value={typeof config.body === 'object' ? JSON.stringify(config.body, null, 2) : str('body', '')} onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        <small style={{ color: 'var(--muted)', fontSize: 10 }}>{fl("Schema depends on the model family (Anthropic / Titan / Llama / \u2026).")}</small>
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("AWS Bedrock InvokeModel (SigV4-signed). Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function EmbeddingConfig({ config, set, str }: ConfigProps) {
  return (
    <>
      {llmEndpointFields(str, set, 'text-embedding-3-small')}
      <div className="field">
        <label>{fl("Input (text or JSON array)")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={3} placeholder='"hello" 或 ["a","b"]' value={typeof config.input === 'object' ? JSON.stringify(config.input) : str('input', '')} onChange={(e) => { try { set('input', JSON.parse(e.target.value)) } catch { set('input', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ embeddings, model, usage }'}</code>
      </p>
    </>
  )
}

export function RerankerConfig({ config, set, str }: ConfigProps) {
  return (
    <>
      {llmEndpointFields(str, set, 'rerank-english-v3.0')}
      <div className="field">
        <label>{fl("Query")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input value={str('query', '')} onChange={(e) => set('query', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Documents (JSON array)")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={3} placeholder='["doc one","doc two"]' value={typeof config.documents === 'object' ? JSON.stringify(config.documents) : str('documents', '')} onChange={(e) => { try { set('documents', JSON.parse(e.target.value)) } catch { set('documents', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Top N")}</label>
        <input type="number" min={1} placeholder="(all)" value={(config['top_n'] as number | undefined) ?? ''} onChange={(e) => set('top_n', e.target.value ? parseInt(e.target.value) : undefined)} />
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Cohere/Jina-style rerank. Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function TextSplitterConfig({ set, str, num }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>{fl("Text")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={4} value={str('text', '')} onChange={(e) => set('text', e.target.value)} />
      </div>
      <div style={{ display: 'flex', gap: 8 }}>
        <div className="field" style={{ flex: 1 }}>
          <label>{fl("Chunk Size")}</label>
          <input type="number" min={1} value={num('chunk_size', 1000)} onChange={(e) => set('chunk_size', Number(e.target.value))} />
        </div>
        <div className="field" style={{ flex: 1 }}>
          <label>{fl("Overlap")}</label>
          <input type="number" min={0} value={num('chunk_overlap', 200)} onChange={(e) => set('chunk_overlap', Number(e.target.value))} />
        </div>
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Char-boundary chunking (UTF-8 safe). Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ chunks, count }'}</code>
      </p>
    </>
  )
}

export function StructuredOutputConfig({ config, set, str }: ConfigProps) {
  return (
    <>
      {llmEndpointFields(str, set, 'gpt-5.4-mini')}
      <div className="field">
        <label>{fl("Prompt Template")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={3} placeholder="Extract fields from: {{input.text}}" value={str('prompt_template', '')} onChange={(e) => set('prompt_template', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("JSON Schema (optional)")}</label>
        <textarea rows={3} placeholder='{"type":"object","properties":{…}}' value={typeof config.schema === 'object' ? JSON.stringify(config.schema, null, 2) : str('schema', '')} onChange={(e) => { try { set('schema', JSON.parse(e.target.value)) } catch { set('schema', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("LLM JSON output. Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ data, raw, model }'}</code>
      </p>
    </>
  )
}

export function ClassifierConfig({ config, set, str }: ConfigProps) {
  return (
    <>
      {llmEndpointFields(str, set, 'gpt-5.4-mini')}
      <div className="field">
        <label>{fl("Input")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={2} value={str('input', '')} onChange={(e) => set('input', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Categories (JSON array)")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={2} placeholder='["positive","neutral","negative"]' value={typeof config.categories === 'object' ? JSON.stringify(config.categories) : str('categories', '')} onChange={(e) => { try { set('categories', JSON.parse(e.target.value)) } catch { set('categories', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ category, raw }'}</code>
      </p>
    </>
  )
}

export function ImageGenConfig({ set, str, num }: ConfigProps) {
  return (
    <>
      {llmEndpointFields(str, set, 'dall-e-3')}
      <div className="field">
        <label>{fl("Prompt")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={3} value={str('prompt', '')} onChange={(e) => set('prompt', e.target.value)} />
      </div>
      <div style={{ display: 'flex', gap: 8 }}>
        <div className="field" style={{ flex: 1 }}>
          <label>{fl("Size")}</label>
          <input placeholder="1024x1024" value={str('size', '')} onChange={(e) => set('size', e.target.value)} />
        </div>
        <div className="field" style={{ flex: 1 }}>
          <label>{fl("N")}</label>
          <input type="number" min={1} value={num('n', 1)} onChange={(e) => set('n', Number(e.target.value))} />
        </div>
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function VideoGenConfig({ config, set, str, num }: ConfigProps) {
  const provider = str('provider', 'seedance')
  return (
    <>
      <div className="field">
        <label>{fl("Provider")}</label>
        <select value={provider} onChange={(e) => set('provider', e.target.value)}>
          <option value="seedance">{fl("Seedance (火山方舟 / Volcengine Ark)")}</option>
          <option value="replicate">Replicate</option>
          <option value="generic">{fl("Generic (OpenAI-compatible)")}</option>
        </select>
      </div>

      {provider === 'replicate' ? (
        <>
          <div className="field">
            <label>{fl("API Token")} <span style={{ color: 'var(--danger)' }}>*</span></label>
            <input value={str('api_token')} placeholder="{{credential.replicate_token}}" onChange={(e) => set('api_token', e.target.value)} />
          </div>
          <div className="field">
            <label>{fl("Model")} <span style={{ color: 'var(--danger)' }}>*</span></label>
            <input value={str('model')} placeholder="owner/model:version" onChange={(e) => set('model', e.target.value)} />
            <span style={{ fontSize: 11, color: 'var(--muted)' }}>{fl("Replicate model version ID (a video model).")}</span>
          </div>
        </>
      ) : (
        <>
          <div className="field">
            <label>{fl("API Key")} <span style={{ color: 'var(--danger)' }}>*</span></label>
            <input value={str('api_key')} placeholder="{{credential.ark_key}}" onChange={(e) => set('api_key', e.target.value)} />
          </div>
          <div className="field">
            <label>{fl("Model")}{provider === 'generic' ? <span style={{ color: 'var(--danger)' }}> *</span> : null}</label>
            <input value={str('model')} placeholder={provider === 'seedance' ? 'doubao-seedance-1-0-pro-250528' : ''} onChange={(e) => set('model', e.target.value)} />
          </div>
          <div className="field">
            <label>{fl("Base URL")} {provider === 'generic' ? <span style={{ color: 'var(--danger)' }}>*</span> : <span style={{ color: 'var(--muted)' }}>{fl("(optional)")}</span>}</label>
            <input value={str('base_url')} placeholder={provider === 'seedance' ? 'https://ark.cn-beijing.volces.com/api/v3' : 'https://your-endpoint/v1/video/generations'} onChange={(e) => set('base_url', e.target.value)} />
          </div>
        </>
      )}

      <div className="field">
        <label>{fl("Prompt")}</label>
        <textarea rows={3} value={str('prompt', '')} placeholder="A cinematic shot of a city at dusk, slow pan" onChange={(e) => set('prompt', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Image URL")} <span style={{ color: 'var(--muted)' }}>{fl("(optional, 图生视频)")}</span></label>
        <input value={str('image_url')} placeholder="https://… or {{prev.image_url}}" onChange={(e) => set('image_url', e.target.value)} />
      </div>

      {provider === 'seedance' && (
        <div style={{ display: 'flex', gap: 8 }}>
          <div className="field" style={{ flex: 1 }}>
            <label>{fl("Duration")}</label>
            <input value={str('duration')} placeholder="5" onChange={(e) => set('duration', e.target.value)} />
          </div>
          <div className="field" style={{ flex: 1 }}>
            <label>{fl("Ratio")}</label>
            <input value={str('ratio')} placeholder="16:9" onChange={(e) => set('ratio', e.target.value)} />
          </div>
          <div className="field" style={{ flex: 1 }}>
            <label>{fl("Resolution")}</label>
            <input value={str('resolution')} placeholder="1080p" onChange={(e) => set('resolution', e.target.value)} />
          </div>
        </div>
      )}
      {provider === 'replicate' && (
        <div className="field">
          <label>{fl("Input (JSON, optional)")}</label>
          <textarea rows={3} placeholder='{"prompt":"…","duration":5}' value={typeof config.input === 'object' ? JSON.stringify(config.input, null, 2) : str('input', '')} onChange={(e) => { try { set('input', JSON.parse(e.target.value)) } catch { set('input', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          <span style={{ fontSize: 11, color: 'var(--muted)' }}>{fl("Overrides prompt/image_url; passed straight to the model.")}</span>
        </div>
      )}

      <div style={{ display: 'flex', gap: 8 }}>
        <div className="field" style={{ flex: 1 }}>
          <label>{fl("Poll max (s)")}</label>
          <input type="number" min={10} max={900} value={num('poll_max_secs', 300)} onChange={(e) => set('poll_max_secs', Number(e.target.value))} />
        </div>
        <div className="field" style={{ flex: 1 }}>
          <label>{fl("Poll interval (s)")}</label>
          <input type="number" min={2} max={30} value={num('poll_interval_secs', 6)} onChange={(e) => set('poll_interval_secs', Number(e.target.value))} />
        </div>
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Async: submits a task then polls. Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ video_url, status, raw }'}</code>
      </p>
    </>
  )
}

export function SpeechToTextConfig({ set, str }: ConfigProps) {
  return (
    <>
      {llmEndpointFields(str, set, 'whisper-1')}
      <div className="field">
        <label>{fl("Audio (base64)")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={3} placeholder="SUQzBAAAA…" value={str('audio_base64', '')} onChange={(e) => set('audio_base64', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div style={{ display: 'flex', gap: 8 }}>
        <div className="field" style={{ flex: 1 }}>
          <label>{fl("Filename")}</label>
          <input placeholder="audio.mp3" value={str('filename', '')} onChange={(e) => set('filename', e.target.value)} />
        </div>
        <div className="field" style={{ flex: 1 }}>
          <label>{fl("Language")}</label>
          <input placeholder="(auto)" value={str('language', '')} onChange={(e) => set('language', e.target.value)} />
        </div>
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Whisper transcription. Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, text }'}</code>
      </p>
    </>
  )
}

export function TtsConfig({ set, str }: ConfigProps) {
  return (
    <>
      {llmEndpointFields(str, set, 'tts-1')}
      <div className="field">
        <label>{fl("Input")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={3} value={str('input', '')} onChange={(e) => set('input', e.target.value)} />
      </div>
      <div style={{ display: 'flex', gap: 8 }}>
        <div className="field" style={{ flex: 1 }}>
          <label>{fl("Voice")}</label>
          <input placeholder="alloy" value={str('voice', '')} onChange={(e) => set('voice', e.target.value)} />
        </div>
        <div className="field" style={{ flex: 1 }}>
          <label>{fl("Format")}</label>
          <select value={str('format', 'mp3')} onChange={(e) => set('format', e.target.value)}>
            {['mp3', 'opus', 'aac', 'flac', 'wav'].map((f) => <option key={f} value={f}>{f}</option>)}
          </select>
        </div>
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ audio_base64, format }'}</code>
      </p>
    </>
  )
}

export function VertexConfig({ set, str, num }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>{fl("Access Token *")}</label>
        <input type="password" placeholder="OAuth2 bearer (cloud-platform scope)" value={str('access_token', '')} onChange={(e) => set('access_token', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Project *")}</label>
        <input placeholder="my-gcp-project" value={str('project', '')} onChange={(e) => set('project', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div style={{ display: 'flex', gap: 8 }}>
        <div className="field" style={{ flex: 1 }}>
          <label>{fl("Location")}</label>
          <input placeholder="us-central1" value={str('location', '')} onChange={(e) => set('location', e.target.value)} />
        </div>
        <div className="field" style={{ flex: 1 }}>
          <label>{fl("Model")}</label>
          <input placeholder="gemini-1.5-flash" value={str('model', '')} onChange={(e) => set('model', e.target.value)} />
        </div>
      </div>
      <CnLlmCommonFields str={str} set={set} num={num} />
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Vertex AI generateContent. Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ content, model, usage }'}</code>
      </p>
    </>
  )
}

export function AzureOpenaiConfig({ set, str, num }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>{fl("Endpoint *")}</label>
        <input placeholder="https://my-res.openai.azure.com" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Deployment *")}</label>
        <input placeholder="gpt-4o" value={str('deployment', '')} onChange={(e) => set('deployment', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("API Version")}</label>
        <input placeholder="2024-02-01" value={str('api_version', '')} onChange={(e) => set('api_version', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("API Key *")}</label>
        <input type="password" placeholder="azure key" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <CnLlmCommonFields str={str} set={set} num={num} />
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Azure OpenAI (deployment-based). Returns")} <code>{'{ content, model, usage }'}</code>
      </p>
    </>
  )
}

export function GrokConfig({ set, str, num }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>{fl("Model")}</label>
        <input list="grok-models" placeholder="grok-4.3" value={str('model', 'grok-4.3')} onChange={(e) => set('model', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        <datalist id="grok-models">
          <option value="grok-4.3">{fl("grok-4.3 (flagship)")}</option>
          <option value="grok-4.20">{fl("grok-4.20 (2M ctx)")}</option>
          <option value="grok-4">{fl("grok-4")}</option>
        </datalist>
      </div>
      <div className="field">
        <label>{fl("API Key *")}</label>
        <input type="password" placeholder="xai-..." value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <CnLlmCommonFields str={str} set={set} num={num} />
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("xAI Grok (OpenAI-compatible). Returns")} <code>{'{ content, model, usage }'}</code>
      </p>
    </>
  )
}

export function OllamaConfig({ set, str, num }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>{fl("Base URL")}</label>
        <input placeholder="http://localhost:11434/v1/chat/completions" value={str('base_url', '')} onChange={(e) => set('base_url', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Model")}</label>
        <input placeholder="llama3.2" value={str('model', 'llama3.2')} onChange={(e) => set('model', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("API Key")}</label>
        <input type="password" placeholder="(optional, ignored by local Ollama)" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <CnLlmCommonFields str={str} set={set} num={num} />
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Self-hosted Ollama (OpenAI-compatible). Returns")} <code>{'{ content, model, usage }'}</code>
      </p>
    </>
  )
}

export function DeepseekConfig({ set, str, num }: ConfigProps) {
  return (
    <>
      <ModelField str={str} set={set} fallback="deepseek-v4-flash" options={[
        ['deepseek-v4-flash', 'deepseek-v4-flash (V4, 1M)'],
        ['deepseek-v4-pro', 'deepseek-v4-pro (V4, 1M)'],
        ['deepseek-chat', 'deepseek-chat (legacy → retires 2026-07-24)'],
        ['deepseek-reasoner', 'deepseek-reasoner (legacy → retires 2026-07-24)'],
      ]} />
      <div className="field">
        <label>{fl("API Key *")}</label>
        <input type="password" placeholder="sk-..." value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <CnLlmCommonFields str={str} set={set} num={num} />
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("DeepSeek — 高性价比推理模型。返回")} <code>{'{ content, model, usage }'}</code>
      </p>
    </>
  )
}

export function QwenConfig({ set, str, num }: ConfigProps) {
  return (
    <>
      <ModelField str={str} set={set} fallback="qwen-max" options={[
        ['qwen3-max', 'qwen3-max (flagship)'],
        ['qwen3.5-plus', 'qwen3.5-plus'],
        ['qwen3.5-flash', 'qwen3.5-flash (fast)'],
        ['qwen-max', 'qwen-max (alias → latest)'],
        ['qwen-plus', 'qwen-plus (alias)'],
        ['qwen-turbo', 'qwen-turbo (alias)'],
        ['qwen-long', 'qwen-long (long-context)'],
      ]} />
      <div className="field">
        <label>{fl("API Key (DashScope) *")}</label>
        <input type="password" placeholder="sk-..." value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <CnLlmCommonFields str={str} set={set} num={num} />
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("阿里云通义千问（DashScope）。返回")} <code>{'{ content, model, usage }'}</code>
      </p>
    </>
  )
}

export function ZhipuConfig({ set, str, num }: ConfigProps) {
  return (
    <>
      <ModelField str={str} set={set} fallback="glm-4.6" options={[
        ['glm-4.7', 'glm-4.7'],
        ['glm-4.7-flash', 'glm-4.7-flash (free)'],
        ['glm-4.6', 'glm-4.6'],
        ['glm-4.5-air', 'glm-4.5-air (fast)'],
        ['glm-4-plus', 'glm-4-plus'],
        ['glm-4-flash', 'glm-4-flash'],
      ]} />
      <div className="field">
        <label>{fl("API Key *")}</label>
        <input type="password" placeholder="智谱 API Key" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <CnLlmCommonFields str={str} set={set} num={num} />
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("智谱 AI（GLM）。返回")} <code>{'{ content, model, usage }'}</code>
      </p>
    </>
  )
}

export function MoonshotConfig({ set, str, num }: ConfigProps) {
  return (
    <>
      <ModelField str={str} set={set} fallback="kimi-latest" options={[
        ['kimi-latest', 'kimi-latest (alias → newest)'],
        ['kimi-k2.6', 'kimi-k2.6'],
        ['kimi-k2.7-code', 'kimi-k2.7-code (coding)'],
        ['moonshot-v1-128k', 'moonshot-v1-128k (legacy)'],
        ['moonshot-v1-32k', 'moonshot-v1-32k (legacy)'],
        ['moonshot-v1-8k', 'moonshot-v1-8k (legacy)'],
      ]} />
      <div className="field">
        <label>{fl("API Key *")}</label>
        <input type="password" placeholder="sk-..." value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <CnLlmCommonFields str={str} set={set} num={num} />
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("月之暗面（Kimi）。返回")} <code>{'{ content, model, usage }'}</code>
      </p>
    </>
  )
}

export function DoubaoConfig({ set, str, num }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>{fl("API Key (火山引擎) *")}</label>
        <input type="password" placeholder="火山方舟 API Key" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Endpoint ID *")}</label>
        <input placeholder="ep-xxxxxxxx" value={str('endpoint_id', '')} onChange={(e) => set('endpoint_id', e.target.value)} />
        <span style={{ fontSize: 11, color: 'var(--muted)' }}>{fl("豆包使用推理接入点 ID 而非模型名")}</span>
      </div>
      <CnLlmCommonFields str={str} set={set} num={num} />
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("字节跳动豆包（火山引擎方舟）。返回")} <code>{'{ content, model, usage }'}</code>
      </p>
    </>
  )
}

export function MinimaxConfig({ set, str, num }: ConfigProps) {
  return (
    <>
      <ModelField str={str} set={set} fallback="MiniMax-Text-01" options={[
        ['MiniMax-Text-01', 'MiniMax-Text-01 (1M ctx)'],
        ['abab6.5s-chat', 'abab6.5s-chat (legacy)'],
        ['abab6.5-chat', 'abab6.5-chat (legacy)'],
        ['abab5.5s-chat', 'abab5.5s-chat (legacy)'],
      ]} />
      <div className="field">
        <label>{fl("API Key *")}</label>
        <input type="password" placeholder="MiniMax API Key" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Group ID *")}</label>
        <input placeholder="MiniMax Group ID" value={str('group_id', '')} onChange={(e) => set('group_id', e.target.value)} />
      </div>
      <CnLlmCommonFields str={str} set={set} num={num} />
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("MiniMax。返回")} <code>{'{ content, model, usage }'}</code>
      </p>
    </>
  )
}

export function ErnieConfig({ set, str, num }: ConfigProps) {
  return (
    <>
      <ModelField str={str} set={set} fallback="ernie-4.0-8k" options={[
        ['ernie-4.0-8k', 'ernie-4.0-8k'],
        ['ernie-3.5-8k', 'ernie-3.5-8k'],
        ['ernie-speed-128k', 'ernie-speed-128k'],
      ]} />
      <div className="field">
        <label>{fl("API Key (Client ID) *")}</label>
        <input type="password" placeholder="百度云 Client ID" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Secret Key (Client Secret) *")}</label>
        <input type="password" placeholder="百度云 Client Secret" value={str('secret_key', '')} onChange={(e) => set('secret_key', e.target.value)} />
      </div>
      <CnLlmCommonFields str={str} set={set} num={num} />
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("百度文心一言（自动 OAuth2 换取 access_token）。返回")} <code>{'{ content, model, usage }'}</code><br />
        {fl("ERNIE 4.5 / 5.0 / X1 走百度千帆 v2 新接口，本节点暂用旧版 wenxinworkshop 接口，仅支持上述模型。")}
      </p>
    </>
  )
}

export function HunyuanConfig({ set, str, num }: ConfigProps) {
  return (
    <>
      <ModelField str={str} set={set} fallback="hunyuan-turbos-latest" options={[
        ['hunyuan-turbos-latest', 'hunyuan-turbos-latest'],
        ['hunyuan-t1-latest', 'hunyuan-t1-latest (reasoning)'],
        ['hunyuan-lite', 'hunyuan-lite'],
        ['hunyuan-standard', 'hunyuan-standard (legacy)'],
      ]} />
      <div className="field">
        <label>{fl("API Key *")}</label>
        <input type="password" placeholder="腾讯混元 API Key" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <CnLlmCommonFields str={str} set={set} num={num} />
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("腾讯混元（OpenAI 兼容接口）。返回")} <code>{'{ content, model, usage }'}</code>
      </p>
    </>
  )
}
