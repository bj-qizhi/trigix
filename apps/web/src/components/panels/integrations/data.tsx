// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import type { ConfigProps } from '../types'
import { fl } from '../i18nLabels'

export function FirebaseConfig({ config, set, str }: ConfigProps) {
  const method = str('method', 'GET')
  const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE']
  const service = str('service', 'firestore')
  return (
    <>
      <div className="field">
        <label>{fl("Project ID")}</label>
        <input placeholder="my-firebase-project" value={str('project_id', '')} onChange={(e) => set('project_id', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("ID Token")}</label>
        <input type="password" placeholder="Firebase ID token or service account token" value={str('id_token', '')} onChange={(e) => set('id_token', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Service")}</label>
        <select value={service} onChange={(e) => set('service', e.target.value)}>
          <option value="firestore">{fl("Firestore")}</option>
          <option value="rtdb">{fl("Realtime Database")}</option>
          <option value="storage">{fl("Cloud Storage")}</option>
        </select>
      </div>
      {service === 'rtdb' && (
        <div className="field">
          <label>{fl("Database URL")}</label>
          <input placeholder="https://PROJECT.firebaseio.com" value={str('database_url', '')} onChange={(e) => set('database_url', e.target.value)} />
        </div>
      )}
      <div className="field">
        <label>{fl("Method")}</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>{fl("Endpoint / Document Path")}</label>
        <input placeholder="/users/USER_ID" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT', 'PATCH'].includes(method) && (
        <div className="field">
          <label>{fl("Body (JSON)")}</label>
          <textarea rows={4}
            placeholder='{"fields": {"name": {"stringValue": "Jane"}}}'
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("RTDB embeds auth in URL. Firestore/Storage use Bearer header.\n        Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

// ── Slice 297: Supabase ────────────────────────────────────────────────────────

export function SupabaseConfig({ config, set, str }: ConfigProps) {
  const method = str('method', 'GET')
  const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE']
  return (
    <>
      <div className="field">
        <label>{fl("Project URL")}</label>
        <input placeholder="https://abcdef.supabase.co" value={str('project_url', '')} onChange={(e) => set('project_url', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("API Key")}</label>
        <input type="password" placeholder="anon or service_role key" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Method")}</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>{fl("Endpoint")}</label>
        <input placeholder="/rest/v1/users" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Prefer (optional)")}</label>
        <input placeholder="return=representation" value={str('prefer', '')} onChange={(e) => set('prefer', e.target.value)} />
      </div>
      {['POST', 'PUT', 'PATCH'].includes(method) && (
        <div className="field">
          <label>{fl("Body (JSON)")}</label>
          <textarea rows={4}
            placeholder='{"name": "Jane", "email": "jane@example.com"}'
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Sends both")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{fl("apikey")}</code> {fl("and")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{fl("Authorization: Bearer")}</code> {fl("headers.\n        Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

// ── Slice 298: Mailchimp ───────────────────────────────────────────────────────

export function PineconeConfig({ config, set, str }: ConfigProps) {
  const operation = str('operation', 'query')
  const OPERATIONS = ['query', 'upsert', 'delete', 'fetch']
  return (
    <>
      <div className="field">
        <label>{fl("API Key")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Index Host")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="https://my-index-abc.svc.pinecone.io" value={str('index_host', '')} onChange={(e) => set('index_host', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Operation")}</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      <div className="field">
        <label>{fl("Namespace")}</label>
        <input placeholder="(optional)" value={str('namespace', '')} onChange={(e) => set('namespace', e.target.value)} />
      </div>
      {operation === 'query' && (
        <>
          <div className="field">
            <label>{fl("Vector (JSON float array)")} <span style={{ color: 'var(--danger)' }}>*</span></label>
            <textarea rows={2} placeholder="[0.1, 0.2, 0.3, …]" value={typeof config.vector === 'object' ? JSON.stringify(config.vector) : str('vector', '')} onChange={(e) => { try { set('vector', JSON.parse(e.target.value)) } catch { set('vector', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>{fl("Top K")}</label>
            <input type="number" min={1} max={10000} placeholder="10" value={(config['top_k'] as number | undefined) ?? ''} onChange={(e) => set('top_k', e.target.value ? parseInt(e.target.value) : undefined)} />
          </div>
          <div className="field" style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
            <input type="checkbox" id="pc-meta" checked={!!config.include_metadata} onChange={(e) => set('include_metadata', e.target.checked)} />
            <label htmlFor="pc-meta" style={{ margin: 0 }}>{fl("Include Metadata")}</label>
          </div>
        </>
      )}
      {operation === 'upsert' && (
        <div className="field">
          <label>{fl("Vectors (JSON array)")} <span style={{ color: 'var(--danger)' }}>*</span></label>
          <textarea rows={4} placeholder='[{"id":"v1","values":[0.1,0.2],"metadata":{"text":"hello"}}]' value={typeof config.vectors === 'object' ? JSON.stringify(config.vectors, null, 2) : str('vectors', '')} onChange={(e) => { try { set('vectors', JSON.parse(e.target.value)) } catch { set('vectors', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {['delete', 'fetch'].includes(operation) && (
        <div className="field">
          <label>{fl("IDs (JSON array)")} <span style={{ color: 'var(--danger)' }}>*</span></label>
          <textarea rows={2} placeholder='["id1","id2"]' value={typeof config.ids === 'object' ? JSON.stringify(config.ids) : str('ids', '')} onChange={(e) => { try { set('ids', JSON.parse(e.target.value)) } catch { set('ids', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
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
        <label>{fl("Server URL")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="https://your-cluster.qdrant.io" value={str('url', '')} onChange={(e) => set('url', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("API Key")}</label>
        <input type="password" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Collection")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="my_collection" value={str('collection', '')} onChange={(e) => set('collection', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Operation")}</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation === 'search' && (
        <>
          <div className="field">
            <label>{fl("Query Vector (JSON array)")}</label>
            <textarea rows={2} placeholder="[0.1, 0.2, 0.3, …]" value={typeof config.vector === 'object' ? JSON.stringify(config.vector) : str('vector', '')} onChange={(e) => { try { set('vector', JSON.parse(e.target.value)) } catch { set('vector', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>{fl("Top K")}</label>
            <input type="number" min={1} max={100} placeholder="10" value={(config['top'] as number | undefined) ?? ''} onChange={(e) => set('top', e.target.value ? parseInt(e.target.value) : undefined)} />
          </div>
        </>
      )}
      {operation === 'upsert' && (
        <div className="field">
          <label>{fl("Points (JSON array)")}</label>
          <textarea rows={4} placeholder='[{"id":1,"vector":[0.1,0.2],"payload":{"text":"…"}}]' value={typeof config.points === 'object' ? JSON.stringify(config.points, null, 2) : str('points', '')} onChange={(e) => { try { set('points', JSON.parse(e.target.value)) } catch { set('points', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {operation === 'delete' && (
        <div className="field">
          <label>{fl("Point IDs (JSON array)")}</label>
          <textarea rows={2} placeholder='[1, 2, 3]' value={typeof config.ids === 'object' ? JSON.stringify(config.ids) : str('ids', '')} onChange={(e) => { try { set('ids', JSON.parse(e.target.value)) } catch { set('ids', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {operation === 'create_collection' && (
        <div className="field">
          <label>{fl("Vector Size")}</label>
          <input type="number" min={1} placeholder="1536" value={(config['vector_size'] as number | undefined) ?? ''} onChange={(e) => set('vector_size', e.target.value ? parseInt(e.target.value) : undefined)} />
        </div>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("High-performance vector similarity search. Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function WeaviateConfig({ config, set, str }: ConfigProps) {
  const operation = str('operation', 'query')
  const OPERATIONS = ['query', 'create_object', 'get_object', 'delete_object']
  return (
    <>
      <div className="field">
        <label>{fl("Host")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="https://xyz.weaviate.network" value={str('host', '')} onChange={(e) => set('host', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("API Key")}</label>
        <input type="password" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Operation")}</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation === 'query' && (
        <div className="field">
          <label>{fl("GraphQL Query")} <span style={{ color: 'var(--danger)' }}>*</span></label>
          <textarea rows={5} placeholder={'{ Get { Article(nearVector: {vector: [0.1, 0.2]}) { title } } }'} value={str('query', '')} onChange={(e) => set('query', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {(operation === 'create_object' || operation === 'get_object' || operation === 'delete_object') && (
        <div className="field">
          <label>{fl("Class")} <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input placeholder="Article" value={str('class', '')} onChange={(e) => set('class', e.target.value)} />
        </div>
      )}
      {operation === 'create_object' && (
        <>
          <div className="field">
            <label>{fl("Properties (JSON object)")}</label>
            <textarea rows={3} placeholder='{"title":"…","body":"…"}' value={typeof config.properties === 'object' ? JSON.stringify(config.properties, null, 2) : str('properties', '')} onChange={(e) => { try { set('properties', JSON.parse(e.target.value)) } catch { set('properties', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>{fl("Vector (JSON array, optional)")}</label>
            <textarea rows={2} placeholder="[0.1, 0.2, 0.3, …]" value={typeof config.vector === 'object' ? JSON.stringify(config.vector) : str('vector', '')} onChange={(e) => { try { set('vector', JSON.parse(e.target.value)) } catch { set('vector', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
        </>
      )}
      {(operation === 'get_object' || operation === 'delete_object' || operation === 'create_object') && (
        <div className="field">
          <label>Object ID {operation !== 'create_object' && <span style={{ color: 'var(--danger)' }}>*</span>}{operation === 'create_object' && ' (optional)'}</label>
          <input placeholder="uuid" value={str('id', '')} onChange={(e) => set('id', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Weaviate vector store (REST + GraphQL). Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function ChromaConfig({ config, set, str }: ConfigProps) {
  const operation = str('operation', 'query')
  const OPERATIONS = ['query', 'add', 'delete', 'get_collection']
  return (
    <>
      <div className="field">
        <label>{fl("Host")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="http://localhost:8000" value={str('host', '')} onChange={(e) => set('host', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("API Key")}</label>
        <input type="password" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Operation")}</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation === 'get_collection' && (
        <div className="field">
          <label>{fl("Collection Name")} <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input placeholder="my_collection" value={str('collection', '')} onChange={(e) => set('collection', e.target.value)} />
        </div>
      )}
      {(operation === 'query' || operation === 'add' || operation === 'delete') && (
        <div className="field">
          <label>{fl("Collection ID")} <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input placeholder="resolve via get_collection" value={str('collection_id', '')} onChange={(e) => set('collection_id', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {operation === 'query' && (
        <>
          <div className="field">
            <label>{fl("Query Embeddings (JSON array)")}</label>
            <textarea rows={2} placeholder="[[0.1, 0.2, 0.3, …]]" value={typeof config.query_embeddings === 'object' ? JSON.stringify(config.query_embeddings) : str('query_embeddings', '')} onChange={(e) => { try { set('query_embeddings', JSON.parse(e.target.value)) } catch { set('query_embeddings', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>{fl("N Results")}</label>
            <input type="number" min={1} max={100} placeholder="10" value={(config['n_results'] as number | undefined) ?? ''} onChange={(e) => set('n_results', e.target.value ? parseInt(e.target.value) : undefined)} />
          </div>
        </>
      )}
      {operation === 'add' && (
        <>
          <div className="field">
            <label>{fl("IDs (JSON array)")}</label>
            <textarea rows={2} placeholder='["id1", "id2"]' value={typeof config.ids === 'object' ? JSON.stringify(config.ids) : str('ids', '')} onChange={(e) => { try { set('ids', JSON.parse(e.target.value)) } catch { set('ids', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>{fl("Embeddings (JSON array)")}</label>
            <textarea rows={2} placeholder="[[0.1, 0.2], [0.3, 0.4]]" value={typeof config.embeddings === 'object' ? JSON.stringify(config.embeddings) : str('embeddings', '')} onChange={(e) => { try { set('embeddings', JSON.parse(e.target.value)) } catch { set('embeddings', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>{fl("Documents (JSON array, optional)")}</label>
            <textarea rows={2} placeholder='["text one", "text two"]' value={typeof config.documents === 'object' ? JSON.stringify(config.documents) : str('documents', '')} onChange={(e) => { try { set('documents', JSON.parse(e.target.value)) } catch { set('documents', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
        </>
      )}
      {operation === 'delete' && (
        <div className="field">
          <label>{fl("IDs (JSON array)")}</label>
          <textarea rows={2} placeholder='["id1", "id2"]' value={typeof config.ids === 'object' ? JSON.stringify(config.ids) : str('ids', '')} onChange={(e) => { try { set('ids', JSON.parse(e.target.value)) } catch { set('ids', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Chroma vector store (REST data API). Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function MongodbConfig({ config, set, str }: ConfigProps) {
  const operation = str('operation', 'find')
  const OPERATIONS = ['find', 'findOne', 'insertOne', 'insertMany', 'updateOne', 'updateMany', 'deleteOne', 'deleteMany', 'aggregate']
  const jsonField = (key: string, label: string, placeholder: string, rows = 2) => (
    <div className="field">
      <label>{label}</label>
      <textarea rows={rows} placeholder={placeholder} value={typeof config[key] === 'object' ? JSON.stringify(config[key], null, 2) : str(key, '')} onChange={(e) => { try { set(key, JSON.parse(e.target.value)) } catch { set(key, e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
    </div>
  )
  return (
    <>
      <div className="field">
        <label>{fl("Data API URL")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="https://<region>.data.mongodb-api.com/app/<app-id>/endpoint/data/v1" value={str('data_api_url', '')} onChange={(e) => set('data_api_url', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("API Key")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Data Source (cluster)")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="Cluster0" value={str('data_source', '')} onChange={(e) => set('data_source', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Database")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="mydb" value={str('database', '')} onChange={(e) => set('database', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Collection")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="users" value={str('collection', '')} onChange={(e) => set('collection', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Operation")}</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {(operation === 'find' || operation === 'findOne' || operation === 'updateOne' || operation === 'updateMany' || operation === 'deleteOne' || operation === 'deleteMany') &&
        jsonField('filter', 'Filter (JSON)', '{"status":"active"}')}
      {operation === 'find' && (
        <>
          {jsonField('sort', 'Sort (JSON)', '{"createdAt":-1}')}
          <div className="field">
            <label>{fl("Limit")}</label>
            <input type="number" min={1} placeholder="100" value={(config['limit'] as number | undefined) ?? ''} onChange={(e) => set('limit', e.target.value ? parseInt(e.target.value) : undefined)} />
          </div>
        </>
      )}
      {operation === 'insertOne' && jsonField('document', 'Document (JSON)', '{"name":"Ada"}', 3)}
      {operation === 'insertMany' && jsonField('documents', 'Documents (JSON array)', '[{"name":"Ada"},{"name":"Lin"}]', 3)}
      {(operation === 'updateOne' || operation === 'updateMany') && jsonField('update', 'Update (JSON)', '{"$set":{"status":"done"}}', 3)}
      {operation === 'aggregate' && jsonField('pipeline', 'Pipeline (JSON array)', '[{"$match":{"x":1}},{"$group":{"_id":"$y"}}]', 4)}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("MongoDB Atlas Data API. Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function ClickhouseConfig({ set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>{fl("Host")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="https://abc.clickhouse.cloud:8443" value={str('host', '')} onChange={(e) => set('host', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("User")}</label>
        <input placeholder="default" value={str('user', '')} onChange={(e) => set('user', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Password")}</label>
        <input type="password" value={str('password', '')} onChange={(e) => set('password', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Database")}</label>
        <input placeholder="default" value={str('database', '')} onChange={(e) => set('database', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Query (SQL)")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={4} placeholder="SELECT * FROM events LIMIT 10" value={str('query', '')} onChange={(e) => set('query', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Format")}</label>
        <select value={str('format', 'JSON')} onChange={(e) => set('format', e.target.value)}>
          {['JSON', 'JSONEachRow', 'TabSeparated', 'CSV'].map((f) => <option key={f} value={f}>{f}</option>)}
        </select>
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("ClickHouse HTTP interface. A")} <code>{fl("FORMAT")}</code> {fl("clause is appended to SELECTs automatically. Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function MilvusConfig({ config, set, str, num }: ConfigProps) {
  const operation = str('operation', 'search')
  const OPERATIONS = ['search', 'insert', 'query', 'delete']
  const jsonField = (key: string, label: string, placeholder: string, rows = 2) => (
    <div className="field">
      <label>{label}</label>
      <textarea rows={rows} placeholder={placeholder} value={typeof config[key] === 'object' ? JSON.stringify(config[key]) : str(key, '')} onChange={(e) => { try { set(key, JSON.parse(e.target.value)) } catch { set(key, e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
    </div>
  )
  return (
    <>
      <div className="field">
        <label>{fl("Host")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="https://xyz.zillizcloud.com" value={str('host', '')} onChange={(e) => set('host', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Token")}</label>
        <input type="password" placeholder="api-key or user:password" value={str('token', '')} onChange={(e) => set('token', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Collection")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="my_collection" value={str('collection', '')} onChange={(e) => set('collection', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Operation")}</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation === 'search' && (
        <>
          {jsonField('data', 'Query Vectors (JSON array of arrays)', '[[0.1, 0.2, 0.3, …]]')}
          <div className="field">
            <label>{fl("ANNS Field")}</label>
            <input placeholder="vector" value={str('anns_field', '')} onChange={(e) => set('anns_field', e.target.value)} />
          </div>
          <div className="field">
            <label>{fl("Limit")}</label>
            <input type="number" min={1} max={100} value={num('limit', 10)} onChange={(e) => set('limit', Number(e.target.value))} />
          </div>
        </>
      )}
      {operation === 'insert' && jsonField('data', 'Rows (JSON array of objects)', '[{"id":1,"vector":[0.1,0.2]}]', 4)}
      {(operation === 'query' || operation === 'delete') && (
        <div className="field">
          <label>{fl("Filter")} <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input placeholder='id in [1,2,3]' value={str('filter', '')} onChange={(e) => set('filter', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {(operation === 'search' || operation === 'query') && jsonField('output_fields', 'Output Fields (JSON array)', '["id","title"]')}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Milvus / Zilliz REST API v2. Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function MysqlConfig({ set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>{fl("Connection URL")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="mysql://user:pass@host:3306/db" value={str('url', '')} onChange={(e) => set('url', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Query")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={4} placeholder="SELECT * FROM users LIMIT 10" value={str('query', '')} onChange={(e) => set('query', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("SELECT/WITH →")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ rows, count }'}</code>{fl("; DML →")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ rows_affected }'}</code>
      </p>
    </>
  )
}

export function SqlserverConfig({ set, str, num }: ConfigProps) {
  return (
    <>
      <div style={{ display: 'flex', gap: 8 }}>
        <div className="field" style={{ flex: 2 }}>
          <label>{fl("Host")} <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input value={str('host', '')} onChange={(e) => set('host', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
        <div className="field" style={{ flex: 1 }}>
          <label>{fl("Port")}</label>
          <input type="number" placeholder="1433" value={num('port', 1433)} onChange={(e) => set('port', Number(e.target.value))} />
        </div>
      </div>
      <div style={{ display: 'flex', gap: 8 }}>
        <div className="field" style={{ flex: 1 }}>
          <label>{fl("Username")} <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input placeholder="sa" value={str('username', '')} onChange={(e) => set('username', e.target.value)} />
        </div>
        <div className="field" style={{ flex: 1 }}>
          <label>{fl("Password")}</label>
          <input type="password" value={str('password', '')} onChange={(e) => set('password', e.target.value)} />
        </div>
      </div>
      <div className="field">
        <label>{fl("Database")}</label>
        <input value={str('database', '')} onChange={(e) => set('database', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Query")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={4} placeholder="SELECT TOP 10 * FROM dbo.Users" value={str('query', '')} onChange={(e) => set('query', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("SQL Server (TDS, trusts self-signed certs). SELECT →")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ rows, count }'}</code>{fl("; DML →")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ rows_affected }'}</code>
      </p>
    </>
  )
}

export function SnowflakeConfig({ set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>{fl("Account")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="myorg-myacct" value={str('account', '')} onChange={(e) => set('account', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Token")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="OAuth / key-pair JWT bearer" value={str('token', '')} onChange={(e) => set('token', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Token Type")}</label>
        <select value={str('token_type', 'OAUTH')} onChange={(e) => set('token_type', e.target.value)}>
          {['OAUTH', 'KEYPAIR_JWT'].map((t) => <option key={t} value={t}>{t}</option>)}
        </select>
      </div>
      <div className="field">
        <label>{fl("Statement (SQL)")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={3} placeholder="SELECT CURRENT_VERSION()" value={str('statement', '')} onChange={(e) => set('statement', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div style={{ display: 'flex', gap: 8 }}>
        <div className="field" style={{ flex: 1 }}>
          <label>{fl("Warehouse")}</label>
          <input value={str('warehouse', '')} onChange={(e) => set('warehouse', e.target.value)} />
        </div>
        <div className="field" style={{ flex: 1 }}>
          <label>{fl("Database")}</label>
          <input value={str('database', '')} onChange={(e) => set('database', e.target.value)} />
        </div>
        <div className="field" style={{ flex: 1 }}>
          <label>{fl("Schema")}</label>
          <input value={str('schema', '')} onChange={(e) => set('schema', e.target.value)} />
        </div>
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Snowflake SQL API v2. Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function BigqueryConfig({ config, set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>{fl("Project")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="my-gcp-project" value={str('project', '')} onChange={(e) => set('project', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Access Token")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="OAuth2 bearer (bigquery scope)" value={str('access_token', '')} onChange={(e) => set('access_token', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Query (SQL)")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={4} placeholder="SELECT name FROM `proj.ds.table` LIMIT 10" value={str('query', '')} onChange={(e) => set('query', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div style={{ display: 'flex', gap: 8 }}>
        <div className="field" style={{ flex: 1 }}>
          <label>{fl("Max Results")}</label>
          <input type="number" min={1} placeholder="(default)" value={(config['max_results'] as number | undefined) ?? ''} onChange={(e) => set('max_results', e.target.value ? parseInt(e.target.value) : undefined)} />
        </div>
        <div className="field" style={{ flex: 1 }}>
          <label>{fl("Location")}</label>
          <input placeholder="US" value={str('location', '')} onChange={(e) => set('location', e.target.value)} />
        </div>
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("BigQuery jobs.query. Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function NeonConfig({ set, str }: ConfigProps) {
  const operation = str('operation', 'list_projects')
  const OPERATIONS = ['list_projects', 'get_project', 'create_project', 'list_branches']
  const needsProjectId = ['get_project', 'list_branches'].includes(operation)
  return (
    <>
      <div className="field">
        <label>{fl("API Key")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="neon_api_…" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Operation")}</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {needsProjectId && (
        <div className="field">
          <label>{fl("Project ID")} <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input placeholder="proud-river-123456" value={str('project_id', '')} onChange={(e) => set('project_id', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {operation === 'create_project' && (
        <div className="field">
          <label>{fl("Project Name")}</label>
          <input placeholder="my-project" value={str('name', '')} onChange={(e) => set('name', e.target.value)} />
        </div>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Neon serverless Postgres console API. Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}
