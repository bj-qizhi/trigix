// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import type { ConfigProps } from '../types'
import { fl, labelLocale } from '../i18nLabels'
import { hostAuthFields, fileOpFields, sshKeyFields } from './_helpers'

export function DropboxConfig({ set, str }: ConfigProps) {
  const op = str('operation', 'list_folder')
  const OPS = ['list_folder', 'get_metadata', 'delete', 'create_folder', 'search']
  const needsPath = ['list_folder', 'get_metadata', 'delete', 'create_folder'].includes(op)
  const needsQuery = op === 'search'
  return (
    <>
      <div className="field">
        <label>{fl("Access Token (OAuth2)")}</label>
        <input type="password" placeholder="sl.…" value={str('token', '')} onChange={(e) => set('token', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Operation")}</label>
        <select value={op} onChange={(e) => set('operation', e.target.value)}>
          {OPS.map((o) => <option key={o} value={o}>{o}</option>)}
        </select>
      </div>
      {needsPath && (
        <div className="field">
          <label>{fl("Path (empty string = root for list_folder)")}</label>
          <input placeholder="/Documents/report.pdf" value={str('path', '')} onChange={(e) => set('path', e.target.value)} />
        </div>
      )}
      {needsQuery && (
        <div className="field">
          <label>{fl("Search Query")}</label>
          <input placeholder="quarterly report" value={str('query', '')} onChange={(e) => set('query', e.target.value)} />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body, operation }'}</code>
      </p>
    </>
  )
}

// ── Slice 277: Cloudflare ─────────────────────────────────────────────────────

export function BoxConfig({ config, set, str }: ConfigProps) {
  const method = str('method', 'GET')
  const METHODS = ['GET', 'POST', 'PUT', 'DELETE', 'OPTIONS']
  return (
    <>
      <div className="field">
        <label>{fl("Access Token")}</label>
        <input type="password" placeholder="Box access token" value={str('token', '')} onChange={(e) => set('token', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Method")}</label>
        <select value={method} onChange={(e) => set('method', e.target.value)}>
          {METHODS.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>{fl("Endpoint")}</label>
        <input placeholder="/2.0/files/FILE_ID" value={str('endpoint', '')} onChange={(e) => set('endpoint', e.target.value)} />
      </div>
      {['POST', 'PUT'].includes(method) && (
        <div className="field">
          <label>{fl("Body (JSON)")}</label>
          <textarea rows={4}
            placeholder='{"name": "example.txt"}'
            value={typeof config.body === 'string' ? config.body : JSON.stringify(config.body ?? {}, null, 2)}
            onChange={(e) => { try { set('body', JSON.parse(e.target.value)) } catch { set('body', e.target.value) } }}
            style={{ fontFamily: 'monospace', fontSize: 12 }}
          />
        </div>
      )}
      <p style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>
        {fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

// ── Slice 279: Okta ────────────────────────────────────────────────────────────

export function GoogledriveConfig({ set, str }: ConfigProps) {
  const operation = str('operation', 'list')
  const OPERATIONS = ['list', 'get', 'delete', 'create_folder']
  return (
    <>
      <div className="field">
        <label>{fl("Access Token")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="ya29.…" value={str('access_token', '')} onChange={(e) => set('access_token', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Operation")}</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {['get', 'delete'].includes(operation) && (
        <div className="field">
          <label>{fl("File ID")} <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input placeholder="1BxiMVs0XRA…" value={str('file_id', '')} onChange={(e) => set('file_id', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {operation === 'list' && (
        <>
          <div className="field">
            <label>{fl("Query (Drive search)")}</label>
            <input placeholder="name contains 'report'" value={str('query', '')} onChange={(e) => set('query', e.target.value)} />
          </div>
          <div className="field">
            <label>{fl("Fields")}</label>
            <input placeholder="files(id,name,mimeType)" value={str('fields', '')} onChange={(e) => set('fields', e.target.value)} />
          </div>
        </>
      )}
      {operation === 'create_folder' && (
        <>
          <div className="field">
            <label>{fl("Folder Name")}</label>
            <input placeholder="New Folder" value={str('name', '')} onChange={(e) => set('name', e.target.value)} />
          </div>
          <div className="field">
            <label>{fl("Parent Folder ID")}</label>
            <input placeholder="(root if blank)" value={str('parent_id', '')} onChange={(e) => set('parent_id', e.target.value)} />
          </div>
        </>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Requires OAuth2 access token with Drive scope. Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function Awss3Config({ set, str }: ConfigProps) {
  const operation = str('operation', 'list')
  const OPERATIONS = ['list', 'get_object', 'put_object', 'delete_object']
  return (
    <>
      <div className="field">
        <label>{fl("Access Key ID")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="AKIA…" value={str('access_key_id', '')} onChange={(e) => set('access_key_id', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Secret Access Key")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" value={str('secret_access_key', '')} onChange={(e) => set('secret_access_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Bucket")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="my-bucket" value={str('bucket', '')} onChange={(e) => set('bucket', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Region")}</label>
        <input placeholder="us-east-1" value={str('region', 'us-east-1')} onChange={(e) => set('region', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Operation")}</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation === 'list' && (
        <div className="field">
          <label>{fl("Prefix")}</label>
          <input placeholder="folder/" value={str('prefix', '')} onChange={(e) => set('prefix', e.target.value)} />
        </div>
      )}
      {['get_object', 'put_object', 'delete_object'].includes(operation) && (
        <div className="field">
          <label>{fl("Key (object path)")} <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input placeholder="folder/file.txt" value={str('key', '')} onChange={(e) => set('key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {operation === 'put_object' && (
        <>
          <div className="field">
            <label>{fl("Content Type")}</label>
            <input placeholder="text/plain" value={str('content_type', 'application/octet-stream')} onChange={(e) => set('content_type', e.target.value)} />
          </div>
          <div className="field">
            <label>{fl("Body")}</label>
            <textarea rows={3} placeholder="{{input.content}}" value={str('body', '')} onChange={(e) => set('body', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
        </>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Note: Uses AWS Signature V4. For production, ensure credentials have minimal required IAM permissions. Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function FtpConfig({ config, set, str }: ConfigProps) {
  const operation = str('operation', 'list')
  return (
    <>
      {hostAuthFields(str, set, 21)}
      <div className="field">
        <label style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
          <input type="checkbox" checked={config.secure === true} onChange={(e) => set('secure', e.target.checked)} />
          FTPS (explicit AUTH TLS)
        </label>
      </div>
      {fileOpFields(operation, str, set)}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Plain FTP or FTPS. list →")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ files, count }'}</code>{fl("; download →")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ content_base64, size }'}</code>
      </p>
    </>
  )
}

export function SftpConfig({ set, str }: ConfigProps) {
  const operation = str('operation', 'list')
  return (
    <>
      {hostAuthFields(str, set, 22)}
      {sshKeyFields(str, set)}
      {fileOpFields(operation, str, set)}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("SFTP over SSH (password or private key). Returns file listings / base64 content.")}
      </p>
    </>
  )
}

export function SshConfig({ set, str }: ConfigProps) {
  return (
    <>
      {hostAuthFields(str, set, 22)}
      {sshKeyFields(str, set)}
      <div className="field">
        <label>{fl("Command")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={2} placeholder="uname -a && df -h" value={str('command', '')} onChange={(e) => set('command', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Runs a command over SSH (password or private key). Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ stdout, stderr, exit_status }'}</code>
      </p>
    </>
  )
}

export function ImapConfig({ set, str, num }: ConfigProps) {
  const operation = str('operation', 'list_messages')
  return (
    <>
      {hostAuthFields(str, set, 993)}
      <div className="field">
        <label>{fl("Operation")}</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {['list_messages', 'list_mailboxes'].map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation === 'list_messages' && (
        <div style={{ display: 'flex', gap: 8 }}>
          <div className="field" style={{ flex: 2 }}>
            <label>{fl("Mailbox")}</label>
            <input placeholder="INBOX" value={str('mailbox', '')} onChange={(e) => set('mailbox', e.target.value)} />
          </div>
          <div className="field" style={{ flex: 1 }}>
            <label>{fl("Limit")}</label>
            <input type="number" min={1} max={100} value={num('limit', 10)} onChange={(e) => set('limit', Number(e.target.value))} />
          </div>
        </div>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("IMAP over TLS. Returns recent message envelopes")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ messages, count }'}</code>
      </p>
    </>
  )
}

export function ZipConfig({ config, set, str }: ConfigProps) {
  const operation = str('operation', 'zip')
  return (
    <>
      <div className="field">
        <label>{fl("Operation")}</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {['zip', 'unzip'].map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation === 'zip' && (
        <div className="field">
          <label>{fl("Files (JSON array)")} <span style={{ color: 'var(--danger)' }}>*</span></label>
          <textarea rows={5} placeholder='[{"name":"a.txt","content":"hello"},{"name":"img.png","content":"<base64>","base64":true}]' value={typeof config.files === 'object' ? JSON.stringify(config.files, null, 2) : str('files', '')} onChange={(e) => { try { set('files', JSON.parse(e.target.value)) } catch { set('files', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          <small style={{ color: 'var(--muted)', fontSize: 10 }}>{labelLocale() === 'zh' ? '每项：' : 'Each entry: '}{'{ name, content }'}{labelLocale() === 'zh' ? '；若 content 为 base64 则设 base64:true。' : '; set base64:true if content is base64.'}</small>
        </div>
      )}
      {operation === 'unzip' && (
        <div className="field">
          <label>{fl("Zip (base64)")} <span style={{ color: 'var(--danger)' }}>*</span></label>
          <textarea rows={4} placeholder="UEsDBBQ…" value={str('zip_base64', '')} onChange={(e) => set('zip_base64', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {operation === 'zip'
          ? <>{fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ zip_base64, file_count, size }'}</code></>
          : <>{fl("Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ files: [{name, content_base64, size}] }'}</code></>}
      </p>
    </>
  )
}

export function GcsConfig({ set, str }: ConfigProps) {
  const operation = str('operation', 'list')
  const OPERATIONS = ['list', 'get', 'download', 'upload', 'delete']
  return (
    <>
      <div className="field">
        <label>{fl("Access Token")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="OAuth2 bearer (storage scope)" value={str('access_token', '')} onChange={(e) => set('access_token', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Bucket")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="my-bucket" value={str('bucket', '')} onChange={(e) => set('bucket', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Operation")}</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation === 'list' && (
        <div className="field">
          <label>{fl("Prefix")}</label>
          <input placeholder="folder/" value={str('prefix', '')} onChange={(e) => set('prefix', e.target.value)} />
        </div>
      )}
      {operation !== 'list' && (
        <div className="field">
          <label>{fl("Object")} <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input placeholder="path/to/file.txt" value={str('object', '')} onChange={(e) => set('object', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {operation === 'upload' && (
        <>
          <div className="field">
            <label>{fl("Content")}</label>
            <textarea rows={3} value={str('content', '')} onChange={(e) => set('content', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>{fl("Content Type")}</label>
            <input placeholder="text/plain" value={str('content_type', '')} onChange={(e) => set('content_type', e.target.value)} />
          </div>
        </>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Google Cloud Storage JSON API. Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function AzureBlobConfig({ set, str }: ConfigProps) {
  const operation = str('operation', 'list')
  const OPERATIONS = ['list', 'get', 'put', 'delete']
  return (
    <>
      <div className="field">
        <label>{fl("Account")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="mystorageacct" value={str('account', '')} onChange={(e) => set('account', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Container")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="my-container" value={str('container', '')} onChange={(e) => set('container', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("SAS Token")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="sv=2022-11-02&ss=b&srt=…" value={str('sas_token', '')} onChange={(e) => set('sas_token', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Operation")}</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation !== 'list' && (
        <div className="field">
          <label>{fl("Blob")} <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input placeholder="path/to/blob.txt" value={str('blob', '')} onChange={(e) => set('blob', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {operation === 'put' && (
        <>
          <div className="field">
            <label>{fl("Content")}</label>
            <textarea rows={3} value={str('content', '')} onChange={(e) => set('content', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
          </div>
          <div className="field">
            <label>{fl("Content Type")}</label>
            <input placeholder="application/octet-stream" value={str('content_type', '')} onChange={(e) => set('content_type', e.target.value)} />
          </div>
        </>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Azure Blob Storage REST API (SAS auth). Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function CloudinaryConfig({ set, str }: ConfigProps) {
  const operation = str('operation', 'upload')
  const OPERATIONS = ['upload', 'transform_url', 'get_resource', 'delete']
  return (
    <>
      <div className="field">
        <label>{fl("Cloud Name")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="my-cloud" value={str('cloud_name', '')} onChange={(e) => set('cloud_name', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("API Key")}</label>
        <input placeholder="123456789012345" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("API Secret")}</label>
        <input type="password" value={str('api_secret', '')} onChange={(e) => set('api_secret', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Operation")}</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation === 'upload' && (
        <>
          <div className="field">
            <label>{fl("File URL or Base64")}</label>
            <input placeholder="https://… or data:image/png;base64,…" value={str('file', '')} onChange={(e) => set('file', e.target.value)} />
          </div>
          <div className="field">
            <label>{fl("Public ID")}</label>
            <input placeholder="my-image" value={str('public_id', '')} onChange={(e) => set('public_id', e.target.value)} />
          </div>
        </>
      )}
      {(operation === 'transform_url' || operation === 'get_resource' || operation === 'delete') && (
        <div className="field">
          <label>{fl("Public ID")} <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input placeholder="my-image" value={str('public_id', '')} onChange={(e) => set('public_id', e.target.value)} />
        </div>
      )}
      {operation === 'transform_url' && (
        <div className="field">
          <label>{fl("Transformation")}</label>
          <input placeholder="w_300,h_300,c_fill" value={str('transformation', '')} onChange={(e) => set('transformation', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        Upload, transform, and manage images &amp; videos. Returns <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}
