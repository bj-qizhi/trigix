// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import type { ConfigProps } from '../types'
import { fl } from '../i18nLabels'

export function TelegramConfig({ config, set, str }: ConfigProps) {
  const operation = str('operation', 'sendMessage')
  const OPERATIONS = ['sendMessage', 'sendPhoto', 'sendDocument', 'sendAudio', 'sendVideo',
                      'editMessageText', 'deleteMessage', 'getUpdates', 'getMe', 'setChatTitle']
  return (
    <>
      <div className="field">
        <label>{fl("Bot Token")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input
          type="password"
          placeholder="123456:ABC-DEF…"
          value={str('bot_token', '')}
          onChange={(e) => set('bot_token', e.target.value)}
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <div className="field">
        <label>{fl("Operation")}</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {['sendMessage', 'sendPhoto', 'sendDocument', 'sendAudio', 'sendVideo', 'editMessageText', 'deleteMessage'].includes(operation) && (
        <div className="field">
          <label>{fl("Chat ID")} <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input
            placeholder="{{input.chat_id}} or -100123456789"
            value={str('chat_id', '')}
            onChange={(e) => set('chat_id', e.target.value)}
          />
        </div>
      )}
      {['sendMessage', 'editMessageText'].includes(operation) && (
        <>
          <div className="field">
            <label>{fl("Text")} <span style={{ color: 'var(--danger)' }}>*</span></label>
            <textarea
              rows={3}
              placeholder="{{input.text}}"
              value={str('text', '')}
              onChange={(e) => set('text', e.target.value)}
              style={{ fontFamily: 'monospace', fontSize: 12 }}
            />
          </div>
          <div className="field">
            <label>{fl("Parse Mode")}</label>
            <select value={str('parse_mode', '')} onChange={(e) => set('parse_mode', e.target.value)}>
              <option value="">{fl("(none)")}</option>
              <option value="Markdown">{fl("Markdown")}</option>
              <option value="MarkdownV2">{fl("MarkdownV2")}</option>
              <option value="HTML">{fl("HTML")}</option>
            </select>
          </div>
        </>
      )}
      <div className="field">
        <label>{fl("Extra Fields (JSON)")}</label>
        <textarea
          rows={2}
          placeholder='{"disable_notification": true}'
          value={typeof config.extra === 'object' ? JSON.stringify(config.extra) : str('extra', '')}
          onChange={(e) => { try { set('extra', JSON.parse(e.target.value)) } catch { set('extra', e.target.value) } }}
          style={{ fontFamily: 'monospace', fontSize: 12 }}
        />
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Calls")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>api.telegram.org/bot&#123;token&#125;/&#123;operation&#125;</code> {fl("with the provided fields.\n        Returns the Telegram API response object.")}
      </p>
    </>
  )
}

export function WhatsappConfig({ set, str }: ConfigProps) {
  const messageType = str('message_type', 'text')
  const MESSAGE_TYPES = ['text', 'template', 'image', 'document', 'audio', 'video']
  return (
    <>
      <div className="field">
        <label>{fl("Access Token")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input type="password" placeholder="EAA…" value={str('access_token', '')} onChange={(e) => set('access_token', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Phone Number ID")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="1234567890" value={str('phone_number_id', '')} onChange={(e) => set('phone_number_id', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("To (Recipient)")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="+1234567890 or {{input.phone}}" value={str('to', '')} onChange={(e) => set('to', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Message Type")}</label>
        <select value={messageType} onChange={(e) => set('message_type', e.target.value)}>
          {MESSAGE_TYPES.map((t) => <option key={t} value={t}>{t}</option>)}
        </select>
      </div>
      {messageType === 'text' && (
        <div className="field">
          <label>{fl("Message Body")}</label>
          <textarea rows={3} placeholder="{{input.message}}" value={str('body', '')} onChange={(e) => set('body', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      {messageType === 'template' && (
        <>
          <div className="field">
            <label>{fl("Template Name")} <span style={{ color: 'var(--danger)' }}>*</span></label>
            <input placeholder="hello_world" value={str('template_name', '')} onChange={(e) => set('template_name', e.target.value)} />
          </div>
          <div className="field">
            <label>{fl("Language Code")}</label>
            <input placeholder="en_US" value={str('language_code', 'en_US')} onChange={(e) => set('language_code', e.target.value)} />
          </div>
        </>
      )}
      {['image', 'document', 'audio', 'video'].includes(messageType) && (
        <div className="field">
          <label>{fl("Media URL")}</label>
          <input placeholder="https://…" value={str('media_url', '')} onChange={(e) => set('media_url', e.target.value)} />
        </div>
      )}
      <div className="field">
        <label>{fl("API Version")}</label>
        <input placeholder="v18.0" value={str('api_version', 'v18.0')} onChange={(e) => set('api_version', e.target.value)} />
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Sends via Meta Graph API. Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function KafkaConfig({ config, set, str }: ConfigProps) {
  return (
    <>
      <div className="field">
        <label>{fl("REST Proxy URL")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="http://localhost:8082" value={str('proxy_url', '')} onChange={(e) => set('proxy_url', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Topic")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="events" value={str('topic', '')} onChange={(e) => set('topic', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Value (JSON or string)")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={3} placeholder='{"event":"signup"}' value={typeof config.value === 'object' ? JSON.stringify(config.value) : str('value', '')} onChange={(e) => { try { set('value', JSON.parse(e.target.value)) } catch { set('value', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div style={{ display: 'flex', gap: 8 }}>
        <div className="field" style={{ flex: 1 }}>
          <label>{fl("Key")}</label>
          <input value={str('key', '')} onChange={(e) => set('key', e.target.value)} />
        </div>
        <div className="field" style={{ flex: 1 }}>
          <label>{fl("Partition")}</label>
          <input type="number" min={0} placeholder="(auto)" value={(config['partition'] as number | undefined) ?? ''} onChange={(e) => set('partition', e.target.value ? parseInt(e.target.value) : undefined)} />
        </div>
      </div>
      <div className="field">
        <label>{fl("API Key / Secret")} <span style={{ color: 'var(--muted)' }}>{fl("(Confluent Cloud)")}</span></label>
        <div style={{ display: 'flex', gap: 8 }}>
          <input placeholder="api_key" value={str('api_key', '')} onChange={(e) => set('api_key', e.target.value)} style={{ flex: 1, fontFamily: 'monospace', fontSize: 12 }} />
          <input type="password" placeholder="api_secret" value={str('api_secret', '')} onChange={(e) => set('api_secret', e.target.value)} style={{ flex: 1, fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("Kafka via the Confluent REST Proxy. Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function RabbitmqConfig({ set, str, num }: ConfigProps) {
  const operation = str('operation', 'publish')
  const OPERATIONS = ['publish', 'get', 'list_queues']
  return (
    <>
      <div className="field">
        <label>{fl("Management API URL")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="http://localhost:15672" value={str('host', '')} onChange={(e) => set('host', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div style={{ display: 'flex', gap: 8 }}>
        <div className="field" style={{ flex: 1 }}>
          <label>{fl("Username")} <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input placeholder="guest" value={str('username', '')} onChange={(e) => set('username', e.target.value)} />
        </div>
        <div className="field" style={{ flex: 1 }}>
          <label>{fl("Password")}</label>
          <input type="password" value={str('password', '')} onChange={(e) => set('password', e.target.value)} />
        </div>
      </div>
      <div className="field">
        <label>{fl("Virtual Host")}</label>
        <input placeholder="/" value={str('vhost', '')} onChange={(e) => set('vhost', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Operation")}</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation === 'publish' && (
        <>
          <div className="field">
            <label>{fl("Exchange")}</label>
            <input placeholder="(default exchange)" value={str('exchange', '')} onChange={(e) => set('exchange', e.target.value)} />
          </div>
          <div className="field">
            <label>{fl("Routing Key")} <span style={{ color: 'var(--muted)' }}>{fl("(queue name for default exchange)")}</span></label>
            <input placeholder="my-queue" value={str('routing_key', '')} onChange={(e) => set('routing_key', e.target.value)} />
          </div>
          <div className="field">
            <label>{fl("Payload")} <span style={{ color: 'var(--danger)' }}>*</span></label>
            <textarea rows={3} value={str('payload', '')} onChange={(e) => set('payload', e.target.value)} />
          </div>
        </>
      )}
      {operation === 'get' && (
        <>
          <div className="field">
            <label>{fl("Queue")} <span style={{ color: 'var(--danger)' }}>*</span></label>
            <input placeholder="my-queue" value={str('queue', '')} onChange={(e) => set('queue', e.target.value)} />
          </div>
          <div className="field">
            <label>{fl("Count")}</label>
            <input type="number" min={1} max={100} value={num('count', 1)} onChange={(e) => set('count', Number(e.target.value))} />
          </div>
        </>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("RabbitMQ Management HTTP API. Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function SqsConfig({ set, str, num }: ConfigProps) {
  const operation = str('operation', 'send')
  const OPERATIONS = ['send', 'receive', 'delete']
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
        <label>{fl("Queue URL")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input placeholder="https://sqs.us-east-1.amazonaws.com/123/my-queue" value={str('queue_url', '')} onChange={(e) => set('queue_url', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Operation")}</label>
        <select value={operation} onChange={(e) => set('operation', e.target.value)}>
          {OPERATIONS.map((op) => <option key={op} value={op}>{op}</option>)}
        </select>
      </div>
      {operation === 'send' && (
        <>
          <div className="field">
            <label>{fl("Message Body")} <span style={{ color: 'var(--danger)' }}>*</span></label>
            <textarea rows={3} value={str('message_body', '')} onChange={(e) => set('message_body', e.target.value)} />
          </div>
          <div className="field">
            <label>{fl("Message Group ID")} <span style={{ color: 'var(--muted)' }}>{fl("(FIFO)")}</span></label>
            <input value={str('message_group_id', '')} onChange={(e) => set('message_group_id', e.target.value)} />
          </div>
        </>
      )}
      {operation === 'receive' && (
        <div className="field">
          <label>{fl("Max Messages")}</label>
          <input type="number" min={1} max={10} value={num('max_messages', 1)} onChange={(e) => set('max_messages', Number(e.target.value))} />
        </div>
      )}
      {operation === 'delete' && (
        <div className="field">
          <label>{fl("Receipt Handle")} <span style={{ color: 'var(--danger)' }}>*</span></label>
          <textarea rows={2} value={str('receipt_handle', '')} onChange={(e) => set('receipt_handle', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("AWS SQS (SigV4-signed). Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function SnsConfig({ set, str }: ConfigProps) {
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
        <label>{fl("Topic ARN")} <span style={{ color: 'var(--muted)' }}>{fl("(or Target ARN / Phone)")}</span></label>
        <input placeholder="arn:aws:sns:us-east-1:123:my-topic" value={str('topic_arn', '')} onChange={(e) => set('topic_arn', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div style={{ display: 'flex', gap: 8 }}>
        <div className="field" style={{ flex: 1 }}>
          <label>{fl("Target ARN")}</label>
          <input value={str('target_arn', '')} onChange={(e) => set('target_arn', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
        <div className="field" style={{ flex: 1 }}>
          <label>{fl("Phone Number")}</label>
          <input placeholder="+15551234567" value={str('phone_number', '')} onChange={(e) => set('phone_number', e.target.value)} />
        </div>
      </div>
      <div className="field">
        <label>{fl("Subject")}</label>
        <input value={str('subject', '')} onChange={(e) => set('subject', e.target.value)} />
      </div>
      <div className="field">
        <label>{fl("Message")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={3} value={str('message', '')} onChange={(e) => set('message', e.target.value)} />
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("AWS SNS Publish (SigV4-signed). Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function FeishuConfig({ config, set, str }: ConfigProps) {
  const msgType = str('msg_type', 'text')
  return (
    <>
      <div className="field">
        <label>{fl("Webhook URL")} <span style={{ color: 'var(--muted)' }}>{fl("(自定义机器人)")}</span></label>
        <input placeholder="https://open.feishu.cn/open-apis/bot/v2/hook/…" value={str('webhook_url', '')} onChange={(e) => set('webhook_url', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        <small style={{ color: 'var(--muted)', fontSize: 10 }}>填了 webhook 用机器人;否则走 App 模式(下方 tenant_access_token + receive_id)。</small>
      </div>
      <div className="field">
        <label>{fl("Message Type")}</label>
        <select value={msgType} onChange={(e) => set('msg_type', e.target.value)}>
          {['text', 'interactive'].map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      {msgType === 'text' && (
        <div className="field">
          <label>{fl("Text")}</label>
          <textarea rows={3} value={str('text', '')} onChange={(e) => set('text', e.target.value)} />
        </div>
      )}
      {msgType === 'interactive' && (
        <div className="field">
          <label>{fl("Card (JSON)")}</label>
          <textarea rows={4} placeholder='{"config":{},"elements":[…]}' value={typeof config.card === 'object' ? JSON.stringify(config.card, null, 2) : str('card', '')} onChange={(e) => { try { set('card', JSON.parse(e.target.value)) } catch { set('card', e.target.value) } }} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        </div>
      )}
      <div className="field">
        <label>{fl("Tenant Access Token")} <span style={{ color: 'var(--muted)' }}>{fl("(App 模式)")}</span></label>
        <input type="password" value={str('tenant_access_token', '')} onChange={(e) => set('tenant_access_token', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div style={{ display: 'flex', gap: 8 }}>
        <div className="field" style={{ flex: 2 }}>
          <label>{fl("Receive ID")}</label>
          <input value={str('receive_id', '')} onChange={(e) => set('receive_id', e.target.value)} />
        </div>
        <div className="field" style={{ flex: 1 }}>
          <label>{fl("ID Type")}</label>
          <select value={str('receive_id_type', 'open_id')} onChange={(e) => set('receive_id_type', e.target.value)}>
            {['open_id', 'user_id', 'union_id', 'email', 'chat_id'].map((t) => <option key={t} value={t}>{t}</option>)}
          </select>
        </div>
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("飞书 / Lark. Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function DingtalkConfig({ set, str }: ConfigProps) {
  const msgType = str('msg_type', 'text')
  return (
    <>
      <div className="field">
        <label>{fl("Access Token")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input value={str('access_token', '')} onChange={(e) => set('access_token', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Secret")} <span style={{ color: 'var(--muted)' }}>{fl("(加签,可选)")}</span></label>
        <input type="password" value={str('secret', '')} onChange={(e) => set('secret', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
      </div>
      <div className="field">
        <label>{fl("Message Type")}</label>
        <select value={msgType} onChange={(e) => set('msg_type', e.target.value)}>
          {['text', 'markdown'].map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      {msgType === 'markdown' && (
        <div className="field">
          <label>{fl("Title")}</label>
          <input placeholder="notice" value={str('title', '')} onChange={(e) => set('title', e.target.value)} />
        </div>
      )}
      <div className="field">
        <label>{fl("Content")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={3} value={str('content', '')} onChange={(e) => set('content', e.target.value)} />
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("钉钉自定义机器人. Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}

export function WecomConfig({ set, str }: ConfigProps) {
  const msgType = str('msg_type', 'text')
  return (
    <>
      <div className="field">
        <label>{fl("Webhook Key")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <input value={str('key', '')} onChange={(e) => set('key', e.target.value)} style={{ fontFamily: 'monospace', fontSize: 12 }} />
        <small style={{ color: 'var(--muted)', fontSize: 10 }}>群机器人 webhook URL 里 key= 后面那段。</small>
      </div>
      <div className="field">
        <label>{fl("Message Type")}</label>
        <select value={msgType} onChange={(e) => set('msg_type', e.target.value)}>
          {['text', 'markdown'].map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
      </div>
      <div className="field">
        <label>{fl("Content")} <span style={{ color: 'var(--danger)' }}>*</span></label>
        <textarea rows={3} value={str('content', '')} onChange={(e) => set('content', e.target.value)} />
      </div>
      <p style={{ fontSize: 11, color: 'var(--muted)', margin: '8px 0 0' }}>
        {fl("企业微信群机器人. Returns")} <code style={{ background: 'var(--panel)', padding: '1px 4px', borderRadius: 3 }}>{'{ status, body }'}</code>
      </p>
    </>
  )
}
