// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react'
import { useAuth } from '../AuthContext'
import * as api from '../api/client'
import { getStoredAuth } from '../auth'
import type { WorkflowRecord, WorkflowVersionRecord, ExecutionRecord, ExecutionSummary, NodeExecutionRecord, NodeType, InputField, EnvSetSummary } from '../types'
import { Canvas, graphFromApi, fromFlowGraph, type FlowNode, type FlowEdge } from './Canvas'
import { NodeConfigPanel } from './NodeConfigPanel'
import { ExecutionPanel } from './ExecutionPanel'
import { TestCasesModal } from './TestCasesModal'
import { CommentsModal } from './CommentsModal'
import { useTheme } from '../useTheme'
import { useLocale } from '../useLocale'

interface Props {
  workflowId: string
  onBack: () => void
  initialInput?: string
}

type Toast = { id: number; message: string; kind: 'success' | 'error' }

const NODE_TYPE_LIST: { type: NodeType; label: string; color: string; icon: string; desc: string; category: string }[] = [
  { type: 'trigger',      label: 'Trigger',      color: 'var(--node-trigger)',      icon: '▶',   desc: 'Starts the workflow. Passes input_json to downstream nodes. Supports manual, webhook, and scheduled runs.',  category: 'Control' },
  { type: 'condition',    label: 'Condition',     color: 'var(--node-condition)',    icon: '◇',   desc: 'Routes to true/false branches by comparing a field. Operators: equals, not_equals, contains, gt, lt, gte, lte, exists.', category: 'Control' },
  { type: 'approval',     label: 'Approval',      color: 'var(--node-approval)',     icon: '✋',  desc: 'Pauses execution until a human approves or rejects. Approve/Reject buttons appear in the execution panel.', category: 'Control' },
  { type: 'assert',       label: 'Assert',        color: 'var(--node-assert)',       icon: '⊘',   desc: 'Fails the execution with a custom message if a condition expression is falsy.', category: 'Control' },
  { type: 'catch',        label: 'Catch',         color: 'var(--node-catch)',        icon: '↻',   desc: 'Receives control when an upstream node fails (connected via an "error" edge). Auto-detects the error.', category: 'Control' },
  { type: 'fan_out',      label: 'Fan-Out',       color: 'var(--node-fan)',          icon: '⇉',   desc: 'Splits execution into parallel branches. All branches run concurrently; use Fan-In to collect results.', category: 'Control' },
  { type: 'fan_in',       label: 'Fan-In',        color: 'var(--node-fan)',          icon: '⇇',   desc: 'Collects outputs from all upstream parallel branches into a single {count, results} object.', category: 'Control' },
  { type: 'delay',        label: 'Delay',         color: 'var(--node-delay)',        icon: '⏱',   desc: 'Waits for N seconds (0–3600) before continuing. Returns {waited_secs}.', category: 'Control' },
  { type: 'sub_workflow', label: 'Sub-Workflow',  color: 'var(--node-sub-workflow)', icon: '⤵',   desc: 'Runs another published workflow as a sub-call. Returns its status and output.', category: 'Control' },
  { type: 'http',         label: 'HTTP',          color: 'var(--node-http)',         icon: '↗',   desc: 'Makes an HTTP request (GET/POST/PUT/PATCH/DELETE). Supports Bearer, OAuth2 auth and custom headers.', category: 'Integration' },
  { type: 'graphql',      label: 'GraphQL',       color: 'var(--node-graphql)',      icon: '◈',   desc: 'Sends a GraphQL query or mutation to an endpoint. Variables support {{...}} templates. Fails on GraphQL errors.', category: 'Integration' },
  { type: 'database',     label: 'Database',      color: 'var(--node-database)',     icon: '⊞',   desc: 'Executes a SQL query against a PostgreSQL database. SELECT returns {rows, count}; DML returns {rows_affected}.', category: 'Integration' },
  { type: 'slack',        label: 'Slack',         color: 'var(--node-slack)',        icon: '#',   desc: 'Sends a message to a Slack channel via an Incoming Webhook URL. Supports {{...}} templates in text.', category: 'Integration' },
  { type: 'email',        label: 'Email',         color: 'var(--node-email)',        icon: '@',   desc: 'Sends an email via the SendGrid API. Configure to, subject, body, and API key (use {{credential.*}}).', category: 'Integration' },
  { type: 'openai',       label: 'OpenAI',        color: 'var(--node-openai)',       icon: '⬡',   desc: 'Calls OpenAI Chat Completions (gpt-4o, gpt-4o-mini, o1). Returns {content, model, usage}.', category: 'AI' },
  { type: 'gemini',       label: 'Gemini',        color: 'var(--node-gemini)',       icon: '✦',   desc: 'Calls Google Gemini (2.0-flash, 1.5-pro, 1.5-flash, thinking). Returns {content, model, usage}.', category: 'AI' },
  { type: 'vertex',       label: 'Vertex AI',     color: 'var(--node-gemini)',       icon: '🔷',  desc: 'Google Vertex AI (Gemini generateContent) via an OAuth2 access token. Config: access_token, project, location, model, prompt_template, system_prompt, max_tokens, temperature. Returns {content, model, usage}.', category: 'AI' },
  { type: 'bedrock',      label: 'AWS Bedrock',   color: 'var(--node-awss3)',        icon: '🧱',  desc: 'AWS Bedrock InvokeModel (SigV4-signed). Config: access_key_id, secret_access_key, region, model_id, body (model-native JSON). Returns {status, body}.', category: 'AI' },
  { type: 'claude',       label: 'Claude',        color: 'var(--node-claude)',        icon: '◆',   desc: 'Calls Anthropic Claude (claude-opus-4-7, claude-sonnet-4-6, claude-haiku-4-5). Returns {content, model, usage}.', category: 'AI' },
  { type: 'agent',        label: 'Agent',         color: 'var(--node-agent)',        icon: '✦',   desc: 'Runs a Python AI agent via the AI Runtime. Configures model, prompt and system instructions.', category: 'AI' },
  { type: 'rag',          label: 'RAG',           color: 'var(--node-rag)',          icon: '⌕',   desc: 'Retrieves the most relevant chunks from a pgvector knowledge base via the AI Runtime. Returns {results}.', category: 'AI' },
  { type: 'rag_ingest',   label: 'RAG Ingest',    color: 'var(--node-rag)',          icon: '⊕',   desc: 'Ingests a document into a pgvector knowledge base via the AI Runtime (chunk + embed + store). Returns {doc_id, chunks}.', category: 'AI' },
  { type: 'custom',       label: 'Custom',        color: 'var(--node-custom)',       icon: '⚙',   desc: 'Runs a community/third-party node served over HTTP (node SDK). Pick a registered custom node.', category: 'AI' },
  { type: 'code',         label: 'Code',          color: 'var(--node-code)',         icon: '{ }', desc: 'Executes a sandboxed Rhai script. Access input and node outputs as maps. Returns the script result.', category: 'Transform' },
  { type: 'transform',    label: 'Transform',     color: 'var(--node-transform)',    icon: '⇄',   desc: 'Renders a JSON template with {{...}} interpolation. The template can be an object, array, or string.', category: 'Transform' },
  { type: 'map',          label: 'Map',           color: 'var(--node-map)',          icon: '⟳',   desc: 'Applies an optional item_template to every element of a JSON array. Returns {count, items}.', category: 'Transform' },
  { type: 'filter',       label: 'Filter',        color: 'var(--node-filter)',       icon: '⊃',   desc: 'Filters a JSON array by field + operator (exists, equals, contains, gt, lt). Returns {count, items}.', category: 'Transform' },
  { type: 'aggregate',    label: 'Aggregate',     color: 'var(--node-aggregate)',    icon: 'Σ',   desc: 'Reduces an array to a scalar via count, sum, avg, min, max, join, first, or last.', category: 'Transform' },
  { type: 'sort',         label: 'Sort',          color: 'var(--node-sort)',         icon: '⇅',   desc: 'Sorts a JSON array by a field, ascending or descending, using string or numeric comparison.', category: 'Transform' },
  { type: 'extract',      label: 'Extract',       color: 'var(--node-extract)',      icon: '⊙',   desc: 'Extracts a single value from a JSON source using a dot-path (e.g. data.users.0.email). Returns {value, found}.', category: 'Transform' },
  { type: 'merge',        label: 'Merge',         color: 'var(--node-merge)',        icon: '⊕',   desc: 'Combines fields from multiple node outputs into one flat object. Each field can have an optional key alias.', category: 'Transform' },
  { type: 'loop',         label: 'Loop',          color: 'var(--node-loop)',         icon: '↻',   desc: 'Iterates over a JSON array, applying an optional template per item. Supports until-path early exit and max_iterations cap.', category: 'Transform' },
  { type: 'validate',     label: 'Validate',      color: 'var(--node-validate)',     icon: '✔',   desc: 'Validates a JSON payload against a simple field schema (required, type checks). Returns {valid, errors[]}. Fails node if invalid.', category: 'Transform' },
  { type: 'note',         label: 'Note',          color: '#b45309',                  icon: '✎',   desc: 'A documentation annotation (sticky note). Does not execute or affect workflow data flow.', category: 'Utility' },
  { type: 'split',        label: 'Split',         color: 'var(--node-split)',        icon: '⊸',   desc: 'Splits a string into an array by a delimiter (default comma). Returns {parts: string[], count}. Trims whitespace by default.', category: 'Transform' },
  { type: 'join',         label: 'Join',          color: 'var(--node-join)',         icon: '⊷',   desc: 'Joins an array into a string by a delimiter (default comma). Optionally extracts a field from each object element. Returns {result, count}.', category: 'Transform' },
  { type: 'switch',       label: 'Switch',        color: 'var(--node-switch)',       icon: '⇢',   desc: 'Evaluates a value expression and routes to a named case branch. Outgoing edges use the case label as condition_label. Returns {value, matched_case, matched}.', category: 'Control' },
  { type: 'random',       label: 'Random',        color: 'var(--node-random)',       icon: '⚂',   desc: 'Generates a random value: UUID, number (with optional min/max), boolean, or a random pick from an items array. Returns {value}.', category: 'Utility' },
  { type: 'dedupe',       label: 'Dedupe',        color: 'var(--node-dedupe)',       icon: '⊟',   desc: 'Removes duplicate elements from a JSON array. Compares by a dot-path field or the entire item. Returns {items, count, removed_count}.', category: 'Transform' },
  { type: 'regex',        label: 'Regex',         color: 'var(--node-regex)',        icon: '.*',  desc: 'Tests a source string against a pattern. Returns {matched, full_match, groups}. Supports case-insensitive "i" flag.', category: 'Transform' },
  { type: 'csv',          label: 'CSV Parse',     color: 'var(--node-csv)',          icon: '⊞',   desc: 'Parses a CSV string into an array of row objects (with headers) or arrays (without). Config: source, delimiter, has_header, trim. Returns {rows, count, headers}.', category: 'Transform' },
  { type: 'rename',       label: 'Rename',        color: 'var(--node-rename)',       icon: '≫',   desc: 'Renames keys in a JSON object. Config: source (object expression), mappings [{from, to}]. Unmapped keys are preserved. Returns the renamed object.', category: 'Transform' },
  { type: 'format',       label: 'Format',        color: 'var(--node-format)',       icon: 'Aa',  desc: 'Formats a string or value: uppercase, lowercase, trim, reverse, length, word_count, to_number, to_bool, replace, pad_start, truncate. Returns {result, operation}.', category: 'Transform' },
  { type: 'github',       label: 'GitHub',        color: 'var(--node-github)',       icon: '⬡',  desc: 'Call GitHub REST API. Config: token (required), endpoint (e.g. /repos/owner/repo/issues), method (GET/POST/PATCH/DELETE), body (optional JSON template). Returns {status, body}.', category: 'Integration' },
  { type: 'webhook',      label: 'Webhook Send',  color: 'var(--node-webhook)',      icon: '↗',  desc: 'Send an HTTP POST to an external webhook URL. Config: url (required), headers (optional object), body_template (optional JSON template). Returns {status, ok}.', category: 'Integration' },
  { type: 'jira',         label: 'Jira',          color: 'var(--node-jira)',         icon: 'J',  desc: 'Call Jira REST API v3 using Basic auth (email + API token). Config: base_url, email, token, endpoint (e.g. /rest/api/3/issue/PROJ-1), method, body. Returns {status, body}.', category: 'Integration' },
  { type: 'notion',       label: 'Notion',        color: 'var(--node-notion)',       icon: 'N',  desc: 'Call Notion REST API using Bearer token. Config: token, endpoint (e.g. /v1/pages), method, body. Returns {status, body}.', category: 'Integration' },
  { type: 'linear',       label: 'Linear',        color: 'var(--node-linear)',       icon: 'L',  desc: 'Query or mutate Linear issues via GraphQL API. Config: token (required), query (GraphQL string), variables (optional JSON). Returns {status, data}.', category: 'Integration' },
  { type: 'airtable',     label: 'Airtable',      color: 'var(--node-airtable)',     icon: 'A',  desc: 'Read or write Airtable records. Config: token, base_id, table, method, record_id (optional), body (for writes), filter_formula (for GET). Returns {status, body}.', category: 'Integration' },
  { type: 'for_each',     label: 'For Each',      color: 'var(--node-for-each)',     icon: '↻',  desc: 'Run a sub-workflow for each item in an array in parallel. Config: items (array expression), workflow_id (resolved to _graph by platform), input_key (default "item"), max_concurrency (default 10). Returns {results, succeeded, failed, total}.', category: 'Control' },
  { type: 'discord',      label: 'Discord',       color: 'var(--node-discord)',      icon: '◈',  desc: 'Send a message to a Discord channel via an incoming webhook. Config: webhook_url, content (message text), username (optional), avatar_url (optional). Returns {ok, content}.', category: 'Integration' },
  { type: 'teams',        label: 'MS Teams',      color: 'var(--node-teams)',        icon: 'T',  desc: 'Send a MessageCard to Microsoft Teams via an incoming webhook. Config: webhook_url, text (required), title (optional), color (hex, default 0078D4). Returns {ok, text}.', category: 'Integration' },
  { type: 'sheets',       label: 'Google Sheets', color: 'var(--node-sheets)',       icon: '⊞',  desc: 'Read or write Google Sheets via Sheets API v4. Config: token (Bearer), spreadsheet_id, range (A1 notation), method (GET/APPEND/UPDATE/CLEAR), values (for writes). Returns {status, body, values}.', category: 'Integration' },
  { type: 'xml',          label: 'XML Parse',     color: 'var(--node-xml)',          icon: '</>',  desc: 'Parse an XML string into a JSON object. Config: source (XML string or {{template}}). Returns {data: object}. Complex XML with namespaces/mixed content may need preprocessing.', category: 'Transform' },
  { type: 'yaml',         label: 'YAML',          color: 'var(--node-yaml)',         icon: '≡',   desc: 'Parse a YAML string to JSON (mode=parse, default) or serialize a JSON value to YAML (mode=serialize). Config: source, mode. Returns {data} or {yaml}.', category: 'Transform' },
  { type: 'twilio',       label: 'Twilio SMS',    color: 'var(--node-twilio)',       icon: '✉',   desc: 'Send an SMS via Twilio REST API. Config: account_sid, auth_token, to, from (E.164 phone numbers), body. Returns {sid, status, to, from}.', category: 'Integration' },
  { type: 'stripe',       label: 'Stripe',        color: 'var(--node-stripe)',       icon: '$',   desc: 'Call the Stripe API v1. Config: api_key (sk_live_/sk_test_), endpoint (e.g. /customers), method (GET/POST/PATCH/DELETE), body (flat object — form-encoded for POST). Returns {status, id, object, body}.', category: 'Integration' },
  { type: 'crypto',       label: 'Crypto',        color: 'var(--node-crypto)',       icon: '⊛',   desc: 'Cryptographic utilities. Operations: sha256, sha512, hmac_sha256 (needs key), base64_encode/decode, hex_encode/decode, random_hex, random_base64. Returns {result, operation}.', category: 'Transform' },
  { type: 'hash',         label: 'Hash / HMAC',   color: 'var(--node-crypto)',       icon: '#️⃣',  desc: 'Compute a SHA-256/384/512 or HMAC digest. Config: operation, input, key (HMAC), encoding (hex/base64/base64url). Returns {hash, algorithm, encoding}.', category: 'Transform' },
  { type: 'jwt',          label: 'JWT',           color: 'var(--node-crypto)',       icon: '🔑',  desc: 'Sign or verify an HMAC JWT (HS256/384/512). Config: operation (sign/verify), algorithm, secret, payload, expires_in_secs, token. Returns {token} or {valid, payload}.', category: 'Transform' },
  { type: 'zip',          label: 'Zip',           color: 'var(--node-transform)',    icon: '🗜️',  desc: 'Create or extract a zip archive (base64). Config: operation (zip/unzip), files [{name, content, base64?}], zip_base64. Returns {zip_base64,…} or {files:[…]}.', category: 'Transform' },
  { type: 'image',        label: 'Image',         color: 'var(--node-transform)',    icon: '🖼️',  desc: 'Resize / convert / inspect an image (base64). Config: operation (resize/convert/metadata), image_base64, width, height, format. Returns {image_base64,…} or {width,height,color}.', category: 'Transform' },
  { type: 'pdf_extract',  label: 'PDF Extract',   color: 'var(--node-transform)',    icon: '📄',  desc: 'Extract text from a base64 PDF. Config: pdf_base64. Returns {text, char_count}.', category: 'Transform' },
  { type: 'ocr',          label: 'OCR',           color: 'var(--node-transform)',    icon: '👁️',  desc: 'OCR an image via the tesseract CLI (must be installed on the executor host). Config: image_base64, lang. Returns {text, lang}.', category: 'Transform' },
  { type: 'hubspot',      label: 'HubSpot',       color: 'var(--node-hubspot)',      icon: 'H',   desc: 'Call HubSpot CRM API (api.hubapi.com). Config: token (Bearer), endpoint (e.g. /crm/v3/objects/contacts), method, body. Returns {status, body}.', category: 'Integration' },
  { type: 'date',         label: 'Date/Time',     color: 'var(--node-date)',         icon: '⏲',   desc: 'Date/time operations: now, parse, format, add, subtract, diff. Config: operation, source (ISO or unix), amount, unit (seconds/minutes/hours/days), format (strftime). Returns {unix, iso, formatted}.', category: 'Transform' },
  { type: 'zendesk',      label: 'Zendesk',       color: 'var(--node-zendesk)',      icon: 'Z',   desc: 'Call Zendesk Support API. Config: subdomain, token (Bearer), endpoint (e.g. /tickets.json), method, body. Returns {status, body}.', category: 'Integration' },
  { type: 'redis',         label: 'Redis Cache',   color: 'var(--node-redis)',        icon: '⊕',   desc: 'Redis key-value cache operations: get/set/del/exists/incr/decr/expire/hget/hset/hgetall/lpush/lpop/keys/ping. Config: url, operation, key, value, field, ttl_secs. Returns {value, operation, key}.', category: 'Integration' },
  { type: 'elasticsearch', label: 'Elasticsearch', color: 'var(--node-elasticsearch)', icon: '🔍',  desc: 'Query or write Elasticsearch/OpenSearch. Config: url, endpoint (e.g. /my-index/_search), method, body, optional api_key or username/password. Returns {status, body, took, hits_total}.', category: 'Integration' },
  { type: 'pagerduty',     label: 'PagerDuty',     color: 'var(--node-pagerduty)',    icon: '🔔',  desc: 'Send events to PagerDuty Events API v2. Config: routing_key, summary, event_action (trigger/acknowledge/resolve), severity, source, dedup_key. Returns {status, message, dedup_key}.', category: 'Integration' },
  { type: 'handlebars',    label: 'HB Template',   color: 'var(--node-handlebars)',   icon: '{}',  desc: 'Render a Handlebars template. Supports {{var}}, {{#if}}, {{#each}}, {{#unless}}, partials. Config: template (string), data (JSON expression used as context). Returns {result}.', category: 'Transform' },
  { type: 'math',          label: 'Math',          color: 'var(--node-math)',         icon: '∑',   desc: 'Numeric operations: add/abs/round/ceil/floor/sqrt/pow/mod/min/max/clamp/log/pct_change/sum/avg/eval. Config: operation, a, b, precision, items (array), expression (for eval). Returns {result, operation}.', category: 'Transform' },
  { type: 'array_utils',   label: 'Array Utils',   color: 'var(--node-array-utils)',  icon: '[]',  desc: 'Array manipulation: chunk/flatten/compact/zip/reverse/shuffle/sample/range/pluck/first_n/last_n. Config: operation, source, size, n, source2, field, start/end/step (for range). Returns {items, count}.', category: 'Transform' },
  { type: 'shopify',       label: 'Shopify',       color: 'var(--node-shopify)',      icon: '🛍',  desc: 'Call Shopify Admin REST API. Config: shop (store subdomain), token (access token), endpoint (e.g. /products.json), method, body, api_version (default 2024-01). Returns {status, body}.', category: 'Integration' },
  { type: 'datadog',       label: 'Datadog',       color: 'var(--node-datadog)',      icon: '📊',  desc: 'Call Datadog API. Config: api_key, endpoint (e.g. /api/v1/validate), method, body, app_key (optional), site (default datadoghq.com). Returns {status, body}.', category: 'Integration' },
  { type: 'salesforce',   label: 'Salesforce',    color: 'var(--node-salesforce)',   icon: '☁',   desc: 'Call Salesforce REST API. Config: token (OAuth access token), instance_url (e.g. https://myorg.salesforce.com), endpoint, method, body. Returns {status, body}.', category: 'Integration' },
  { type: 'freshdesk',    label: 'Freshdesk',     color: 'var(--node-freshdesk)',    icon: '🎫',  desc: 'Call Freshdesk REST API. Config: api_key, domain (e.g. yourco.freshdesk.com), endpoint (e.g. /api/v2/tickets), method, body. Returns {status, body}.', category: 'Integration' },
  { type: 'mailgun',      label: 'Mailgun',       color: 'var(--node-mailgun)',      icon: '✉',   desc: 'Send email via Mailgun. Config: api_key, domain (sending domain), to, from, subject, html or text, region (us/eu). Returns {status, body}.', category: 'Integration' },
  { type: 'asana',        label: 'Asana',         color: 'var(--node-asana)',        icon: '✅',  desc: 'Call Asana API. Config: token (Personal Access Token), endpoint (e.g. /tasks), method, body. Returns {status, body}.', category: 'Integration' },
  { type: 'servicenow',   label: 'ServiceNow',    color: 'var(--node-servicenow)',   icon: '⚙',   desc: 'Call ServiceNow REST API (Table API, etc.). Config: instance (e.g. myco.service-now.com), username, password, endpoint, method, body. Returns {status, body}.', category: 'Integration' },
  { type: 'confluence',   label: 'Confluence',    color: 'var(--node-confluence)',   icon: '📄',  desc: 'Call Atlassian Confluence REST API. Config: base_url, token (Bearer) OR email+api_token (Basic), endpoint, method, body. Returns {status, body}.', category: 'Integration' },
  { type: 'bitbucket',    label: 'Bitbucket',     color: 'var(--node-bitbucket)',    icon: '⑂',   desc: 'Call Bitbucket REST API v2. Config: username, app_password, endpoint (e.g. /repositories/ws/repo), method, body. Returns {status, body}.', category: 'Integration' },
  { type: 'azure_devops', label: 'Azure DevOps',  color: 'var(--node-azure-devops)', icon: '🔷',  desc: 'Call Azure DevOps REST API. Config: pat (Personal Access Token), organization, project (optional), endpoint (e.g. /build/builds), method, body, api_version. Returns {status, body}.', category: 'Integration' },
  { type: 'twitch',       label: 'Twitch',        color: 'var(--node-twitch)',       icon: '🎮',  desc: 'Call Twitch Helix API. Config: client_id, access_token (OAuth), endpoint (e.g. /helix/streams), method, body. Returns {status, body}.', category: 'Integration' },
  { type: 'figma',        label: 'Figma',         color: 'var(--node-figma)',        icon: '✏',   desc: 'Call Figma REST API. Config: token (personal access token), endpoint (e.g. /v1/files/KEY), method. Returns {status, body}.', category: 'Integration' },
  { type: 'dropbox',      label: 'Dropbox',       color: 'var(--node-dropbox)',      icon: '📦',  desc: 'Dropbox file operations: list_folder/get_metadata/delete/create_folder/search. Config: token (OAuth2), operation, path (for most ops), query (for search). Returns {status, body, operation}.', category: 'Integration' },
  { type: 'cloudflare',   label: 'Cloudflare',    color: 'var(--node-cloudflare)',   icon: '☁',   desc: 'Call Cloudflare API v4. Config: api_token, endpoint (e.g. /zones/ZONE_ID/dns_records), method, body. Returns {status, body, success}.', category: 'Integration' },
  { type: 'box',          label: 'Box',           color: 'var(--node-box)',          icon: '📂',  desc: 'Call Box Content API. Config: token (OAuth2), endpoint (e.g. /folders/0/items), method, body. Returns {status, body}.', category: 'Integration' },
  { type: 'okta',         label: 'Okta',          color: 'var(--node-okta)',         icon: '🔑',  desc: 'Call Okta API. Config: domain (e.g. myco.okta.com), token (SSWS API token or Bearer OAuth), token_type (SSWS/Bearer), endpoint (e.g. /api/v1/users), method, body. Returns {status, body}.', category: 'Integration' },
  { type: 'zoom',         label: 'Zoom',          color: 'var(--node-zoom)',         icon: '📹',  desc: 'Call Zoom API v2. Config: token (OAuth2 access token), endpoint (e.g. /users/me/meetings), method, body. Returns {status, body}.', category: 'Integration' },
  { type: 'spotify',      label: 'Spotify',       color: 'var(--node-spotify)',      icon: '🎵',  desc: 'Call Spotify Web API. Config: token (OAuth2 access token), endpoint (e.g. /me/player/currently-playing), method, body. Returns {status, body}.', category: 'Integration' },
  { type: 'typeform',     label: 'Typeform',      color: 'var(--node-typeform)',     icon: '📋',  desc: 'Call Typeform API. Config: token (personal token), endpoint (e.g. /forms/FORM_ID/responses), method, body. Returns {status, body}.', category: 'Integration' },
  { type: 'webflow',      label: 'Webflow',       color: 'var(--node-webflow)',      icon: '🌐',  desc: 'Call Webflow CMS API v2. Config: token (OAuth2/API token), endpoint (e.g. /sites), method, body. Returns {status, body}.', category: 'Integration' },
  { type: 'intercom',     label: 'Intercom',      color: 'var(--node-intercom)',     icon: '💬',  desc: 'Call Intercom API. Config: token (access token), endpoint (e.g. /contacts), method, body. Returns {status, body}.', category: 'Integration' },
  { type: 'pipedrive',    label: 'Pipedrive',     color: 'var(--node-pipedrive)',    icon: '🔗',  desc: 'Call Pipedrive CRM API. Config: api_token, endpoint (e.g. /deals), method, body. Returns {status, body}.', category: 'Integration' },
  { type: 'trello',       label: 'Trello',        color: 'var(--node-trello)',       icon: '📌',  desc: 'Call Trello REST API. Config: api_key, token, endpoint (e.g. /boards/BOARD_ID), method, body. Returns {status, body}.', category: 'Integration' },
  { type: 'monday',       label: 'Monday',        color: 'var(--node-monday)',       icon: '📅',  desc: 'Call Monday.com GraphQL API. Config: token (API token), query (GraphQL string), variables (object). Returns {status, body}.', category: 'Integration' },
  { type: 'clickup',      label: 'ClickUp',       color: 'var(--node-clickup)',      icon: '✅',  desc: 'Call ClickUp API v2. Config: token (personal/OAuth token), endpoint (e.g. /team), method, body. Returns {status, body}.', category: 'Integration' },
  { type: 'amplitude',    label: 'Amplitude',     color: 'var(--node-amplitude)',    icon: '📈',  desc: 'Call Amplitude Analytics API. Config: api_key, secret_key, operation (track/identify/export), events/identification array or start/end dates. Returns {status, body}.', category: 'Integration' },
  { type: 'mixpanel',     label: 'Mixpanel',      color: 'var(--node-mixpanel)',     icon: '📊',  desc: 'Call Mixpanel API. Config: project_token, api_secret, operation (track/import/query), events array or params. Returns {status, body}.', category: 'Integration' },
  { type: 'segment',      label: 'Segment',       color: 'var(--node-segment)',      icon: '🔀',  desc: 'Call Segment Tracking API. Config: write_key, operation (track/identify/page/group/alias/batch), body object. Returns {status, body}.', category: 'Integration' },
  { type: 'sendgrid',     label: 'SendGrid',      color: 'var(--node-sendgrid)',     icon: '📧',  desc: 'Call SendGrid API v3. Config: api_key (SG.xxx), endpoint (e.g. /mail/send), method, body. Returns {status, body}.', category: 'Integration' },
  { type: 'braintree',    label: 'Braintree',     color: 'var(--node-braintree)',    icon: '💳',  desc: 'Call Braintree Gateway API. Config: merchant_id, public_key, private_key, environment (sandbox/production), endpoint, method, body. Returns {status, body}.', category: 'Integration' },
  { type: 'paypal',       label: 'PayPal',        color: 'var(--node-paypal)',       icon: '🅿',  desc: 'Call PayPal REST API. Config: client_id, client_secret, endpoint (e.g. /v2/checkout/orders), method, body, environment (sandbox/live). Optionally provide access_token to skip token exchange. Returns {status, body}.', category: 'Integration' },
  { type: 'razorpay',     label: 'Razorpay',      color: 'var(--node-razorpay)',     icon: '💸',  desc: 'Call Razorpay API v1. Config: key_id, key_secret, endpoint (e.g. /orders), method, body. Returns {status, body}.', category: 'Integration' },
  { type: 'firebase',     label: 'Firebase',      color: 'var(--node-firebase)',     icon: '🔥',  desc: 'Call Firebase REST API. Config: project_id, id_token, service (firestore/rtdb/storage), endpoint, method, body. Returns {status, body}.', category: 'Integration' },
  { type: 'supabase',     label: 'Supabase',      color: 'var(--node-supabase)',     icon: '⚡',  desc: 'Call Supabase PostgREST or Functions API. Config: project_url (https://xyz.supabase.co), api_key (anon or service_role), endpoint (e.g. /rest/v1/users), method, body, prefer. Returns {status, body}.', category: 'Integration' },
  { type: 'mailchimp',    label: 'Mailchimp',     color: 'var(--node-mailchimp)',    icon: '🐒',  desc: 'Call Mailchimp Marketing API v3. Config: api_key (format: key-us1), server (e.g. us1, auto-extracted from key), endpoint (e.g. /lists), method, body. Returns {status, body}.', category: 'Integration' },
  { type: 'activecampaign', label: 'ActiveCampaign', color: 'var(--node-activecampaign)', icon: '📣', desc: 'Call ActiveCampaign API v3. Config: api_key, base_url (https://ACCOUNT.api-us1.com), endpoint (e.g. /contacts), method, body. Returns {status, body}.', category: 'Integration' },
  { type: 'klaviyo',      label: 'Klaviyo',       color: 'var(--node-klaviyo)',      icon: '📩',  desc: 'Call Klaviyo API. Config: api_key (private key), endpoint (e.g. /profiles), method, body. Returns {status, body}.', category: 'Integration' },
  { type: 'resend',       label: 'Resend',        color: 'var(--node-resend)',       icon: '✉',  desc: 'Call Resend email API. Config: api_key (re_xxx), endpoint (e.g. /emails), method, body. Returns {status, body}.', category: 'Integration' },
  { type: 'contentful',   label: 'Contentful',    color: 'var(--node-contentful)',   icon: '📄',  desc: 'Call Contentful API. Config: access_token, space_id, api_type (delivery/preview/management), endpoint, method, body. Returns {status, body}.', category: 'Integration' },
  { type: 'algolia',      label: 'Algolia',       color: 'var(--node-algolia)',      icon: '🔍',  desc: 'Call Algolia Search API. Config: app_id, api_key, endpoint (e.g. /1/indexes/INDEX/query), method, body. Returns {status, body}.', category: 'Integration' },
  { type: 'postmark',     label: 'Postmark',      color: 'var(--node-postmark)',     icon: '📮',  desc: 'Call Postmark API. Config: server_token, endpoint (e.g. /email), method, body. Returns {status, body}.', category: 'Integration' },
  { type: 'vonage',       label: 'Vonage',        color: 'var(--node-vonage)',       icon: '📱',  desc: 'Call Vonage/Nexmo API. Config: api_key, api_secret, operation (sms/voice/verify), to/from/text (SMS) or endpoint/body (voice/verify). Returns {status, body}.', category: 'Integration' },
  { type: 'telegram',     label: 'Telegram',      color: 'var(--node-telegram)',     icon: '✈',   desc: 'Send Telegram Bot API requests. Config: bot_token, operation (e.g. sendMessage), chat_id, text, parse_mode, extra (additional fields). Returns Telegram API response.', category: 'Integration' },
  { type: 'replicate',    label: 'Replicate',     color: 'var(--node-replicate)',    icon: '🔁',  desc: 'Run AI models via Replicate. Config: api_token, operation (run/get_prediction/list_models), version (model version ID), input (JSON). Returns {status, body}.', category: 'AI' },
  { type: 'mistral',      label: 'Mistral',       color: 'var(--node-mistral)',      icon: '🌬',  desc: 'Call Mistral AI API. Config: api_key, operation (chat/embeddings/list_models), model, messages or prompt, temperature, max_tokens. Returns {status, body}.', category: 'AI' },
  { type: 'whatsapp',     label: 'WhatsApp',      color: 'var(--node-whatsapp)',     icon: '💬',  desc: 'Send WhatsApp Business messages via Meta API. Config: access_token, phone_number_id, to, message_type (text/template/image), body or template_name. Returns {status, body}.', category: 'Integration' },
  { type: 'googledocs',   label: 'Google Docs',   color: 'var(--node-googledocs)',   icon: '📝',  desc: 'Read and write Google Docs. Config: access_token, operation (get/create/batch_update), document_id, title (create), requests (batch_update). Returns {status, body}.', category: 'Integration' },
  { type: 'perplexity',   label: 'Perplexity',    color: 'var(--node-perplexity)',   icon: '🔎',  desc: 'AI-powered search via Perplexity API. Config: api_key, model, prompt or messages, temperature, max_tokens, return_citations. Returns {status, body}.', category: 'AI' },
  { type: 'cohere',       label: 'Cohere',         color: 'var(--node-cohere)',       icon: '🧠',  desc: 'Call Cohere NLP API. Config: api_key, operation (chat/embed/classify/rerank), message/texts/inputs/query per operation. Returns {status, body}.', category: 'AI' },
  { type: 'googledrive',  label: 'Google Drive',   color: 'var(--node-googledrive)',  icon: '📁',  desc: 'Manage Google Drive files. Config: access_token, operation (list/get/delete/create_folder), file_id, query, name, parent_id. Returns {status, body}.', category: 'Integration' },
  { type: 'woocommerce',  label: 'WooCommerce',    color: 'var(--node-woocommerce)',  icon: '🛒',  desc: 'Call WooCommerce REST API. Config: consumer_key, consumer_secret, site_url, endpoint (e.g. /wp-json/wc/v3/products), method, body. Returns {status, body}.', category: 'Integration' },
  { type: 'pinecone',     label: 'Pinecone',       color: 'var(--node-pinecone)',     icon: '🌲',  desc: 'Vector database operations via Pinecone. Config: api_key, index_host, operation (query/upsert/delete/fetch), vector/vectors/ids, top_k, namespace. Returns {status, body}.', category: 'AI' },
  { type: 'togetherai',   label: 'Together AI',    color: 'var(--node-togetherai)',   icon: '🤝',  desc: 'Run open-source LLMs via Together AI. Config: api_key, operation (chat/completions/embeddings), model, prompt or messages, temperature. Returns {status, body}.', category: 'AI' },
  { type: 'awss3',        label: 'AWS S3',         color: 'var(--node-awss3)',        icon: '🪣',  desc: 'Interact with AWS S3. Config: access_key_id, secret_access_key, bucket, region, operation (list/get_object/put_object/delete_object), key, body. Returns {status, body}.', category: 'Integration' },
  { type: 'gcs',          label: 'Cloud Storage',  color: 'var(--node-awss3)',        icon: '🪣',  desc: 'Google Cloud Storage (JSON API). Config: access_token (OAuth2), bucket, operation (list/get/download/upload/delete), object, prefix, content, content_type. Returns {status, body}.', category: 'Integration' },
  { type: 'azure_blob',   label: 'Azure Blob',     color: 'var(--node-awss3)',        icon: '📦',  desc: 'Azure Blob Storage (REST + SAS). Config: account, container, sas_token, operation (list/get/put/delete), blob, content, content_type. Returns {status, body}.', category: 'Integration' },
  { type: 'sqs',          label: 'AWS SQS',        color: 'var(--node-awss3)',        icon: '📨',  desc: 'AWS SQS (SigV4-signed). Config: access_key_id, secret_access_key, region, queue_url, operation (send/receive/delete), message_body, message_group_id, max_messages, receipt_handle. Returns {status, body}.', category: 'Integration' },
  { type: 'sns',          label: 'AWS SNS',        color: 'var(--node-awss3)',        icon: '📢',  desc: 'AWS SNS Publish (SigV4-signed). Config: access_key_id, secret_access_key, region, topic_arn/target_arn/phone_number, subject, message. Returns {status, body}.', category: 'Integration' },
  { type: 'kafka',        label: 'Kafka',          color: 'var(--node-redis)',        icon: '🟧',  desc: 'Produce to a Kafka topic via the Confluent REST Proxy. Config: proxy_url, topic, value, key, partition, api_key, api_secret. Returns {status, body}.', category: 'Integration' },
  { type: 'rabbitmq',     label: 'RabbitMQ',       color: 'var(--node-redis)',        icon: '🐰',  desc: 'RabbitMQ Management HTTP API. Config: host, username, password, vhost, operation (publish/get/list_queues), exchange, routing_key, payload, queue, count. Returns {status, body}.', category: 'Integration' },
  { type: 'huggingface',  label: 'Hugging Face',   color: 'var(--node-huggingface)',  icon: '🤗',  desc: 'Call Hugging Face Inference API. Config: api_token, model (e.g. gpt2), operation (inference/model_info/list_models), inputs, parameters. Returns {status, body}.', category: 'AI' },
  { type: 'groq',         label: 'Groq',           color: 'var(--node-groq)',         icon: '⚡',  desc: 'Run fast LLM inference via Groq. Config: api_key, operation (chat/models), model (e.g. llama3-8b-8192), messages, temperature, max_tokens. Returns {status, body}.', category: 'AI' },
  { type: 'grok',         label: 'xAI Grok',       color: 'var(--node-claude)',       icon: '🤖',  desc: 'Call xAI Grok (OpenAI-compatible). Config: api_key, model (grok-2-latest), prompt_template, system_prompt, max_tokens, temperature. Returns {content, model, usage}.', category: 'AI' },
  { type: 'ollama',       label: 'Ollama',         color: 'var(--node-openai)',       icon: '🦙',  desc: 'Call a self-hosted Ollama server (OpenAI-compatible). Config: base_url, model (llama3.2), api_key (optional), prompt_template, system_prompt, max_tokens, temperature. Returns {content, model, usage}.', category: 'AI' },
  { type: 'azure_openai', label: 'Azure OpenAI',   color: 'var(--node-openai)',       icon: '☁️',  desc: 'Call Azure OpenAI deployments. Config: endpoint, deployment, api_version (2024-02-01), api_key, prompt_template, system_prompt, max_tokens, temperature. Returns {content, model, usage}.', category: 'AI' },
  { type: 'openrouter',   label: 'OpenRouter',     color: 'var(--node-openrouter)',   icon: '🔀',  desc: 'Access 100+ LLMs via OpenRouter. Config: api_key, operation (chat/models), model (e.g. openai/gpt-4o), messages, temperature, max_tokens. Returns {status, body}.', category: 'AI' },
  { type: 'qdrant',       label: 'Qdrant',         color: 'var(--node-qdrant)',       icon: '🎯',  desc: 'Vector search with Qdrant. Config: url, api_key, collection, operation (search/upsert/delete/get_collection/create_collection), vector, top, points. Returns {status, body}.', category: 'AI' },
  { type: 'weaviate',     label: 'Weaviate',       color: 'var(--node-qdrant)',       icon: '🧬',  desc: 'Weaviate vector store. Config: host, api_key, operation (query/create_object/get_object/delete_object), query (GraphQL), class, properties, vector, id. Returns {status, body}.', category: 'AI' },
  { type: 'chroma',       label: 'Chroma',         color: 'var(--node-qdrant)',       icon: '🌈',  desc: 'Chroma vector store. Config: host, api_key, operation (query/add/delete/get_collection), collection, collection_id, query_embeddings, n_results, ids, embeddings, documents. Returns {status, body}.', category: 'AI' },
  { type: 'milvus',       label: 'Milvus',         color: 'var(--node-qdrant)',       icon: '🐦',  desc: 'Milvus / Zilliz vector store (REST API v2). Config: host, token, collection, operation (search/insert/query/delete), data, anns_field, filter, output_fields, limit. Returns {status, body}.', category: 'AI' },
  { type: 'mongodb',      label: 'MongoDB',        color: 'var(--node-database)',     icon: '🍃',  desc: 'MongoDB via the Atlas Data API. Config: data_api_url, api_key, data_source, database, collection, operation (find/findOne/insert*/update*/delete*/aggregate), filter, document(s), update, pipeline, limit, sort. Returns {status, body}.', category: 'Integration' },
  { type: 'clickhouse',   label: 'ClickHouse',     color: 'var(--node-database)',     icon: '🗄️',  desc: 'Run SQL against ClickHouse over HTTP. Config: host, user, password, database, query, format (JSON/JSONEachRow/…). FORMAT is appended to SELECTs. Returns {status, body}.', category: 'Integration' },
  { type: 'cloudinary',   label: 'Cloudinary',     color: 'var(--node-cloudinary)',   icon: '☁',  desc: 'Media management via Cloudinary. Config: cloud_name, api_key, api_secret, operation (upload/transform_url/get_resource/delete), file, public_id, transformation. Returns {status, body}.', category: 'Integration' },
  { type: 'gcal',         label: 'Google Calendar',color: 'var(--node-gcal)',         icon: '📅',  desc: 'Manage Google Calendar events. Config: access_token, calendar_id (default: primary), operation (list_calendars/list_events/get_event/create_event/delete_event), event_id, summary, start_time, end_time. Returns {status, body}.', category: 'Integration' },
  { type: 'docusign',     label: 'DocuSign',       color: 'var(--node-docusign)',     icon: '✍',  desc: 'E-signature workflows via DocuSign. Config: access_token, account_id, base_url, operation (list_envelopes/get_envelope/create_envelope/void_envelope), envelope_id, body. Returns {status, body}.', category: 'Integration' },
  { type: 'xero',         label: 'Xero',           color: 'var(--node-xero)',         icon: '💹',  desc: 'Accounting automation via Xero. Config: access_token, tenant_id, endpoint (e.g. /Contacts), method, body. Returns {status, body}.', category: 'Integration' },
  { type: 'calendly',     label: 'Calendly',       color: 'var(--node-calendly)',     icon: '🗓',  desc: 'Scheduling automation via Calendly. Config: api_key, operation (get_current_user/list_event_types/list_scheduled_events/get_scheduled_event/cancel_event), user_uri, event_uuid. Returns {status, body}.', category: 'Integration' },
  { type: 'apify',        label: 'Apify',          color: 'var(--node-apify)',        icon: '🕷',  desc: 'Web scraping via Apify. Config: api_token, operation (run_actor/get_run/get_dataset_items/list_actors), actor_id, run_id, dataset_id, input. Returns {status, body}.', category: 'Integration' },
  { type: 'ganalytics',   label: 'Google Analytics',color: 'var(--node-ganalytics)', icon: '📊',  desc: 'Query Google Analytics 4. Config: access_token, property_id, operation (run_report/run_realtime_report/get_metadata), date_ranges, dimensions, metrics. Returns {status, body}.', category: 'Integration' },
  { type: 'neon',         label: 'Neon',           color: 'var(--node-neon)',         icon: '🌀',  desc: 'Manage Neon serverless Postgres. Config: api_key, operation (list_projects/get_project/create_project/list_branches), project_id, name. Returns {status, body}.', category: 'Integration' },
  { type: 'copper',       label: 'Copper CRM',     color: 'var(--node-copper)',       icon: '🔶',  desc: 'CRM automation via Copper. Config: api_key, user_email, resource (people/leads/opportunities/companies), operation (list/get/create/update/delete), record_id, body, filter. Returns {status, body}.', category: 'Integration' },
]

