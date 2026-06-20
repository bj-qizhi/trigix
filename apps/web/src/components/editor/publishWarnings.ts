// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import type { FlowNode, FlowEdge } from '../Canvas'

// Per-node-type required-config fields used by the pre-publish validation.
// [fieldKey, human description] — a missing field yields
// `${name} node "${label}" has no ${desc}`.
interface NodeFieldSpec { name: string; fields: Array<[string, string]> }

const NODE_REQUIRED_FIELDS: Record<string, NodeFieldSpec> = {
  http: { name: 'HTTP', fields: [['url', 'URL']] },
  openai: { name: 'OpenAI', fields: [['api_key', 'API key']] },
  gemini: { name: 'Gemini', fields: [['api_key', 'API key']] },
  claude: { name: 'Claude', fields: [['api_key', 'API key']] },
  slack: { name: 'Slack', fields: [['webhook_url', 'Webhook URL']] },
  email: { name: 'Email', fields: [['to', 'recipient'], ['api_key', 'API key']] },
  github: { name: 'GitHub', fields: [['token', 'token'], ['endpoint', 'endpoint']] },
  webhook: { name: 'Webhook Send', fields: [['url', 'URL']] },
  jira: { name: 'Jira', fields: [['base_url', 'base URL'], ['token', 'API token'], ['endpoint', 'endpoint']] },
  notion: { name: 'Notion', fields: [['token', 'token'], ['endpoint', 'endpoint']] },
  linear: { name: 'Linear', fields: [['token', 'token'], ['query', 'GraphQL query']] },
  airtable: { name: 'Airtable', fields: [['token', 'token'], ['base_id', 'base ID'], ['table', 'table name']] },
  for_each: { name: 'For Each', fields: [['workflow_id', 'target workflow']] },
  discord: { name: 'Discord', fields: [['webhook_url', 'webhook URL'], ['content', 'message content']] },
  teams: { name: 'Teams', fields: [['webhook_url', 'webhook URL'], ['text', 'message text']] },
  sheets: { name: 'Google Sheets', fields: [['token', 'token'], ['spreadsheet_id', 'spreadsheet ID']] },
  xml: { name: 'XML Parse', fields: [['source', 'source']] },
  yaml: { name: 'YAML', fields: [['source', 'source']] },
  twilio: { name: 'Twilio', fields: [['account_sid', 'account SID'], ['auth_token', 'auth token'], ['to', '\'to\' number'], ['from', '\'from\' number']] },
  stripe: { name: 'Stripe', fields: [['api_key', 'API key'], ['endpoint', 'endpoint']] },
  crypto: { name: 'Crypto', fields: [['source', 'source']] },
  hubspot: { name: 'HubSpot', fields: [['token', 'token'], ['endpoint', 'endpoint']] },
  zendesk: { name: 'Zendesk', fields: [['subdomain', 'subdomain'], ['token', 'token'], ['endpoint', 'endpoint']] },
  redis: { name: 'Redis', fields: [['url', 'URL'], ['key', 'key (ping doesn\'t need one)']] },
  elasticsearch: { name: 'Elasticsearch', fields: [['url', 'URL']] },
  pagerduty: { name: 'PagerDuty', fields: [['routing_key', 'routing key'], ['summary', 'summary']] },
  handlebars: { name: 'HB Template', fields: [['template', 'template']] },
  math: { name: 'Math', fields: [['operation', 'operation set']] },
  array_utils: { name: 'Array Utils', fields: [['operation', 'operation set']] },
  shopify: { name: 'Shopify', fields: [['shop', 'shop name'], ['token', 'access token']] },
  datadog: { name: 'Datadog', fields: [['api_key', 'API key'], ['endpoint', 'endpoint']] },
  salesforce: { name: 'Salesforce', fields: [['token', 'access token'], ['instance_url', 'instance URL']] },
  freshdesk: { name: 'Freshdesk', fields: [['api_key', 'API key'], ['domain', 'domain']] },
  mailgun: { name: 'Mailgun', fields: [['api_key', 'API key'], ['domain', 'sending domain'], ['to', 'recipient address']] },
  asana: { name: 'Asana', fields: [['token', 'access token'], ['endpoint', 'endpoint']] },
  servicenow: { name: 'ServiceNow', fields: [['instance', 'instance'], ['username', 'username']] },
  confluence: { name: 'Confluence', fields: [['base_url', 'base URL'], ['endpoint', 'endpoint']] },
  bitbucket: { name: 'Bitbucket', fields: [['username', 'username'], ['app_password', 'app password']] },
  azure_devops: { name: 'Azure DevOps', fields: [['pat', 'PAT'], ['organization', 'organization']] },
  twitch: { name: 'Twitch', fields: [['client_id', 'client ID'], ['access_token', 'access token']] },
  figma: { name: 'Figma', fields: [['token', 'access token'], ['endpoint', 'endpoint']] },
  dropbox: { name: 'Dropbox', fields: [['token', 'access token']] },
  cloudflare: { name: 'Cloudflare', fields: [['api_token', 'API token'], ['endpoint', 'endpoint']] },
  box: { name: 'Box', fields: [['token', 'access token'], ['endpoint', 'endpoint']] },
  okta: { name: 'Okta', fields: [['domain', 'domain'], ['token', 'token']] },
  zoom: { name: 'Zoom', fields: [['token', 'access token'], ['endpoint', 'endpoint']] },
  spotify: { name: 'Spotify', fields: [['token', 'access token'], ['endpoint', 'endpoint']] },
  typeform: { name: 'Typeform', fields: [['token', 'token'], ['endpoint', 'endpoint']] },
  webflow: { name: 'Webflow', fields: [['token', 'token'], ['endpoint', 'endpoint']] },
  intercom: { name: 'Intercom', fields: [['token', 'token'], ['endpoint', 'endpoint']] },
  pipedrive: { name: 'Pipedrive', fields: [['api_token', 'API token'], ['endpoint', 'endpoint']] },
  trello: { name: 'Trello', fields: [['api_key', 'API key'], ['token', 'token'], ['endpoint', 'endpoint']] },
  monday: { name: 'Monday', fields: [['token', 'token'], ['query', 'GraphQL query']] },
  clickup: { name: 'ClickUp', fields: [['token', 'token'], ['endpoint', 'endpoint']] },
  amplitude: { name: 'Amplitude', fields: [['api_key', 'API key'], ['secret_key', 'secret key']] },
  mixpanel: { name: 'Mixpanel', fields: [['project_token', 'project token'], ['api_secret', 'API secret']] },
  segment: { name: 'Segment', fields: [['write_key', 'write key']] },
  sendgrid: { name: 'SendGrid', fields: [['api_key', 'API key'], ['endpoint', 'endpoint']] },
  braintree: { name: 'Braintree', fields: [['merchant_id', 'merchant ID'], ['public_key', 'public key'], ['private_key', 'private key'], ['endpoint', 'endpoint']] },
  paypal: { name: 'PayPal', fields: [['client_id', 'client ID'], ['client_secret', 'client secret'], ['endpoint', 'endpoint']] },
  razorpay: { name: 'Razorpay', fields: [['key_id', 'key ID'], ['key_secret', 'key secret'], ['endpoint', 'endpoint']] },
  firebase: { name: 'Firebase', fields: [['project_id', 'project ID'], ['id_token', 'ID token'], ['endpoint', 'endpoint']] },
  supabase: { name: 'Supabase', fields: [['project_url', 'project URL'], ['api_key', 'API key'], ['endpoint', 'endpoint']] },
  mailchimp: { name: 'Mailchimp', fields: [['api_key', 'API key'], ['endpoint', 'endpoint']] },
  activecampaign: { name: 'ActiveCampaign', fields: [['api_key', 'API key'], ['base_url', 'base URL'], ['endpoint', 'endpoint']] },
  klaviyo: { name: 'Klaviyo', fields: [['api_key', 'API key'], ['endpoint', 'endpoint']] },
  resend: { name: 'Resend', fields: [['api_key', 'API key'], ['endpoint', 'endpoint']] },
  contentful: { name: 'Contentful', fields: [['access_token', 'access token'], ['space_id', 'space ID'], ['endpoint', 'endpoint']] },
  algolia: { name: 'Algolia', fields: [['app_id', 'app ID'], ['api_key', 'API key'], ['endpoint', 'endpoint']] },
  postmark: { name: 'Postmark', fields: [['server_token', 'server token'], ['endpoint', 'endpoint']] },
  vonage: { name: 'Vonage', fields: [['api_key', 'API key'], ['api_secret', 'API secret']] },
  telegram: { name: 'Telegram', fields: [['bot_token', 'bot token'], ['chat_id', 'chat ID']] },
  replicate: { name: 'Replicate', fields: [['api_token', 'API token'], ['version', 'model version']] },
  mistral: { name: 'Mistral', fields: [['api_key', 'API key']] },
  whatsapp: { name: 'WhatsApp', fields: [['access_token', 'access token'], ['phone_number_id', 'phone number ID'], ['to', 'recipient']] },
  googledocs: { name: 'Google Docs', fields: [['access_token', 'access token']] },
  perplexity: { name: 'Perplexity', fields: [['api_key', 'API key']] },
  cohere: { name: 'Cohere', fields: [['api_key', 'API key']] },
  googledrive: { name: 'Google Drive', fields: [['access_token', 'access token']] },
  woocommerce: { name: 'WooCommerce', fields: [['consumer_key', 'consumer key'], ['site_url', 'site URL']] },
  pinecone: { name: 'Pinecone', fields: [['api_key', 'API key'], ['index_host', 'index host']] },
  togetherai: { name: 'Together AI', fields: [['api_key', 'API key']] },
  awss3: { name: 'AWS S3', fields: [['access_key_id', 'access key ID'], ['bucket', 'bucket']] },
  huggingface: { name: 'Hugging Face', fields: [['api_token', 'API token'], ['model', 'model']] },
  groq: { name: 'Groq', fields: [['api_key', 'API key']] },
  openrouter: { name: 'OpenRouter', fields: [['api_key', 'API key']] },
  qdrant: { name: 'Qdrant', fields: [['url', 'server URL'], ['collection', 'collection']] },
  cloudinary: { name: 'Cloudinary', fields: [['cloud_name', 'cloud name']] },
  gcal: { name: 'Google Calendar', fields: [['access_token', 'access token']] },
  docusign: { name: 'DocuSign', fields: [['access_token', 'access token'], ['account_id', 'account ID']] },
  xero: { name: 'Xero', fields: [['access_token', 'access token'], ['tenant_id', 'tenant ID']] },
  calendly: { name: 'Calendly', fields: [['api_key', 'API key']] },
  apify: { name: 'Apify', fields: [['api_token', 'API token']] },
  ganalytics: { name: 'Google Analytics', fields: [['access_token', 'access token'], ['property_id', 'property ID']] },
  neon: { name: 'Neon', fields: [['api_key', 'API key']] },
  copper: { name: 'Copper CRM', fields: [['api_key', 'API key'], ['user_email', 'user email']] },
  database: { name: 'Database', fields: [['query', 'SQL query']] },
  condition: { name: 'Condition', fields: [['field', 'field set']] },
  sub_workflow: { name: 'Sub-Workflow', fields: [['workflow_id', 'target workflow']] },
  graphql: { name: 'GraphQL', fields: [['url', 'endpoint URL']] },
  validate: { name: 'Validate', fields: [['source', 'source expression']] },
  agent: { name: 'Agent', fields: [['prompt_template', 'prompt template']] },
  code: { name: 'Code', fields: [['code', 'script']] },
}