const PALETTE_CATEGORY_ORDER = ['Control', 'Integration', 'AI', 'Transform', 'Utility']

const NODE_ZH: Record<string, { labelZh: string; descZh: string }> = {
  // Control
  trigger:      { labelZh: '触发器',      descZh: '启动工作流，将 input_json 传递给下游节点，支持手动、Webhook 和定时运行。' },
  condition:    { labelZh: '条件判断',    descZh: '对字段或模板表达式求值，路由到 true/false 分支，支持等于、包含、大小比较、正则。' },
  approval:     { labelZh: '人工审批',    descZh: '暂停执行，等待人工批准或拒绝；执行面板会显示批准/拒绝按钮。' },
  assert:       { labelZh: '断言',        descZh: '若条件表达式为假，则以自定义消息终止执行。' },
  catch:        { labelZh: '捕获错误',    descZh: '当上游节点通过 error 边失败时接管控制，自动检测错误信息。' },
  fan_out:      { labelZh: '并行分发',    descZh: '将执行拆分为多条并行分支，同时运行；用 Fan-In 汇总结果。' },
  fan_in:       { labelZh: '并行汇总',    descZh: '将所有上游并行分支的输出合并为 {count, results} 对象。' },
  delay:        { labelZh: '延时',        descZh: '等待 N 秒（0–3600）后继续，返回 {waited_secs}。' },
  sub_workflow: { labelZh: '子工作流',    descZh: '将另一个已发布的工作流作为子调用运行，返回其状态和输出。' },
  switch:       { labelZh: '多路选择',    descZh: '对值表达式求值并路由到指定命名分支，返回 {value, matched_case, matched}。' },
  for_each:     { labelZh: '遍历执行',    descZh: '对数组每个元素并行运行子工作流，返回 {results, succeeded, failed, total}。' },
  // Integration
  http:         { labelZh: 'HTTP 请求',   descZh: '发送 HTTP 请求（GET/POST/PUT/PATCH/DELETE），支持 Bearer、OAuth2 认证和自定义请求头。' },
  graphql:      { labelZh: 'GraphQL',     descZh: '向端点发送 GraphQL 查询或变更，变量支持 {{...}} 模板。' },
  database:     { labelZh: '数据库',      descZh: '对 PostgreSQL 执行 SQL 查询；SELECT 返回 {rows, count}，DML 返回 {rows_affected}。' },
  slack:        { labelZh: 'Slack',       descZh: '通过 Incoming Webhook 向 Slack 频道发送消息，文本支持 {{...}} 模板。' },
  email:        { labelZh: '发送邮件',    descZh: '通过 SendGrid API 发送邮件，配置收件人、主题、正文和 API 密钥。' },
  github:       { labelZh: 'GitHub',      descZh: '调用 GitHub REST API，配置 token、端点、请求方法和请求体。' },
  webhook:      { labelZh: '发送 Webhook', descZh: '向外部 Webhook URL 发送 HTTP POST，支持自定义请求头和请求体模板。' },
  jira:         { labelZh: 'Jira',        descZh: '使用 Basic 认证（邮箱 + API token）调用 Jira REST API v3。' },
  notion:       { labelZh: 'Notion',      descZh: '使用 Bearer token 调用 Notion REST API，配置端点和请求体。' },
  linear:       { labelZh: 'Linear',      descZh: '通过 GraphQL API 查询或变更 Linear 工单，配置 token 和查询语句。' },
  airtable:     { labelZh: 'Airtable',    descZh: '读写 Airtable 记录，配置 token、base_id、表名和操作方式。' },
  discord:      { labelZh: 'Discord',     descZh: '通过 Incoming Webhook 向 Discord 频道发送消息，支持自定义用户名。' },
  teams:        { labelZh: 'MS Teams',    descZh: '通过 Incoming Webhook 向 Microsoft Teams 发送 MessageCard。' },
  sheets:       { labelZh: 'Google 表格', descZh: '通过 Sheets API v4 读写 Google 表格，支持 GET/APPEND/UPDATE/CLEAR。' },
  twilio:       { labelZh: 'Twilio 短信', descZh: '通过 Twilio REST API 发送短信，配置 account_sid、auth_token 和收发号码。' },
  stripe:       { labelZh: 'Stripe',      descZh: '调用 Stripe API v1，配置 api_key、端点和请求体。' },
  hubspot:      { labelZh: 'HubSpot',     descZh: '调用 HubSpot CRM API，配置 Bearer token、端点和请求体。' },
  zendesk:      { labelZh: 'Zendesk',     descZh: '调用 Zendesk 支持 API，配置子域名、Bearer token 和端点。' },
  redis:        { labelZh: 'Redis 缓存',  descZh: 'Redis 键值缓存操作，支持 get/set/del/incr/hget/hset/lpush 等。' },
  elasticsearch:{ labelZh: 'Elasticsearch', descZh: '查询或写入 Elasticsearch/OpenSearch，支持 API 密钥或用户名密码认证。' },
  pagerduty:    { labelZh: 'PagerDuty',   descZh: '向 PagerDuty Events API v2 发送事件，配置 routing_key、摘要和操作。' },
  shopify:      { labelZh: 'Shopify',     descZh: '调用 Shopify Admin REST API，配置店铺子域名、access token 和端点。' },
  datadog:      { labelZh: 'Datadog',     descZh: '调用 Datadog API，配置 api_key、端点和请求体。' },
  salesforce:   { labelZh: 'Salesforce',  descZh: '调用 Salesforce REST API，配置 OAuth token 和 instance_url。' },
  freshdesk:    { labelZh: 'Freshdesk',   descZh: '调用 Freshdesk REST API，配置 api_key、域名和端点。' },
  mailgun:      { labelZh: 'Mailgun',     descZh: '通过 Mailgun 发送邮件，配置 api_key、发送域名、收发人和内容。' },
  asana:        { labelZh: 'Asana',       descZh: '调用 Asana API，配置个人访问 token 和端点。' },
  servicenow:   { labelZh: 'ServiceNow',  descZh: '调用 ServiceNow REST API（Table API 等），配置实例地址和凭据。' },
  confluence:   { labelZh: 'Confluence',  descZh: '调用 Atlassian Confluence REST API，支持 Bearer 或 Basic 认证。' },
  bitbucket:    { labelZh: 'Bitbucket',   descZh: '调用 Bitbucket REST API v2，配置用户名、App Password 和端点。' },
  azure_devops: { labelZh: 'Azure DevOps', descZh: '调用 Azure DevOps REST API，配置 PAT、organization 和端点。' },
  twitch:       { labelZh: 'Twitch',      descZh: '调用 Twitch Helix API，配置 client_id 和 OAuth access_token。' },
  figma:        { labelZh: 'Figma',       descZh: '调用 Figma REST API，配置个人访问 token 和端点。' },
  dropbox:      { labelZh: 'Dropbox',     descZh: 'Dropbox 文件操作，支持列目录、获取元数据、删除和搜索。' },
  cloudflare:   { labelZh: 'Cloudflare',  descZh: '调用 Cloudflare API v4，配置 api_token 和端点。' },
  box:          { labelZh: 'Box',         descZh: '调用 Box Content API，配置 OAuth2 token 和端点。' },
  okta:         { labelZh: 'Okta',        descZh: '调用 Okta API，配置域名、SSWS/Bearer token 和端点。' },
  zoom:         { labelZh: 'Zoom',        descZh: '调用 Zoom API v2，配置 OAuth2 access_token 和端点。' },
  spotify:      { labelZh: 'Spotify',     descZh: '调用 Spotify Web API，配置 OAuth2 access_token 和端点。' },
  typeform:     { labelZh: 'Typeform',    descZh: '调用 Typeform API，配置个人 token 和端点。' },
  webflow:      { labelZh: 'Webflow',     descZh: '调用 Webflow CMS API v2，配置 token 和端点。' },
  intercom:     { labelZh: 'Intercom',    descZh: '调用 Intercom API，配置 access_token 和端点。' },
  pipedrive:    { labelZh: 'Pipedrive',   descZh: '调用 Pipedrive CRM API，配置 api_token 和端点。' },
  trello:       { labelZh: 'Trello',      descZh: '调用 Trello REST API，配置 api_key、token 和端点。' },
  monday:       { labelZh: 'Monday',      descZh: '调用 Monday.com GraphQL API，配置 API token 和查询语句。' },
  clickup:      { labelZh: 'ClickUp',     descZh: '调用 ClickUp API v2，配置 token 和端点。' },
  amplitude:    { labelZh: 'Amplitude',   descZh: '调用 Amplitude Analytics API，支持 track/identify/export 操作。' },
  mixpanel:     { labelZh: 'Mixpanel',    descZh: '调用 Mixpanel API，支持 track/import/query 操作。' },
  segment:      { labelZh: 'Segment',     descZh: '调用 Segment Tracking API，支持 track/identify/page/group 等操作。' },
  sendgrid:     { labelZh: 'SendGrid',    descZh: '调用 SendGrid API v3，配置 api_key 和端点。' },
  braintree:    { labelZh: 'Braintree',   descZh: '调用 Braintree Gateway API，配置 merchant_id 和密钥对。' },
  paypal:       { labelZh: 'PayPal',      descZh: '调用 PayPal REST API，配置 client_id、client_secret 和端点。' },
  razorpay:     { labelZh: 'Razorpay',    descZh: '调用 Razorpay API v1，配置 key_id、key_secret 和端点。' },
  firebase:     { labelZh: 'Firebase',    descZh: '调用 Firebase REST API，支持 Firestore、RTDB 和 Storage。' },
  supabase:     { labelZh: 'Supabase',    descZh: '调用 Supabase PostgREST 或 Functions API，配置 project_url 和 api_key。' },
  mailchimp:    { labelZh: 'Mailchimp',   descZh: '调用 Mailchimp Marketing API v3，配置 api_key 和端点。' },
  activecampaign:{ labelZh: 'ActiveCampaign', descZh: '调用 ActiveCampaign API v3，配置 api_key 和 base_url。' },
  klaviyo:      { labelZh: 'Klaviyo',     descZh: '调用 Klaviyo API，配置私钥和端点。' },
  resend:       { labelZh: 'Resend',      descZh: '调用 Resend 邮件 API，配置 api_key 和端点。' },
  contentful:   { labelZh: 'Contentful',  descZh: '调用 Contentful API，支持 delivery/preview/management 类型。' },
  algolia:      { labelZh: 'Algolia',     descZh: '调用 Algolia 搜索 API，配置 app_id、api_key 和端点。' },
  postmark:     { labelZh: 'Postmark',    descZh: '调用 Postmark API，配置 server_token 和端点。' },
  vonage:       { labelZh: 'Vonage',      descZh: '调用 Vonage/Nexmo API，支持 SMS、语音和验证码操作。' },
  telegram:     { labelZh: 'Telegram',    descZh: '发送 Telegram Bot API 请求，配置 bot_token、操作和 chat_id。' },
  whatsapp:     { labelZh: 'WhatsApp',    descZh: '通过 Meta API 发送 WhatsApp Business 消息，支持文本、模板和图片类型。' },
  googledocs:   { labelZh: 'Google Docs', descZh: '读写 Google 文档，支持获取、创建和批量更新操作。' },
  googledrive:  { labelZh: 'Google Drive', descZh: '管理 Google Drive 文件，支持列出、获取、删除和创建文件夹。' },
  woocommerce:  { labelZh: 'WooCommerce', descZh: '调用 WooCommerce REST API，配置消费者密钥和站点 URL。' },
  awss3:        { labelZh: 'AWS S3',      descZh: '与 AWS S3 交互，支持列出、上传、下载和删除对象。' },
  cloudinary:   { labelZh: 'Cloudinary',  descZh: '通过 Cloudinary 管理媒体资源，支持上传、转换 URL 和删除。' },
  gcal:         { labelZh: 'Google 日历', descZh: '管理 Google Calendar 事件，支持列出、创建和删除事件。' },
  docusign:     { labelZh: 'DocuSign',    descZh: '通过 DocuSign 管理电子签名信封，支持创建、查询和作废。' },
  xero:         { labelZh: 'Xero',        descZh: '通过 Xero 实现财务自动化，配置 access_token 和 tenant_id。' },
  calendly:     { labelZh: 'Calendly',    descZh: '通过 Calendly 实现日程自动化，支持查询事件类型和预约。' },
  apify:        { labelZh: 'Apify',       descZh: '通过 Apify 进行网页抓取，支持运行 Actor 和获取数据集。' },
  ganalytics:   { labelZh: 'Google Analytics', descZh: '查询 Google Analytics 4 数据，支持运行报告和获取元数据。' },
  neon:         { labelZh: 'Neon',        descZh: '管理 Neon Serverless Postgres，支持列出和创建项目及分支。' },
  copper:       { labelZh: 'Copper CRM',  descZh: '通过 Copper 实现 CRM 自动化，支持联系人、线索和商机管理。' },
  // AI
  openai:       { labelZh: 'OpenAI',      descZh: '调用 OpenAI Chat Completions（gpt-4o、gpt-4o-mini、o1），返回 {content, model, usage}。' },
  gemini:       { labelZh: 'Gemini',      descZh: '调用 Google Gemini（2.0-flash、1.5-pro、thinking），返回 {content, model, usage}。' },
  claude:       { labelZh: 'Claude',      descZh: '调用 Anthropic Claude（opus-4-7、sonnet-4-6、haiku-4-5），返回 {content, model, usage}。' },
  agent:        { labelZh: 'AI 智能体',   descZh: '通过 AI Runtime 运行 Python AI 智能体，配置模型、提示词和系统指令。' },
  replicate:    { labelZh: 'Replicate',   descZh: '通过 Replicate 运行 AI 模型，配置 api_token、版本 ID 和输入参数。' },
  mistral:      { labelZh: 'Mistral',     descZh: '调用 Mistral AI API，支持对话、嵌入向量和模型列表操作。' },
  perplexity:   { labelZh: 'Perplexity',  descZh: '通过 Perplexity API 进行 AI 搜索，可获取引用来源。' },
  cohere:       { labelZh: 'Cohere',      descZh: '调用 Cohere NLP API，支持对话、嵌入、分类和重排操作。' },
  pinecone:     { labelZh: 'Pinecone',    descZh: '向量数据库操作，支持查询、插入、删除和集合管理。' },
  togetherai:   { labelZh: 'Together AI', descZh: '通过 Together AI 运行开源大语言模型，配置 api_key 和模型参数。' },
  huggingface:  { labelZh: 'Hugging Face', descZh: '调用 Hugging Face 推理 API，配置 api_token、模型和输入。' },
  groq:         { labelZh: 'Groq',        descZh: '通过 Groq 进行高速 LLM 推理，配置 api_key、模型和对话消息。' },
  openrouter:   { labelZh: 'OpenRouter',  descZh: '通过 OpenRouter 访问 100+ 大语言模型，配置 api_key 和模型。' },
  qdrant:       { labelZh: 'Qdrant',      descZh: '使用 Qdrant 进行向量搜索，支持查询、插入、删除和集合管理。' },
  // Transform
  code:         { labelZh: '代码',        descZh: '在沙箱中执行 Rhai 脚本，以映射方式访问输入和节点输出，返回脚本结果。' },
  transform:    { labelZh: '数据转换',    descZh: '通过 {{...}} 插值渲染 JSON 模板，模板可为对象、数组或字符串。' },
  map:          { labelZh: '数组映射',    descZh: '对 JSON 数组每个元素应用可选的 item_template，返回 {count, items}。' },
  filter:       { labelZh: '数组过滤',    descZh: '按字段和运算符过滤 JSON 数组，返回 {count, items}。' },
  aggregate:    { labelZh: '聚合',        descZh: '通过 count/sum/avg/min/max/join/first/last 将数组归约为标量。' },
  sort:         { labelZh: '排序',        descZh: '对 JSON 数组按字段进行升序或降序排序，支持字符串和数值比较。' },
  extract:      { labelZh: '提取字段',    descZh: '通过点路径从 JSON 数据源提取单个值，返回 {value, found}。' },
  merge:        { labelZh: '合并',        descZh: '将多个节点输出的字段合并为一个扁平对象，支持可选键别名。' },
  loop:         { labelZh: '循环',        descZh: '遍历 JSON 数组，支持逐元素模板和提前退出，可设置最大迭代次数。' },
  validate:     { labelZh: '验证',        descZh: '对 JSON 载荷进行字段模式验证（必填、类型检查），返回 {valid, errors[]}。' },
  split:        { labelZh: '字符串拆分',  descZh: '按分隔符将字符串拆分为数组（默认逗号），返回 {parts, count}。' },
  join:         { labelZh: '数组拼接',    descZh: '按分隔符将数组拼接为字符串（默认逗号），返回 {result, count}。' },
  dedupe:       { labelZh: '去重',        descZh: '从 JSON 数组中删除重复元素，返回 {items, count, removed_count}。' },
  regex:        { labelZh: '正则匹配',    descZh: '对字符串进行正则表达式测试，返回 {matched, full_match, groups}。' },
  csv:          { labelZh: '解析 CSV',    descZh: '将 CSV 字符串解析为行对象数组，返回 {rows, count, headers}。' },
  rename:       { labelZh: '重命名键',    descZh: '重命名 JSON 对象中的键，未映射的键保留原名。' },
  format:       { labelZh: '格式化',      descZh: '字符串/值格式化，支持大小写、截断、替换、数字转换等操作。' },
  xml:          { labelZh: '解析 XML',    descZh: '将 XML 字符串解析为 JSON 对象，返回 {data: object}。' },
  yaml:         { labelZh: 'YAML',        descZh: '解析 YAML 为 JSON，或将 JSON 序列化为 YAML，返回 {data} 或 {yaml}。' },
  crypto:       { labelZh: '加密工具',    descZh: '密码学工具，支持 sha256/sha512/hmac/base64/hex 编解码和随机值生成。' },
  handlebars:   { labelZh: 'HB 模板',     descZh: '渲染 Handlebars 模板，支持 {{var}}、{{#if}}、{{#each}} 等语法。' },
  math:         { labelZh: '数学计算',    descZh: '数值运算，支持加减乘除、取整、开方、幂、聚合和表达式求值。' },
  array_utils:  { labelZh: '数组工具',    descZh: '数组操作，支持分块、扁平化、压缩、合并、随机采样和范围生成。' },
  date:         { labelZh: '日期时间',    descZh: '日期时间操作，支持获取当前时间、解析、格式化、加减和差值计算。' },
  // Utility
  note:         { labelZh: '备注',        descZh: '文档注释（便利贴），不执行也不影响工作流数据流。' },
  random:       { labelZh: '随机',        descZh: '生成随机值，支持 UUID、数字（可设范围）、布尔值和从数组随机选取。' },
}

function highlightMatch(text: string, query: string): ReactNode {
  if (!query) return text
  const idx = text.toLowerCase().indexOf(query.toLowerCase())
  if (idx === -1) return text
  return (
    <>
      {text.slice(0, idx)}
      <mark style={{ background: 'var(--link)', color: '#fff', borderRadius: 2, padding: '0 1px' }}>
        {text.slice(idx, idx + query.length)}
      </mark>
      {text.slice(idx + query.length)}
    </>
  )
}

export function WorkflowEditor({ workflowId, onBack, initialInput }: Props) {
  const { auth } = useAuth()
  const { theme, toggle: toggleTheme } = useTheme()
  const { locale, toggle: toggleLocale, t } = useLocale()
  const zh = locale === 'zh'
  const [workflow, setWorkflow]       = useState<WorkflowRecord | null>(null)
  const [version, setVersion]         = useState<WorkflowVersionRecord | null>(null)
  const [nodes, setNodes]             = useState<FlowNode[]>([])
  const [edges, setEdges]             = useState<FlowEdge[]>([])
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null)
  const [execution, setExecution]     = useState<ExecutionRecord | null>(null)
  const [recentExecutions, setRecentExecutions] = useState<ExecutionSummary[]>([])
  const [inputJson, setInputJson]     = useState(initialInput ?? '{}')
  const [running, setRunning]         = useState(false)
  const [saving, setSaving]           = useState(false)
  const [publishing, setPublishing]   = useState(false)
  const [toasts, setToasts]           = useState<Toast[]>([])
  const [renaming, setRenaming]       = useState(false)
  const [newName, setNewName]         = useState('')
  const [editingDescription, setEditingDescription] = useState(false)
  const [newDescription, setNewDescription] = useState('')
  const [editingSla, setEditingSla] = useState(false)
  const [newSlaInput, setNewSlaInput] = useState('')
  const [editingRateLimit, setEditingRateLimit] = useState(false)
  const [editingMaxConcurrent, setEditingMaxConcurrent] = useState(false)
  const [newMaxConcurrentInput, setNewMaxConcurrentInput] = useState('')
  const [newRateLimitInput, setNewRateLimitInput] = useState('')
  const [addingTag, setAddingTag] = useState(false)
  const [newTagInput, setNewTagInput] = useState('')
  const [editingBudget, setEditingBudget] = useState(false)
  const [newBudgetInput, setNewBudgetInput] = useState('')
  const [webhookUrl, setWebhookUrl]   = useState<string | null>(null)
  const [webhookSecret, setWebhookSecret] = useState<string | null>(null)
  const [showVersions, setShowVersions] = useState(false)
  const [versions, setVersions]       = useState<WorkflowVersionRecord[]>([])
  const [loadingVersions, setLoadingVersions] = useState(false)
  const [diffVersionId, setDiffVersionId] = useState<string | null>(null)
  const [diffCompareId, setDiffCompareId] = useState<string | null>(null)
  const [showComparePicker, setShowComparePicker] = useState<string | null>(null)
  const [rollingBack, setRollingBack] = useState<string | null>(null)
  const [inputSchema, setInputSchema] = useState<InputField[]>([])
  const [showSchema, setShowSchema]   = useState(false)
  const [showVars, setShowVars]       = useState(false)
  const [variables, setVariables]     = useState<api.Variable[]>([])
  const [showPalette, setShowPalette] = useState(false)
  const [paletteQuery, setPaletteQuery] = useState('')
  const [showHelp, setShowHelp] = useState(false)
  const [envSets, setEnvSets]         = useState<EnvSetSummary[]>([{ name: 'default', var_count: 0 }])
  const [envSet, setEnvSet]           = useState('default')
  const [runLabel, setRunLabel]         = useState('')
  const [callbackUrl, setCallbackUrl]   = useState('')
  const [dryRun, setDryRun]             = useState(false)
  const [paletteSearch, setPaletteSearch] = useState('')
  const [snapToGrid, setSnapToGrid] = useState(false)
  const [showMinimap, setShowMinimap] = useState(true)
  const [bgVariant, setBgVariant] = useState<'dots' | 'grid' | 'lines' | 'none'>(() => {
    try { return (localStorage.getItem('af:canvas_bg') as 'dots' | 'grid' | 'lines' | 'none' | null) ?? 'dots' } catch { return 'dots' }
  })
  const savedViewport = (() => {
    try {
      const raw = localStorage.getItem(`af:viewport:${workflowId}`)
      return raw ? (JSON.parse(raw) as { x: number; y: number; zoom: number }) : undefined
    } catch { return undefined }
  })()
  const handleViewportChange = (vp: { x: number; y: number; zoom: number }) => {
    try { localStorage.setItem(`af:viewport:${workflowId}`, JSON.stringify(vp)) } catch { /* ignore */ }
  }
  const fitViewRef = useRef<(() => void) | null>(null)
  const fitToNodeRef = useRef<((id: string) => void) | null>(null)
  const [showNodeFind, setShowNodeFind] = useState(false)
  const [nodeFindQuery, setNodeFindQuery] = useState('')
  const [nodeFindIdx, setNodeFindIdx] = useState(0)
  const nodeFindInputRef = useRef<HTMLInputElement>(null)
  const [wfStats, setWfStats] = useState<api.WorkflowStats | null>(null)
  const [wfEstimate, setWfEstimate] = useState<api.WorkflowEstimate | null>(null)
  const [wfHealth, setWfHealth] = useState<api.WorkflowHealthReport | null>(null)
  const [showNodeHeatmap, setShowNodeHeatmap] = useState(false)
  const [nodeStats, setNodeStats] = useState<api.NodeStat[]>([])
  const [showReport, setShowReport] = useState(false)
  const [reportExecs, setReportExecs] = useState<api.ExecutionSummary[]>([])
  const [latestExec, setLatestExec] = useState<api.ExecutionSummary | null>(null)
  const [saveMessage, setSaveMessage] = useState('')
  const [showSaveMessage, setShowSaveMessage] = useState(false)
  const [showSchedule, setShowSchedule] = useState(false)
  const [showReadme, setShowReadme] = useState(false)
  const [showForms, setShowForms] = useState(false)
  const [showTests, setShowTests] = useState(false)
  const [showComments, setShowComments] = useState(false)
  const [showApiDocs, setShowApiDocs] = useState(false)
  const [showCopilot, setShowCopilot] = useState(false)
  const toastId = useRef(0)
  const handleSaveRef = useRef<() => void>(() => {})
  const handleRunRef  = useRef<() => void>(() => {})
  const handleDuplicateNodeRef = useRef<() => void>(() => {})
  const nodesRef = useRef<FlowNode[]>([])
  const edgesRef = useRef<FlowEdge[]>([])
  const selectedNodeIdRef = useRef<string | null>(null)
  const undoStack = useRef<{ nodes: FlowNode[]; edges: FlowEdge[] }[]>([])
  const redoStack = useRef<{ nodes: FlowNode[]; edges: FlowEdge[] }[]>([])

  // Keep refs in sync with latest nodes/edges/selectedNodeId for the keyboard handler
  useEffect(() => { nodesRef.current = nodes; edgesRef.current = edges }, [nodes, edges])
  useEffect(() => { selectedNodeIdRef.current = selectedNodeId }, [selectedNodeId])

  // Scroll canvas to focused node find match
  useEffect(() => {
    if (!showNodeFind || !nodeFindQuery) return
    const q = nodeFindQuery.toLowerCase().trim()
    const matches = q ? nodes.filter((n) => {
      const label = (n.data?.label as string | undefined) ?? n.type ?? ''
      return n.id.toLowerCase().includes(q) || label.toLowerCase().includes(q) || (n.type ?? '').toLowerCase().includes(q)
    }) : []
    if (matches.length === 0) return
    const idx = nodeFindIdx % matches.length
    fitToNodeRef.current?.(matches[idx].id)
  }, [nodeFindIdx, nodeFindQuery, showNodeFind])

  const pushHistory = () => {
    undoStack.current = [...undoStack.current, { nodes: nodesRef.current, edges: edgesRef.current }].slice(-50)
    redoStack.current = []
  }

  const toast = useCallback((message: string, kind: 'success' | 'error' = 'success') => {
    const id = ++toastId.current
    setToasts((t) => [...t, { id, message, kind }])
    setTimeout(() => setToasts((t) => t.filter((x) => x.id !== id)), 3000)
  }, [])

  const refreshHistory = useCallback(() => {
    api.listExecutions(auth!.tenantId, workflowId)
      .then(setRecentExecutions)
      .catch(() => {})
  }, [workflowId])

  // Load env sets once
  useEffect(() => {
    api.listEnvSets(auth!.tenantId)
      .then((s) => {
        if (s.length > 0) {
          const hasDefault = s.some((x) => x.name === 'default')
          setEnvSets(hasDefault ? s : [{ name: 'default', var_count: 0 }, ...s])
        }
      })
      .catch(() => {})
  }, [])

  // Load workflow + latest version
  useEffect(() => {
    api.getWorkflow(auth!.tenantId, workflowId).then((wf) => {
      setWorkflow(wf)
      setNewName(wf.name)
      if (wf.latest_version_id) {
        return api.getVersion(auth!.tenantId, wf.latest_version_id)
      }
      return null
    }).then((ver) => {
      if (ver) {
        setVersion(ver)
        const { nodes: fn, edges: fe } = graphFromApi(ver.graph.nodes, ver.graph.edges)
        setNodes(fn)
        setEdges(fe)
        setInputSchema(ver.graph.input_schema ?? [])
        if (ver.status === 'published') {
          api.getWebhook(auth!.tenantId, ver.id)
            .then((info) => { setWebhookUrl(window.location.origin + info.url); setWebhookSecret(info.secret ?? null) })
            .catch(() => {})
        }
      } else {
        // No version yet — start with a trigger node
        const { nodes: fn, edges: fe } = graphFromApi(
          [{ id: 'trigger', type: 'trigger' }],
          [],
        )
        setNodes(fn)
        setEdges(fe)
      }
    }).catch((e: unknown) => toast(String(e), 'error'))
    refreshHistory()
    api.getWorkflowStats(auth!.tenantId, workflowId).then(setWfStats).catch(() => {})
    api.getWorkflowEstimate(auth!.tenantId, workflowId).then(setWfEstimate).catch(() => {})
    api.getLatestExecution(auth!.tenantId, workflowId).then(setLatestExec).catch(() => {})
    api.getWorkflowHealth(auth!.tenantId, workflowId).then(setWfHealth).catch(() => {})
  }, [workflowId, toast, refreshHistory])

  // Stream live execution updates via SSE (fall back to polling)
  useEffect(() => {
    if (!execution || execution.status !== 'running') return
    const stored = getStoredAuth()
    let source: EventSource | null = null
    let pollTimer: ReturnType<typeof setInterval> | null = null

    if (typeof EventSource !== 'undefined' && stored?.token) {
      try {
        source = new EventSource(`/v1/executions/${execution.id}/events?token=${encodeURIComponent(stored.token)}`)
        source.onmessage = (ev) => {
          try {
            const updated = JSON.parse(ev.data) as ExecutionRecord
            setExecution(updated)
            if (updated.status !== 'running') {
              refreshHistory()
              source?.close()
            }
          } catch { /* ignore parse errors */ }
        }
        source.onerror = () => {
          source?.close()
          source = null
          // Fall back to polling
          pollTimer = setInterval(async () => {
            try {
              const updated = await api.getExecution(auth!.tenantId, execution.id)
              setExecution(updated)
              if (updated.status !== 'running') { refreshHistory(); clearInterval(pollTimer!) }
            } catch { /* ignore */ }
          }, 1500)
        }
      } catch {
        source = null
      }
    }

    if (!source) {
      pollTimer = setInterval(async () => {
        try {
          const updated = await api.getExecution(auth!.tenantId, execution.id)
          setExecution(updated)
          if (updated.status !== 'running') { refreshHistory(); clearInterval(pollTimer!) }
        } catch { /* ignore */ }
      }, 1000)
    }

    return () => {
      source?.close()
      if (pollTimer) clearInterval(pollTimer)
    }
  }, [execution?.id, execution?.status, refreshHistory])

  const [recentNodeTypes, setRecentNodeTypes] = useState<NodeType[]>(() => {
    try { return JSON.parse(localStorage.getItem('af:recentNodes') ?? '[]') as NodeType[] } catch { return [] }
  })

  // Add node from palette
  const addNode = useCallback((type: NodeType) => {
    pushHistory()
    const id = `${type}-${Date.now()}`
    const existing = nodes.length
    const newNode: FlowNode = {
      id,
      type,
      position: { x: (existing % 4) * 280 + 80, y: Math.floor(existing / 4) * 140 + 80 },
      data: { label: id, nodeType: type, config: {} },
    }
    setNodes((prev) => [...prev, newNode])
    setSelectedNodeId(id)
    // Track recently used
    setRecentNodeTypes((prev) => {
      const next = [type, ...prev.filter((t) => t !== type)].slice(0, 5)
      try { localStorage.setItem('af:recentNodes', JSON.stringify(next)) } catch { /* ignore */ }
      return next
    })
  }, [nodes.length])

  // Update node config from panel
  const handleUpdateConfig = useCallback((nodeId: string, config: Record<string, unknown>) => {
    setNodes((prev) =>
      prev.map((n) =>
        n.id === nodeId ? { ...n, data: { ...n.data, config } } : n,
      ),
    )
  }, [])

  // Duplicate the currently selected node
  const handleDuplicateNode = useCallback(() => {
    const node = nodes.find((n) => n.id === selectedNodeId)
    if (!node) return
    pushHistory()
    const newId = `${node.data.nodeType ?? 'node'}-${Date.now()}`
    const newNode: FlowNode = {
      ...node,
      id: newId,
      position: { x: node.position.x + 60, y: node.position.y + 60 },
      data: { ...node.data, label: newId, config: { ...(node.data.config ?? {}) } },
    }
    setNodes((prev) => [...prev, newNode])
    setSelectedNodeId(newId)
    toast(zh ? `已复制为 ${newId}` : `Duplicated as ${newId}`)
  }, [nodes, selectedNodeId, toast])

  // Re-run topological layout on current graph
  const handleAutoLayout = useCallback(() => {
    pushHistory()
    const { nodes: apiNodes, edges: apiEdges } = fromFlowGraph(nodes, edges)
    const { nodes: layoutedNodes, edges: layoutedEdges } = graphFromApi(apiNodes, apiEdges)
    setNodes(layoutedNodes)
    setEdges(layoutedEdges)
    toast(zh ? '布局已应用' : 'Layout applied')
  }, [nodes, edges, toast])

  // Keyboard shortcuts: Ctrl+S = save, Ctrl+Enter = run, Ctrl+Z/Y = undo/redo, Escape = deselect
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement).tagName
      if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return
      if ((e.ctrlKey || e.metaKey) && e.key === 's') { e.preventDefault(); handleSaveRef.current() }
      if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') { e.preventDefault(); handleRunRef.current() }
      if ((e.ctrlKey || e.metaKey) && e.key === 'k') { e.preventDefault(); setPaletteQuery(''); setShowPalette(true) }
      if ((e.ctrlKey || e.metaKey) && e.key === 'd') { e.preventDefault(); handleDuplicateNodeRef.current() }
      if ((e.ctrlKey || e.metaKey) && !e.shiftKey && e.key === 'z') {
        e.preventDefault()
        const snap = undoStack.current.pop()
        if (snap) {
          redoStack.current.push({ nodes: nodesRef.current, edges: edgesRef.current })
          setNodes(snap.nodes)
          setEdges(snap.edges)
        }
      }
      if ((e.ctrlKey || e.metaKey) && (e.key === 'y' || (e.shiftKey && e.key === 'z'))) {
        e.preventDefault()
        const snap = redoStack.current.pop()
        if (snap) {
          undoStack.current.push({ nodes: nodesRef.current, edges: edgesRef.current })
          setNodes(snap.nodes)
          setEdges(snap.edges)
        }
      }
      if (e.key === 'Delete' || e.key === 'Backspace') {
        const id = selectedNodeIdRef.current
        if (id) {
          e.preventDefault()
          undoStack.current.push({ nodes: nodesRef.current, edges: edgesRef.current })
          redoStack.current = []
          setNodes((prev) => prev.filter((n) => n.id !== id))
          setEdges((prev) => prev.filter((ed) => ed.source !== id && ed.target !== id))
          setSelectedNodeId(null)
        }
      }
      if (e.key === '?') { e.preventDefault(); setShowHelp(true) }
      if (e.key === 'f' && !e.ctrlKey && !e.metaKey) { e.preventDefault(); fitViewRef.current?.() }
      if ((e.ctrlKey || e.metaKey) && e.key === 'f') { e.preventDefault(); setShowNodeFind((v) => !v); setTimeout(() => nodeFindInputRef.current?.focus(), 50) }
      if (e.key === 'Escape') { setSelectedNodeId(null); setShowPalette(false); setShowHelp(false); setShowNodeFind(false) }
    }
    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [])

  // Warn browser before navigating away with unsaved changes
  useEffect(() => {
    const handler = (e: BeforeUnloadEvent) => {
      if (isDirtyRef.current) { e.preventDefault(); e.returnValue = '' }
    }
    window.addEventListener('beforeunload', handler)
    return () => window.removeEventListener('beforeunload', handler)
  }, [])

  // Save new version
  const handleSave = async () => {
    if (!workflow) return
    setSaving(true)
    try {
      const { nodes: apiNodes, edges: apiEdges } = fromFlowGraph(nodes, edges)
      const tempVersionId = `v-${Date.now()}`
      const graph = {
        workflow_version_id: tempVersionId,
        nodes: apiNodes,
        edges: apiEdges,
        input_schema: inputSchema,
      }
      const ver = await api.createVersion(auth!.tenantId, workflowId, graph, saveMessage.trim() || undefined)
      setVersion(ver)
      setSaveMessage('')
      setShowSaveMessage(false)
      toast(zh ? '版本已保存' : 'Version saved')
    } catch (e) {
      toast(String(e), 'error')
    } finally {
      setSaving(false)
    }
  }

  // Validate node configs before publishing — returns list of warnings
  const collectPublishWarnings = (): string[] => {
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

      // Config checks
      if (nt === 'http' && !c.url) warnings.push(`HTTP node "${label}" has no URL`)
      if (nt === 'openai' && !c.api_key) warnings.push(`OpenAI node "${label}" has no API key`)
      if (nt === 'gemini' && !c.api_key) warnings.push(`Gemini node "${label}" has no API key`)
      if (nt === 'claude' && !c.api_key) warnings.push(`Claude node "${label}" has no API key`)
      if (nt === 'slack' && !c.webhook_url) warnings.push(`Slack node "${label}" has no Webhook URL`)
      if (nt === 'email' && !c.to) warnings.push(`Email node "${label}" has no recipient`)
      if (nt === 'email' && !c.api_key) warnings.push(`Email node "${label}" has no API key`)
      if (nt === 'github' && !c.token) warnings.push(`GitHub node "${label}" has no token`)
      if (nt === 'github' && !c.endpoint) warnings.push(`GitHub node "${label}" has no endpoint`)
      if (nt === 'webhook' && !c.url) warnings.push(`Webhook Send node "${label}" has no URL`)
      if (nt === 'jira' && !c.base_url) warnings.push(`Jira node "${label}" has no base URL`)
      if (nt === 'jira' && !c.token) warnings.push(`Jira node "${label}" has no API token`)
      if (nt === 'jira' && !c.endpoint) warnings.push(`Jira node "${label}" has no endpoint`)
      if (nt === 'notion' && !c.token) warnings.push(`Notion node "${label}" has no token`)
      if (nt === 'notion' && !c.endpoint) warnings.push(`Notion node "${label}" has no endpoint`)
      if (nt === 'linear' && !c.token) warnings.push(`Linear node "${label}" has no token`)
      if (nt === 'linear' && !c.query) warnings.push(`Linear node "${label}" has no GraphQL query`)
      if (nt === 'airtable' && !c.token) warnings.push(`Airtable node "${label}" has no token`)
      if (nt === 'airtable' && !c.base_id) warnings.push(`Airtable node "${label}" has no base ID`)
      if (nt === 'airtable' && !c.table) warnings.push(`Airtable node "${label}" has no table name`)
      if (nt === 'for_each' && !c.workflow_id) warnings.push(`For Each node "${label}" has no target workflow`)
      if (nt === 'discord' && !c.webhook_url) warnings.push(`Discord node "${label}" has no webhook URL`)
      if (nt === 'discord' && !c.content) warnings.push(`Discord node "${label}" has no message content`)
      if (nt === 'teams' && !c.webhook_url) warnings.push(`Teams node "${label}" has no webhook URL`)
      if (nt === 'teams' && !c.text) warnings.push(`Teams node "${label}" has no message text`)
      if (nt === 'sheets' && !c.token) warnings.push(`Google Sheets node "${label}" has no token`)
      if (nt === 'sheets' && !c.spreadsheet_id) warnings.push(`Google Sheets node "${label}" has no spreadsheet ID`)
      if (nt === 'xml' && !c.source) warnings.push(`XML Parse node "${label}" has no source`)
      if (nt === 'yaml' && !c.source) warnings.push(`YAML node "${label}" has no source`)
      if (nt === 'twilio' && !c.account_sid) warnings.push(`Twilio node "${label}" has no account SID`)
      if (nt === 'twilio' && !c.auth_token) warnings.push(`Twilio node "${label}" has no auth token`)
      if (nt === 'twilio' && !c.to) warnings.push(`Twilio node "${label}" has no 'to' number`)
      if (nt === 'twilio' && !c.from) warnings.push(`Twilio node "${label}" has no 'from' number`)
      if (nt === 'stripe' && !c.api_key) warnings.push(`Stripe node "${label}" has no API key`)
      if (nt === 'stripe' && !c.endpoint) warnings.push(`Stripe node "${label}" has no endpoint`)
      if (nt === 'crypto' && !c.source) warnings.push(`Crypto node "${label}" has no source`)
      if (nt === 'hubspot' && !c.token) warnings.push(`HubSpot node "${label}" has no token`)
      if (nt === 'hubspot' && !c.endpoint) warnings.push(`HubSpot node "${label}" has no endpoint`)
      if (nt === 'zendesk' && !c.subdomain) warnings.push(`Zendesk node "${label}" has no subdomain`)
      if (nt === 'zendesk' && !c.token) warnings.push(`Zendesk node "${label}" has no token`)
      if (nt === 'zendesk' && !c.endpoint) warnings.push(`Zendesk node "${label}" has no endpoint`)
      if (nt === 'redis' && !c.url) warnings.push(`Redis node "${label}" has no URL`)
      if (nt === 'redis' && !c.key) warnings.push(`Redis node "${label}" has no key (ping doesn't need one)`)
      if (nt === 'elasticsearch' && !c.url) warnings.push(`Elasticsearch node "${label}" has no URL`)
      if (nt === 'pagerduty' && !c.routing_key) warnings.push(`PagerDuty node "${label}" has no routing key`)
      if (nt === 'pagerduty' && !c.summary) warnings.push(`PagerDuty node "${label}" has no summary`)
      if (nt === 'handlebars' && !c.template) warnings.push(`HB Template node "${label}" has no template`)
      if (nt === 'math' && !c.operation) warnings.push(`Math node "${label}" has no operation set`)
      if (nt === 'array_utils' && !c.operation) warnings.push(`Array Utils node "${label}" has no operation set`)
      if (nt === 'shopify' && !c.shop) warnings.push(`Shopify node "${label}" has no shop name`)
      if (nt === 'shopify' && !c.token) warnings.push(`Shopify node "${label}" has no access token`)
      if (nt === 'datadog' && !c.api_key) warnings.push(`Datadog node "${label}" has no API key`)
      if (nt === 'datadog' && !c.endpoint) warnings.push(`Datadog node "${label}" has no endpoint`)
      if (nt === 'salesforce' && !c.token) warnings.push(`Salesforce node "${label}" has no access token`)
      if (nt === 'salesforce' && !c.instance_url) warnings.push(`Salesforce node "${label}" has no instance URL`)
      if (nt === 'freshdesk' && !c.api_key) warnings.push(`Freshdesk node "${label}" has no API key`)
      if (nt === 'freshdesk' && !c.domain) warnings.push(`Freshdesk node "${label}" has no domain`)
      if (nt === 'mailgun' && !c.api_key) warnings.push(`Mailgun node "${label}" has no API key`)
      if (nt === 'mailgun' && !c.domain) warnings.push(`Mailgun node "${label}" has no sending domain`)
      if (nt === 'mailgun' && !c.to) warnings.push(`Mailgun node "${label}" has no recipient address`)
      if (nt === 'asana' && !c.token) warnings.push(`Asana node "${label}" has no access token`)
      if (nt === 'asana' && !c.endpoint) warnings.push(`Asana node "${label}" has no endpoint`)
      if (nt === 'servicenow' && !c.instance) warnings.push(`ServiceNow node "${label}" has no instance`)
      if (nt === 'servicenow' && !c.username) warnings.push(`ServiceNow node "${label}" has no username`)
      if (nt === 'confluence' && !c.base_url) warnings.push(`Confluence node "${label}" has no base URL`)
      if (nt === 'confluence' && !c.endpoint) warnings.push(`Confluence node "${label}" has no endpoint`)
      if (nt === 'bitbucket' && !c.username) warnings.push(`Bitbucket node "${label}" has no username`)
      if (nt === 'bitbucket' && !c.app_password) warnings.push(`Bitbucket node "${label}" has no app password`)
      if (nt === 'azure_devops' && !c.pat) warnings.push(`Azure DevOps node "${label}" has no PAT`)
      if (nt === 'azure_devops' && !c.organization) warnings.push(`Azure DevOps node "${label}" has no organization`)
      if (nt === 'twitch' && !c.client_id) warnings.push(`Twitch node "${label}" has no client ID`)
      if (nt === 'twitch' && !c.access_token) warnings.push(`Twitch node "${label}" has no access token`)
      if (nt === 'figma' && !c.token) warnings.push(`Figma node "${label}" has no access token`)
      if (nt === 'figma' && !c.endpoint) warnings.push(`Figma node "${label}" has no endpoint`)
      if (nt === 'dropbox' && !c.token) warnings.push(`Dropbox node "${label}" has no access token`)
      if (nt === 'cloudflare' && !c.api_token) warnings.push(`Cloudflare node "${label}" has no API token`)
      if (nt === 'cloudflare' && !c.endpoint) warnings.push(`Cloudflare node "${label}" has no endpoint`)
      if (nt === 'box' && !c.token) warnings.push(`Box node "${label}" has no access token`)
      if (nt === 'box' && !c.endpoint) warnings.push(`Box node "${label}" has no endpoint`)
      if (nt === 'okta' && !c.domain) warnings.push(`Okta node "${label}" has no domain`)
      if (nt === 'okta' && !c.token) warnings.push(`Okta node "${label}" has no token`)
      if (nt === 'zoom' && !c.token) warnings.push(`Zoom node "${label}" has no access token`)
      if (nt === 'zoom' && !c.endpoint) warnings.push(`Zoom node "${label}" has no endpoint`)
      if (nt === 'spotify' && !c.token) warnings.push(`Spotify node "${label}" has no access token`)
      if (nt === 'spotify' && !c.endpoint) warnings.push(`Spotify node "${label}" has no endpoint`)
      if (nt === 'typeform' && !c.token) warnings.push(`Typeform node "${label}" has no token`)
      if (nt === 'typeform' && !c.endpoint) warnings.push(`Typeform node "${label}" has no endpoint`)
      if (nt === 'webflow' && !c.token) warnings.push(`Webflow node "${label}" has no token`)
      if (nt === 'webflow' && !c.endpoint) warnings.push(`Webflow node "${label}" has no endpoint`)
      if (nt === 'intercom' && !c.token) warnings.push(`Intercom node "${label}" has no token`)
      if (nt === 'intercom' && !c.endpoint) warnings.push(`Intercom node "${label}" has no endpoint`)
      if (nt === 'pipedrive' && !c.api_token) warnings.push(`Pipedrive node "${label}" has no API token`)
      if (nt === 'pipedrive' && !c.endpoint) warnings.push(`Pipedrive node "${label}" has no endpoint`)
      if (nt === 'trello' && !c.api_key) warnings.push(`Trello node "${label}" has no API key`)
      if (nt === 'trello' && !c.token) warnings.push(`Trello node "${label}" has no token`)
      if (nt === 'trello' && !c.endpoint) warnings.push(`Trello node "${label}" has no endpoint`)
      if (nt === 'monday' && !c.token) warnings.push(`Monday node "${label}" has no token`)
      if (nt === 'monday' && !c.query) warnings.push(`Monday node "${label}" has no GraphQL query`)
      if (nt === 'clickup' && !c.token) warnings.push(`ClickUp node "${label}" has no token`)
      if (nt === 'clickup' && !c.endpoint) warnings.push(`ClickUp node "${label}" has no endpoint`)
      if (nt === 'amplitude' && !c.api_key) warnings.push(`Amplitude node "${label}" has no API key`)
      if (nt === 'amplitude' && !c.secret_key) warnings.push(`Amplitude node "${label}" has no secret key`)
      if (nt === 'mixpanel' && !c.project_token) warnings.push(`Mixpanel node "${label}" has no project token`)
      if (nt === 'mixpanel' && !c.api_secret) warnings.push(`Mixpanel node "${label}" has no API secret`)
      if (nt === 'segment' && !c.write_key) warnings.push(`Segment node "${label}" has no write key`)
      if (nt === 'sendgrid' && !c.api_key) warnings.push(`SendGrid node "${label}" has no API key`)
      if (nt === 'sendgrid' && !c.endpoint) warnings.push(`SendGrid node "${label}" has no endpoint`)
      if (nt === 'braintree' && !c.merchant_id) warnings.push(`Braintree node "${label}" has no merchant ID`)
      if (nt === 'braintree' && !c.public_key) warnings.push(`Braintree node "${label}" has no public key`)
      if (nt === 'braintree' && !c.private_key) warnings.push(`Braintree node "${label}" has no private key`)
      if (nt === 'braintree' && !c.endpoint) warnings.push(`Braintree node "${label}" has no endpoint`)
      if (nt === 'paypal' && !c.client_id) warnings.push(`PayPal node "${label}" has no client ID`)
      if (nt === 'paypal' && !c.client_secret) warnings.push(`PayPal node "${label}" has no client secret`)
      if (nt === 'paypal' && !c.endpoint) warnings.push(`PayPal node "${label}" has no endpoint`)
      if (nt === 'razorpay' && !c.key_id) warnings.push(`Razorpay node "${label}" has no key ID`)
      if (nt === 'razorpay' && !c.key_secret) warnings.push(`Razorpay node "${label}" has no key secret`)
      if (nt === 'razorpay' && !c.endpoint) warnings.push(`Razorpay node "${label}" has no endpoint`)
      if (nt === 'firebase' && !c.project_id) warnings.push(`Firebase node "${label}" has no project ID`)
      if (nt === 'firebase' && !c.id_token) warnings.push(`Firebase node "${label}" has no ID token`)
      if (nt === 'firebase' && !c.endpoint) warnings.push(`Firebase node "${label}" has no endpoint`)
      if (nt === 'supabase' && !c.project_url) warnings.push(`Supabase node "${label}" has no project URL`)
      if (nt === 'supabase' && !c.api_key) warnings.push(`Supabase node "${label}" has no API key`)
      if (nt === 'supabase' && !c.endpoint) warnings.push(`Supabase node "${label}" has no endpoint`)
      if (nt === 'mailchimp' && !c.api_key) warnings.push(`Mailchimp node "${label}" has no API key`)
      if (nt === 'mailchimp' && !c.endpoint) warnings.push(`Mailchimp node "${label}" has no endpoint`)
      if (nt === 'activecampaign' && !c.api_key) warnings.push(`ActiveCampaign node "${label}" has no API key`)
      if (nt === 'activecampaign' && !c.base_url) warnings.push(`ActiveCampaign node "${label}" has no base URL`)
      if (nt === 'activecampaign' && !c.endpoint) warnings.push(`ActiveCampaign node "${label}" has no endpoint`)
      if (nt === 'klaviyo' && !c.api_key) warnings.push(`Klaviyo node "${label}" has no API key`)
      if (nt === 'klaviyo' && !c.endpoint) warnings.push(`Klaviyo node "${label}" has no endpoint`)
      if (nt === 'resend' && !c.api_key) warnings.push(`Resend node "${label}" has no API key`)
      if (nt === 'resend' && !c.endpoint) warnings.push(`Resend node "${label}" has no endpoint`)
      if (nt === 'contentful' && !c.access_token) warnings.push(`Contentful node "${label}" has no access token`)
      if (nt === 'contentful' && !c.space_id) warnings.push(`Contentful node "${label}" has no space ID`)
      if (nt === 'contentful' && !c.endpoint) warnings.push(`Contentful node "${label}" has no endpoint`)
      if (nt === 'algolia' && !c.app_id) warnings.push(`Algolia node "${label}" has no app ID`)
      if (nt === 'algolia' && !c.api_key) warnings.push(`Algolia node "${label}" has no API key`)
      if (nt === 'algolia' && !c.endpoint) warnings.push(`Algolia node "${label}" has no endpoint`)
      if (nt === 'postmark' && !c.server_token) warnings.push(`Postmark node "${label}" has no server token`)
      if (nt === 'postmark' && !c.endpoint) warnings.push(`Postmark node "${label}" has no endpoint`)
      if (nt === 'vonage' && !c.api_key) warnings.push(`Vonage node "${label}" has no API key`)
      if (nt === 'vonage' && !c.api_secret) warnings.push(`Vonage node "${label}" has no API secret`)
      if (nt === 'telegram' && !c.bot_token) warnings.push(`Telegram node "${label}" has no bot token`)
      if (nt === 'telegram' && !c.chat_id) warnings.push(`Telegram node "${label}" has no chat ID`)
      if (nt === 'replicate' && !c.api_token) warnings.push(`Replicate node "${label}" has no API token`)
      if (nt === 'replicate' && !c.version) warnings.push(`Replicate node "${label}" has no model version`)
      if (nt === 'mistral' && !c.api_key) warnings.push(`Mistral node "${label}" has no API key`)
      if (nt === 'whatsapp' && !c.access_token) warnings.push(`WhatsApp node "${label}" has no access token`)
      if (nt === 'whatsapp' && !c.phone_number_id) warnings.push(`WhatsApp node "${label}" has no phone number ID`)
      if (nt === 'whatsapp' && !c.to) warnings.push(`WhatsApp node "${label}" has no recipient`)
      if (nt === 'googledocs' && !c.access_token) warnings.push(`Google Docs node "${label}" has no access token`)
      if (nt === 'perplexity' && !c.api_key) warnings.push(`Perplexity node "${label}" has no API key`)
      if (nt === 'cohere' && !c.api_key) warnings.push(`Cohere node "${label}" has no API key`)
      if (nt === 'googledrive' && !c.access_token) warnings.push(`Google Drive node "${label}" has no access token`)
      if (nt === 'woocommerce' && !c.consumer_key) warnings.push(`WooCommerce node "${label}" has no consumer key`)
      if (nt === 'woocommerce' && !c.site_url) warnings.push(`WooCommerce node "${label}" has no site URL`)
      if (nt === 'pinecone' && !c.api_key) warnings.push(`Pinecone node "${label}" has no API key`)
      if (nt === 'pinecone' && !c.index_host) warnings.push(`Pinecone node "${label}" has no index host`)
      if (nt === 'togetherai' && !c.api_key) warnings.push(`Together AI node "${label}" has no API key`)
      if (nt === 'awss3' && !c.access_key_id) warnings.push(`AWS S3 node "${label}" has no access key ID`)
      if (nt === 'awss3' && !c.bucket) warnings.push(`AWS S3 node "${label}" has no bucket`)
      if (nt === 'huggingface' && !c.api_token) warnings.push(`Hugging Face node "${label}" has no API token`)
      if (nt === 'huggingface' && !c.model) warnings.push(`Hugging Face node "${label}" has no model`)
      if (nt === 'groq' && !c.api_key) warnings.push(`Groq node "${label}" has no API key`)
      if (nt === 'openrouter' && !c.api_key) warnings.push(`OpenRouter node "${label}" has no API key`)
      if (nt === 'qdrant' && !c.url) warnings.push(`Qdrant node "${label}" has no server URL`)
      if (nt === 'qdrant' && !c.collection) warnings.push(`Qdrant node "${label}" has no collection`)
      if (nt === 'cloudinary' && !c.cloud_name) warnings.push(`Cloudinary node "${label}" has no cloud name`)
      if (nt === 'gcal' && !c.access_token) warnings.push(`Google Calendar node "${label}" has no access token`)
      if (nt === 'docusign' && !c.access_token) warnings.push(`DocuSign node "${label}" has no access token`)
      if (nt === 'docusign' && !c.account_id) warnings.push(`DocuSign node "${label}" has no account ID`)
      if (nt === 'xero' && !c.access_token) warnings.push(`Xero node "${label}" has no access token`)
      if (nt === 'xero' && !c.tenant_id) warnings.push(`Xero node "${label}" has no tenant ID`)
      if (nt === 'calendly' && !c.api_key) warnings.push(`Calendly node "${label}" has no API key`)
      if (nt === 'apify' && !c.api_token) warnings.push(`Apify node "${label}" has no API token`)
      if (nt === 'ganalytics' && !c.access_token) warnings.push(`Google Analytics node "${label}" has no access token`)
      if (nt === 'ganalytics' && !c.property_id) warnings.push(`Google Analytics node "${label}" has no property ID`)
      if (nt === 'neon' && !c.api_key) warnings.push(`Neon node "${label}" has no API key`)
      if (nt === 'copper' && !c.api_key) warnings.push(`Copper CRM node "${label}" has no API key`)
      if (nt === 'copper' && !c.user_email) warnings.push(`Copper CRM node "${label}" has no user email`)
      if (nt === 'database' && !c.query) warnings.push(`Database node "${label}" has no SQL query`)
      if (nt === 'condition' && !c.field) warnings.push(`Condition node "${label}" has no field set`)
      if (nt === 'sub_workflow' && !c.workflow_id) warnings.push(`Sub-Workflow node "${label}" has no target workflow`)
      if (nt === 'graphql' && !c.url) warnings.push(`GraphQL node "${label}" has no endpoint URL`)
      if (nt === 'validate' && !c.source) warnings.push(`Validate node "${label}" has no source expression`)
      if (nt === 'agent' && !c.prompt_template) warnings.push(`Agent node "${label}" has no prompt template`)
      if (nt === 'code' && !c.code) warnings.push(`Code node "${label}" has no script`)
    }
    return warnings
  }

  const [showValidate, setShowValidate] = useState(false)
  const [validateWarnings, setValidateWarnings] = useState<string[]>([])

  // Publish latest draft version
  const handlePublish = async () => {
    if (!version || version.status === 'published') return
    const warnings = collectPublishWarnings()
    if (warnings.length > 0) {
      const msg = `Publishing with ${warnings.length} warning${warnings.length > 1 ? 's' : ''}:\n\n${warnings.map((w) => `• ${w}`).join('\n')}\n\nPublish anyway?`
      if (!window.confirm(msg)) return
    }
    setPublishing(true)
    try {
      const ver = await api.publishVersion(auth!.tenantId, version.id)
      setVersion(ver)
      const wf = await api.getWorkflow(auth!.tenantId, workflowId)
      setWorkflow(wf)
      const info = await api.getWebhook(auth!.tenantId, ver.id)
      setWebhookUrl(window.location.origin + info.url)
      setWebhookSecret(info.secret ?? null)
      toast(zh ? '版本已发布' : 'Version published')
    } catch (e) {
      toast(String(e), 'error')
    } finally {
      setPublishing(false)
    }
  }

  const [publishingAndRunning, setPublishingAndRunning] = useState(false)
  const handlePublishAndRun = async () => {
    if (!version || version.status === 'published') return
    const warnings = collectPublishWarnings()
    if (warnings.length > 0) {
      const msg = `Publishing with ${warnings.length} warning${warnings.length > 1 ? 's' : ''}:\n\n${warnings.map((w) => `• ${w}`).join('\n')}\n\nPublish and run anyway?`
      if (!window.confirm(msg)) return
    }
    setPublishingAndRunning(true)
    try {
      const ver = await api.publishVersion(auth!.tenantId, version.id)
      setVersion(ver)
      const wf = await api.getWorkflow(auth!.tenantId, workflowId)
      setWorkflow(wf)
      const info = await api.getWebhook(auth!.tenantId, ver.id)
      setWebhookUrl(window.location.origin + info.url)
      setWebhookSecret(info.secret ?? null)
      const rec = await api.startExecutionFromVersion(auth!.tenantId, ver.id, inputJson || '{}')
      setExecution(rec)
      toast(zh ? '已发布并开始运行' : 'Published and started run')
    } catch (e) {
      toast(String(e), 'error')
    } finally {
      setPublishingAndRunning(false)
    }
  }

  // Approve / Reject human approval gate
  const handleApprove = async (comment?: string) => {
    if (!execution) return
    try {
      await api.approveExecution(execution.id, comment)
    } catch (e) {
      toast(String(e), 'error')
    }
  }

  const handleReject = async (comment?: string) => {
    if (!execution) return
    try {
      await api.rejectExecution(execution.id, comment)
    } catch (e) {
      toast(String(e), 'error')
    }
  }

  // Run execution
  const handleRun = async () => {
    if (!workflow?.latest_version_id) return
    let parsed: unknown
    try { parsed = JSON.parse(inputJson) } catch { toast(zh ? '输入 JSON 格式无效' : 'Input JSON is invalid', 'error'); return }
    void parsed
    setRunning(true)
    setExecution(null)
    try {
      const result = await api.startExecutionFromWorkflow(auth!.tenantId, workflowId, inputJson, envSet === 'default' ? undefined : envSet, runLabel || undefined, callbackUrl || undefined, dryRun || undefined)
      setExecution(result)
      refreshHistory()
    } catch (e) {
      toast(String(e), 'error')
    } finally {
      setRunning(false)
    }
  }

  // Export current published version as JSON download
  const handleExport = async () => {
    if (!workflow?.latest_version_id) { toast(zh ? '请先发布一个版本后再导出' : 'Publish a version first to export', 'error'); return }
    try {
      const exported = await api.exportWorkflow(auth!.tenantId, workflowId)
      const blob = new Blob([JSON.stringify(exported, null, 2)], { type: 'application/json' })
      const url = URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = `${exported.name.replace(/\s+/g, '-').toLowerCase()}.json`
      a.click()
      URL.revokeObjectURL(url)
    } catch (e) {
      toast(String(e), 'error')
    }
  }

  // Open version history modal
  const handleShowVersions = async () => {
    setShowVersions(true)
    setLoadingVersions(true)
    try {
      const vers = await api.listVersions(auth!.tenantId, workflowId)
      setVersions(vers.sort((a, b) => b.version - a.version))
    } catch (e) {
      toast(String(e), 'error')
      setShowVersions(false)
    } finally {
      setLoadingVersions(false)
    }
  }

  // Load a specific version's graph into the canvas
  const handleLoadVersion = async (versionId: string) => {
    try {
      const ver = await api.getVersion(auth!.tenantId, versionId)
      setVersion(ver)
      const { nodes: fn, edges: fe } = graphFromApi(ver.graph.nodes, ver.graph.edges)
      setNodes(fn)
      setEdges(fe)
      setInputSchema(ver.graph.input_schema ?? [])
      setSelectedNodeId(null)
      setShowVersions(false)
      toast(zh ? `已加载 v${ver.version}` : `Loaded v${ver.version}`)
    } catch (e) {
      toast(String(e), 'error')
    }
  }

  // Rollback to a historical version (creates new draft version)
  const handleRollback = async (versionId: string, versionNum: number) => {
    if (!window.confirm(zh ? `回滚到 v${versionNum}？这将基于 v${versionNum} 创建一个新草稿版本。` : `Rollback to v${versionNum}? This creates a new draft version based on v${versionNum}.`)) return
    setRollingBack(versionId)
    try {
      const newVer = await api.rollbackVersion(auth!.tenantId, workflowId, versionId)
      setVersions((prev) => [newVer, ...prev])
      setVersion(newVer)
      const { nodes: fn, edges: fe } = graphFromApi(newVer.graph.nodes, newVer.graph.edges)
      setNodes(fn)
      setEdges(fe)
      setInputSchema(newVer.graph.input_schema ?? [])
      setSelectedNodeId(null)
      setShowVersions(false)
      toast(zh ? `已回滚到 v${versionNum} — 新草稿 v${newVer.version} 已创建` : `Rolled back to v${versionNum} — new draft v${newVer.version} created`)
    } catch (e) {
      toast(String(e), 'error')
    } finally {
      setRollingBack(null)
    }
  }

  // Rename workflow
  const handleRename = async () => {
    if (!newName.trim() || newName === workflow?.name) { setRenaming(false); return }
    try {
      const wf = await api.renameWorkflow(auth!.tenantId, workflowId, newName.trim())
      setWorkflow(wf)
      toast(zh ? '已重命名' : 'Renamed')
    } catch (e) {
      toast(String(e), 'error')
    } finally {
      setRenaming(false)
    }
  }

  // Update description
  const handleSaveDescription = async () => {
    if (!workflow) return
    try {
      const wf = await api.updateWorkflowDescription(auth!.tenantId, workflowId, workflow.name, newDescription.trim())
      setWorkflow(wf)
      toast(zh ? '描述已保存' : 'Description saved')
    } catch (e) {
      toast(String(e), 'error')
    } finally {
      setEditingDescription(false)
    }
  }

  const handleSaveRateLimit = async () => {
    if (!workflow) return
    const limit = newRateLimitInput.trim() === '' ? null : parseInt(newRateLimitInput.trim(), 10)
    if (newRateLimitInput.trim() !== '' && (isNaN(limit!) || limit! <= 0)) {
      toast(zh ? '速率限制必须是正整数' : 'Rate limit must be a positive integer', 'error')
      return
    }
    try {
      const wf = await api.updateWorkflowRateLimit(auth!.tenantId, workflowId, workflow.name, limit)
      setWorkflow(wf)
      toast(limit == null ? (zh ? '速率限制已清除' : 'Rate limit cleared') : (zh ? `速率限制设为每小时 ${limit} 次` : `Rate limit set to ${limit}/hr`))
    } catch (e) {
      toast(String(e), 'error')
    } finally {
      setEditingRateLimit(false)
    }
  }

  const handleSaveMaxConcurrent = async () => {
    if (!workflow) return
    const limit = newMaxConcurrentInput.trim() === '' ? null : parseInt(newMaxConcurrentInput.trim(), 10)
    if (newMaxConcurrentInput.trim() !== '' && (isNaN(limit!) || limit! <= 0)) {
      toast(zh ? '并发限制必须是正整数' : 'Concurrent limit must be a positive integer', 'error')
      return
    }
    try {
      const wf = await api.updateWorkflowMaxConcurrentRuns(auth!.tenantId, workflowId, workflow.name, limit)
      setWorkflow(wf)
      toast(limit == null ? (zh ? '并发限制已清除' : 'Concurrent limit cleared') : (zh ? `并发限制设为 ${limit}` : `Concurrent limit set to ${limit}`))
    } catch (e) {
      toast(String(e), 'error')
    } finally {
      setEditingMaxConcurrent(false)
    }
  }

  const handleSaveBudget = async () => {
    if (!workflow) return
    const budget = newBudgetInput.trim() === '' ? null : parseFloat(newBudgetInput.trim())
    if (newBudgetInput.trim() !== '' && (isNaN(budget!) || budget! <= 0)) {
      toast(zh ? '预算必须是正数（美元）' : 'Budget must be a positive number (USD)', 'error')
      return
    }
    try {
      const wf = await api.updateWorkflowBudget(auth!.tenantId, workflowId, workflow.name, budget)
      setWorkflow(wf)
      toast(budget == null ? (zh ? 'AI 成本预算已清除' : 'AI cost budget cleared') : (zh ? `预算设为 $${budget.toFixed(2)}` : `Budget set to $${budget.toFixed(2)}`))
    } catch (e) { toast(String(e), 'error') }
    setEditingBudget(false)
  }

  const handleAddTag = async (tag: string) => {
    if (!workflow) return
    const trimmed = tag.trim().toLowerCase().replace(/\s+/g, '-').slice(0, 40)
    if (!trimmed || workflow.tags?.includes(trimmed)) { setAddingTag(false); setNewTagInput(''); return }
    const newTags = [...(workflow.tags ?? []), trimmed]
    try {
      const wf = await api.updateWorkflowTags(auth!.tenantId, workflowId, workflow.name, newTags)
      setWorkflow(wf)
    } catch (e) { toast(String(e), 'error') }
    setAddingTag(false); setNewTagInput('')
  }

  const handleRemoveTag = async (tag: string) => {
    if (!workflow) return
    const newTags = (workflow.tags ?? []).filter(t => t !== tag)
    try {
      const wf = await api.updateWorkflowTags(auth!.tenantId, workflowId, workflow.name, newTags)
      setWorkflow(wf)
    } catch (e) { toast(String(e), 'error') }
  }

  const handleSaveSla = async () => {
    if (!workflow) return
    const secs = newSlaInput.trim() === '' ? null : parseInt(newSlaInput.trim(), 10)
    if (newSlaInput.trim() !== '' && (isNaN(secs!) || secs! <= 0)) {
      toast(zh ? 'SLA 必须是正整数秒数' : 'SLA must be a positive integer (seconds)', 'error')
      return
    }
    try {
      const wf = await api.updateWorkflowSla(auth!.tenantId, workflowId, workflow.name, secs)
      setWorkflow(wf)
      toast(secs == null ? (zh ? 'SLA 已清除' : 'SLA cleared') : (zh ? `SLA 设为 ${secs}s` : `SLA set to ${secs}s`))
    } catch (e) {
      toast(String(e), 'error')
    } finally {
      setEditingSla(false)
    }
  }

  const nodeStatuses = useMemo<Record<string, NodeExecutionRecord>>(() => {
    if (!execution) return {}
    return Object.fromEntries(execution.node_results.map((r) => [r.node_id, r]))
  }, [execution])

  const handleArchive = async () => {
    if (!workflow) return
    // Check for dependent workflows before archiving
    try {
      const { edges } = await api.getWorkflowDeps(auth!.tenantId)
      const dependents = edges.filter((e) => e.to_workflow_id === workflowId)
      if (dependents.length > 0) {
        const depList = [...new Set(dependents.map((e) => e.from_workflow_id))].slice(0, 3).join(', ')
        const msg = zh
          ? `⚠ 以下工作流引用了此工作流：${depList}${dependents.length > 3 ? ` 等 ${dependents.length} 个` : ''}。归档后这些工作流可能无法正常运行。是否继续？`
          : `⚠ ${dependents.length} workflow${dependents.length !== 1 ? 's' : ''} reference this workflow (${depList}${dependents.length > 3 ? ` and ${dependents.length - 3} more` : ''}). Archiving may break those workflows. Continue?`
        if (!window.confirm(msg)) return
      } else if (!window.confirm(zh ? '归档此工作流？它将不再按计划或 Webhook 触发运行。' : 'Archive this workflow? It will no longer run on schedules or webhooks.')) {
        return
      }
    } catch {
      if (!window.confirm(zh ? '归档此工作流？它将不再按计划或 Webhook 触发运行。' : 'Archive this workflow? It will no longer run on schedules or webhooks.')) return
    }
    try {
      const wf = await api.archiveWorkflow(auth!.tenantId, workflowId)
      setWorkflow(wf)
      toast(zh ? '工作流已归档' : 'Workflow archived')
    } catch (e) { toast(String(e), 'error') }
  }

  const handleRestore = async () => {
    try {
      const wf = await api.restoreWorkflow(auth!.tenantId, workflowId)
      setWorkflow(wf)
      toast(zh ? '工作流已恢复为草稿' : 'Workflow restored to draft')
    } catch (e) { toast(String(e), 'error') }
  }

  const handleLockToggle = async () => {
    if (!workflow) return
    try {
      const wf = workflow.locked
        ? await api.unlockWorkflow(auth!.tenantId, workflowId)
        : await api.lockWorkflow(auth!.tenantId, workflowId)
      setWorkflow(wf)
      toast(wf.locked ? (zh ? '工作流已锁定 — 解锁前禁止保存' : 'Workflow locked — saves blocked until unlocked') : (zh ? '工作流已解锁' : 'Workflow unlocked'))
    } catch (e) { toast(String(e), 'error') }
  }

  const selectedNode = nodes.find((n) => n.id === selectedNodeId) ?? null

  const upstreamNodes = useMemo(() => {
    if (!selectedNodeId) return []
    const sourcIds = edges.filter((e) => e.target === selectedNodeId).map((e) => e.source)
    return nodes.filter((n) => sourcIds.includes(n.id))
  }, [selectedNodeId, edges, nodes])

  const canRun = !!workflow?.latest_version_id && workflow.status !== 'archived'
  const canPublish = !!version && version.status === 'draft'

  // Keep shortcut refs up to date (avoids stale closures in the keydown listener)
  handleSaveRef.current = handleSave
  handleRunRef.current = canRun ? handleRun : () => {}
  handleDuplicateNodeRef.current = handleDuplicateNode


  // Live structural warning count for the stats bar
  const liveWarningCount = useMemo(() => {
    let n = 0
    const triggers = nodes.filter((nd) => nd.data.nodeType === 'trigger')
    if (triggers.length === 0) n++
    if (triggers.length > 1) n++
    const connectedSources = new Set(edges.map((e) => e.source))
    const connectedTargets = new Set(edges.map((e) => e.target))
    for (const nd of nodes) {
      const nt = nd.data.nodeType
      const c = nd.data.config ?? {}
      if (nt !== 'trigger' && nt !== 'note' && !connectedTargets.has(nd.id)) n++
      if (nt !== 'note' && nt !== 'approval' && !connectedSources.has(nd.id)) n++
      if (nt === 'http' && !c.url) n++
      if (nt === 'openai' && !c.api_key) n++
      if (nt === 'gemini' && !c.api_key) n++
      if (nt === 'claude' && !c.api_key) n++
      if (nt === 'slack' && !c.webhook_url) n++
      if (nt === 'email' && (!c.to || !c.api_key)) n++
      if (nt === 'database' && !c.query) n++
      if (nt === 'condition' && !c.field) n++
      if (nt === 'sub_workflow' && !c.workflow_id) n++
      if (nt === 'graphql' && !c.url) n++
      if (nt === 'validate' && !c.source) n++
      if (nt === 'github' && (!c.token || !c.endpoint)) n++
      if (nt === 'webhook' && !c.url) n++
      if (nt === 'jira' && (!c.base_url || !c.token || !c.endpoint)) n++
      if (nt === 'notion' && (!c.token || !c.endpoint)) n++
      if (nt === 'linear' && (!c.token || !c.query)) n++
      if (nt === 'airtable' && (!c.token || !c.base_id || !c.table)) n++
      if (nt === 'for_each' && !c.workflow_id) n++
      if (nt === 'discord' && (!c.webhook_url || !c.content)) n++
      if (nt === 'teams' && (!c.webhook_url || !c.text)) n++
      if (nt === 'sheets' && (!c.token || !c.spreadsheet_id)) n++
      if (nt === 'xml' && !c.source) n++
      if (nt === 'yaml' && !c.source) n++
      if (nt === 'twilio' && (!c.account_sid || !c.auth_token || !c.to || !c.from)) n++
      if (nt === 'stripe' && (!c.api_key || !c.endpoint)) n++
      if (nt === 'crypto' && !c.source) n++
      if (nt === 'hubspot' && (!c.token || !c.endpoint)) n++
      if (nt === 'zendesk' && (!c.subdomain || !c.token || !c.endpoint)) n++
      if (nt === 'redis' && (!c.url || !c.key)) n++
      if (nt === 'elasticsearch' && !c.url) n++
      if (nt === 'pagerduty' && (!c.routing_key || !c.summary)) n++
      if (nt === 'handlebars' && !c.template) n++
      if (nt === 'math' && !c.operation) n++
      if (nt === 'array_utils' && !c.operation) n++
      if (nt === 'shopify' && (!c.shop || !c.token)) n++
      if (nt === 'datadog' && (!c.api_key || !c.endpoint)) n++
      if (nt === 'salesforce' && (!c.token || !c.instance_url)) n++
      if (nt === 'freshdesk' && (!c.api_key || !c.domain || !c.endpoint)) n++
      if (nt === 'mailgun' && (!c.api_key || !c.domain || !c.to)) n++
      if (nt === 'asana' && (!c.token || !c.endpoint)) n++
      if (nt === 'servicenow' && (!c.instance || !c.username || !c.password)) n++
      if (nt === 'confluence' && (!c.base_url || !c.endpoint)) n++
      if (nt === 'bitbucket' && (!c.username || !c.app_password || !c.endpoint)) n++
      if (nt === 'azure_devops' && (!c.pat || !c.organization || !c.endpoint)) n++
      if (nt === 'twitch' && (!c.client_id || !c.access_token || !c.endpoint)) n++
      if (nt === 'figma' && (!c.token || !c.endpoint)) n++
      if (nt === 'dropbox' && !c.token) n++
      if (nt === 'cloudflare' && (!c.api_token || !c.endpoint)) n++
      if (nt === 'box' && (!c.token || !c.endpoint)) n++
      if (nt === 'okta' && (!c.domain || !c.token || !c.endpoint)) n++
      if (nt === 'zoom' && (!c.token || !c.endpoint)) n++
      if (nt === 'spotify' && (!c.token || !c.endpoint)) n++
      if (nt === 'typeform' && (!c.token || !c.endpoint)) n++
      if (nt === 'webflow' && (!c.token || !c.endpoint)) n++
      if (nt === 'intercom' && (!c.token || !c.endpoint)) n++
      if (nt === 'pipedrive' && (!c.api_token || !c.endpoint)) n++
      if (nt === 'trello' && (!c.api_key || !c.token || !c.endpoint)) n++
      if (nt === 'monday' && (!c.token || !c.query)) n++
      if (nt === 'clickup' && (!c.token || !c.endpoint)) n++
      if (nt === 'amplitude' && (!c.api_key || !c.secret_key)) n++
      if (nt === 'mixpanel' && (!c.project_token || !c.api_secret)) n++
      if (nt === 'segment' && !c.write_key) n++
      if (nt === 'sendgrid' && (!c.api_key || !c.endpoint)) n++
      if (nt === 'braintree' && (!c.merchant_id || !c.public_key || !c.private_key || !c.endpoint)) n++
      if (nt === 'paypal' && (!c.client_id || !c.client_secret || !c.endpoint)) n++
      if (nt === 'razorpay' && (!c.key_id || !c.key_secret || !c.endpoint)) n++
      if (nt === 'firebase' && (!c.project_id || !c.id_token || !c.endpoint)) n++
      if (nt === 'supabase' && (!c.project_url || !c.api_key || !c.endpoint)) n++
      if (nt === 'mailchimp' && (!c.api_key || !c.endpoint)) n++
      if (nt === 'activecampaign' && (!c.api_key || !c.base_url || !c.endpoint)) n++
      if (nt === 'klaviyo' && (!c.api_key || !c.endpoint)) n++
      if (nt === 'resend' && (!c.api_key || !c.endpoint)) n++
      if (nt === 'contentful' && (!c.access_token || !c.space_id || !c.endpoint)) n++
      if (nt === 'algolia' && (!c.app_id || !c.api_key || !c.endpoint)) n++
      if (nt === 'postmark' && (!c.server_token || !c.endpoint)) n++
      if (nt === 'vonage' && (!c.api_key || !c.api_secret)) n++
      if (nt === 'telegram' && (!c.bot_token || !c.chat_id)) n++
      if (nt === 'replicate' && (!c.api_token || !c.version)) n++
      if (nt === 'mistral' && !c.api_key) n++
      if (nt === 'whatsapp' && (!c.access_token || !c.phone_number_id || !c.to)) n++
      if (nt === 'googledocs' && !c.access_token) n++
      if (nt === 'perplexity' && !c.api_key) n++
      if (nt === 'cohere' && !c.api_key) n++
      if (nt === 'googledrive' && !c.access_token) n++
      if (nt === 'woocommerce' && (!c.consumer_key || !c.site_url)) n++
      if (nt === 'pinecone' && (!c.api_key || !c.index_host)) n++
      if (nt === 'togetherai' && !c.api_key) n++
      if (nt === 'awss3' && (!c.access_key_id || !c.bucket)) n++
      if (nt === 'huggingface' && (!c.api_token || !c.model)) n++
      if (nt === 'groq' && !c.api_key) n++
      if (nt === 'openrouter' && !c.api_key) n++
      if (nt === 'qdrant' && (!c.url || !c.collection)) n++
      if (nt === 'cloudinary' && !c.cloud_name) n++
      if (nt === 'gcal' && !c.access_token) n++
      if (nt === 'docusign' && (!c.access_token || !c.account_id)) n++
      if (nt === 'xero' && (!c.access_token || !c.tenant_id)) n++
      if (nt === 'calendly' && !c.api_key) n++
      if (nt === 'apify' && !c.api_token) n++
      if (nt === 'ganalytics' && (!c.access_token || !c.property_id)) n++
      if (nt === 'neon' && !c.api_key) n++
      if (nt === 'copper' && (!c.api_key || !c.user_email)) n++
    }
    return n
  }, [nodes, edges])

  // Live warning indicators — same logic as pre-publish checks
  const warningNodeIds = useMemo(() => {
    const ids = new Set<string>()
    for (const node of nodes) {
      const nt = node.data.nodeType
      const c = node.data.config ?? {}
      if (nt === 'http' && !c.url) ids.add(node.id)
      if (nt === 'openai' && !c.api_key) ids.add(node.id)
      if (nt === 'gemini' && !c.api_key) ids.add(node.id)
      if (nt === 'claude' && !c.api_key) ids.add(node.id)
      if (nt === 'slack' && !c.webhook_url) ids.add(node.id)
      if (nt === 'email' && (!c.to || !c.api_key)) ids.add(node.id)
      if (nt === 'database' && !c.query) ids.add(node.id)
      if (nt === 'condition' && !c.field) ids.add(node.id)
      if (nt === 'sub_workflow' && !c.workflow_id) ids.add(node.id)
      if (nt === 'graphql' && !c.url) ids.add(node.id)
      if (nt === 'validate' && !c.source) ids.add(node.id)
      if (nt === 'github' && (!c.token || !c.endpoint)) ids.add(node.id)
      if (nt === 'webhook' && !c.url) ids.add(node.id)
      if (nt === 'jira' && (!c.base_url || !c.token || !c.endpoint)) ids.add(node.id)
      if (nt === 'notion' && (!c.token || !c.endpoint)) ids.add(node.id)
      if (nt === 'linear' && (!c.token || !c.query)) ids.add(node.id)
      if (nt === 'airtable' && (!c.token || !c.base_id || !c.table)) ids.add(node.id)
      if (nt === 'for_each' && !c.workflow_id) ids.add(node.id)
      if (nt === 'discord' && (!c.webhook_url || !c.content)) ids.add(node.id)
      if (nt === 'teams' && (!c.webhook_url || !c.text)) ids.add(node.id)
      if (nt === 'sheets' && (!c.token || !c.spreadsheet_id)) ids.add(node.id)
      if (nt === 'xml' && !c.source) ids.add(node.id)
      if (nt === 'yaml' && !c.source) ids.add(node.id)
      if (nt === 'twilio' && (!c.account_sid || !c.auth_token || !c.to || !c.from)) ids.add(node.id)
      if (nt === 'stripe' && (!c.api_key || !c.endpoint)) ids.add(node.id)
      if (nt === 'crypto' && !c.source) ids.add(node.id)
      if (nt === 'hubspot' && (!c.token || !c.endpoint)) ids.add(node.id)
      if (nt === 'zendesk' && (!c.subdomain || !c.token || !c.endpoint)) ids.add(node.id)
      if (nt === 'redis' && (!c.url || !c.key)) ids.add(node.id)
      if (nt === 'elasticsearch' && !c.url) ids.add(node.id)
      if (nt === 'pagerduty' && (!c.routing_key || !c.summary)) ids.add(node.id)
      if (nt === 'handlebars' && !c.template) ids.add(node.id)
      if (nt === 'math' && !c.operation) ids.add(node.id)
      if (nt === 'array_utils' && !c.operation) ids.add(node.id)
      if (nt === 'shopify' && (!c.shop || !c.token)) ids.add(node.id)
      if (nt === 'datadog' && (!c.api_key || !c.endpoint)) ids.add(node.id)
      if (nt === 'salesforce' && (!c.token || !c.instance_url)) ids.add(node.id)
      if (nt === 'freshdesk' && (!c.api_key || !c.domain || !c.endpoint)) ids.add(node.id)
      if (nt === 'mailgun' && (!c.api_key || !c.domain || !c.to)) ids.add(node.id)
      if (nt === 'asana' && (!c.token || !c.endpoint)) ids.add(node.id)
      if (nt === 'servicenow' && (!c.instance || !c.username || !c.password)) ids.add(node.id)
      if (nt === 'confluence' && (!c.base_url || !c.endpoint)) ids.add(node.id)
      if (nt === 'bitbucket' && (!c.username || !c.app_password || !c.endpoint)) ids.add(node.id)
      if (nt === 'azure_devops' && (!c.pat || !c.organization || !c.endpoint)) ids.add(node.id)
      if (nt === 'twitch' && (!c.client_id || !c.access_token || !c.endpoint)) ids.add(node.id)
      if (nt === 'figma' && (!c.token || !c.endpoint)) ids.add(node.id)
      if (nt === 'dropbox' && !c.token) ids.add(node.id)
      if (nt === 'cloudflare' && (!c.api_token || !c.endpoint)) ids.add(node.id)
      if (nt === 'box' && (!c.token || !c.endpoint)) ids.add(node.id)
      if (nt === 'okta' && (!c.domain || !c.token || !c.endpoint)) ids.add(node.id)
      if (nt === 'zoom' && (!c.token || !c.endpoint)) ids.add(node.id)
      if (nt === 'spotify' && (!c.token || !c.endpoint)) ids.add(node.id)
      if (nt === 'typeform' && (!c.token || !c.endpoint)) ids.add(node.id)
      if (nt === 'webflow' && (!c.token || !c.endpoint)) ids.add(node.id)
      if (nt === 'intercom' && (!c.token || !c.endpoint)) ids.add(node.id)
      if (nt === 'pipedrive' && (!c.api_token || !c.endpoint)) ids.add(node.id)
      if (nt === 'trello' && (!c.api_key || !c.token || !c.endpoint)) ids.add(node.id)
      if (nt === 'monday' && (!c.token || !c.query)) ids.add(node.id)
      if (nt === 'clickup' && (!c.token || !c.endpoint)) ids.add(node.id)
      if (nt === 'amplitude' && (!c.api_key || !c.secret_key)) ids.add(node.id)
      if (nt === 'mixpanel' && (!c.project_token || !c.api_secret)) ids.add(node.id)
      if (nt === 'segment' && !c.write_key) ids.add(node.id)
      if (nt === 'sendgrid' && (!c.api_key || !c.endpoint)) ids.add(node.id)
      if (nt === 'braintree' && (!c.merchant_id || !c.public_key || !c.private_key || !c.endpoint)) ids.add(node.id)
      if (nt === 'paypal' && (!c.client_id || !c.client_secret || !c.endpoint)) ids.add(node.id)
      if (nt === 'razorpay' && (!c.key_id || !c.key_secret || !c.endpoint)) ids.add(node.id)
      if (nt === 'firebase' && (!c.project_id || !c.id_token || !c.endpoint)) ids.add(node.id)
      if (nt === 'supabase' && (!c.project_url || !c.api_key || !c.endpoint)) ids.add(node.id)
      if (nt === 'mailchimp' && (!c.api_key || !c.endpoint)) ids.add(node.id)
      if (nt === 'activecampaign' && (!c.api_key || !c.base_url || !c.endpoint)) ids.add(node.id)
      if (nt === 'klaviyo' && (!c.api_key || !c.endpoint)) ids.add(node.id)
      if (nt === 'resend' && (!c.api_key || !c.endpoint)) ids.add(node.id)
      if (nt === 'contentful' && (!c.access_token || !c.space_id || !c.endpoint)) ids.add(node.id)
      if (nt === 'algolia' && (!c.app_id || !c.api_key || !c.endpoint)) ids.add(node.id)
      if (nt === 'postmark' && (!c.server_token || !c.endpoint)) ids.add(node.id)
      if (nt === 'vonage' && (!c.api_key || !c.api_secret)) ids.add(node.id)
      if (nt === 'telegram' && (!c.bot_token || !c.chat_id)) ids.add(node.id)
      if (nt === 'replicate' && (!c.api_token || !c.version)) ids.add(node.id)
      if (nt === 'mistral' && !c.api_key) ids.add(node.id)
      if (nt === 'whatsapp' && (!c.access_token || !c.phone_number_id || !c.to)) ids.add(node.id)
      if (nt === 'googledocs' && !c.access_token) ids.add(node.id)
      if (nt === 'perplexity' && !c.api_key) ids.add(node.id)
      if (nt === 'cohere' && !c.api_key) ids.add(node.id)
      if (nt === 'googledrive' && !c.access_token) ids.add(node.id)
      if (nt === 'woocommerce' && (!c.consumer_key || !c.site_url)) ids.add(node.id)
      if (nt === 'pinecone' && (!c.api_key || !c.index_host)) ids.add(node.id)
      if (nt === 'togetherai' && !c.api_key) ids.add(node.id)
      if (nt === 'awss3' && (!c.access_key_id || !c.bucket)) ids.add(node.id)
      if (nt === 'huggingface' && (!c.api_token || !c.model)) ids.add(node.id)
      if (nt === 'groq' && !c.api_key) ids.add(node.id)
      if (nt === 'openrouter' && !c.api_key) ids.add(node.id)
      if (nt === 'qdrant' && (!c.url || !c.collection)) ids.add(node.id)
      if (nt === 'cloudinary' && !c.cloud_name) ids.add(node.id)
      if (nt === 'gcal' && !c.access_token) ids.add(node.id)
      if (nt === 'docusign' && (!c.access_token || !c.account_id)) ids.add(node.id)
      if (nt === 'xero' && (!c.access_token || !c.tenant_id)) ids.add(node.id)
      if (nt === 'calendly' && !c.api_key) ids.add(node.id)
      if (nt === 'apify' && !c.api_token) ids.add(node.id)
      if (nt === 'ganalytics' && (!c.access_token || !c.property_id)) ids.add(node.id)
      if (nt === 'neon' && !c.api_key) ids.add(node.id)
      if (nt === 'copper' && (!c.api_key || !c.user_email)) ids.add(node.id)
    }
    return ids
  }, [nodes])

  // Build heatmap color map: nodeId → CSS color based on failure rate
  const nodeHeatmapMap = useMemo<Map<string, string>>(() => {
    if (!showNodeHeatmap || nodeStats.length === 0) return new Map()
    const map = new Map<string, string>()
    for (const ns of nodeStats) {
      const rate = ns.total > 0 ? ns.succeeded / ns.total : 1
      const color = rate >= 0.9 ? 'rgba(34,197,94,0.25)' : rate >= 0.7 ? 'rgba(245,158,11,0.3)' : 'rgba(239,68,68,0.3)'
      map.set(ns.node_id, color)
    }
    return map
  }, [showNodeHeatmap, nodeStats])

  const isDirty = useMemo(() => {
    if (!version) return false
    const { nodes: apiNodes, edges: apiEdges } = fromFlowGraph(nodes, edges)
    const currentSig = JSON.stringify({ nodes: apiNodes.map((n) => ({ id: n.id, type: n.type, config: n.config })).sort((a, b) => a.id.localeCompare(b.id)), edges: apiEdges.map((e) => `${e.source}→${e.target}:${e.condition_label ?? ''}`).sort() })
    const savedSig = JSON.stringify({ nodes: version.graph.nodes.map((n) => ({ id: n.id, type: n.type, config: n.config })).sort((a, b) => a.id.localeCompare(b.id)), edges: version.graph.edges.map((e) => `${e.source}→${e.target}:${e.condition_label ?? ''}`).sort() })
    return currentSig !== savedSig
  }, [nodes, edges, version])

  // Autosave: save a draft every 30s when there are unsaved changes
  const isDirtyRef = useRef(false)
  isDirtyRef.current = isDirty
  useEffect(() => {
    const timer = setInterval(() => {
      if (isDirtyRef.current) handleSaveRef.current()
    }, 30_000)
    return () => clearInterval(timer)
  }, [])

  return (
    <div className="app">
      {/* ── Top bar ─────────────────────────────────────────────── */}
      <header className="topbar">
        <button className="btn btn-sm btn-icon" onClick={onBack} title={zh ? '返回列表' : 'Back to list'}>←</button>
        <span className="topbar-sep">|</span>

        {renaming ? (
          <input
            autoFocus
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            onBlur={handleRename}
            onKeyDown={(e) => { if (e.key === 'Enter') handleRename(); if (e.key === 'Escape') setRenaming(false) }}
            style={{ width: 200, fontSize: 14, fontWeight: 600 }}
          />
        ) : (
          <span
            className="topbar-title"
            style={{ cursor: 'pointer' }}
            onClick={() => setRenaming(true)}
            title={zh ? '点击重命名' : 'Click to rename'}
          >
            {workflow?.name ?? '…'}
          </span>
        )}

        {workflow && (
          <span className={`badge badge-${workflow.status}`}>{workflow.status}</span>
        )}
        {workflow && (
          editingDescription ? (
            <input
              autoFocus
              value={newDescription}
              onChange={(e) => setNewDescription(e.target.value)}
              onBlur={handleSaveDescription}
              onKeyDown={(e) => { if (e.key === 'Enter') handleSaveDescription(); if (e.key === 'Escape') setEditingDescription(false) }}
              placeholder={zh ? '添加描述…' : 'Add a description…'}
              style={{ width: 260, fontSize: 12, color: 'var(--muted)', fontStyle: 'normal' }}
            />
          ) : (
            <span
              style={{ fontSize: 12, color: 'var(--muted)', cursor: 'pointer', fontStyle: workflow.description ? 'normal' : 'italic', maxWidth: 260, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}
              onClick={() => { setEditingDescription(true); setNewDescription(workflow.description ?? '') }}
              title={workflow.description ? (zh ? '点击编辑描述' : 'Click to edit description') : (zh ? '点击添加描述' : 'Click to add description')}
            >
              {workflow.description ?? (zh ? '添加描述…' : 'Add description…')}
            </span>
          )
        )}
        {workflow && (
          editingSla ? (
            <input
              autoFocus
              type="number"
              min={1}
              value={newSlaInput}
              onChange={(e) => setNewSlaInput(e.target.value)}
              onBlur={handleSaveSla}
              onKeyDown={(e) => { if (e.key === 'Enter') handleSaveSla(); if (e.key === 'Escape') setEditingSla(false) }}
              placeholder={zh ? 'SLA秒数（留空清除）' : 'SLA seconds (blank to clear)'}
              style={{ width: 160, fontSize: 12, color: 'var(--muted)' }}
            />
          ) : (
            <span
              style={{ fontSize: 12, color: 'var(--muted)', cursor: 'pointer' }}
              onClick={() => { setEditingSla(true); setNewSlaInput(workflow.sla_seconds != null ? String(workflow.sla_seconds) : '') }}
              title={zh ? '点击设置 SLA（超时告警阈值，单位秒）' : 'Click to set SLA threshold (seconds). A notification fires if execution exceeds this duration.'}
            >
              {workflow.sla_seconds != null
                ? (zh ? `SLA: ${workflow.sla_seconds}s` : `SLA: ${workflow.sla_seconds}s`)
                : (zh ? '+ SLA' : '+ SLA')}
            </span>
          )
        )}
        {workflow && (
          editingRateLimit ? (
            <input
              autoFocus
              type="number"
              min={1}
              value={newRateLimitInput}
              onChange={(e) => setNewRateLimitInput(e.target.value)}
              onBlur={handleSaveRateLimit}
              onKeyDown={(e) => { if (e.key === 'Enter') handleSaveRateLimit(); if (e.key === 'Escape') setEditingRateLimit(false) }}
              placeholder={zh ? '每小时最大运行次数' : 'Max runs/hour (blank to clear)'}
              style={{ width: 160, fontSize: 12, color: 'var(--muted)' }}
            />
          ) : (
            <span
              style={{ fontSize: 12, color: 'var(--muted)', cursor: 'pointer' }}
              onClick={() => { setEditingRateLimit(true); setNewRateLimitInput(workflow.max_runs_per_hour != null ? String(workflow.max_runs_per_hour) : '') }}
              title={zh ? '点击设置每小时最大运行次数（速率限制）' : 'Click to set max executions per hour (rate limit). Returns 429 when exceeded.'}
            >
              {workflow.max_runs_per_hour != null
                ? (zh ? `限速: ${workflow.max_runs_per_hour}/hr` : `Limit: ${workflow.max_runs_per_hour}/hr`)
                : (zh ? '+ 限速' : '+ Rate limit')}
            </span>
          )
        )}
        {workflow && (
          editingMaxConcurrent ? (
            <input
              autoFocus
              type="number"
              min={1}
              value={newMaxConcurrentInput}
              onChange={(e) => setNewMaxConcurrentInput(e.target.value)}
              onBlur={handleSaveMaxConcurrent}
              onKeyDown={(e) => { if (e.key === 'Enter') handleSaveMaxConcurrent(); if (e.key === 'Escape') setEditingMaxConcurrent(false) }}
              placeholder={zh ? '最大并发运行数（留空清除）' : 'Max concurrent runs (blank to clear)'}
              style={{ width: 180, fontSize: 12, color: 'var(--muted)' }}
            />
          ) : (
            <span
              style={{ fontSize: 12, color: 'var(--muted)', cursor: 'pointer' }}
              onClick={() => { setEditingMaxConcurrent(true); setNewMaxConcurrentInput(workflow.max_concurrent_runs != null ? String(workflow.max_concurrent_runs) : '') }}
              title={zh ? '点击设置最大并发运行数（防止同一工作流同时有太多运行实例）' : 'Click to set max concurrent runs. Returns 429 when this many runs are already active for this workflow.'}
            >
              {workflow.max_concurrent_runs != null
                ? (zh ? `并发: ${workflow.max_concurrent_runs}` : `Concur: ${workflow.max_concurrent_runs}`)
                : (zh ? '+ 并发限制' : '+ Concur limit')}
            </span>
          )
        )}
        {workflow && (workflow.tags ?? []).map(tag => (
          <span key={tag} style={{ display: 'inline-flex', alignItems: 'center', gap: 2, background: 'var(--border)', borderRadius: 4, padding: '1px 5px', fontSize: 11, color: 'var(--fg)' }}>
            #{tag}
            <button
              onClick={() => handleRemoveTag(tag)}
              style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--muted)', fontSize: 11, padding: '0 1px', lineHeight: 1 }}
              title={zh ? `移除标签 #${tag}` : `Remove tag #${tag}`}
            >×</button>
          </span>
        ))}
        {workflow && (
          addingTag ? (
            <input
              autoFocus
              value={newTagInput}
              onChange={(e) => setNewTagInput(e.target.value)}
              onBlur={() => { if (newTagInput.trim()) handleAddTag(newTagInput); else { setAddingTag(false); setNewTagInput('') } }}
              onKeyDown={(e) => { if (e.key === 'Enter') handleAddTag(newTagInput); if (e.key === 'Escape') { setAddingTag(false); setNewTagInput('') } }}
              placeholder={zh ? '标签名…' : 'tag name…'}
              style={{ width: 100, fontSize: 11 }}
            />
          ) : (
            <span
              style={{ fontSize: 11, color: 'var(--muted)', cursor: 'pointer' }}
              onClick={() => setAddingTag(true)}
              title={zh ? '添加标签' : 'Add tag'}
            >+ tag</span>
          )
        )}
        {workflow && (
          editingBudget ? (
            <input
              autoFocus
              type="number"
              min={0.01}
              step={0.01}
              value={newBudgetInput}
              onChange={(e) => setNewBudgetInput(e.target.value)}
              onBlur={handleSaveBudget}
              onKeyDown={(e) => { if (e.key === 'Enter') handleSaveBudget(); if (e.key === 'Escape') setEditingBudget(false) }}
              placeholder={zh ? 'AI 预算（美元，留空清除）' : 'AI budget USD (blank to clear)'}
              style={{ width: 160, fontSize: 12, color: 'var(--muted)' }}
            />
          ) : (
            <span
              style={{ fontSize: 12, color: 'var(--muted)', cursor: 'pointer' }}
              onClick={() => { setEditingBudget(true); setNewBudgetInput(workflow.budget_usd != null ? String(workflow.budget_usd) : '') }}
              title={zh ? '点击设置每次执行的 AI 成本预算（超出时发送通知）' : 'Click to set per-run AI cost budget. Notification fires when estimated cost exceeds this amount.'}
            >
              {workflow.budget_usd != null
                ? (zh ? `预算: $${workflow.budget_usd.toFixed(2)}` : `Budget: $${workflow.budget_usd.toFixed(2)}`)
                : (zh ? '+ AI 预算' : '+ AI budget')}
            </span>
          )
        )}
        {version && (
          <span style={{ color: 'var(--muted)', fontSize: 12 }}>
            v{version.version} <span className={`badge badge-${version.status}`}>{version.status}</span>
          </span>
        )}
        {workflow?.updated_at ? (
          <span style={{ fontSize: 11, color: 'var(--muted)', opacity: 0.7 }} title={zh ? `创建：${workflow.created_at ? new Date(workflow.created_at * 1000).toLocaleString() : '—'} · 修改：${new Date(workflow.updated_at * 1000).toLocaleString()}` : `Created: ${workflow.created_at ? new Date(workflow.created_at * 1000).toLocaleString() : '—'} · Modified: ${new Date(workflow.updated_at * 1000).toLocaleString()}`}>
            {zh ? '改' : 'mod.'} {(() => { const d = Date.now() / 1000 - workflow.updated_at; return zh ? (d < 60 ? `${Math.floor(d)}秒前` : d < 3600 ? `${Math.floor(d / 60)}分钟前` : d < 86400 ? `${Math.floor(d / 3600)}小时前` : `${Math.floor(d / 86400)}天前`) : (d < 60 ? `${Math.floor(d)}s ago` : d < 3600 ? `${Math.floor(d / 60)}m ago` : d < 86400 ? `${Math.floor(d / 3600)}h ago` : `${Math.floor(d / 86400)}d ago`) })()}
          </span>
        ) : null}

        <div className="topbar-actions">
          {latestExec && (
            <span
              style={{ fontSize: 11, color: 'var(--muted)', cursor: 'default' }}
              title={zh ? `上次运行：${latestExec.status} · ${new Date(latestExec.started_at * 1000).toLocaleString()}` : `Last run: ${latestExec.status} · ${new Date(latestExec.started_at * 1000).toLocaleString()}`}
            >
              {zh ? '最近：' : 'last: '}<span className={`badge badge-${latestExec.status}`} style={{ fontSize: 10 }}>{latestExec.status}</span>
            </span>
          )}
          <button
            className="btn btn-sm"
            onClick={() => setShowSchema(true)}
            title={zh ? '定义此工作流的预期输入字段' : 'Define expected input fields for this workflow'}
          >
            {zh ? '输入模式' : 'Input Schema'}
          </button>
          <button
            className="btn btn-sm"
            onClick={() => {
              api.listVariables(auth!.tenantId, workflowId)
                .then(setVariables)
                .catch(() => {})
              setShowVars(true)
            }}
            title="View and manage workflow variables (persistent per-workflow state)"
          >
            {t('we.variables')}
          </button>
          <button
            className="btn btn-sm"
            onClick={handleShowVersions}
            title="Browse version history"
          >
            {t('we.history')}
          </button>
          {wfStats && wfStats.total > 0 && (
            <button
              className="btn btn-sm"
              onClick={() => {
                api.listExecutions(auth!.tenantId, workflowId)
                  .then(setReportExecs)
                  .catch(() => {})
                setShowReport(true)
              }}
              title={zh ? '查看此工作流的性能分析报告' : 'View workflow performance report'}
              style={{ fontSize: 11 }}
            >
              📊 {zh ? '报告' : 'Report'}
            </button>
          )}
          <button
            className="btn btn-sm"
            disabled={!workflow?.latest_version_id}
            onClick={handleExport}
            title={zh ? '下载已发布版本为 JSON' : 'Download published version as JSON'}
          >
            {zh ? '↓ 导出' : '↓ Export'}
          </button>
          <button
            className={`btn btn-sm${workflow?.locked ? ' btn-primary' : ''}`}
            onClick={handleLockToggle}
            title={workflow?.locked ? 'Workflow is locked — click to unlock and allow edits' : 'Lock workflow to prevent accidental edits'}
            style={{ fontSize: 11 }}
          >
            {workflow?.locked ? t('we.locked') : t('we.lock')}
          </button>
          {workflow?.status === 'archived' ? (
            <button className="btn btn-sm" onClick={handleRestore} title="Restore to draft">
              {t('we.restore')}
            </button>
          ) : (
            <button
              className="btn btn-sm btn-danger"
              onClick={handleArchive}
              title="Archive this workflow"
              style={{ fontSize: 11 }}
            >
              {t('we.archive')}
            </button>
          )}
          <button
            className="btn btn-sm"
            onClick={handleAutoLayout}
            title="Auto-arrange nodes in topological order"
          >
            {t('we.layout')}
          </button>
          <button
            className={`btn btn-sm${snapToGrid ? ' btn-primary' : ''}`}
            onClick={() => setSnapToGrid((v) => !v)}
            title={snapToGrid ? 'Snap to grid: ON (click to disable)' : 'Snap to grid: OFF (click to enable)'}
            style={{ fontSize: 11 }}
          >
            {t('we.snap')}
          </button>
          <button
            className={`btn btn-sm${showMinimap ? '' : ' btn-secondary'}`}
            onClick={() => setShowMinimap((v) => !v)}
            title={showMinimap ? 'Hide minimap' : 'Show minimap'}
            style={{ fontSize: 11 }}
          >
            {showMinimap ? t('we.minimap') : '□ Map'}
          </button>
          <select
            value={bgVariant}
            onChange={(e) => {
              const v = e.target.value as 'dots' | 'grid' | 'lines' | 'none'
              setBgVariant(v)
              try { localStorage.setItem('af:canvas_bg', v) } catch { /* ignore */ }
            }}
            title={zh ? '画布背景样式' : 'Canvas background style'}
            style={{ fontSize: 11, padding: '2px 4px', height: 26, borderRadius: 4, border: '1px solid var(--border)', background: 'var(--panel)', color: 'var(--text)', cursor: 'pointer' }}
          >
            <option value="dots">{zh ? '· 点' : '· Dots'}</option>
            <option value="grid">{zh ? '⊹ 网格' : '⊹ Grid'}</option>
            <option value="lines">{zh ? '— 线' : '— Lines'}</option>
            <option value="none">{zh ? '□ 无' : '□ None'}</option>
          </select>
          <button
            className="btn btn-sm"
            onClick={() => fitViewRef.current?.()}
            title="Fit all nodes into view (F)"
            style={{ fontSize: 11 }}
          >
            {t('we.fit')}
          </button>
          <button
            className={`btn btn-sm${showNodeFind ? ' btn-primary' : ''}`}
            onClick={() => { setShowNodeFind((v) => !v); if (!showNodeFind) setTimeout(() => nodeFindInputRef.current?.focus(), 50) }}
            title={zh ? '查找节点 (Ctrl+F)' : 'Find node (Ctrl+F)'}
            style={{ fontSize: 11 }}
          >
            🔍 {zh ? '查找' : 'Find'}
          </button>
          <button
            className="btn btn-sm"
            onClick={() => { const w = collectPublishWarnings(); setValidateWarnings(w); setShowValidate(true) }}
            title="Validate workflow configuration"
            style={{ fontSize: 11 }}
          >
            {t('we.validate')}
          </button>
          <button
            className="btn btn-sm"
            onClick={() => setShowSchedule(true)}
            title="Configure auto-run schedule for the trigger node"
            style={{ fontSize: 11 }}
          >
            {t('we.schedule')}
          </button>
          <button
            className="btn btn-sm"
            onClick={() => setShowForms(true)}
            title="Publish a shareable form URL for this workflow"
            style={{ fontSize: 11 }}
          >
            {t('we.form')}
          </button>
          <button
            className="btn btn-sm"
            onClick={() => setShowTests(true)}
            title="Manage workflow test cases"
            style={{ fontSize: 11 }}
          >
            {t('we.tests')}
          </button>
          <button
            className="btn btn-sm"
            onClick={() => setShowComments(true)}
            title="View and add comments on this workflow"
            style={{ fontSize: 11 }}
          >
            {t('we.comments')}
          </button>
          <button
            className="btn btn-sm"
            onClick={() => setShowApiDocs(true)}
            title={zh ? '查看此工作流的 API 调用文档' : 'View API usage docs for this workflow'}
            style={{ fontSize: 11 }}
          >
            📖 {zh ? 'API 文档' : 'API Docs'}
          </button>
          <button
            className={`btn btn-sm${showCopilot ? ' btn-primary' : ''}`}
            onClick={() => setShowCopilot((v) => !v)}
            title="AI Copilot — ask questions about this workflow"
            style={{ fontSize: 11, background: showCopilot ? 'var(--node-claude)' : undefined, color: showCopilot ? '#fff' : undefined, border: showCopilot ? 'none' : undefined }}
          >
            ✦ {locale === 'zh' ? 'AI 助手' : 'Copilot'}
          </button>
          <div style={{ position: 'relative', display: 'inline-flex', alignItems: 'center' }}>
            <button
              className={`btn btn-sm${isDirty ? ' btn-primary' : ''}`}
              disabled={saving}
              onClick={showSaveMessage ? handleSave : () => setShowSaveMessage(true)}
              title={isDirty ? 'Unsaved changes — save a new version' : 'Save current graph as a new version'}
            >
              {saving ? (locale === 'zh' ? '保存中…' : 'Saving…') : isDirty ? t('we.save.dirty') : t('we.save')}
            </button>
            {showSaveMessage && (
              <div style={{ position: 'absolute', top: '100%', right: 0, zIndex: 200, background: 'var(--panel)', border: '1px solid var(--border)', borderRadius: 6, padding: '8px 10px', width: 240, marginTop: 4, boxShadow: '0 4px 12px rgba(0,0,0,0.3)' }}>
                <input
                  autoFocus
                  placeholder={zh ? '保存备注（可选）' : 'Save message (optional)'}
                  value={saveMessage}
                  onChange={(e) => setSaveMessage(e.target.value)}
                  onKeyDown={(e) => { if (e.key === 'Enter') handleSave(); if (e.key === 'Escape') { setShowSaveMessage(false); setSaveMessage('') } }}
                  style={{ width: '100%', fontSize: 12, marginBottom: 6, boxSizing: 'border-box' }}
                />
                <div style={{ display: 'flex', gap: 6, justifyContent: 'flex-end' }}>
                  <button className="btn btn-sm" onClick={() => { setShowSaveMessage(false); setSaveMessage('') }}>{zh ? '取消' : 'Cancel'}</button>
                  <button className="btn btn-sm btn-primary" disabled={saving} onClick={handleSave}>{zh ? '保存' : 'Save'}</button>
                </div>
              </div>
            )}
          </div>
          <button
            className="btn btn-sm btn-primary"
            disabled={!canPublish || publishing}
            onClick={handlePublish}
            title={canPublish ? 'Publish this draft version' : 'No draft version to publish'}
          >
            {publishing ? (locale === 'zh' ? '发布中…' : 'Publishing…') : t('we.publish')}
          </button>
          {canPublish && (
            <button
              className="btn btn-sm"
              disabled={publishingAndRunning || publishing}
              onClick={handlePublishAndRun}
              title={zh ? '发布版本并立即运行' : 'Publish and immediately run'}
              style={{ fontSize: 11 }}
            >
              {publishingAndRunning ? '…' : (zh ? '▶ 发布并运行' : '▶ Publish & Run')}
            </button>
          )}
          <button className="btn btn-sm" onClick={toggleTheme} title="Toggle dark/light theme">
            {theme === 'dark' ? '☀' : '◑'}
          </button>
          <button className="btn btn-sm" onClick={toggleLocale} title="切换语言 / Switch language">
            {locale === 'zh' ? 'EN' : '中'}
          </button>
          <button className="btn btn-sm" onClick={() => setShowHelp(true)} title="Keyboard shortcuts and tips">
            ?
          </button>
          <button
            className={`btn btn-sm${workflow?.readme ? ' btn-primary' : ''}`}
            onClick={() => setShowReadme(true)}
            title={workflow?.readme ? 'View/edit workflow documentation' : 'Add workflow documentation'}
            style={{ fontSize: 11 }}
          >
            {t('we.docs')}
          </button>
        </div>
      </header>

      {/* ── Editor body ──────────────────────────────────────────── */}
      <div className="editor" style={{ flex: 1, overflow: 'hidden', display: 'flex', flexDirection: 'column' }}>
        {/* Workflow stats bar */}
        <div style={{ display: 'flex', alignItems: 'center', gap: 16, padding: '4px 14px', background: 'var(--panel)', borderBottom: '1px solid var(--border)', fontSize: 11, color: 'var(--muted)' }}>
          {/* Graph stats */}
          <span title={zh ? `${nodes.filter(n => n.data.nodeType !== 'note').length} 个可执行节点 + ${nodes.filter(n => n.data.nodeType === 'note').length} 个注释` : `${nodes.filter(n => n.data.nodeType !== 'note').length} executable nodes + ${nodes.filter(n => n.data.nodeType === 'note').length} note(s)`}>
            <strong style={{ color: 'var(--fg)' }}>{nodes.length}</strong> {zh ? '个节点' : `node${nodes.length !== 1 ? 's' : ''}`}
          </span>
          <span><strong style={{ color: 'var(--fg)' }}>{edges.length}</strong> {zh ? '条边' : `edge${edges.length !== 1 ? 's' : ''}`}</span>
          {(() => {
            const execNodes = nodes.filter(n => n.data.nodeType !== 'note' && n.data.nodeType !== 'trigger')
            const aiNodes = execNodes.filter(n => ['openai','gemini','claude','agent'].includes(n.data.nodeType ?? ''))
            const httpNodes = execNodes.filter(n => ['http','graphql','github','webhook','jira','notion','linear','airtable'].includes(n.data.nodeType ?? ''))
            const integNodes = execNodes.filter(n => ['slack','email','database'].includes(n.data.nodeType ?? ''))
            const parts: string[] = []
            if (aiNodes.length > 0) parts.push(`${aiNodes.length} AI`)
            if (httpNodes.length > 0) parts.push(`${httpNodes.length} HTTP`)
            if (integNodes.length > 0) parts.push(zh ? `${integNodes.length} 集成` : `${integNodes.length} integration`)
            if (parts.length > 0) return <span style={{ color: 'var(--muted)' }} title={zh ? '节点类型分布' : 'Node type breakdown'}>{parts.join(' · ')}</span>
            return null
          })()}
          {(() => {
            const execNodes = nodes.filter(n => n.data.nodeType !== 'note' && n.data.nodeType !== 'trigger')
            const aiNodes = execNodes.filter(n => ['openai','gemini','claude','agent'].includes(n.data.nodeType ?? ''))
            const score = Math.min(10, Math.floor(execNodes.length * 0.5 + edges.length * 0.3 + aiNodes.length * 1.5))
            if (score < 2) return null
            const label = zh ? (score <= 3 ? '简单' : score <= 6 ? '中等' : '复杂') : (score <= 3 ? 'simple' : score <= 6 ? 'moderate' : 'complex')
            const color = score <= 3 ? 'var(--success-text)' : score <= 6 ? '#d97706' : 'var(--danger-text)'
            return <span title={zh ? `复杂度分数：${score}/10（基于节点/边数量和 AI 节点）` : `Complexity score: ${score}/10 (based on node/edge count and AI nodes)`} style={{ color }}>{zh ? '复杂度：' : 'complexity: '}<strong>{label}</strong></span>
          })()}
          {isDirty && (
            <span style={{ color: 'var(--muted)', fontSize: 11, animation: 'none' }}>{zh ? '30 秒后自动保存' : 'autosave in 30s'}</span>
          )}
          {liveWarningCount > 0 && (
            <span
              style={{ color: 'var(--warning-text)', cursor: 'pointer', fontWeight: 600 }}
              title={zh ? '点击校验' : 'Click to validate'}
              onClick={() => { const w = collectPublishWarnings(); setValidateWarnings(w); setShowValidate(true) }}
            >
              ⚠ {zh ? `${liveWarningCount} 个问题` : `${liveWarningCount} issue${liveWarningCount !== 1 ? 's' : ''}`}
            </span>
          )}
          {version?.status === 'published' && (
            <span style={{ color: 'var(--muted)', fontSize: 11, fontStyle: 'italic' }}>
              {zh ? '正在查看已发布版本 — ' : 'viewing published — '}<span style={{ color: 'var(--link)', cursor: 'pointer', textDecoration: 'underline' }} onClick={() => setShowSaveMessage(true)}>{zh ? '保存版本' : 'save a version'}</span>{zh ? ' 以起草变更' : ' to draft changes'}
            </span>
          )}
          {wfHealth && (
            <>
              <span style={{ color: 'var(--border)', userSelect: 'none' }}>│</span>
              <span
                title={wfHealth.issues.length > 0 ? wfHealth.issues.map(i => `${i.severity}: ${i.message}`).join('\n') : (zh ? '工作流健康' : 'Workflow health')}
                style={{ color: wfHealth.status === 'healthy' ? 'var(--success-text)' : wfHealth.status === 'error' ? 'var(--danger-text)' : '#d97706', cursor: wfHealth.issues.length > 0 ? 'help' : 'default' }}
              >
                {wfHealth.status === 'healthy' ? '✓ ' : wfHealth.status === 'error' ? '✗ ' : '⚠ '}
                {zh ? (wfHealth.status === 'healthy' ? '健康' : wfHealth.status === 'error' ? '错误' : '警告') : wfHealth.status}
                {wfHealth.issues.length > 0 && ` (${wfHealth.issues.length})`}
              </span>
            </>
          )}
          {wfStats && wfStats.total > 0 && (
            <>
              <span style={{ color: 'var(--border)', userSelect: 'none' }}>│</span>
              <span><strong style={{ color: 'var(--fg)' }}>{wfStats.total}</strong> {zh ? '次运行' : 'runs'}</span>
              <span style={{ color: 'var(--success-text)' }}><strong>{wfStats.succeeded}</strong> {zh ? '成功' : 'ok'}</span>
              {wfStats.failed > 0 && <span style={{ color: 'var(--danger-text)' }}><strong>{wfStats.failed}</strong> {zh ? '失败' : 'failed'}</span>}
              {wfStats.running > 0 && <span style={{ color: 'var(--link)' }}><strong>{wfStats.running}</strong> {zh ? '运行中' : 'running'}</span>}
              {wfStats.avg_duration_secs != null && (
                <span>{zh ? '均' : 'avg'} <strong style={{ color: 'var(--fg)' }}>{wfStats.avg_duration_secs < 60 ? `${wfStats.avg_duration_secs.toFixed(1)}s` : `${(wfStats.avg_duration_secs / 60).toFixed(1)}m`}</strong></span>
              )}
              {wfEstimate && wfEstimate.sample_count >= 3 && wfEstimate.p50_secs != null && (
                <span style={{ color: 'var(--muted)', fontSize: 11 }} title={zh ? `基于 ${wfEstimate.sample_count} 次历史运行的估算` : `Estimate based on ${wfEstimate.sample_count} historical runs`}>
                  p50 <strong style={{ color: 'var(--fg)' }}>{wfEstimate.p50_secs < 60 ? `${wfEstimate.p50_secs.toFixed(1)}s` : `${(wfEstimate.p50_secs / 60).toFixed(1)}m`}</strong>
                  {wfEstimate.p95_secs != null && <> · p95 <strong style={{ color: 'var(--fg)' }}>{wfEstimate.p95_secs < 60 ? `${wfEstimate.p95_secs.toFixed(1)}s` : `${(wfEstimate.p95_secs / 60).toFixed(1)}m`}</strong></>}
                </span>
              )}
              <span style={{ marginLeft: 'auto', color: wfStats.succeeded / wfStats.total >= 0.9 ? 'var(--success-text)' : wfStats.succeeded / wfStats.total >= 0.7 ? '#d97706' : 'var(--danger-text)' }}>
                {Math.round((wfStats.succeeded / wfStats.total) * 100)}% {zh ? '成功率' : 'success'}
              </span>
              <button
                className={`btn btn-sm ${showNodeHeatmap ? 'btn-primary' : ''}`}
                style={{ fontSize: 10, padding: '1px 6px', marginLeft: 4 }}
                title={zh ? '按历史成功率对节点着色' : 'Color nodes by historical success rate'}
                onClick={() => {
                  const next = !showNodeHeatmap
                  setShowNodeHeatmap(next)
                  if (next && nodeStats.length === 0) {
                    api.getWorkflowNodeStats(auth!.tenantId, workflowId).then(setNodeStats).catch(() => {})
                  }
                }}
              >
                🔥 {zh ? '热图' : 'Heat'}
              </button>
            </>
          )}
        </div>
        <div style={{ flex: 1, display: 'flex', overflow: 'hidden', position: 'relative' }}>
          {/* Left palette */}
          <aside className="palette">
            <div style={{ padding: '6px 6px 4px' }}>
              <input
                placeholder={zh ? '筛选节点…' : 'Filter nodes…'}
                value={paletteSearch}
                onChange={(e) => setPaletteSearch(e.target.value)}
                style={{ width: '100%', fontSize: 12, padding: '3px 6px', boxSizing: 'border-box' }}
              />
            </div>
            {!paletteSearch && recentNodeTypes.length > 0 && (
              <>
                <div className="palette-label" style={{ fontSize: 10, opacity: 0.7 }}>{zh ? '最近使用' : 'Recent'}</div>
                {recentNodeTypes.map((type) => {
                  const entry = NODE_TYPE_LIST.find((n) => n.type === type)
                  if (!entry) return null
                  const labelDisplay = zh ? (NODE_ZH[type]?.labelZh ?? entry.label) : entry.label
                  const descDisplay  = zh ? (NODE_ZH[type]?.descZh  ?? entry.desc)  : entry.desc
                  return (
                    <button key={`recent-${type}`} className="palette-node" onClick={() => addNode(type)} title={descDisplay}>
                      <span className="palette-dot" style={{ background: entry.color }} />
                      <span>{entry.icon} {labelDisplay}</span>
                    </button>
                  )
                })}
            </>
            )}
            {paletteSearch ? (
              <>
                {NODE_TYPE_LIST
                  .filter(({ label, type }) => {
                    const lz = NODE_ZH[type]?.labelZh ?? ''
                    const q = paletteSearch.toLowerCase()
                    return label.toLowerCase().includes(q) || (zh && lz.includes(q))
                  })
                  .map(({ type, label, color, icon, desc }) => {
                    const labelDisplay = zh ? (NODE_ZH[type]?.labelZh ?? label) : label
                    const descDisplay  = zh ? (NODE_ZH[type]?.descZh  ?? desc)  : desc
                    return (
                      <button key={type} className="palette-node" onClick={() => addNode(type)} title={descDisplay}>
                        <span className="palette-dot" style={{ background: color }} />
                        <span>{icon} {highlightMatch(labelDisplay, paletteSearch)}</span>
                      </button>
                    )
                  })}
                {NODE_TYPE_LIST.filter(({ label, type }) => { const lz = NODE_ZH[type]?.labelZh ?? ''; const q = paletteSearch.toLowerCase(); return label.toLowerCase().includes(q) || (zh && lz.includes(q)) }).length === 0 && (
                  <div style={{ padding: '6px 10px', fontSize: 11, color: 'var(--muted)' }}>{zh ? '无匹配' : 'No match'}</div>
                )}
              </>
            ) : (
              PALETTE_CATEGORY_ORDER.map((cat) => {
                const catNodes = NODE_TYPE_LIST.filter((n) => n.category === cat)
                if (catNodes.length === 0) return null
                return (
                  <div key={cat}>
                    <div className="palette-label" style={{ fontSize: 10, opacity: 0.6, marginTop: 8, display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                      <span>{zh ? { Control: '控制流', Integration: '集成', AI: 'AI', Transform: '数据处理', Utility: '工具' }[cat] ?? cat : cat}</span>
                      <span style={{ fontSize: 9, opacity: 0.7, background: 'var(--panel)', padding: '0 5px', borderRadius: 8 }}>{catNodes.length}</span>
                    </div>
                    {catNodes.map(({ type, label, color, icon, desc }) => {
                      const labelDisplay = zh ? (NODE_ZH[type]?.labelZh ?? label) : label
                      const descDisplay  = zh ? (NODE_ZH[type]?.descZh  ?? desc)  : desc
                      return (
                        <button key={type} className="palette-node" onClick={() => addNode(type)} title={descDisplay}>
                          <span className="palette-dot" style={{ background: color }} />
                          <span>{icon} {labelDisplay}</span>
                        </button>
                      )
                    })}
                  </div>
                )
              })
            )}
            <div className="palette-label" style={{ marginTop: 12 }}>{zh ? '提示' : 'Tips'}</div>
            <div style={{ fontSize: 11, color: 'var(--muted)', padding: '0 8px', lineHeight: 1.6 }}>
              <div>• {zh ? '拖拽节点以移动' : 'Drag nodes to move'}</div>
              <div>• {zh ? '拖拽连接点以连线' : 'Drag handle → handle to connect'}</div>
              <div>• {zh ? '选中 + Delete 以删除' : 'Select + Delete to remove'}</div>
              <div>• {zh ? '点击节点以配置' : 'Click node to configure'}</div>
              <div>• {zh ? '⧉ 按钮以复制节点' : '⧉ button to duplicate node'}</div>
              <div>• Ctrl+K {zh ? '搜索节点' : 'to search nodes'}</div>
              <div>• Ctrl+S {zh ? '保存版本' : 'to save version'}</div>
            </div>
          </aside>

          {/* Canvas */}
          <div className="canvas-wrap" style={{ position: 'relative' }}>
            {/* Node Find bar */}
            {showNodeFind && (() => {
              const q = nodeFindQuery.toLowerCase().trim()
              const matches = q ? nodes.filter((n) => {
                const label = (n.data?.label as string | undefined) ?? n.type ?? ''
                return n.id.toLowerCase().includes(q) || label.toLowerCase().includes(q) || (n.type ?? '').toLowerCase().includes(q)
              }) : []
              return (
                <div style={{
                  position: 'absolute', top: 8, left: '50%', transform: 'translateX(-50%)',
                  zIndex: 50, display: 'flex', alignItems: 'center', gap: 6,
                  background: 'var(--panel)', border: '1px solid var(--border)',
                  borderRadius: 8, padding: '6px 10px', boxShadow: '0 4px 16px rgba(0,0,0,0.2)',
                  minWidth: 280,
                }}>
                  <span style={{ fontSize: 12, color: 'var(--muted)', flexShrink: 0 }}>🔍</span>
                  <input
                    ref={nodeFindInputRef}
                    value={nodeFindQuery}
                    onChange={(e) => { setNodeFindQuery(e.target.value); setNodeFindIdx(0) }}
                    onKeyDown={(e) => {
                      if (e.key === 'Escape') { setShowNodeFind(false); setNodeFindQuery('') }
                      if (e.key === 'Enter' || e.key === 'ArrowDown') { e.preventDefault(); setNodeFindIdx((i) => (i + 1) % Math.max(matches.length, 1)) }
                      if (e.key === 'ArrowUp') { e.preventDefault(); setNodeFindIdx((i) => (i - 1 + Math.max(matches.length, 1)) % Math.max(matches.length, 1)) }
                    }}
                    placeholder={zh ? '查找节点（ID/类型/标签）…' : 'Find node (id/type/label)…'}
                    style={{ border: 'none', background: 'transparent', outline: 'none', fontSize: 13, flex: 1, color: 'var(--text)' }}
                  />
                  {q && (
                    <span style={{ fontSize: 11, color: matches.length > 0 ? 'var(--success, #16a34a)' : 'var(--danger-text, #dc2626)', flexShrink: 0, fontWeight: 600 }}>
                      {matches.length > 0 ? `${(nodeFindIdx % matches.length) + 1}/${matches.length}` : (zh ? '无匹配' : 'none')}
                    </span>
                  )}
                  {matches.length > 1 && (
                    <>
                      <button className="btn btn-sm" style={{ fontSize: 10, padding: '1px 6px' }} onClick={() => setNodeFindIdx((i) => (i - 1 + matches.length) % matches.length)}>↑</button>
                      <button className="btn btn-sm" style={{ fontSize: 10, padding: '1px 6px' }} onClick={() => setNodeFindIdx((i) => (i + 1) % matches.length)}>↓</button>
                    </>
                  )}
                  <button className="btn btn-sm btn-icon" style={{ fontSize: 11 }} onClick={() => { setShowNodeFind(false); setNodeFindQuery('') }}>✕</button>
                </div>
              )
            })()}
            {nodes.length > 0 ? (
              <Canvas
                initialNodes={nodes}
                initialEdges={edges}
                selectedNodeId={selectedNodeId}
                onSelectionChange={setSelectedNodeId}
                onNodesUpdated={setNodes}
                onEdgesUpdated={setEdges}
                nodeStatuses={nodeStatuses}
                warningNodeIds={warningNodeIds}
                snapToGrid={snapToGrid}
                showMinimap={showMinimap}
                fitViewRef={fitViewRef}
                fitToNodeRef={fitToNodeRef}
                bgVariant={bgVariant}
                nodeHeatmapMap={showNodeHeatmap ? nodeHeatmapMap : undefined}
                defaultViewport={savedViewport}
                onViewportChange={handleViewportChange}
                highlightedNodeIds={(() => {
                  if (!showNodeFind || !nodeFindQuery) return undefined
                  const q = nodeFindQuery.toLowerCase()
                  return new Set(nodes.filter((n) => {
                    const label = (n.data?.label as string | undefined) ?? n.type ?? ''
                    return n.id.toLowerCase().includes(q) || label.toLowerCase().includes(q) || (n.type ?? '').toLowerCase().includes(q)
                  }).map((n) => n.id))
                })()}
              />
            ) : (
              <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100%', color: 'var(--muted)' }}>
                {zh ? '加载图形中…' : 'Loading graph…'}
              </div>
            )}
          </div>

          {/* Right config panel */}
          <NodeConfigPanel
            node={selectedNode}
            onUpdateConfig={handleUpdateConfig}
            recentExecutions={recentExecutions}
            executionResult={selectedNode ? (nodeStatuses[selectedNode.id] ?? null) : null}
            webhookUrl={webhookUrl}
            webhookSecret={webhookSecret}
            onDuplicate={selectedNode ? handleDuplicateNode : undefined}
            upstreamNodes={upstreamNodes}
            onSelectExecution={async (id) => {
              try {
                const rec = await api.getExecution(auth!.tenantId, id)
                setExecution(rec)
              } catch (e) {
                toast(String(e), 'error')
              }
            }}
          />

          {/* AI Copilot panel overlay */}
          {showCopilot && (
            <CopilotPanel
              onClose={() => setShowCopilot(false)}
              graphJson={nodes.length > 0 ? JSON.stringify(fromFlowGraph(nodes, edges)) : ''}
              tenantId={auth!.tenantId}
              zh={zh}
            />
          )}
        </div>

        {/* Bottom execution panel */}
        <ExecutionPanel
          execution={execution}
          running={running}
          inputJson={inputJson}
          onInputChange={setInputJson}
          onRun={handleRun}
          canRun={canRun}
          onApprove={handleApprove}
          onReject={handleReject}
          inputSchema={inputSchema}
          envSets={envSets}
          envSet={envSet}
          onEnvSetChange={setEnvSet}
          label={runLabel}
          onLabelChange={setRunLabel}
          callbackUrl={callbackUrl}
          onCallbackUrlChange={setCallbackUrl}
          workflowId={workflowId}
          dryRun={dryRun}
          onDryRunChange={setDryRun}
          lastRunInput={execution?.input_json ?? undefined}
        />
        {recentExecutions.length > 0 && (
          <RecentRunsMini
            executions={recentExecutions}
            onLoad={async (id) => {
              try {
                const rec = await api.getExecution(auth!.tenantId, id)
                setExecution(rec)
                toast(zh ? `已加载运行 ${id.slice(-8)}` : `Loaded run ${id.slice(-8)}`)
              } catch (e) { toast(String(e), 'error') }
            }}
          />
        )}
      </div>

      {/* Toasts */}
      {toasts.map((t) => (
        <div key={t.id} className={`toast toast-${t.kind}`}>
          {t.message}
        </div>
      ))}

      {/* Version history modal */}
      {showVersions && (
        <div className="modal-backdrop" onClick={() => setShowVersions(false)}>
          <div className="modal" style={{ width: 560, maxHeight: '80vh', display: 'flex', flexDirection: 'column', gap: 0, padding: 0 }} onClick={(e) => e.stopPropagation()}>
            <div style={{ padding: '18px 20px 14px', borderBottom: '1px solid var(--border)', display: 'flex', alignItems: 'center', justifyContent: 'space-between', flexShrink: 0 }}>
              <div>
                <h2 style={{ margin: 0, fontSize: 15 }}>{zh ? '版本历史' : 'Version History'}</h2>
                <p style={{ margin: '3px 0 0', fontSize: 12, color: 'var(--muted)' }}>
                  {zh ? '加载某个版本，或与上一版本对比差异。' : 'Load a version or Diff it against the previous one.'}
                </p>
              </div>
              <button className="btn btn-sm" onClick={() => setShowVersions(false)}>✕</button>
            </div>
            <div style={{ overflowY: 'auto', flex: 1 }}>
              {loadingVersions && (
                <div style={{ padding: 24, color: 'var(--muted)', textAlign: 'center', fontSize: 13 }}>{zh ? '加载中…' : 'Loading…'}</div>
              )}
              {!loadingVersions && versions.length === 0 && (
                <div style={{ padding: 24, color: 'var(--muted)', textAlign: 'center', fontSize: 13 }}>{zh ? '暂无版本。' : 'No versions yet.'}</div>
              )}
              {!loadingVersions && versions.map((ver, i) => {
                const prev = versions[i + 1]
                const isDiffOpen = diffVersionId === ver.id
                // For "Compare with..." — use diffCompareId if set for this version, else fall back to prev
                const compareTarget = (diffVersionId === ver.id && diffCompareId)
                  ? versions.find((v) => v.id === diffCompareId) ?? prev
                  : prev

                // Compute diff vs selected compare target
                const diffBase = compareTarget
                const diff = diffBase ? (() => {
                  const nodeMapA = new Map(diffBase.graph.nodes.map(n => [n.id, n]))
                  const nodeMapB = new Map(ver.graph.nodes.map(n => [n.id, n]))
                  const addedNodes = ver.graph.nodes.filter(n => !nodeMapA.has(n.id))
                  const removedNodes = prev.graph.nodes.filter(n => !nodeMapB.has(n.id))
                  // Config-level changes for nodes that exist in both versions
                  const modifiedNodes: { id: string; type: string; changedKeys: string[] }[] = []
                  for (const [id, nodeB] of nodeMapB) {
                    const nodeA = nodeMapA.get(id)
                    if (!nodeA) continue
                    const cfgA = (nodeA.config ?? {}) as Record<string, unknown>
                    const cfgB = (nodeB.config ?? {}) as Record<string, unknown>
                    const allKeys = new Set([...Object.keys(cfgA), ...Object.keys(cfgB)])
                    const changedKeys = [...allKeys].filter(k => JSON.stringify(cfgA[k]) !== JSON.stringify(cfgB[k]))
                    if (changedKeys.length > 0) modifiedNodes.push({ id, type: nodeB.type, changedKeys })
                  }
                  const edgeKey = (e: { source: string; target: string }) => `${e.source}→${e.target}`
                  const edgesA = new Set(diffBase.graph.edges.map(edgeKey))
                  const edgesB = new Set(ver.graph.edges.map(edgeKey))
                  const addedEdges = ver.graph.edges.filter(e => !edgesA.has(edgeKey(e)))
                  const removedEdges = diffBase.graph.edges.filter(e => !edgesB.has(edgeKey(e)))
                  return { addedNodes, removedNodes, modifiedNodes, addedEdges, removedEdges }
                })() : null

                const hasDiff = diff && (diff.addedNodes.length + diff.removedNodes.length + diff.modifiedNodes.length + diff.addedEdges.length + diff.removedEdges.length) > 0

                return (
                  <div key={ver.id} style={{ borderBottom: '1px solid var(--border)' }}>
                    <div style={{
                      display: 'flex', alignItems: 'center', gap: 10,
                      padding: '10px 20px',
                      background: ver.id === version?.id ? 'var(--panel)' : 'transparent',
                    }}>
                      <span style={{ fontWeight: 600, fontSize: 13, minWidth: 28 }}>v{ver.version}</span>
                      <span className={`badge badge-${ver.status}`}>{ver.status}</span>
                      {i === 0 && <span style={{ fontSize: 11, color: 'var(--muted)' }}>{zh ? '最新' : 'latest'}</span>}
                      <span style={{ fontSize: 11, color: 'var(--muted)', fontFamily: 'monospace' }}>{ver.id.slice(0, 12)}…</span>
                      {ver.message && <span style={{ fontSize: 11, color: 'var(--fg)', fontStyle: 'italic', flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>"{ver.message}"</span>}
                      {!ver.message && <span style={{ flex: 1 }} />}
                      {versions.length > 1 && (
                        <div style={{ display: 'flex', alignItems: 'center', gap: 4, position: 'relative' }}>
                          <button
                            className={`btn btn-sm${isDiffOpen ? ' btn-primary' : ''}`}
                            onClick={() => { setDiffVersionId(isDiffOpen ? null : ver.id); if (isDiffOpen) setShowComparePicker(null) }}
                            title={zh ? '与另一版本对比' : 'Diff vs another version'}
                          >
                            {isDiffOpen ? (zh ? '隐藏' : 'Hide') : (zh ? '差异' : 'Diff')}
                            {isDiffOpen && compareTarget ? ` v${compareTarget.version}` : ''}
                            {hasDiff && !isDiffOpen && <span style={{ marginLeft: 4, fontSize: 10, color: 'var(--warning-text)' }}>●</span>}
                          </button>
                          {isDiffOpen && (
                            <div style={{ position: 'relative' }}>
                              <button
                                className="btn btn-sm"
                                style={{ fontSize: 10 }}
                                onClick={() => setShowComparePicker(showComparePicker === ver.id ? null : ver.id)}
                                title={zh ? '选择对比版本' : 'Pick version to compare'}
                              >
                                {zh ? '对比…' : 'vs…'}
                              </button>
                              {showComparePicker === ver.id && (
                                <div style={{ position: 'absolute', top: '100%', right: 0, zIndex: 999, background: 'var(--surface)', border: '1px solid var(--border)', borderRadius: 6, boxShadow: '0 4px 16px rgba(0,0,0,0.2)', minWidth: 140 }}>
                                  {versions.filter((v) => v.id !== ver.id).map((v) => (
                                    <button
                                      key={v.id}
                                      className="btn btn-sm"
                                      style={{ display: 'block', width: '100%', textAlign: 'left', borderRadius: 0, padding: '6px 12px', fontWeight: v.id === diffCompareId ? 700 : 400 }}
                                      onClick={() => { setDiffCompareId(v.id); setShowComparePicker(null) }}
                                    >
                                      v{v.version} <span style={{ fontSize: 10, color: 'var(--muted)' }}>{v.status}</span>
                                    </button>
                                  ))}
                                </div>
                              )}
                            </div>
                          )}
                        </div>
                      )}
                      <button
                        className="btn btn-sm"
                        disabled={ver.id === version?.id || rollingBack === ver.id}
                        title={`Create a new draft version from v${ver.version}`}
                        onClick={() => handleRollback(ver.id, ver.version)}
                        style={{ opacity: rollingBack === ver.id ? 0.6 : 1 }}
                      >
                        {rollingBack === ver.id ? '…' : '↩'}
                      </button>
                      <button
                        className="btn btn-sm"
                        disabled={ver.id === version?.id}
                        onClick={() => handleLoadVersion(ver.id)}
                      >
                        {ver.id === version?.id ? (zh ? '当前' : 'Current') : (zh ? '加载' : 'Load')}
                      </button>
                    </div>
                    {isDiffOpen && diff && (
                      <div style={{ padding: '8px 20px 12px', background: 'var(--canvas-bg)', fontSize: 12 }}>
                        <p style={{ color: 'var(--muted)', margin: '0 0 8px', fontSize: 11 }}>
                          {zh ? `对比 v${ver.version} 与 v${compareTarget?.version ?? '?'}` : `Comparing v${ver.version} vs v${compareTarget?.version ?? '?'}`} — {
                            hasDiff
                              ? (zh ? `${diff.addedNodes.length + diff.removedNodes.length} 个节点，${diff.modifiedNodes.length} 处配置变更，${diff.addedEdges.length + diff.removedEdges.length} 条边` : `${diff.addedNodes.length + diff.removedNodes.length} node(s), ${diff.modifiedNodes.length} config change(s), ${diff.addedEdges.length + diff.removedEdges.length} edge(s)`)
                              : (zh ? '完全相同' : 'identical')
                          }
                        </p>
                        {!hasDiff && (
                          <p style={{ color: 'var(--muted)' }}>{zh ? '无变更。' : 'No changes.'}</p>
                        )}
                        {diff.addedNodes.map(n => (
                          <div key={n.id} style={{ color: 'var(--success-text)', fontFamily: 'monospace', marginBottom: 2 }}>
                            + node: {n.id} ({n.type})
                          </div>
                        ))}
                        {diff.removedNodes.map(n => (
                          <div key={n.id} style={{ color: 'var(--danger-text)', fontFamily: 'monospace', marginBottom: 2 }}>
                            − node: {n.id} ({n.type})
                          </div>
                        ))}
                        {diff.modifiedNodes.map(m => (
                          <div key={m.id} style={{ marginBottom: 4 }}>
                            <div style={{ color: 'var(--warning-text)', fontFamily: 'monospace' }}>
                              ~ node: {m.id} ({m.type}) — {zh ? `${m.changedKeys.length} 个字段已变更` : `${m.changedKeys.length} field(s) changed`}
                            </div>
                            <div style={{ paddingLeft: 16 }}>
                              {m.changedKeys.map(k => (
                                <div key={k} style={{ color: 'var(--muted)', fontFamily: 'monospace', fontSize: 11, marginBottom: 1 }}>
                                  ↳ {k}
                                </div>
                              ))}
                            </div>
                          </div>
                        ))}
                        {diff.addedEdges.map((e, idx) => (
                          <div key={idx} style={{ color: 'var(--success-text)', fontFamily: 'monospace', marginBottom: 2 }}>
                            + edge: {e.source} → {e.target}{e.condition_label ? ` [${e.condition_label}]` : ''}
                          </div>
                        ))}
                        {diff.removedEdges.map((e, idx) => (
                          <div key={idx} style={{ color: 'var(--danger-text)', fontFamily: 'monospace', marginBottom: 2 }}>
                            − edge: {e.source} → {e.target}{e.condition_label ? ` [${e.condition_label}]` : ''}
                          </div>
                        ))}
                      </div>
                    )}
                  </div>
                )
              })}
            </div>
          </div>
        </div>
      )}

      {/* Input schema modal */}
      {showSchema && (
        <InputSchemaModal
          schema={inputSchema}
          onChange={setInputSchema}
          onClose={() => setShowSchema(false)}
        />
      )}

      {/* Node command palette (Ctrl+K) */}
      {showPalette && (
        <div className="modal-backdrop" onClick={() => setShowPalette(false)}>
          <div className="modal" style={{ width: 440, padding: 0, overflow: 'hidden' }} onClick={(e) => e.stopPropagation()}>
            <div style={{ padding: '10px 14px', borderBottom: '1px solid var(--border)' }}>
              <input
                autoFocus
                placeholder={zh ? '搜索节点…（输入筛选，回车跳转）' : 'Search nodes… (type to filter, Enter to select)'}
                value={paletteQuery}
                onChange={(e) => setPaletteQuery(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Escape') { setShowPalette(false); return }
                  if (e.key === 'Enter') {
                    const q = paletteQuery.toLowerCase()
                    const match = nodes.find((n) =>
                      n.id.toLowerCase().includes(q) ||
                      (n.data.config?.node_label as string | undefined ?? '').toLowerCase().includes(q)
                    )
                    if (match) { setSelectedNodeId(match.id); setShowPalette(false) }
                  }
                }}
                style={{ width: '100%', border: 'none', outline: 'none', background: 'transparent', fontSize: 14 }}
              />
            </div>
            <div style={{ maxHeight: 320, overflowY: 'auto' }}>
              {nodes
                .filter((n) => {
                  const q = paletteQuery.toLowerCase()
                  return !q || n.id.toLowerCase().includes(q) || (n.data.nodeType ?? '').includes(q) || ((n.data.config?.node_label as string | undefined) ?? '').toLowerCase().includes(q)
                })
                .map((n) => (
                  <div
                    key={n.id}
                    onClick={() => { setSelectedNodeId(n.id); setShowPalette(false) }}
                    style={{
                      display: 'flex', alignItems: 'center', gap: 10,
                      padding: '8px 14px', cursor: 'pointer',
                      background: n.id === selectedNodeId ? 'var(--panel)' : undefined,
                    }}
                    onMouseEnter={(e) => (e.currentTarget.style.background = 'var(--panel)')}
                    onMouseLeave={(e) => (e.currentTarget.style.background = n.id === selectedNodeId ? 'var(--panel)' : '')}
                  >
                    <span style={{ fontSize: 11, color: 'var(--muted)', width: 70, flexShrink: 0 }}>{n.data.nodeType}</span>
                    <code style={{ fontSize: 13, flex: 1 }}>{n.id}</code>
                    {(n.data.config?.node_label as string | undefined) && (
                      <span style={{ fontSize: 11, color: 'var(--link)' }}>{n.data.config!.node_label as string}</span>
                    )}
                  </div>
                ))}
              {nodes.filter((n) => {
                const q = paletteQuery.toLowerCase()
                return !q || n.id.toLowerCase().includes(q) || (n.data.nodeType ?? '').includes(q) || ((n.data.config?.node_label as string | undefined) ?? '').toLowerCase().includes(q)
              }).length === 0 && (
                <div style={{ padding: '16px', color: 'var(--muted)', fontSize: 13, textAlign: 'center' }}>{zh ? '无匹配节点' : 'No nodes match'}</div>
              )}
            </div>
            <div style={{ padding: '6px 14px', borderTop: '1px solid var(--border)', fontSize: 11, color: 'var(--muted)' }}>
              {zh ? '↑↓ 导航 · Enter 跳转 · Esc 关闭' : '↑↓ navigate · Enter to jump · Esc to close'}
            </div>
          </div>
        </div>
      )}

      {/* Variables modal */}
      {showVars && (
        <VariablesModal
          workflowId={workflowId}
          tenantId={auth!.tenantId}
          variables={variables}
          onChanged={setVariables}
          onClose={() => setShowVars(false)}
        />
      )}

      {/* Help modal */}
      {showHelp && (
        <div className="modal-backdrop" onClick={() => setShowHelp(false)}>
          <div className="modal" style={{ width: 480, maxHeight: '80vh', overflowY: 'auto' }} onClick={(e) => e.stopPropagation()}>
            <h2>{zh ? '键盘快捷键与提示' : 'Keyboard Shortcuts & Tips'}</h2>
            <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 12, marginBottom: 16 }}>
              <tbody>
                {(zh ? [
                  ['Ctrl+S', '将当前图保存为新版本'],
                  ['Ctrl+Enter', '运行工作流'],
                  ['Ctrl+K', '打开节点命令面板（搜索/跳转到节点）'],
                  ['Escape', '取消选中节点'],
                  ['Ctrl+D', '复制选中节点'],
                  ['Ctrl+Z', '撤销最近的节点添加/删除/布局'],
                  ['Ctrl+Shift+Z / Ctrl+Y', '重做'],
                  ['Delete / Backspace', '删除选中节点及其关联边'],
                  ['f', '将所有节点适配到视图'],
                  ['点击边标签', '切换条件分支（true ↔ false）'],
                ] : [
                  ['Ctrl+S', 'Save current graph as a new version'],
                  ['Ctrl+Enter', 'Run the workflow'],
                  ['Ctrl+K', 'Open node command palette (search/jump to node)'],
                  ['Escape', 'Deselect selected node'],
                  ['Ctrl+D', 'Duplicate selected node'],
                  ['Ctrl+Z', 'Undo last node add/delete/layout'],
                  ['Ctrl+Shift+Z / Ctrl+Y', 'Redo'],
                  ['Delete / Backspace', 'Delete selected node and its edges'],
                  ['f', 'Fit all nodes into view'],
                  ['Click edge label', 'Toggle condition branch (true ↔ false)'],
                ]).map(([k, v]) => (
                  <tr key={k} style={{ borderBottom: '1px solid var(--border)' }}>
                    <td style={{ padding: '6px 8px', fontFamily: 'monospace', fontWeight: 700, whiteSpace: 'nowrap', color: 'var(--link)' }}>{k}</td>
                    <td style={{ padding: '6px 8px', color: 'var(--fg)' }}>{v}</td>
                  </tr>
                ))}
              </tbody>
            </table>
            <h3 style={{ fontSize: 13, marginBottom: 6 }}>{zh ? '画布提示' : 'Canvas tips'}</h3>
            <ul style={{ fontSize: 12, color: 'var(--fg)', lineHeight: 1.7, paddingLeft: 18, marginBottom: 14 }}>
              <li>{zh ? '从节点连接点拖出以连线' : 'Drag from a node handle to connect nodes'}</li>
              <li>{zh ? <>使用 <strong>⊞ 布局</strong> 自动拓扑排列节点</> : <>Use <strong>⊞ Layout</strong> to auto-arrange nodes topologically</>}</li>
              <li>{zh ? <>启用 <strong>⊹ 对齐</strong> 以在拖拽时 16px 网格对齐</> : <>Enable <strong>⊹ Snap</strong> for 16px grid alignment while dragging</>}</li>
              <li>{zh ? <>切换 <strong>▣ 地图</strong> 以显示/隐藏小地图</> : <>Toggle <strong>▣ Map</strong> to hide/show the minimap</>}</li>
              <li>{zh ? <>按 <strong>f</strong> 或点击 <strong>⊡ 适配</strong> 将所有节点适配到视图</> : <>Press <strong>f</strong> or click <strong>⊡ Fit</strong> to fit all nodes into view</>}</li>
              <li>{zh ? '点击节点以在右侧面板中打开其配置' : 'Click a node to open its config in the right panel'}</li>
              <li>{zh ? <>在配置面板标题栏中使用 <strong>⧉</strong> 复制节点</> : <>Use <strong>⧉</strong> in the config panel header to duplicate a node</>}</li>
            </ul>
            <h3 style={{ fontSize: 13, marginBottom: 6 }}>{zh ? '模板变量' : 'Template variables'}</h3>
            <ul style={{ fontSize: 12, color: 'var(--fg)', lineHeight: 1.7, paddingLeft: 18, marginBottom: 14 }}>
              <li><code>{'{{input.field}}'}</code> — {zh ? '工作流输入 JSON 字段' : 'workflow input JSON field'}</li>
              <li><code>{'{{node_id.field}}'}</code> — {zh ? '前置节点的输出字段' : 'output field from a previous node'}</li>
              <li><code>{'{{credential.name}}'}</code> — {zh ? '已存储的凭证值' : 'stored credential value'}</li>
              <li><code>{'{{env.KEY}}'}</code> — {zh ? '当前环境集中的环境变量' : 'environment variable from active env set'}</li>
              <li><code>{'{{variable.KEY}}'}</code> — {zh ? '持久化工作流变量' : 'persistent workflow variable'}</li>
            </ul>
            <div className="modal-actions">
              <button className="btn btn-primary" onClick={() => setShowHelp(false)}>{zh ? '关闭' : 'Close'}</button>
            </div>
          </div>
        </div>
      )}

      {/* Validate modal */}
      {showValidate && (
        <div className="modal-backdrop" onClick={() => setShowValidate(false)}>
          <div className="modal" style={{ width: 480 }} onClick={(e) => e.stopPropagation()}>
            <h2 style={{ marginBottom: 8 }}>
              {zh ? '工作流校验' : 'Workflow Validation'}
              {validateWarnings.length === 0
                ? <span style={{ color: 'var(--success-text)', fontSize: 14, fontWeight: 400, marginLeft: 8 }}>✓ {zh ? '无问题' : 'No issues'}</span>
                : <span style={{ color: 'var(--warning-text)', fontSize: 14, fontWeight: 400, marginLeft: 8 }}>{zh ? `${validateWarnings.length} 个问题` : `${validateWarnings.length} issue${validateWarnings.length !== 1 ? 's' : ''}`}</span>
              }
            </h2>
            {validateWarnings.length === 0 ? (
              <p style={{ color: 'var(--muted)', fontSize: 13 }}>
                {zh ? '所有节点均已连接并配置完成。工作流已准备好发布。' : 'All nodes are connected and configured. The workflow is ready to publish.'}
              </p>
            ) : (
              <ul style={{ margin: '8px 0 16px', padding: '0 0 0 18px', fontSize: 13, lineHeight: 1.8 }}>
                {validateWarnings.map((w, i) => (
                  <li key={i} style={{ color: 'var(--warning-text)' }}>{w}</li>
                ))}
              </ul>
            )}
            <div className="modal-actions">
              <button className="btn btn-primary" onClick={() => setShowValidate(false)}>{zh ? '关闭' : 'Close'}</button>
            </div>
          </div>
        </div>
      )}

      {/* Readme modal */}
      {showReadme && workflow && (
        <ReadmeModal
          readme={workflow.readme ?? ''}
          onSave={async (text) => {
            try {
              const updated = await api.updateWorkflowReadme(auth!.tenantId, workflowId, workflow.name, text)
              setWorkflow(updated)
              toast(zh ? '文档已保存' : 'Documentation saved')
            } catch (e) {
              toast(String(e), 'error')
            }
            setShowReadme(false)
          }}
          onClose={() => setShowReadme(false)}
        />
      )}

      {/* Schedule modal */}
      {showSchedule && (
        <ScheduleModal
          triggerNode={nodes.find((n) => n.data.nodeType === 'trigger') ?? null}
          onSave={(config) => {
            const trigger = nodes.find((n) => n.data.nodeType === 'trigger')
            if (trigger) handleUpdateConfig(trigger.id, { ...trigger.data.config, ...config })
            setShowSchedule(false)
          }}
          onClose={() => setShowSchedule(false)}
        />
      )}

      {/* Forms modal */}
      {showForms && (
        <FormsModal
          tenantId={auth!.tenantId}
          workflowId={workflowId}
          onClose={() => setShowForms(false)}
        />
      )}

      {/* Tests modal */}
      {showTests && (
        <TestCasesModal
          tenantId={auth!.tenantId}
          workflowId={workflowId}
          onClose={() => setShowTests(false)}
        />
      )}

      {/* Comments modal */}
      {showComments && (
        <CommentsModal
          tenantId={auth!.tenantId}
          workflowId={workflowId}
          author={auth!.tenantId}
          onClose={() => setShowComments(false)}
        />
      )}

      {/* API Docs modal */}
      {showApiDocs && (() => {
        const apiBase = window.location.origin
        const tenantParam = `tenant_id=${encodeURIComponent(auth!.tenantId)}`
        const schemaFields = inputSchema.length > 0 ? inputSchema : []
        const exampleInput = schemaFields.length > 0
          ? JSON.stringify(Object.fromEntries(schemaFields.map((f) => [f.key, f.field_type === 'number' ? 0 : f.field_type === 'boolean' ? false : f.default_value ?? `<${f.key}>`])), null, 2)
          : '{\n  "key": "value"\n}'
        const curlManual = `curl -X POST '${apiBase}/v1/workflows/${workflowId}/executions?${tenantParam}' \\
  -H 'Content-Type: application/json' \\
  -H 'Authorization: Bearer <token>' \\
  -d '{"input_json": ${JSON.stringify(exampleInput)}}'`
        const curlWebhook = webhookUrl
          ? `curl -X POST '${webhookUrl}' \\
  -H 'Content-Type: application/json' \\
  -d '${exampleInput}'`
          : null
        return (
          <div className="modal-backdrop" onClick={() => setShowApiDocs(false)}>
            <div className="modal" style={{ width: 680, maxHeight: '85vh', overflowY: 'auto' }} onClick={(e) => e.stopPropagation()}>
              <h2>📖 {zh ? 'API 文档' : 'API Documentation'}</h2>
              <p style={{ fontSize: 13, color: 'var(--text-secondary)', marginBottom: 20 }}>
                {zh ? '以下是调用此工作流的 API 示例。' : 'API usage examples for triggering this workflow.'}
              </p>

              {/* REST Trigger */}
              <section style={{ marginBottom: 24 }}>
                <h3 style={{ fontSize: 13, fontWeight: 600, marginBottom: 8 }}>{zh ? '手动触发（REST）' : 'Manual Trigger (REST API)'}</h3>
                <pre style={{ background: '#0d1117', color: '#e6edf3', padding: '12px 16px', borderRadius: 8, fontSize: 12, overflowX: 'auto', whiteSpace: 'pre-wrap', wordBreak: 'break-all', margin: 0 }}>
                  {curlManual}
                </pre>
              </section>

              {/* Webhook Trigger */}
              {curlWebhook && (
                <section style={{ marginBottom: 24 }}>
                  <h3 style={{ fontSize: 13, fontWeight: 600, marginBottom: 8 }}>{zh ? 'Webhook 触发' : 'Webhook Trigger'}</h3>
                  <p style={{ fontSize: 12, color: 'var(--text-secondary)', marginBottom: 8 }}>
                    {zh ? '此工作流已发布 Webhook，可通过以下 URL 触发：' : 'This workflow has a published webhook. Trigger it via:'}
                  </p>
                  <pre style={{ background: '#0d1117', color: '#e6edf3', padding: '12px 16px', borderRadius: 8, fontSize: 12, overflowX: 'auto', whiteSpace: 'pre-wrap', wordBreak: 'break-all', margin: 0 }}>
                    {curlWebhook}
                  </pre>
                </section>
              )}

              {/* Input Schema */}
              {schemaFields.length > 0 && (
                <section style={{ marginBottom: 24 }}>
                  <h3 style={{ fontSize: 13, fontWeight: 600, marginBottom: 8 }}>{zh ? '输入字段' : 'Input Fields'}</h3>
                  <table style={{ width: '100%', fontSize: 12, borderCollapse: 'collapse' }}>
                    <thead>
                      <tr style={{ borderBottom: '1px solid var(--border)' }}>
                        <th style={{ textAlign: 'left', padding: '4px 8px', color: 'var(--text-secondary)', fontWeight: 500 }}>{zh ? '字段名' : 'Field'}</th>
                        <th style={{ textAlign: 'left', padding: '4px 8px', color: 'var(--text-secondary)', fontWeight: 500 }}>{zh ? '类型' : 'Type'}</th>
                        <th style={{ textAlign: 'left', padding: '4px 8px', color: 'var(--text-secondary)', fontWeight: 500 }}>{zh ? '必填' : 'Required'}</th>
                        <th style={{ textAlign: 'left', padding: '4px 8px', color: 'var(--text-secondary)', fontWeight: 500 }}>{zh ? '说明' : 'Description'}</th>
                      </tr>
                    </thead>
                    <tbody>
                      {schemaFields.map((f) => (
                        <tr key={f.key} style={{ borderBottom: '1px solid var(--border)' }}>
                          <td style={{ padding: '6px 8px', fontFamily: 'monospace', fontWeight: 600 }}>{f.key}</td>
                          <td style={{ padding: '6px 8px' }}><span className="badge">{f.field_type}</span></td>
                          <td style={{ padding: '6px 8px', color: f.required ? 'var(--danger-text)' : 'var(--muted)' }}>{f.required ? '✓' : '—'}</td>
                          <td style={{ padding: '6px 8px', color: 'var(--text-secondary)' }}>{f.description || '—'}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </section>
              )}

              {/* JSON Schema */}
              {schemaFields.length > 0 && (() => {
                const jsonSchema = {
                  $schema: 'https://json-schema.org/draft/2020-12/schema',
                  title: workflow?.name,
                  type: 'object',
                  properties: Object.fromEntries(schemaFields.map((f) => [
                    f.key,
                    {
                      type: f.field_type === 'number' ? 'number' : f.field_type === 'boolean' ? 'boolean' : f.field_type === 'json' ? 'object' : 'string',
                      ...(f.description ? { description: f.description } : {}),
                      ...(f.default_value != null ? { default: f.default_value } : {}),
                    },
                  ])),
                  ...(schemaFields.some((f) => f.required) ? { required: schemaFields.filter((f) => f.required).map((f) => f.key) } : {}),
                }
                return (
                  <section style={{ marginBottom: 24 }}>
                    <h3 style={{ fontSize: 13, fontWeight: 600, marginBottom: 8 }}>
                      {zh ? 'JSON Schema' : 'JSON Schema'}
                      <button
                        className="btn btn-sm"
                        style={{ marginLeft: 8, fontSize: 10, padding: '2px 6px' }}
                        onClick={() => navigator.clipboard?.writeText(JSON.stringify(jsonSchema, null, 2))}
                      >⎘ {zh ? '复制' : 'Copy'}</button>
                    </h3>
                    <pre style={{ background: '#0d1117', color: '#e6edf3', padding: '12px 16px', borderRadius: 8, fontSize: 11, overflowX: 'auto', whiteSpace: 'pre', margin: 0 }}>
                      {JSON.stringify(jsonSchema, null, 2)}
                    </pre>
                  </section>
                )
              })()}

              {/* Workflow info */}
              <section style={{ marginBottom: 16, fontSize: 12, color: 'var(--text-secondary)' }}>
                <div><strong>{zh ? '工作流 ID:' : 'Workflow ID:'}</strong> <code>{workflowId}</code></div>
                <div><strong>{zh ? '状态:' : 'Status:'}</strong> {workflow?.status ?? '—'}</div>
                {version && <div><strong>{zh ? '版本:' : 'Version:'}</strong> v{version.version} ({version.id.slice(0, 12)}…)</div>}
              </section>

              <div className="modal-actions">
                <button className="btn btn-primary" onClick={() => setShowApiDocs(false)}>{zh ? '关闭' : 'Close'}</button>
              </div>
            </div>
          </div>
        )
      })()}

      {/* Performance Report modal */}
      {showReport && (
        <div className="modal-backdrop" onClick={() => setShowReport(false)}>
          <div className="modal" style={{ width: 700, maxHeight: '85vh', overflowY: 'auto' }} onClick={(e) => e.stopPropagation()}>
            <h2>📊 {zh ? '工作流性能报告' : 'Workflow Performance Report'}</h2>

            {/* Overview */}
            {wfStats && (
              <section style={{ marginBottom: 20 }}>
                <h3 style={{ fontSize: 13, fontWeight: 600, marginBottom: 10, color: 'var(--text-secondary)' }}>{zh ? '概览' : 'Overview'}</h3>
                <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4,1fr)', gap: 10 }}>
                  {[
                    { label: zh ? '总执行' : 'Total', value: wfStats.total, color: undefined },
                    { label: zh ? '成功' : 'Succeeded', value: wfStats.succeeded, color: '#22c55e' },
                    { label: zh ? '失败' : 'Failed', value: wfStats.failed, color: '#ef4444' },
                    { label: zh ? '成功率' : 'Success Rate', value: wfStats.total > 0 ? `${Math.round((wfStats.succeeded / wfStats.total) * 100)}%` : '—', color: undefined },
                  ].map((s) => (
                    <div key={s.label} style={{ background: 'var(--bg-secondary)', borderRadius: 8, padding: '12px 14px' }}>
                      <div style={{ fontSize: 11, color: 'var(--text-secondary)', marginBottom: 4 }}>{s.label}</div>
                      <div style={{ fontSize: 22, fontWeight: 700, color: s.color }}>{s.value}</div>
                    </div>
                  ))}
                </div>
                {wfStats.avg_duration_secs != null && (
                  <div style={{ marginTop: 8, fontSize: 12, color: 'var(--text-secondary)' }}>
                    {zh ? '平均耗时' : 'Avg duration'}: <strong>{wfStats.avg_duration_secs < 1 ? `${Math.round(wfStats.avg_duration_secs * 1000)}ms` : `${wfStats.avg_duration_secs.toFixed(1)}s`}</strong>
                  </div>
                )}
              </section>
            )}

            {/* SLA compliance */}
            {workflow?.sla_seconds != null && reportExecs.length > 0 && (() => {
              const slaSecs = workflow.sla_seconds!
              const finished = reportExecs.filter((e) => e.finished_at != null)
              const breached = finished.filter((e) => ((e.finished_at! - e.started_at) / 1000) > slaSecs)
              const rate = finished.length > 0 ? Math.round(((finished.length - breached.length) / finished.length) * 100) : 100
              return (
                <section style={{ marginBottom: 20 }}>
                  <h3 style={{ fontSize: 13, fontWeight: 600, marginBottom: 10, color: 'var(--text-secondary)' }}>{zh ? 'SLA 合规' : 'SLA Compliance'}</h3>
                  <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3,1fr)', gap: 10 }}>
                    <div style={{ background: 'var(--bg-secondary)', borderRadius: 8, padding: '12px 14px' }}>
                      <div style={{ fontSize: 11, color: 'var(--text-secondary)', marginBottom: 4 }}>{zh ? 'SLA 阈值' : 'SLA Threshold'}</div>
                      <div style={{ fontSize: 18, fontWeight: 700 }}>{slaSecs}s</div>
                    </div>
                    <div style={{ background: 'var(--bg-secondary)', borderRadius: 8, padding: '12px 14px' }}>
                      <div style={{ fontSize: 11, color: 'var(--text-secondary)', marginBottom: 4 }}>{zh ? '合规率' : 'Compliance Rate'}</div>
                      <div style={{ fontSize: 18, fontWeight: 700, color: rate >= 95 ? '#22c55e' : rate >= 80 ? '#f59e0b' : '#ef4444' }}>{rate}%</div>
                    </div>
                    <div style={{ background: 'var(--bg-secondary)', borderRadius: 8, padding: '12px 14px' }}>
                      <div style={{ fontSize: 11, color: 'var(--text-secondary)', marginBottom: 4 }}>{zh ? '超时次数' : 'Breaches'}</div>
                      <div style={{ fontSize: 18, fontWeight: 700, color: breached.length > 0 ? '#ef4444' : '#22c55e' }}>{breached.length}</div>
                    </div>
                  </div>
                </section>
              )
            })()}

            {/* Trigger breakdown */}
            {reportExecs.length > 0 && (() => {
              const counts: Record<string, number> = {}
              reportExecs.forEach((e) => { const t = e.trigger_type ?? 'unknown'; counts[t] = (counts[t] ?? 0) + 1 })
              return (
                <section style={{ marginBottom: 20 }}>
                  <h3 style={{ fontSize: 13, fontWeight: 600, marginBottom: 10, color: 'var(--text-secondary)' }}>{zh ? '触发方式分布' : 'Trigger Breakdown'}</h3>
                  <div style={{ display: 'flex', gap: 10, flexWrap: 'wrap' }}>
                    {Object.entries(counts).map(([type, count]) => (
                      <div key={type} style={{ background: 'var(--bg-secondary)', borderRadius: 6, padding: '8px 14px', fontSize: 13 }}>
                        <span style={{ textTransform: 'capitalize', fontWeight: 600 }}>{type}</span>
                        <span style={{ marginLeft: 8, color: 'var(--text-secondary)' }}>{count}</span>
                      </div>
                    ))}
                  </div>
                </section>
              )
            })()}

            {/* Recent failures */}
            {reportExecs.filter((e) => e.status === 'failed').length > 0 && (
              <section style={{ marginBottom: 20 }}>
                <h3 style={{ fontSize: 13, fontWeight: 600, marginBottom: 10, color: 'var(--text-secondary)' }}>{zh ? '近期失败' : 'Recent Failures'}</h3>
                <table style={{ width: '100%', fontSize: 12, borderCollapse: 'collapse' }}>
                  <thead>
                    <tr style={{ borderBottom: '1px solid var(--border)' }}>
                      <th style={{ textAlign: 'left', padding: '4px 8px', color: 'var(--text-secondary)', fontWeight: 500 }}>{zh ? '执行 ID' : 'Execution ID'}</th>
                      <th style={{ textAlign: 'left', padding: '4px 8px', color: 'var(--text-secondary)', fontWeight: 500 }}>{zh ? '开始时间' : 'Started'}</th>
                      <th style={{ textAlign: 'left', padding: '4px 8px', color: 'var(--text-secondary)', fontWeight: 500 }}>{zh ? '耗时' : 'Duration'}</th>
                      <th style={{ textAlign: 'left', padding: '4px 8px', color: 'var(--text-secondary)', fontWeight: 500 }}>{zh ? '触发' : 'Trigger'}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {reportExecs.filter((e) => e.status === 'failed').slice(0, 10).map((e) => {
                      const durMs = e.finished_at != null ? (e.finished_at - e.started_at) : null
                      return (
                        <tr key={e.id} style={{ borderBottom: '1px solid var(--border)' }}>
                          <td style={{ padding: '6px 8px', fontFamily: 'monospace', fontSize: 11 }}>{e.id.slice(0, 8)}…</td>
                          <td style={{ padding: '6px 8px', color: 'var(--text-secondary)' }}>{new Date(e.started_at * 1000).toLocaleString()}</td>
                          <td style={{ padding: '6px 8px' }}>{durMs != null ? (durMs < 1000 ? `${durMs}ms` : `${(durMs / 1000).toFixed(1)}s`) : '—'}</td>
                          <td style={{ padding: '6px 8px' }}><span className="badge">{e.trigger_type ?? 'manual'}</span></td>
                        </tr>
                      )
                    })}
                  </tbody>
                </table>
              </section>
            )}

            <div className="modal-actions">
              <button className="btn btn-primary" onClick={() => setShowReport(false)}>{zh ? '关闭' : 'Close'}</button>
            </div>
          </div>
        </div>
      )}

      {/* Rename modal */}
      {renaming && (
        <div className="modal-backdrop" onClick={() => setRenaming(false)}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            <h2>{zh ? '重命名工作流' : 'Rename Workflow'}</h2>
            <div className="field">
              <label>{zh ? '名称' : 'Name'}</label>
              <input
                autoFocus
                value={newName}
                onChange={(e) => setNewName(e.target.value)}
                onKeyDown={(e) => { if (e.key === 'Enter') handleRename(); if (e.key === 'Escape') setRenaming(false) }}
              />
            </div>
            <div className="modal-actions">
              <button className="btn" onClick={() => setRenaming(false)}>{zh ? '取消' : 'Cancel'}</button>
              <button className="btn btn-primary" onClick={handleRename}>{zh ? '保存' : 'Save'}</button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}

function InputSchemaModal({
  schema, onChange, onClose,
}: {
  schema: InputField[]
  onChange: (s: InputField[]) => void
  onClose: () => void
}) {
  const { locale } = useLocale()
  const zh = locale === 'zh'
  const [fields, setFields] = useState<InputField[]>(schema)

  const addField = () => setFields((f) => [...f, { key: '', field_type: 'string', required: false, description: '', default_value: '' }])

  const updateField = (i: number, patch: Partial<InputField>) =>
    setFields((f) => f.map((x, j) => (j === i ? { ...x, ...patch } : x)))

  const removeField = (i: number) => setFields((f) => f.filter((_, j) => j !== i))

  const handleSave = () => {
    onChange(fields.filter((f) => f.key.trim()))
    onClose()
  }

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" style={{ width: 580, maxHeight: '80vh', overflow: 'auto' }} onClick={(e) => e.stopPropagation()}>
        <h2>{zh ? '输入模式' : 'Input Schema'}</h2>
        <p style={{ color: 'var(--muted)', fontSize: 12, marginBottom: 16 }}>
          {zh ? <>定义预期输入字段。运行面板将根据此模式显示表单。在节点配置中使用 <code>{'{{input.FIELD}}'}</code> 引用这些值。</> : <>Define the expected input fields. The run panel will show a form based on this schema. Use <code>{'{{input.FIELD}}'}</code> in node configs to reference these values.</>}
        </p>

        {fields.length === 0 && (
          <p style={{ color: 'var(--muted)', fontSize: 13, textAlign: 'center', padding: '12px 0' }}>
            {zh ? '暂无字段。在下方添加一个。' : 'No fields yet. Add one below.'}
          </p>
        )}

        {fields.map((f, i) => (
          <div key={i} style={{ display: 'grid', gridTemplateColumns: '1fr 100px 80px auto', gap: 8, marginBottom: 8, alignItems: 'center' }}>
            <input
              placeholder={zh ? '键名（如 lead_id）' : 'key (e.g. lead_id)'}
              value={f.key}
              onChange={(e) => updateField(i, { key: e.target.value })}
              style={{ fontFamily: 'monospace', fontSize: 12 }}
            />
            <select
              value={f.field_type}
              onChange={(e) => updateField(i, { field_type: e.target.value as InputField['field_type'] })}
              style={{ fontSize: 12 }}
            >
              <option value="string">string</option>
              <option value="number">number</option>
              <option value="boolean">boolean</option>
              <option value="json">json</option>
            </select>
            <label style={{ fontSize: 12, display: 'flex', alignItems: 'center', gap: 4, color: 'var(--muted)' }}>
              <input type="checkbox" checked={f.required} onChange={(e) => updateField(i, { required: e.target.checked })} />
              {zh ? '必填' : 'required'}
            </label>
            <button className="btn btn-sm btn-danger" onClick={() => removeField(i)}>✕</button>
            <input
              placeholder={zh ? '描述（可选）' : 'description (optional)'}
              value={f.description}
              onChange={(e) => updateField(i, { description: e.target.value })}
              style={{ fontSize: 12, gridColumn: '1 / 3' }}
            />
            <input
              placeholder={zh ? '默认值（可选）' : 'default value (optional)'}
              value={f.default_value ?? ''}
              onChange={(e) => updateField(i, { default_value: e.target.value })}
              style={{ fontSize: 12, gridColumn: '3 / 5' }}
            />
          </div>
        ))}

        <button className="btn btn-sm" onClick={addField} style={{ marginTop: 8, marginBottom: 16 }}>
          {zh ? '+ 添加字段' : '+ Add Field'}
        </button>

        <div className="modal-actions">
          <button className="btn" onClick={onClose}>{zh ? '取消' : 'Cancel'}</button>
          <button className="btn btn-primary" onClick={handleSave}>{zh ? '保存模式' : 'Save Schema'}</button>
        </div>
      </div>
    </div>
  )
}

function VariablesModal({
  workflowId, tenantId, variables, onChanged, onClose,
}: {
  workflowId: string
  tenantId: string
  variables: api.Variable[]
  onChanged: (v: api.Variable[]) => void
  onClose: () => void
}) {
  const { locale } = useLocale()
  const zh = locale === 'zh'
  const [newKey, setNewKey] = useState('')
  const [newVal, setNewVal] = useState('')
  const [saving, setSaving] = useState(false)
  const [error, setError]   = useState<string | null>(null)

  const handleSet = async () => {
    if (!newKey.trim()) return
    let parsed: unknown
    try { parsed = JSON.parse(newVal) } catch { parsed = newVal }
    setSaving(true)
    setError(null)
    try {
      await api.setVariable(tenantId, workflowId, newKey.trim(), parsed)
      const updated = await api.listVariables(tenantId, workflowId)
      onChanged(updated)
      setNewKey('')
      setNewVal('')
    } catch (e) {
      setError(String(e))
    } finally {
      setSaving(false)
    }
  }

  const handleDelete = async (key: string) => {
    try {
      await api.deleteVariable(tenantId, workflowId, key)
      onChanged(variables.filter((v) => v.key !== key))
    } catch (e) {
      setError(String(e))
    }
  }

  const handleIncrement = async (key: string) => {
    try {
      const updated = await api.incrementVariable(tenantId, workflowId, key)
      onChanged(variables.map((v) => v.key === key ? updated : v))
    } catch (e) {
      setError(String(e))
    }
  }

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" style={{ width: 520, maxHeight: '75vh', overflow: 'auto' }} onClick={(e) => e.stopPropagation()}>
        <h2>{zh ? '工作流变量' : 'Workflow Variables'}</h2>
        <p style={{ color: 'var(--muted)', fontSize: 12, marginBottom: 14 }}>
          {zh ? <>此工作流的持久化键值存储。在节点配置中通过 <code>{'{{variable.KEY}}'}</code> 访问。值在多次执行间保留。</> : <>Persistent key-value store for this workflow. Access via <code>{'{{variable.KEY}}'}</code> in node configs. Values survive across executions.</>}
        </p>
        {error && <p style={{ color: 'var(--danger-text)', fontSize: 12, marginBottom: 8 }}>{error}</p>}

        {variables.length === 0 ? (
          <p style={{ color: 'var(--muted)', fontSize: 13, textAlign: 'center', padding: '12px 0' }}>
            {zh ? '暂无变量。' : 'No variables yet.'}
          </p>
        ) : (
          <table style={{ width: '100%', borderCollapse: 'collapse', marginBottom: 16, fontSize: 12 }}>
            <thead>
              <tr style={{ borderBottom: '1px solid var(--border)' }}>
                <th style={{ textAlign: 'left', padding: '4px 8px', color: 'var(--muted)', fontWeight: 600 }}>{zh ? '键' : 'Key'}</th>
                <th style={{ textAlign: 'left', padding: '4px 8px', color: 'var(--muted)', fontWeight: 600 }}>{zh ? '值' : 'Value'}</th>
                <th style={{ width: 80 }}></th>
              </tr>
            </thead>
            <tbody>
              {variables.map((v) => (
                <tr key={v.key} style={{ borderBottom: '1px solid var(--border)' }}>
                  <td style={{ padding: '6px 8px', fontFamily: 'monospace', fontWeight: 600 }}>{v.key}</td>
                  <td style={{ padding: '6px 8px', fontFamily: 'monospace', color: 'var(--muted)', maxWidth: 240, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                    {JSON.stringify(v.value)}
                  </td>
                  <td style={{ padding: '4px 8px', display: 'flex', gap: 4 }}>
                    {typeof v.value === 'number' && (
                      <button className="btn btn-sm btn-icon" onClick={() => handleIncrement(v.key)} title={zh ? '加 1' : 'Increment by 1'} style={{ fontSize: 11 }}>+1</button>
                    )}
                    <button className="btn btn-sm btn-danger" onClick={() => handleDelete(v.key)} title={zh ? '删除变量' : 'Delete variable'} style={{ fontSize: 11 }}>✕</button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}

        <div style={{ display: 'flex', gap: 8, alignItems: 'center', marginBottom: 16 }}>
          <input
            placeholder={zh ? '键名' : 'key'}
            value={newKey}
            onChange={(e) => setNewKey(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && handleSet()}
            style={{ fontFamily: 'monospace', fontSize: 12, flex: '0 0 140px' }}
          />
          <input
            placeholder={zh ? '值（JSON 或字符串）' : 'value (JSON or string)'}
            value={newVal}
            onChange={(e) => setNewVal(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && handleSet()}
            style={{ fontFamily: 'monospace', fontSize: 12, flex: 1 }}
          />
          <button className="btn btn-sm btn-primary" disabled={!newKey.trim() || saving} onClick={handleSet}>
            {zh ? '设置' : 'Set'}
          </button>
        </div>

        <div className="modal-actions">
          <button className="btn" onClick={onClose}>{zh ? '关闭' : 'Close'}</button>
        </div>
      </div>
    </div>
  )
}

function ReadmeModal({
  readme,
  onSave,
  onClose,
}: {
  readme: string
  onSave: (text: string) => Promise<void>
  onClose: () => void
}) {
  const { locale } = useLocale()
  const zh = locale === 'zh'
  const [text, setText] = useState(readme)
  const [saving, setSaving] = useState(false)

  const handleSave = async () => {
    setSaving(true)
    try { await onSave(text) } finally { setSaving(false) }
  }

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" style={{ width: 640, maxHeight: '85vh', display: 'flex', flexDirection: 'column', gap: 0, padding: 0 }} onClick={(e) => e.stopPropagation()}>
        <div style={{ padding: '16px 20px 12px', borderBottom: '1px solid var(--border)', display: 'flex', alignItems: 'center', justifyContent: 'space-between', flexShrink: 0 }}>
          <div>
            <h2 style={{ margin: 0, fontSize: 15 }}>{zh ? '工作流文档' : 'Workflow Documentation'}</h2>
            <p style={{ margin: '3px 0 0', fontSize: 12, color: 'var(--muted)' }}>
              {zh ? '支持 Markdown。描述输入、输出、依赖项和使用说明。' : 'Markdown supported. Describe inputs, outputs, dependencies, and usage notes.'}
            </p>
          </div>
          <button className="btn btn-sm" onClick={onClose}>✕</button>
        </div>
        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: 0, overflow: 'hidden' }}>
          <textarea
            autoFocus
            value={text}
            onChange={(e) => setText(e.target.value)}
            placeholder={zh ? `# 我的工作流\n\n描述此工作流的功能、期望的输入以及输出内容。\n\n## 输入\n- \`lead_id\` — 要处理的线索\n\n## 输出\n返回丰富后的线索记录。` : `# My Workflow\n\nDescribe what this workflow does, what inputs it expects, and what it outputs.\n\n## Inputs\n- \`lead_id\` — The lead to process\n\n## Outputs\nReturns the enriched lead record.`}
            style={{
              flex: 1, width: '100%', minHeight: 340, resize: 'none',
              fontFamily: 'monospace', fontSize: 13, padding: '14px 20px',
              border: 'none', outline: 'none', background: 'var(--bg)',
              color: 'var(--fg)', boxSizing: 'border-box', lineHeight: 1.6,
            }}
          />
        </div>
        <div style={{ padding: '10px 20px', borderTop: '1px solid var(--border)', display: 'flex', justifyContent: 'flex-end', gap: 8, flexShrink: 0 }}>
          <span style={{ fontSize: 11, color: 'var(--muted)', alignSelf: 'center', marginRight: 'auto' }}>
            {zh ? `${text.length} 个字符` : `${text.length} chars`}
          </span>
          {text && (
            <button className="btn btn-sm btn-danger" onClick={() => setText('')} disabled={saving}>{zh ? '清除' : 'Clear'}</button>
          )}
          <button className="btn" onClick={onClose}>{zh ? '取消' : 'Cancel'}</button>
          <button className="btn btn-primary" onClick={handleSave} disabled={saving}>
            {saving ? (zh ? '保存中…' : 'Saving…') : (zh ? '保存' : 'Save')}
          </button>
        </div>
      </div>
    </div>
  )
}

function ScheduleModal({
  triggerNode,
  onSave,
  onClose,
}: {
  triggerNode: FlowNode | null
  onSave: (config: Record<string, unknown>) => void
  onClose: () => void
}) {
  const cfg = (triggerNode?.data.config ?? {}) as Record<string, unknown>
  const initCron = (cfg.cron_expression as string) ?? ''
  const initInterval = (cfg.interval_secs as number) ?? 0
  const initMode = initCron ? 'cron' : initInterval > 0 ? 'interval' : 'none'

  const { locale } = useLocale()
  const zh = locale === 'zh'
  const [mode, setMode] = useState<'none' | 'interval' | 'cron'>(initMode)
  const [cronExpr, setCronExpr] = useState(initCron || '0 9 * * 1-5')
  const [intervalSecs, setIntervalSecs] = useState(initInterval > 0 ? initInterval : 3600)
  const [nextTimes, setNextTimes] = useState<string[]>([])
  const [cronError, setCronError] = useState<string | null>(null)
  const [previewing, setPreviewing] = useState(false)

  useEffect(() => {
    if (mode !== 'cron') { setNextTimes([]); setCronError(null); return }
    const id = setTimeout(async () => {
      if (!cronExpr.trim()) return
      setPreviewing(true)
      try {
        const res = await api.previewCron(cronExpr.trim(), 5)
        if (res.error) { setCronError(res.error); setNextTimes([]) }
        else { setCronError(null); setNextTimes(res.next_times) }
      } catch { setCronError('Preview failed') }
      finally { setPreviewing(false) }
    }, 500)
    return () => clearTimeout(id)
  }, [cronExpr, mode])

  const handleSave = () => {
    if (mode === 'none') {
      onSave({ cron_expression: undefined, interval_secs: undefined })
    } else if (mode === 'interval') {
      onSave({ cron_expression: undefined, interval_secs: intervalSecs })
    } else {
      if (cronError) return
      onSave({ interval_secs: undefined, cron_expression: cronExpr.trim() })
    }
  }

  const PRESETS = zh ? [
    { label: '工作日 9 点',   expr: '0 9 * * 1-5' },
    { label: '每小时',        expr: '0 * * * *' },
    { label: '每天午夜',      expr: '0 0 * * *' },
    { label: '周一 8 点',     expr: '0 8 * * 1' },
    { label: '每 15 分钟',    expr: '*/15 * * * *' },
  ] : [
    { label: 'Weekdays 9am',     expr: '0 9 * * 1-5' },
    { label: 'Every hour',       expr: '0 * * * *' },
    { label: 'Daily midnight',   expr: '0 0 * * *' },
    { label: 'Mon 8am',          expr: '0 8 * * 1' },
    { label: 'Every 15 min',     expr: '*/15 * * * *' },
  ]

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" style={{ width: 480 }} onClick={(e) => e.stopPropagation()}>
        <h2 style={{ marginBottom: 4 }}>{zh ? '自动运行计划' : 'Auto-Run Schedule'}</h2>
        <p style={{ color: 'var(--muted)', fontSize: 12, marginBottom: 16 }}>
          {zh ? '配置触发节点以自动运行。计划在发布后生效。' : 'Configure the trigger node to run automatically. Schedule activates after publishing.'}
        </p>

        {!triggerNode && (
          <p style={{ color: 'var(--danger-text)', fontSize: 12, marginBottom: 12 }}>
            {zh ? '未找到触发节点，请先在画布中添加一个。' : 'No trigger node found. Add one to the canvas first.'}
          </p>
        )}

        <div className="field">
          <label>{zh ? '计划类型' : 'Schedule type'}</label>
          <select value={mode} onChange={(e) => setMode(e.target.value as typeof mode)} disabled={!triggerNode}>
            <option value="none">{zh ? '无（手动 / Webhook）' : 'None (manual / webhook only)'}</option>
            <option value="interval">{zh ? '固定间隔' : 'Fixed interval'}</option>
            <option value="cron">{zh ? 'Cron 表达式' : 'Cron expression'}</option>
          </select>
        </div>

        {mode === 'interval' && (
          <div className="field">
            <label>{zh ? '间隔' : 'Interval'}</label>
            <select value={intervalSecs} onChange={(e) => setIntervalSecs(Number(e.target.value))}>
              <option value={60}>{zh ? '每分钟' : 'Every minute'}</option>
              <option value={300}>{zh ? '每 5 分钟' : 'Every 5 minutes'}</option>
              <option value={900}>{zh ? '每 15 分钟' : 'Every 15 minutes'}</option>
              <option value={1800}>{zh ? '每 30 分钟' : 'Every 30 minutes'}</option>
              <option value={3600}>{zh ? '每小时' : 'Every hour'}</option>
              <option value={21600}>{zh ? '每 6 小时' : 'Every 6 hours'}</option>
              <option value={86400}>{zh ? '每天' : 'Every day'}</option>
            </select>
          </div>
        )}

        {mode === 'cron' && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
            <div className="field">
              <label>{zh ? 'Cron 表达式' : 'Cron expression'} <span style={{ color: 'var(--muted)', fontWeight: 400 }}>{zh ? '（5 字段 UTC）' : '(5-field UTC)'}</span></label>
              <input
                value={cronExpr}
                onChange={(e) => setCronExpr(e.target.value)}
                placeholder="0 9 * * 1-5"
                style={{ fontFamily: 'monospace' }}
              />
              {cronError && <p style={{ color: 'var(--danger-text)', fontSize: 11, marginTop: 4 }}>{cronError}</p>}
            </div>
            <div>
              <div style={{ fontSize: 11, color: 'var(--muted)', marginBottom: 4 }}>{zh ? '预设：' : 'Presets:'}</div>
              <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6 }}>
                {PRESETS.map((p) => (
                  <button
                    key={p.expr}
                    className={`btn btn-sm${cronExpr === p.expr ? ' btn-primary' : ''}`}
                    onClick={() => setCronExpr(p.expr)}
                    style={{ fontSize: 11 }}
                  >
                    {p.label}
                  </button>
                ))}
              </div>
            </div>
            <div>
              <div style={{ fontSize: 11, color: 'var(--muted)', marginBottom: 4 }}>
                {zh ? '接下来 5 次运行（UTC）：' : 'Next 5 runs (UTC):'} {previewing && '…'}
              </div>
              {nextTimes.length > 0 && (
                <ul style={{ margin: 0, padding: 0, listStyle: 'none', fontSize: 12, fontFamily: 'monospace', color: 'var(--fg)' }}>
                  {nextTimes.map((t, i) => (
                    <li key={i} style={{ padding: '2px 0', color: i === 0 ? 'var(--link)' : undefined }}>{t}</li>
                  ))}
                </ul>
              )}
              {!previewing && !cronError && nextTimes.length === 0 && cronExpr.trim() && (
                <div style={{ fontSize: 11, color: 'var(--muted)' }}>{zh ? '请输入有效的 Cron 表达式以预览。' : 'Enter a valid cron expression to preview.'}</div>
              )}
            </div>
          </div>
        )}

        <div className="modal-actions" style={{ marginTop: 20 }}>
          <button className="btn" onClick={onClose}>{zh ? '取消' : 'Cancel'}</button>
          <button
            className="btn btn-primary"
            onClick={handleSave}
            disabled={!triggerNode || (mode === 'cron' && !!cronError)}
          >
            {zh ? '应用到触发节点' : 'Apply to Trigger Node'}
          </button>
        </div>
        <p style={{ fontSize: 11, color: 'var(--muted)', marginTop: 8 }}>
          {zh ? <>修改在 <strong>保存版本</strong> + <strong>发布</strong> 后生效。</> : <>Changes take effect after <strong>Save Version</strong> + <strong>Publish</strong>.</>}
        </p>
      </div>
    </div>
  )
}

function RecentRunsMini({
  executions,
  onLoad,
}: {
  executions: ExecutionSummary[]
  onLoad: (id: string) => Promise<void>
}) {
  const { locale } = useLocale()
  const zh = locale === 'zh'
  const [open, setOpen] = useState(true)
  const [loading, setLoading] = useState<string | null>(null)

  const recent = executions.slice(0, 5)

  const statusColor = (s: string) => {
    if (s === 'succeeded') return 'var(--success-text)'
    if (s === 'failed')    return 'var(--danger-text)'
    if (s === 'running')   return 'var(--link)'
    return 'var(--muted)'
  }

  const age = (ts: number) => {
    const secs = Math.floor(Date.now() / 1000) - ts
    if (zh) {
      if (secs < 60)    return `${secs}秒前`
      if (secs < 3600)  return `${Math.floor(secs / 60)}分钟前`
      if (secs < 86400) return `${Math.floor(secs / 3600)}小时前`
      return `${Math.floor(secs / 86400)}天前`
    }
    if (secs < 60)    return `${secs}s ago`
    if (secs < 3600)  return `${Math.floor(secs / 60)}m ago`
    if (secs < 86400) return `${Math.floor(secs / 3600)}h ago`
    return `${Math.floor(secs / 86400)}d ago`
  }

  const handleLoad = async (id: string) => {
    setLoading(id)
    try { await onLoad(id) } finally { setLoading(null) }
  }

  return (
    <div style={{ marginTop: 12, borderTop: '1px solid var(--border)', paddingTop: 8 }}>
      <button
        onClick={() => setOpen((o) => !o)}
        style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--muted)', fontSize: 11, fontWeight: 600, padding: '2px 0', display: 'flex', alignItems: 'center', gap: 4 }}
      >
        <span>{open ? '▾' : '▸'}</span> {zh ? '最近运行' : 'Recent Runs'}
      </button>
      {open && (
        <table style={{ width: '100%', borderCollapse: 'collapse', marginTop: 6, fontSize: 11 }}>
          <tbody>
            {recent.map((ex) => (
              <tr key={ex.id} style={{ borderBottom: '1px solid var(--border)' }}>
                <td style={{ padding: '4px 4px', color: statusColor(ex.status), fontWeight: 600, width: 72 }}>
                  {ex.status}
                </td>
                <td style={{ padding: '4px 4px', color: 'var(--muted)', fontFamily: 'monospace', fontSize: 10 }}>
                  {ex.label || ex.id.slice(-8)}
                </td>
                <td style={{ padding: '4px 4px', color: 'var(--muted)', textAlign: 'right' }}>
                  {age(ex.started_at)}
                </td>
                <td style={{ padding: '4px 4px', textAlign: 'right', width: 48 }}>
                  <button
                    className="btn btn-sm"
                    disabled={loading === ex.id}
                    onClick={() => handleLoad(ex.id)}
                    style={{ fontSize: 10, padding: '1px 6px' }}
                  >
                    {loading === ex.id ? '…' : (zh ? '加载' : 'Load')}
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  )
}

// ── FormsModal ──────────────────────────────────────────────────────────────

function FormsModal({
  tenantId,
  workflowId,
  onClose,
}: {
  tenantId: string
  workflowId: string
  onClose: () => void
}) {
  const { locale } = useLocale()
  const zh = locale === 'zh'
  const [forms, setForms] = useState<api.FormTokenRecord[]>([])
  const [title, setTitle] = useState('')
  const [desc, setDesc] = useState('')
  const [publishing, setPublishing] = useState(false)
  const [copied, setCopied] = useState<string | null>(null)

  useEffect(() => {
    api.listForms(tenantId, workflowId).then(setForms).catch(() => {})
  }, [tenantId, workflowId])

  const handlePublish = async () => {
    if (!title.trim()) return
    setPublishing(true)
    try {
      await api.publishForm(tenantId, workflowId, title.trim(), desc.trim() || undefined)
      const updated = await api.listForms(tenantId, workflowId)
      setForms(updated)
      setTitle('')
      setDesc('')
    } catch {
      // ignore
    } finally {
      setPublishing(false)
    }
  }

  const handleDelete = async (token: string) => {
    await api.deleteForm(token).catch(() => {})
    setForms((prev) => prev.filter((f) => f.token !== token))
  }

  const formUrl = (token: string) => `${window.location.origin}/forms/${token}`

  const copyLink = (token: string) => {
    navigator.clipboard.writeText(formUrl(token)).catch(() => {})
    setCopied(token)
    setTimeout(() => setCopied(null), 2000)
  }

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" style={{ width: 520 }} onClick={(e) => e.stopPropagation()}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 16 }}>
          <h2 style={{ margin: 0, fontSize: 16 }}>{zh ? '表单发布器' : 'Form Publisher'}</h2>
          <button className="btn btn-sm" onClick={onClose}>✕</button>
        </div>

        <div style={{ display: 'flex', flexDirection: 'column', gap: 8, marginBottom: 16, padding: '12px', background: 'var(--bg)', borderRadius: 6, border: '1px solid var(--border)' }}>
          <input
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder={zh ? '表单标题 *' : 'Form title *'}
            style={{ fontSize: 13, padding: '6px 8px' }}
          />
          <input
            value={desc}
            onChange={(e) => setDesc(e.target.value)}
            placeholder={zh ? '描述（可选）' : 'Description (optional)'}
            style={{ fontSize: 13, padding: '6px 8px' }}
          />
          <button
            className="btn btn-primary btn-sm"
            disabled={publishing || !title.trim()}
            onClick={handlePublish}
            style={{ alignSelf: 'flex-end' }}
          >
            {publishing ? (zh ? '发布中…' : 'Publishing…') : (zh ? '发布表单' : 'Publish Form')}
          </button>
        </div>

        {forms.length === 0 ? (
          <div style={{ color: 'var(--muted)', fontSize: 13, textAlign: 'center', padding: '12px 0' }}>
            {zh ? '暂无已发布表单。' : 'No published forms yet.'}
          </div>
        ) : (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
            {forms.map((f) => (
              <div key={f.token} style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '8px 10px', background: 'var(--bg)', borderRadius: 6, border: '1px solid var(--border)' }}>
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ fontSize: 13, fontWeight: 500 }}>{f.title}</div>
                  <div style={{ fontSize: 11, color: 'var(--muted)', fontFamily: 'monospace', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                    {formUrl(f.token)}
                  </div>
                </div>
                <button
                  className="btn btn-sm"
                  onClick={() => copyLink(f.token)}
                  title={zh ? '复制表单链接' : 'Copy form URL'}
                >
                  {copied === f.token ? (zh ? '✓ 已复制' : '✓ Copied') : (zh ? '复制' : 'Copy')}
                </button>
                <a
                  href={formUrl(f.token)}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="btn btn-sm"
                  title="Open form"
                >
                  ↗
                </a>
                <button
                  className="btn btn-sm btn-danger"
                  onClick={() => handleDelete(f.token)}
                  title={zh ? '删除表单' : 'Delete form'}
                  style={{ fontSize: 11 }}
                >
                  ✕
                </button>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  )
}

interface CopilotPanelProps {
  onClose: () => void
  graphJson: string
  tenantId: string
  zh: boolean
}

interface CopilotMessage {
  role: 'user' | 'assistant'
  content: string
}

function CopilotPanel({ onClose, graphJson, tenantId, zh }: CopilotPanelProps) {
  const [messages, setCopMessages] = useState<CopilotMessage[]>([])
  const [copInput, setCopInput] = useState('')
  const [copLoading, setCopLoading] = useState(false)
  const [copApiKey, setCopApiKey] = useState(() => localStorage.getItem('af:claude_key') ?? '')
  const [showKeyInput, setShowKeyInput] = useState(false)
  const bottomRef = useRef<HTMLDivElement>(null)

  const QUICK_ACTIONS = zh
    ? ['解释这个工作流', '找出潜在问题', '如何添加错误处理？', '建议性能优化']
    : ['Explain this workflow', 'Find potential issues', 'How to add error handling?', 'Suggest improvements']

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [messages])

  const sendMsg = async (msg: string) => {
    if (!msg.trim()) return
    const key = copApiKey.trim() || undefined
    setCopMessages((prev) => [...prev, { role: 'user', content: msg }])
    setCopInput('')
    setCopLoading(true)
    try {
      const res = await api.copilotQuery(msg, { tenantId, graphJson: graphJson || undefined, apiKey: key })
      setCopMessages((prev) => [...prev, { role: 'assistant', content: res.reply }])
    } catch (e: unknown) {
      setCopMessages((prev) => [...prev, { role: 'assistant', content: `⚠ ${String(e)}` }])
    } finally {
      setCopLoading(false)
    }
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); sendMsg(copInput) }
  }

  return (
    <div style={{
      position: 'absolute', top: 0, right: 0, bottom: 0, width: 340,
      background: 'var(--surface)', borderLeft: '1px solid var(--border)',
      display: 'flex', flexDirection: 'column', zIndex: 20,
      boxShadow: '-4px 0 16px rgba(0,0,0,0.1)',
    }}>
      <div style={{ padding: '10px 12px', borderBottom: '1px solid var(--border)', display: 'flex', alignItems: 'center', gap: 8 }}>
        <span style={{ fontWeight: 700, fontSize: 14, color: 'var(--node-claude)' }}>✦ {zh ? 'AI 助手' : 'Copilot'}</span>
        <span style={{ fontSize: 11, color: 'var(--muted)', flex: 1 }}>{zh ? '询问关于此工作流的任何问题' : 'Ask anything about this workflow'}</span>
        <button onClick={() => setShowKeyInput((v) => !v)} title={zh ? 'API Key' : 'API Key'} style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--muted)', fontSize: 14 }}>🔑</button>
        <button onClick={onClose} style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--muted)', fontSize: 18, lineHeight: 1 }}>×</button>
      </div>

      {showKeyInput && (
        <div style={{ padding: '8px 12px', borderBottom: '1px solid var(--border)', background: 'var(--code-bg, rgba(0,0,0,0.04))' }}>
          <div style={{ fontSize: 11, color: 'var(--muted)', marginBottom: 4 }}>{zh ? 'Anthropic API Key（本地存储）：' : 'Anthropic API Key (stored locally):'}</div>
          <input type="password" placeholder="sk-ant-..." value={copApiKey}
            onChange={(e) => { setCopApiKey(e.target.value); localStorage.setItem('af:claude_key', e.target.value) }}
            style={{ width: '100%', fontSize: 12, padding: '4px 6px', boxSizing: 'border-box' }} />
        </div>
      )}

      {messages.length === 0 && (
        <div style={{ padding: '12px', display: 'flex', flexWrap: 'wrap', gap: 6 }}>
          {QUICK_ACTIONS.map((action) => (
            <button key={action} onClick={() => sendMsg(action)} style={{
              background: 'var(--bg)', border: '1px solid var(--border)', borderRadius: 12,
              padding: '4px 10px', fontSize: 11, cursor: 'pointer', color: 'var(--text)',
            }}
            onMouseEnter={(e) => { e.currentTarget.style.borderColor = 'var(--accent)'; e.currentTarget.style.color = 'var(--accent)' }}
            onMouseLeave={(e) => { e.currentTarget.style.borderColor = 'var(--border)'; e.currentTarget.style.color = 'var(--text)' }}>
              {action}
            </button>
          ))}
        </div>
      )}

      <div style={{ flex: 1, overflowY: 'auto', padding: '8px 12px', display: 'flex', flexDirection: 'column', gap: 10 }}>
        {messages.map((m, i) => (
          <div key={i} style={{ display: 'flex', flexDirection: 'column', alignItems: m.role === 'user' ? 'flex-end' : 'flex-start' }}>
            <div style={{
              maxWidth: '90%', padding: '8px 12px', fontSize: 13, lineHeight: 1.5,
              whiteSpace: 'pre-wrap', wordBreak: 'break-word',
              borderRadius: m.role === 'user' ? '12px 12px 2px 12px' : '12px 12px 12px 2px',
              background: m.role === 'user' ? 'var(--accent)' : 'var(--bg)',
              color: m.role === 'user' ? '#fff' : 'var(--text)',
              border: m.role === 'user' ? 'none' : '1px solid var(--border)',
            }}>{m.content}</div>
          </div>
        ))}
        {copLoading && (
          <div style={{ display: 'flex', alignItems: 'flex-start' }}>
            <div style={{ padding: '8px 12px', borderRadius: '12px 12px 12px 2px', background: 'var(--bg)', border: '1px solid var(--border)', fontSize: 13, color: 'var(--muted)' }}>
              {zh ? '思考中…' : 'Thinking…'}
            </div>
          </div>
        )}
        <div ref={bottomRef} />
      </div>

      <div style={{ padding: '8px 12px', borderTop: '1px solid var(--border)', display: 'flex', gap: 6 }}>
        <textarea value={copInput} onChange={(e) => setCopInput(e.target.value)} onKeyDown={handleKeyDown}
          placeholder={zh ? '输入消息… (Enter 发送)' : 'Ask a question… (Enter to send)'}
          rows={2} style={{
            flex: 1, resize: 'none', fontSize: 13, padding: '6px 8px', borderRadius: 6,
            border: '1px solid var(--border)', background: 'var(--bg)', color: 'var(--text)', fontFamily: 'inherit',
          }} />
        <button onClick={() => sendMsg(copInput)} disabled={copLoading || !copInput.trim()} style={{
          background: 'var(--node-claude)', color: '#fff', border: 'none', borderRadius: 6,
          padding: '0 12px', cursor: 'pointer', fontWeight: 600, fontSize: 16,
          opacity: copLoading || !copInput.trim() ? 0.5 : 1,
        }}>↑</button>
      </div>
    </div>
  )
}