// Pre-publish validation: structural sanity (trigger present, no orphan nodes)
// plus required-config checks driven by NODE_REQUIRED_FIELDS. Pure function so
// it can be reused and tested independently of the editor component.
export function collectPublishWarnings(nodes: FlowNode[], edges: FlowEdge[]): string[] {
  const warnings: string[] = []
  const connectedSources = new Set(edges.map((e) => e.source))
  const connectedTargets = new Set(edges.map((e) => e.target))
  const triggers = nodes.filter((n) => n.data.nodeType === 'trigger')

  // Structural checks
  if (triggers.length === 0) warnings.push('No trigger node — add a Trigger to start the workflow')
  if (triggers.length > 1) warnings.push(`Multiple trigger nodes (${triggers.length}) — only one should exist`)
  if (nodes.length === 1 && triggers.length === 1) warnings.push('Only a trigger node — add more nodes to build a workflow')

  for (const node of nodes) {
    const nt = node.data.nodeType
    const c = node.data.config ?? {}
    const id = node.id
    const label = (c.node_label as string) || id

    // Orphaned node checks (skip trigger, fan-in, catch which can have no incoming)
    if (nt !== 'trigger' && nt !== 'note' && !connectedTargets.has(id)) {
      warnings.push(`Node "${label}" has no incoming connections`)
    }
    if (nt !== 'note' && nt !== 'approval' && !connectedSources.has(id)) {
      warnings.push(`Node "${label}" has no outgoing connections`)
    }

    // Required-config checks (data-driven)
    const spec = nt ? NODE_REQUIRED_FIELDS[nt] : undefined
    if (spec) {
      for (const [field, desc] of spec.fields) {
        if (!c[field]) warnings.push(`${spec.name} node "${label}" has no ${desc}`)
      }
    }
  }
  return warnings
}
