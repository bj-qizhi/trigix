// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

import { useEffect, useRef, useState } from 'react'
import type { FlowNode } from './Canvas'
import type { NodeType, ExecutionSummary, NodeExecutionRecord } from '../types'
import type { TranslationKey } from '../i18n'
import type { ConfigProps } from './panels/types'
import { useLocale } from '../useLocale'

import {
  TriggerConfig, HttpConfig, AgentConfig, ApprovalConfig, CodeConfig, SubWorkflowConfig, CustomConfig,
} from './panels/CorePanels'
import {
  ConditionConfig, FanOutConfig, FanInConfig, CatchConfig, AssertConfig, DelayConfig,
  ForEachConfig, SwitchConfig, LoopConfig, MapConfig, SortConfig, AggregateConfig,
  FilterConfig, SplitConfig, JoinConfig,
} from './panels/ControlPanels'
import {
  OpenAIConfig, GeminiConfig, ClaudeConfig, RagConfig, RagIngestConfig, DatabaseConfig, GraphQLConfig,
  MistralConfig, PerplexityConfig, CohereConfig, ReplicateConfig, GroqConfig,
  OpenrouterConfig, TogetheraiConfig, HuggingfaceConfig,
} from './panels/AiPanels'
import {
  TransformConfig, ExtractConfig, MergeConfig, DedupeConfig, RegexConfig, CsvConfig,
  RenameConfig, FormatConfig, XmlConfig, YamlConfig, HandlebarsConfig, MathConfig,
  ArrayUtilsConfig, ValidateConfig, RandomConfig, CryptoConfig, DateConfig, NoteConfig,
} from './panels/TransformPanels'
import {
  SlackConfig, EmailConfig, GithubConfig, WebhookSendConfig, JiraConfig, NotionConfig,
  LinearConfig, AirtableConfig, DiscordConfig, TeamsConfig, SheetsConfig, HubspotConfig,
  ZendeskConfig, TwilioConfig, StripeConfig, ShopifyConfig, DatadogConfig, SalesforceConfig,
  FreshdeskConfig, MailgunConfig, AsanaConfig, ServiceNowConfig, ConfluenceConfig,
  BitbucketConfig, AzureDevOpsConfig, RedisConfig, ElasticsearchConfig, PagerdutyConfig,
} from './panels/IntegrationPanels'
import {
  TwitchConfig, FigmaConfig, DropboxConfig, CloudflareConfig, BoxConfig, OktaConfig,
  ZoomConfig, SpotifyConfig, TypeformConfig, WebflowConfig, IntercomConfig, PipedriveConfig,
  TrelloConfig, MondayConfig, ClickupConfig, AmplitudeConfig, MixpanelConfig, SegmentConfig,
  SendgridConfig, BraintreeConfig, PaypalConfig, RazorpayConfig, FirebaseConfig, SupabaseConfig,
  MailchimpConfig, ActivecampaignConfig, KlaviyoConfig, ResendConfig, ContentfulConfig,
  AlgoliaConfig, PostmarkConfig, VonageConfig, TelegramConfig, WhatsappConfig, GoogledocsConfig,
  GoogledriveConfig, WoocommerceConfig, PineconeConfig, Awss3Config, QdrantConfig,
  CloudinaryConfig, GcalConfig, DocusignConfig, XeroConfig, CalendlyConfig, ApifyConfig,
  GanalyticsConfig, NeonConfig, CopperConfig,
  AzureOpenaiConfig, GrokConfig, OllamaConfig, WeaviateConfig, ChromaConfig, MongodbConfig, ClickhouseConfig, GcsConfig, AzureBlobConfig, HashConfig, JwtConfig, VertexConfig, SqsConfig, SnsConfig, BedrockConfig, MilvusConfig, KafkaConfig, RabbitmqConfig, ZipConfig, ImageConfig, PdfExtractConfig, OcrConfig, FeishuConfig, DingtalkConfig, WecomConfig,
  EmbeddingConfig, RerankerConfig, TextSplitterConfig, StructuredOutputConfig, ClassifierConfig, ImageGenConfig, SpeechToTextConfig, TtsConfig, HtmlExtractConfig, RssConfig,
  DeepseekConfig, QwenConfig, ZhipuConfig, MoonshotConfig,
  DoubaoConfig, MinimaxConfig, ErnieConfig, HunyuanConfig,
} from './panels/IntegrationPanels2'

const NODE_DESCRIPTIONS: Partial<Record<NodeType, { en: string; zh: string }>> = {
  trigger:      { en: 'Starts the workflow. Supports manual, schedule (interval/cron), and webhook triggers.', zh: '工作流入口节点。支持手动触发、定时调度（间隔/Cron表达式）和 Webhook 触发。' },
  http:         { en: 'Makes an HTTP request to any URL. Supports GET/POST/PUT/DELETE with custom headers and body.', zh: '发起 HTTP 请求。支持 GET/POST/PUT/DELETE，可设置自定义请求头和请求体。' },
  agent:        { en: 'Runs an AI agent loop with tool use. Connects to the Python AI runtime.', zh: 'AI 智能体节点，支持工具调用循环，连接 Python AI 运行时。' },
  condition:    { en: 'Routes flow based on a condition expression. True/false branches.', zh: '条件判断节点，根据表达式结果选择 true 或 false 分支。' },
  approval:     { en: 'Pauses execution and waits for a human to approve or reject before continuing.', zh: '暂停执行，等待人工审批（批准/拒绝）后继续。' },
  map:          { en: 'Applies a template to each item in an array, producing a transformed array.', zh: '对数组中每个元素应用模板，输出转换后的数组。' },
  filter:       { en: 'Filters an array by a field condition (equals, contains, gt, lt, etc.).', zh: '按字段条件（等于、包含、大于、小于等）过滤数组。' },
  aggregate:    { en: 'Reduces an array to a single value: count, sum, avg, min, max, join, first, last.', zh: '将数组聚合为单个值：计数、求和、平均值、最大/最小值、拼接、首/末元素。' },
  sort:         { en: 'Sorts an array by a field in ascending or descending order.', zh: '按指定字段对数组进行升序或降序排列。' },
  transform:    { en: 'Applies a JSON template with {{variable}} interpolation to reshape data.', zh: '使用 {{变量}} 插值的 JSON 模板对数据进行重塑转换。' },
  delay:        { en: 'Pauses execution for a specified number of seconds (max 3600).', zh: '暂停执行指定秒数（最多 3600 秒）。' },
  sub_workflow: { en: 'Runs another published workflow as a nested step.', zh: '将另一个已发布的工作流作为子步骤嵌套执行。' },
  assert:       { en: 'Asserts a condition is truthy. Fails the workflow with a custom message if not.', zh: '断言条件为真，否则以自定义错误信息终止工作流。' },
  catch:        { en: 'Catches errors from upstream nodes and routes them to a recovery branch.', zh: '捕获上游节点的错误，将流程路由到错误恢复分支。' },
  fan_out:      { en: 'Splits execution into parallel branches. Pair with Fan-In to merge results.', zh: '将执行分叉为并行分支，配合 Fan-In 节点汇总结果。' },
  fan_in:       { en: 'Merges results from parallel branches started by Fan-Out.', zh: '汇总 Fan-Out 启动的并行分支结果。' },
  code:         { en: 'Executes a Rhai script in a sandboxed environment. Access {{input}} and {{node_id}} variables.', zh: '在沙箱中执行 Rhai 脚本。可访问 {{input}} 和上游节点输出变量。' },
  slack:        { en: 'Sends a message to a Slack channel via Incoming Webhook or Bot token.', zh: '通过 Incoming Webhook 或 Bot Token 向 Slack 频道发送消息。' },
  email:        { en: 'Sends an email via SMTP. Configure host, port, credentials, and recipients.', zh: '通过 SMTP 发送邮件，可配置服务器、端口、凭证和收件人。' },
  openai:       { en: 'Calls OpenAI Chat Completions API (GPT-4o, GPT-4o-mini, o1). Returns content and token usage.', zh: '调用 OpenAI 对话 API（GPT-4o、GPT-4o-mini、o1），返回内容和 Token 用量。' },
  gemini:       { en: 'Calls Google Gemini API (gemini-2.0-flash, 1.5-pro, thinking). Returns content and usage.', zh: '调用 Google Gemini API，返回内容和用量。' },
  claude:       { en: 'Calls Anthropic Claude API (claude-opus-4, sonnet-4, haiku-4). Returns content and usage.', zh: '调用 Anthropic Claude API，返回内容和 Token 用量。' },
  database:     { en: 'Executes a SQL query against a PostgreSQL database. SELECT returns rows; DML returns rows_affected.', zh: '对 PostgreSQL 数据库执行 SQL 查询。SELECT 返回行数据，DML 返回影响行数。' },
  extract:      { en: 'Extracts a value from a JSON object using a dot-path (e.g. user.address.city).', zh: '使用点路径（如 user.address.city）从 JSON 对象中提取字段值。' },
  merge:        { en: 'Merges multiple source fields into a single output object.', zh: '将多个来源字段合并为一个输出对象。' },
  loop:         { en: 'Iterates over an array, applying an optional template per item, until a condition or max count.', zh: '遍历数组，对每个元素应用可选模板，支持条件终止或最大迭代次数。' },
  graphql:      { en: 'Executes a GraphQL query or mutation against any endpoint.', zh: '向任意 GraphQL 端点执行查询或变更操作。' },
  validate:     { en: 'Validates a JSON value against a schema (required fields, types). Fails if invalid.', zh: '根据 Schema（必填字段、类型）验证 JSON 数据，不符合则报错。' },
  note:         { en: 'A visual annotation node. Does not affect execution.', zh: '注释节点，仅用于画布标注，不影响工作流执行。' },
  split:        { en: 'Splits a string into an array by a delimiter.', zh: '按分隔符将字符串拆分为数组。' },
  join:         { en: 'Joins an array into a string with a delimiter.', zh: '用分隔符将数组元素拼接为字符串。' },
  switch:       { en: 'Routes flow to a matching case branch based on a value. Supports wildcard (*).', zh: '根据值匹配对应的 case 分支，支持通配符（*）。' },
  random:       { en: 'Generates a random UUID, number, integer, boolean, or picks from an array.', zh: '生成随机 UUID、数字、整数、布尔值，或从数组中随机取值。' },
  dedupe:       { en: 'Removes duplicate items from an array by field or whole-item comparison.', zh: '按字段或整体对比去除数组中的重复元素。' },
  regex:        { en: 'Tests a string against a regex pattern. Returns matched/groups.', zh: '用正则表达式测试字符串，返回匹配结果和捕获组。' },
  csv:          { en: 'Parses a CSV string into an array of row objects or arrays.', zh: '将 CSV 字符串解析为行对象数组或二维数组。' },
  rename:       { en: 'Renames fields in an object using from/to mappings.', zh: '使用 from/to 映射对对象字段进行重命名。' },
  format:       { en: 'Applies a string operation (uppercase, trim, base64, URL encode, etc.) to a value.', zh: '对字符串进行处理操作（大写、去空格、Base64、URL 编码等）。' },
  github:       { en: 'Calls the GitHub REST API with a personal access token.', zh: '使用个人访问令牌调用 GitHub REST API。' },
  webhook:      { en: 'Sends an HTTP POST to an arbitrary URL (outbound webhook).', zh: '向任意 URL 发送 HTTP POST 请求（出站 Webhook）。' },
  for_each:     { en: 'Runs a sub-workflow for each item in an array in parallel batches.', zh: '对数组中每个元素并行执行子工作流（批量并发）。' },
  math:         { en: 'Performs math operations: add, round, sqrt, pow, mod, clamp, avg, eval (Rhai).', zh: '执行数学运算：加减、取整、开方、幂、取模、钳制、平均值、Rhai 表达式。' },
  date:         { en: 'Date/time operations: now, parse, format, add, subtract, diff, unix↔ISO.', zh: '日期时间操作：获取当前时间、解析、格式化、加减、差值、Unix↔ISO 转换。' },
  crypto:       { en: 'Cryptographic operations: SHA256/512, HMAC, Base64, hex encode/decode, random bytes.', zh: '加密操作：SHA256/512、HMAC、Base64、十六进制编解码、随机字节。' },
  handlebars:   { en: 'Renders a Handlebars template with data from upstream nodes.', zh: '使用上游节点数据渲染 Handlebars 模板。' },
  xml:          { en: 'Parses an XML string into a JSON object.', zh: '将 XML 字符串解析为 JSON 对象。' },
  yaml:         { en: 'Parses YAML to JSON or serializes JSON to YAML.', zh: '在 YAML 和 JSON 之间相互转换。' },
  array_utils:  { en: 'Array utilities: chunk, flatten, compact, zip, reverse, shuffle, sample, range, pluck, first_n, last_n.', zh: '数组工具：分块、展平、去空值、合并、反转、随机打乱、采样、范围生成、字段提取、首/末 N 个。' },
  azure_openai: { en: 'Calls Azure OpenAI (deployment-based, api-key header). Returns content and usage.', zh: '调用 Azure OpenAI（按 deployment、api-key 头），返回内容和 Token 用量。' },
  grok:      { en: 'Calls xAI Grok (OpenAI-compatible). Returns content and usage.', zh: '调用 xAI Grok（OpenAI 兼容），返回内容和 Token 用量。' },
  ollama:    { en: 'Calls a self-hosted Ollama server (OpenAI-compatible) at a configurable base URL.', zh: '调用自托管 Ollama 服务（OpenAI 兼容，可配置 base URL）。' },
  weaviate:  { en: 'Weaviate vector store: GraphQL search or object create/get/delete over REST.', zh: 'Weaviate 向量库：GraphQL 检索或对象增删查（REST）。' },
  chroma:    { en: 'Chroma vector store: query/add/delete embeddings or resolve a collection over REST.', zh: 'Chroma 向量库：查询/添加/删除向量或解析 collection（REST）。' },
  mongodb:   { en: 'MongoDB via the Atlas Data API: find/insert/update/delete/aggregate over HTTP.', zh: 'MongoDB（Atlas Data API）：HTTP 增删改查与聚合。' },
  clickhouse:{ en: 'Runs SQL against ClickHouse over its HTTP interface; FORMAT appended to SELECTs.', zh: '通过 HTTP 接口对 ClickHouse 执行 SQL；SELECT 自动追加 FORMAT。' },
  gcs:       { en: 'Google Cloud Storage (JSON API): list/get/download/upload/delete objects with an OAuth2 token.', zh: 'Google Cloud Storage（JSON API）：用 OAuth2 令牌增删查对象与上传下载。' },
  azure_blob:{ en: 'Azure Blob Storage (REST + SAS): list/get/put/delete blobs.', zh: 'Azure Blob 存储（REST + SAS）：增删查 Blob。' },
  hash:      { en: 'Computes a SHA-256/384/512 or HMAC digest, output as hex/base64/base64url.', zh: '计算 SHA-256/384/512 或 HMAC 摘要，输出 hex/base64/base64url。' },
  jwt:       { en: 'Signs or verifies an HMAC JWT (HS256/384/512), with exp handling.', zh: '签发或校验 HMAC JWT（HS256/384/512），支持过期校验。' },
  vertex:    { en: 'Google Vertex AI (Gemini generateContent) via a caller-supplied OAuth2 token.', zh: 'Google Vertex AI（Gemini generateContent），用调用方提供的 OAuth2 令牌。' },
  sqs:       { en: 'AWS SQS (SigV4-signed): send/receive/delete messages.', zh: 'AWS SQS（SigV4 签名）：发送/接收/删除消息。' },
  sns:       { en: 'AWS SNS Publish (SigV4-signed) to a topic, target ARN, or phone number.', zh: 'AWS SNS Publish（SigV4 签名）：发到 topic / target ARN / 手机号。' },
  bedrock:   { en: 'AWS Bedrock InvokeModel (SigV4-signed) with a model-native request body.', zh: 'AWS Bedrock InvokeModel（SigV4 签名），请求体按模型原生 schema。' },
  milvus:    { en: 'Milvus / Zilliz vector store (REST API v2): search/insert/query/delete.', zh: 'Milvus / Zilliz 向量库（REST API v2）：检索/插入/查询/删除。' },
  kafka:     { en: 'Produce to a Kafka topic via the Confluent REST Proxy.', zh: '通过 Confluent REST Proxy 向 Kafka topic 生产消息。' },
  rabbitmq:  { en: 'RabbitMQ Management HTTP API: publish / get messages / list queues.', zh: 'RabbitMQ 管理 HTTP API：发布/拉取消息/列出队列。' },
  zip:       { en: 'Create or extract a zip archive (base64 payloads).', zh: '创建或解压 zip 包（base64 载荷）。' },
  image:     { en: 'Resize / convert / inspect an image (base64 in/out).', zh: '图片缩放/格式转换/取元数据（base64 进出）。' },
  pdf_extract:{ en: 'Extract text from a base64 PDF.', zh: '从 base64 PDF 抽取文本。' },
  ocr:       { en: 'OCR an image via the tesseract CLI (must be installed on the executor host).', zh: '用 tesseract CLI 对图片做 OCR（执行机需装 tesseract）。' },
  feishu:    { en: 'Send a Feishu/Lark message via a custom-bot webhook or the app message API.', zh: '飞书/Lark 发消息：自定义机器人 webhook 或 App 消息 API。' },
  dingtalk:  { en: 'Send a DingTalk custom-robot message (optional HMAC sign).', zh: '钉钉自定义机器人发消息（可选加签）。' },
  wecom:     { en: 'Send a WeChat Work group-robot message (text/markdown).', zh: '企业微信群机器人发消息（text/markdown）。' },
  embedding: { en: 'Embed text via an OpenAI-compatible embeddings endpoint.', zh: '调用 OpenAI 兼容嵌入接口做文本向量化。' },
  reranker:  { en: 'Rerank documents against a query (Cohere/Jina-style).', zh: '按 query 对文档重排（Cohere/Jina 风格）。' },
  text_splitter: { en: 'Split text into overlapping chunks (UTF-8 safe).', zh: '把文本切成带重叠的块（UTF-8 安全）。' },
  structured_output: { en: 'Get a JSON object from an LLM (json_object mode).', zh: '让 LLM 输出 JSON 对象（json_object 模式）。' },
  classifier:{ en: 'Classify input into one of N categories via an LLM.', zh: '用 LLM 把输入分到 N 个类别之一。' },
  image_gen: { en: 'Generate images via an OpenAI-compatible images endpoint.', zh: '调用 OpenAI 兼容图像接口生成图片。' },
  speech_to_text: { en: 'Transcribe audio (Whisper-compatible).', zh: '语音转文字（Whisper 兼容）。' },
  tts:       { en: 'Synthesize speech from text; returns base64 audio.', zh: '文字转语音，返回 base64 音频。' },
  html_extract: { en: 'Extract content from HTML by CSS selector (text/html/attr).', zh: '按 CSS 选择器从 HTML 抽取内容（text/html/attr）。' },
  rss:       { en: 'Read an RSS/Atom/JSON feed and return its entries.', zh: '读取 RSS/Atom/JSON 订阅源并返回条目。' },
  deepseek:  { en: 'Calls DeepSeek API (deepseek-chat V3, deepseek-reasoner R1). Returns content and usage.', zh: '调用 DeepSeek API（deepseek-chat V3、deepseek-reasoner R1），返回内容和 Token 用量。' },
  qwen:      { en: 'Calls Alibaba Qwen via DashScope (qwen-max, qwen-plus, qwen-turbo, qwen-long). Returns content and usage.', zh: '通过 DashScope 调用通义千问，返回内容和 Token 用量。' },
  zhipu:     { en: 'Calls Zhipu AI GLM (glm-4, glm-4-air, glm-4-flash, glm-3-turbo). Returns content and usage.', zh: '调用智谱 AI GLM 系列模型，返回内容和 Token 用量。' },
  moonshot:  { en: 'Calls Moonshot AI (Kimi) with 8K/32K/128K context windows. Returns content and usage.', zh: '调用月之暗面（Kimi），支持 8K/32K/128K 上下文，返回内容和 Token 用量。' },
  doubao:    { en: 'Calls Bytedance Doubao via Volces Ark. Uses endpoint_id instead of model name. Returns content and usage.', zh: '通过火山引擎方舟调用字节豆包，使用推理接入点 ID，返回内容和 Token 用量。' },
  minimax:   { en: 'Calls MiniMax chatcompletion API. Requires group_id in addition to api_key. Returns content and usage.', zh: '调用 MiniMax 对话 API，需要 group_id，返回内容和 Token 用量。' },
  ernie:     { en: 'Calls Baidu ERNIE via OAuth2 token exchange (ernie-4.0, 3.5, speed). Returns content and usage.', zh: '通过 OAuth2 换取令牌后调用百度文心一言，返回内容和 Token 用量。' },
  hunyuan:   { en: 'Calls Tencent Hunyuan (hunyuan-standard, turbo, lite) via OpenAI-compatible API. Returns content and usage.', zh: '通过 OpenAI 兼容接口调用腾讯混元，返回内容和 Token 用量。' },
}

const NODE_LABELS: Partial<Record<NodeType, string>> = {
  trigger: 'Trigger',
  http: 'HTTP',
  agent: 'Agent',
  condition: 'Condition',
  approval: 'Approval',
  map: 'Map',
  filter: 'Filter',
  aggregate: 'Aggregate',
  sort: 'Sort',
  transform: 'Transform',
  delay: 'Delay',
  sub_workflow: 'Sub-Workflow',
  assert: 'Assert',
  catch: 'Catch',
  fan_out: 'Fan-Out',
  fan_in: 'Fan-In',
  code: 'Code',
  slack: 'Slack',
  email: 'Email',
  openai: 'OpenAI',
  gemini: 'Gemini',
  database: 'Database',
  extract: 'Extract',
  merge: 'Merge',
  loop: 'Loop',
  graphql: 'GraphQL',
  validate: 'Validate',
  note: 'Note',
  azure_openai: 'Azure OpenAI',
  grok: 'xAI Grok',
  ollama: 'Ollama',
  weaviate: 'Weaviate',
  chroma: 'Chroma',
  mongodb: 'MongoDB',
  clickhouse: 'ClickHouse',
  gcs: 'Google Cloud Storage',
  azure_blob: 'Azure Blob',
  hash: 'Hash / HMAC',
  jwt: 'JWT',
  vertex: 'Vertex AI',
  sqs: 'AWS SQS',
  sns: 'AWS SNS',
  bedrock: 'AWS Bedrock',
  milvus: 'Milvus',
  kafka: 'Kafka',
  rabbitmq: 'RabbitMQ',
  zip: 'Zip',
  image: 'Image',
  pdf_extract: 'PDF Extract',
  ocr: 'OCR',
  feishu: '飞书 / Lark',
  dingtalk: '钉钉',
  wecom: '企业微信',
  embedding: 'Embedding',
  reranker: 'Reranker',
  text_splitter: 'Text Splitter',
  structured_output: 'Structured Output',
  classifier: 'Classifier',
  image_gen: 'Image Gen',
  speech_to_text: 'Speech → Text',
  tts: 'Text → Speech',
  html_extract: 'HTML Extract',
  rss: 'RSS Feed',
  deepseek: 'DeepSeek',
  qwen: '通义千问',
  zhipu: '智谱 GLM',
  moonshot: 'Moonshot',
  doubao: '豆包',
  minimax: 'MiniMax',
  ernie: '文心一言',
  hunyuan: '混元',
}

const NODE_COLORS: Partial<Record<NodeType, string>> = {
  trigger: 'var(--node-trigger)',
  http: 'var(--node-http)',
  agent: 'var(--node-agent)',
  condition: 'var(--node-condition)',
  approval: 'var(--node-approval)',
  map: 'var(--node-map)',
  filter: 'var(--node-filter)',
  aggregate: 'var(--node-aggregate)',
  sort: 'var(--node-sort)',
  transform: 'var(--node-transform)',
  delay: 'var(--node-delay)',
  sub_workflow: 'var(--node-sub-workflow)',
  assert: 'var(--node-assert)',
  catch: 'var(--node-catch)',
  fan_out: 'var(--node-fan)',
  fan_in: 'var(--node-fan)',
  code: 'var(--node-code)',
  slack: 'var(--node-slack)',
  email: 'var(--node-email)',
  openai: 'var(--node-openai)',
  gemini: 'var(--node-gemini)',
  database: 'var(--node-database)',
  extract: 'var(--node-extract)',
  merge: 'var(--node-merge)',
  loop: 'var(--node-loop)',
  graphql: 'var(--node-graphql)',
  validate: 'var(--node-validate)',
  note: 'var(--node-note)',
  claude: 'var(--node-claude)',
  azure_openai: 'var(--node-openai)',
  grok: 'var(--node-claude)',
  ollama: 'var(--node-openai)',
  weaviate: 'var(--node-qdrant)',
  chroma: 'var(--node-qdrant)',
  mongodb: 'var(--node-database)',
  clickhouse: 'var(--node-database)',
  gcs: 'var(--node-awss3)',
  azure_blob: 'var(--node-awss3)',
  hash: 'var(--node-crypto)',
  jwt: 'var(--node-crypto)',
  vertex: 'var(--node-gemini)',
  sqs: 'var(--node-awss3)',
  sns: 'var(--node-awss3)',
  bedrock: 'var(--node-awss3)',
  milvus: 'var(--node-qdrant)',
  kafka: 'var(--node-redis)',
  rabbitmq: 'var(--node-redis)',
  zip: 'var(--node-transform)',
  image: 'var(--node-transform)',
  pdf_extract: 'var(--node-transform)',
  ocr: 'var(--node-transform)',
  feishu: 'var(--node-slack)',
  dingtalk: 'var(--node-slack)',
  wecom: 'var(--node-slack)',
  embedding: 'var(--node-openai)',
  reranker: 'var(--node-cohere)',
  text_splitter: 'var(--node-transform)',
  structured_output: 'var(--node-openai)',
  classifier: 'var(--node-openai)',
  image_gen: 'var(--node-openai)',
  speech_to_text: 'var(--node-openai)',
  tts: 'var(--node-openai)',
  html_extract: 'var(--node-transform)',
  rss: 'var(--node-transform)',
  deepseek: 'var(--node-deepseek)',
  qwen: 'var(--node-qwen)',
  zhipu: 'var(--node-zhipu)',
  moonshot: 'var(--node-moonshot)',
  doubao: 'var(--node-doubao)',
  minimax: 'var(--node-minimax)',
  ernie: 'var(--node-ernie)',
  hunyuan: 'var(--node-hunyuan)',
}

const NODE_OUTPUTS: Partial<Record<NodeType, string[]>> = {
  trigger:      ['input'],
  http:         ['status', 'body', 'headers'],
  agent:        ['result'],
  approval:     ['approved', 'comment'],
  map:          ['count', 'items'],
  filter:       ['count', 'items'],
  aggregate:    ['result'],
  sort:         ['items'],
  transform:    ['result'],
  delay:        ['waited_secs'],
  sub_workflow: ['status', 'output'],
  catch:        ['error'],
  fan_in:       ['count', 'results'],
  code:         ['result'],
  slack:        ['sent'],
  email:        ['sent'],
  openai:       ['content', 'model', 'usage'],
  gemini:       ['content', 'model', 'usage'],
  claude:       ['content', 'model', 'usage'],
  azure_openai: ['content', 'model', 'usage'],
  grok:         ['content', 'model', 'usage'],
  ollama:       ['content', 'model', 'usage'],
  weaviate:     ['status', 'body'],
  chroma:       ['status', 'body'],
  mongodb:      ['status', 'body'],
  clickhouse:   ['status', 'body'],
  gcs:          ['status', 'body'],
  azure_blob:   ['status', 'body'],
  hash:         ['hash', 'algorithm', 'encoding'],
  jwt:          ['token', 'valid', 'payload'],
  vertex:       ['content', 'model', 'usage'],
  sqs:          ['status', 'body'],
  sns:          ['status', 'body'],
  bedrock:      ['status', 'body'],
  milvus:       ['status', 'body'],
  kafka:        ['status', 'body'],
  rabbitmq:     ['status', 'body'],
  zip:          ['zip_base64', 'files', 'file_count'],
  image:        ['image_base64', 'width', 'height', 'format'],
  pdf_extract:  ['text', 'char_count'],
  ocr:          ['text', 'lang'],
  feishu:       ['status', 'body'],
  dingtalk:     ['status', 'body'],
  wecom:        ['status', 'body'],
  embedding:    ['embeddings', 'model', 'usage'],
  reranker:     ['status', 'body'],
  text_splitter:['chunks', 'count'],
  structured_output: ['data', 'raw', 'model'],
  classifier:   ['category', 'raw'],
  image_gen:    ['status', 'body'],
  speech_to_text: ['status', 'text'],
  tts:          ['audio_base64', 'format'],
  html_extract: ['matches', 'count', 'first'],
  rss:          ['feed_title', 'items', 'count'],
  deepseek:     ['content', 'model', 'usage'],
  qwen:         ['content', 'model', 'usage'],
  zhipu:        ['content', 'model', 'usage'],
  moonshot:     ['content', 'model', 'usage'],
  doubao:       ['content', 'model', 'usage'],
  minimax:      ['content', 'model', 'usage'],
  ernie:        ['content', 'model', 'usage'],
  hunyuan:      ['content', 'model', 'usage'],
  database:     ['rows', 'count', 'rows_affected'],
  extract:      ['value', 'found'],
  loop:         ['count', 'items'],
  graphql:      ['data'],
  validate:     ['valid', 'errors'],
}

function useNodePresets(nodeType: string) {
  const key = `af:node-presets:${nodeType}`
  const load = (): Array<{ name: string; config: Record<string, unknown> }> => {
    try { return JSON.parse(localStorage.getItem(key) ?? '[]') } catch { return [] }
  }
  const save = (presets: Array<{ name: string; config: Record<string, unknown> }>) =>
    localStorage.setItem(key, JSON.stringify(presets))
  const addPreset = (name: string, config: Record<string, unknown>) => {
    const existing = load().filter((p) => p.name !== name)
    save([...existing, { name, config }].slice(-20))
  }
  const deletePreset = (name: string) => save(load().filter((p) => p.name !== name))
  return { load, addPreset, deletePreset }
}

interface Props {
  node: FlowNode | null
  onUpdateConfig: (nodeId: string, config: Record<string, unknown>) => void
  recentExecutions?: ExecutionSummary[]
  onSelectExecution?: (id: string) => void
  executionResult?: NodeExecutionRecord | null
  webhookUrl?: string | null
  webhookSecret?: string | null
  onDuplicate?: () => void
  upstreamNodes?: FlowNode[]
}

function CopyIdButton({ id }: { id: string }) {
  const [copied, setCopied] = useState(false)
  const timer = useRef<ReturnType<typeof setTimeout>>()
  return (
    <button
      className="btn btn-sm btn-icon"
      style={{ fontSize: 10, padding: '1px 5px', opacity: 0.6 }}
      title={`Copy node ID: ${id}`}
      onClick={() => {
        navigator.clipboard.writeText(id).then(() => {
          setCopied(true)
          clearTimeout(timer.current)
          timer.current = setTimeout(() => setCopied(false), 1500)
        }).catch(() => {})
      }}
    >
      {copied ? '✓' : '⎘'}
    </button>
  )
}

function CopyConfigButton({ config }: { config: Record<string, unknown> }) {
  const [copied, setCopied] = useState(false)
  const timer = useRef<ReturnType<typeof setTimeout>>()
  return (
    <button
      className="btn btn-sm btn-icon"
      style={{ fontSize: 10, padding: '1px 5px', opacity: 0.6 }}
      title="Copy node config as JSON"
      onClick={() => {
        const text = JSON.stringify(config, null, 2)
        navigator.clipboard.writeText(text).then(() => {
          setCopied(true)
          clearTimeout(timer.current)
          timer.current = setTimeout(() => setCopied(false), 1500)
        }).catch(() => {})
      }}
    >
      {copied ? '✓ Config' : '⎘ Config'}
    </button>
  )
}

function PasteConfigButton({ onPaste }: { onPaste: (cfg: Record<string, unknown>) => void }) {
  const [state, setState] = useState<'idle' | 'ok' | 'err'>('idle')
  const timer = useRef<ReturnType<typeof setTimeout>>()
  return (
    <button
      className="btn btn-sm btn-icon"
      style={{ fontSize: 10, padding: '1px 5px', opacity: 0.6 }}
      title="Paste config from clipboard JSON"
      onClick={() => {
        navigator.clipboard.readText().then((text) => {
          try {
            const parsed = JSON.parse(text)
            if (typeof parsed === 'object' && parsed !== null && !Array.isArray(parsed)) {
              onPaste(parsed as Record<string, unknown>)
              setState('ok')
            } else {
              setState('err')
            }
          } catch {
            setState('err')
          }
          clearTimeout(timer.current)
          timer.current = setTimeout(() => setState('idle'), 1500)
        }).catch(() => { setState('err'); clearTimeout(timer.current); timer.current = setTimeout(() => setState('idle'), 1500) })
      }}
    >
      {state === 'ok' ? '✓ Pasted' : state === 'err' ? '✗ Invalid' : '⊕ Paste'}
    </button>
  )
}


export function NodeConfigPanel({ node, onUpdateConfig, recentExecutions, onSelectExecution, executionResult, webhookUrl, webhookSecret, onDuplicate, upstreamNodes }: Props) {
  const { locale, t } = useLocale()

  // ── Hooks must all be called before any early return (Rules of Hooks) ──
  const nt = (node?.data.nodeType ?? 'trigger') as NodeType
  const config = node?.data.config ?? {}
  const { load: loadPresets, addPreset } = useNodePresets(nt)
  const [presets, setPresets] = useState<Array<{ name: string; config: Record<string, unknown> }>>([])
  const [showPresetInput, setShowPresetInput] = useState(false)
  const [presetName, setPresetName] = useState('')
  useEffect(() => { setPresets(loadPresets()) }, [nt]) // eslint-disable-line react-hooks/exhaustive-deps

  if (!node) {
    return (
      <div className="config-panel">
        {recentExecutions && recentExecutions.length > 0 ? (
          <>
            <div className="config-panel-header" style={{ borderBottom: '1px solid var(--border)' }}>
              {t('node.recent')}
            </div>
            <div className="config-panel-body" style={{ overflowY: 'auto' }}>
              {recentExecutions.map((ex) => (
                <div
                  key={ex.id}
                  className="exec-node-row"
                  style={{ cursor: 'pointer', padding: '6px 8px' }}
                  onClick={() => onSelectExecution?.(ex.id)}
                  title="Click to view execution details"
                >
                  <span className={`dot dot-${ex.status}`} style={{ flexShrink: 0 }} />
                  <span style={{ fontSize: 11, color: 'var(--muted)', fontFamily: 'monospace', flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                    {ex.id.slice(0, 8)}…
                  </span>
                  <span className={`badge badge-${ex.status}`} style={{ fontSize: 10, padding: '1px 5px', flexShrink: 0 }}>
                    {ex.status}
                  </span>
                </div>
              ))}
            </div>
          </>
        ) : (
          <div className="config-empty">
            <span>{t('node.click')}</span>
          </div>
        )}
      </div>
    )
  }

  const set = (key: string, value: unknown) => {
    onUpdateConfig(node.id, { ...config, [key]: value })
  }

  const str = (key: string, fallback = '') => (config[key] as string) ?? fallback
  const num = (key: string, fallback: number) => (config[key] as number) ?? fallback

  return (
    <div className="config-panel">
      <div className="config-panel-header">
        <span
          style={{
            width: 10, height: 10, borderRadius: '50%', background: NODE_COLORS[nt] ?? '#8b949e',
            display: 'inline-block', flexShrink: 0,
          }}
        />
        {NODE_LABELS[nt] ?? nt}
        <span style={{ color: 'var(--muted)', fontWeight: 400, fontSize: 12, flex: 1 }}>— {node.id}</span>
        <CopyIdButton id={node.id} />
        <CopyConfigButton config={config} />
        <PasteConfigButton onPaste={(cfg) => onUpdateConfig(node.id, cfg)} />
        {onDuplicate && (
          <button
            className="btn btn-sm btn-icon"
            onClick={onDuplicate}
            title="Duplicate this node"
            style={{ fontSize: 12, padding: '2px 6px' }}
          >
            ⧉
          </button>
        )}
        {presets.length > 0 && (
          <select
            style={{ fontSize: 10, maxWidth: 90, color: 'var(--muted)', background: 'var(--panel)', border: '1px solid var(--border)', borderRadius: 3 }}
            title="Apply a saved config preset"
            defaultValue=""
            onChange={(e) => {
              const p = presets.find((x) => x.name === e.target.value)
              if (p) {
                // eslint-disable-next-line @typescript-eslint/no-unused-vars
                const { node_label: _nl, ...rest } = p.config as Record<string, unknown>
                onUpdateConfig(node.id, { ...config, ...rest })
              }
              e.target.value = ''
            }}
          >
            <option value="" disabled>Presets…</option>
            {presets.map((p) => (
              <option key={p.name} value={p.name}>{p.name}</option>
            ))}
          </select>
        )}
        {showPresetInput ? (
          <input
            autoFocus
            style={{ fontSize: 10, width: 90, padding: '1px 4px' }}
            placeholder="Preset name"
            value={presetName}
            onChange={(e) => setPresetName(e.target.value)}
            onBlur={() => setShowPresetInput(false)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && presetName.trim()) {
                addPreset(presetName.trim(), config)
                setPresets(loadPresets())
                setPresetName('')
                setShowPresetInput(false)
              } else if (e.key === 'Escape') {
                setShowPresetInput(false)
              }
            }}
          />
        ) : (
          <button
            className="btn btn-sm btn-icon"
            onClick={() => setShowPresetInput(true)}
            title="Save current config as preset"
            style={{ fontSize: 10, padding: '1px 5px', opacity: 0.6 }}
          >
            ★
          </button>
        )}
      </div>

      <div className="config-panel-body">
        {executionResult && <NodeResultBox result={executionResult} locale={locale} t={t} />}
        {NODE_DESCRIPTIONS[nt] && (
          <div style={{
            margin: '0 0 8px 0',
            padding: '6px 8px',
            background: 'var(--canvas-bg)',
            border: '1px solid var(--border)',
            borderRadius: 4,
            fontSize: 11,
            color: 'var(--muted)',
            lineHeight: 1.5,
          }}>
            {locale === 'zh' ? NODE_DESCRIPTIONS[nt]!.zh : NODE_DESCRIPTIONS[nt]!.en}
          </div>
        )}
        <div className="field" style={{ marginBottom: 6 }}>
          <label style={{ fontSize: 11 }}>{t('node.canvas.label')} <span style={{ color: 'var(--muted)' }}>{t('node.canvas.label.hint')}</span></label>
          <input
            placeholder={NODE_LABELS[nt] ?? nt}
            value={str('node_label')}
            onChange={(e) => set('node_label', e.target.value)}
            style={{ fontSize: 12 }}
          />
        </div>
        {nt === 'trigger' && <TriggerConfig config={config} set={set} str={str} num={num} webhookUrl={webhookUrl} webhookSecret={webhookSecret} />}
        {nt === 'http' && <HttpConfig config={config} set={set} str={str} num={num} />}
        {nt === 'agent' && <AgentConfig config={config} set={set} str={str} num={num} />}
        {nt === 'condition' && <ConditionConfig config={config} set={set} str={str} num={num} />}
        {nt === 'approval' && <ApprovalConfig />}
        {nt === 'map' && <MapConfig config={config} set={set} str={str} num={num} />}
        {nt === 'filter' && <FilterConfig config={config} set={set} str={str} num={num} />}
        {nt === 'aggregate' && <AggregateConfig config={config} set={set} str={str} num={num} />}
        {nt === 'sort' && <SortConfig config={config} set={set} str={str} num={num} />}
        {nt === 'transform' && <TransformConfig config={config} set={set} str={str} num={num} />}
        {nt === 'delay' && <DelayConfig config={config} set={set} str={str} num={num} />}
        {nt === 'sub_workflow' && <SubWorkflowConfig config={config} set={set} str={str} num={num} />}
        {nt === 'assert' && <AssertConfig config={config} set={set} str={str} num={num} />}
        {nt === 'catch' && <CatchConfig config={config} set={set} str={str} num={num} />}
        {nt === 'fan_out' && <FanOutConfig />}
        {nt === 'fan_in' && <FanInConfig />}
        {nt === 'code' && <CodeConfig config={config} set={set} str={str} num={num} />}
        {nt === 'slack' && <SlackConfig config={config} set={set} str={str} num={num} />}
        {nt === 'email' && <EmailConfig config={config} set={set} str={str} num={num} />}
        {nt === 'openai' && <OpenAIConfig config={config} set={set} str={str} num={num} />}
        {nt === 'rag' && <RagConfig config={config} set={set} str={str} num={num} />}
        {nt === 'rag_ingest' && <RagIngestConfig config={config} set={set} str={str} num={num} />}
        {nt === 'custom' && <CustomConfig config={config} set={set} str={str} num={num} />}
        {nt === 'gemini' && <GeminiConfig config={config} set={set} str={str} num={num} />}
        {nt === 'database' && <DatabaseConfig config={config} set={set} str={str} num={num} />}
        {nt === 'extract' && <ExtractConfig config={config} set={set} str={str} num={num} />}
        {nt === 'merge' && <MergeConfig config={config} set={set} str={str} num={num} />}
        {nt === 'loop' && <LoopConfig config={config} set={set} str={str} num={num} />}
        {nt === 'graphql' && <GraphQLConfig config={config} set={set} str={str} num={num} />}
        {nt === 'validate' && <ValidateConfig config={config} set={set} str={str} num={num} />}
        {nt === 'note' && <NoteConfig config={config} set={set} str={str} num={num} />}
        {nt === 'claude' && <ClaudeConfig config={config} set={set} str={str} num={num} />}
        {nt === 'split' && <SplitConfig config={config} set={set} str={str} num={num} />}
        {nt === 'join' && <JoinConfig config={config} set={set} str={str} num={num} />}
        {nt === 'switch' && <SwitchConfig config={config} set={set} str={str} num={num} />}
        {nt === 'random' && <RandomConfig config={config} set={set} str={str} num={num} />}
        {nt === 'dedupe' && <DedupeConfig config={config} set={set} str={str} num={num} />}
        {nt === 'regex'  && <RegexConfig  config={config} set={set} str={str} num={num} />}
        {nt === 'csv'    && <CsvConfig    config={config} set={set} str={str} num={num} />}
        {nt === 'rename'  && <RenameConfig  config={config} set={set} str={str} num={num} />}
        {nt === 'format'  && <FormatConfig  config={config} set={set} str={str} num={num} />}
        {nt === 'github'  && <GithubConfig  config={config} set={set} str={str} num={num} />}
        {nt === 'webhook' && <WebhookSendConfig config={config} set={set} str={str} num={num} />}
        {nt === 'jira'     && <JiraConfig     config={config} set={set} str={str} num={num} />}
        {nt === 'notion'   && <NotionConfig   config={config} set={set} str={str} num={num} />}
        {nt === 'linear'   && <LinearConfig   config={config} set={set} str={str} num={num} />}
        {nt === 'airtable' && <AirtableConfig config={config} set={set} str={str} num={num} />}
        {nt === 'for_each' && <ForEachConfig  config={config} set={set} str={str} num={num} />}
        {nt === 'discord' && <DiscordConfig   config={config} set={set} str={str} num={num} />}
        {nt === 'teams'   && <TeamsConfig     config={config} set={set} str={str} num={num} />}
        {nt === 'sheets'  && <SheetsConfig    config={config} set={set} str={str} num={num} />}
        {nt === 'xml'     && <XmlConfig       config={config} set={set} str={str} num={num} />}
        {nt === 'yaml'    && <YamlConfig      config={config} set={set} str={str} num={num} />}
        {nt === 'twilio'  && <TwilioConfig    config={config} set={set} str={str} num={num} />}
        {nt === 'stripe'  && <StripeConfig    config={config} set={set} str={str} num={num} />}
        {nt === 'crypto'        && <CryptoConfig        config={config} set={set} str={str} num={num} />}
        {nt === 'hubspot'       && <HubspotConfig       config={config} set={set} str={str} num={num} />}
        {nt === 'date'          && <DateConfig          config={config} set={set} str={str} num={num} />}
        {nt === 'zendesk'       && <ZendeskConfig       config={config} set={set} str={str} num={num} />}
        {nt === 'redis'         && <RedisConfig         config={config} set={set} str={str} num={num} />}
        {nt === 'elasticsearch' && <ElasticsearchConfig config={config} set={set} str={str} num={num} />}
        {nt === 'pagerduty'     && <PagerdutyConfig     config={config} set={set} str={str} num={num} />}
        {nt === 'handlebars'    && <HandlebarsConfig    config={config} set={set} str={str} num={num} />}
        {nt === 'math'          && <MathConfig          config={config} set={set} str={str} num={num} />}
        {nt === 'array_utils'   && <ArrayUtilsConfig    config={config} set={set} str={str} num={num} />}
        {nt === 'shopify'       && <ShopifyConfig       config={config} set={set} str={str} num={num} />}
        {nt === 'datadog'       && <DatadogConfig       config={config} set={set} str={str} num={num} />}
        {nt === 'salesforce'    && <SalesforceConfig    config={config} set={set} str={str} num={num} />}
        {nt === 'freshdesk'     && <FreshdeskConfig     config={config} set={set} str={str} num={num} />}
        {nt === 'mailgun'       && <MailgunConfig       config={config} set={set} str={str} num={num} />}
        {nt === 'asana'         && <AsanaConfig         config={config} set={set} str={str} num={num} />}
        {nt === 'servicenow'    && <ServiceNowConfig    config={config} set={set} str={str} num={num} />}
        {nt === 'confluence'    && <ConfluenceConfig    config={config} set={set} str={str} num={num} />}
        {nt === 'bitbucket'     && <BitbucketConfig     config={config} set={set} str={str} num={num} />}
        {nt === 'azure_devops'  && <AzureDevOpsConfig   config={config} set={set} str={str} num={num} />}
        {nt === 'twitch'        && <TwitchConfig        config={config} set={set} str={str} num={num} />}
        {nt === 'figma'         && <FigmaConfig         config={config} set={set} str={str} num={num} />}
        {nt === 'dropbox'       && <DropboxConfig       config={config} set={set} str={str} num={num} />}
        {nt === 'cloudflare'    && <CloudflareConfig    config={config} set={set} str={str} num={num} />}
        {nt === 'box'           && <BoxConfig           config={config} set={set} str={str} num={num} />}
        {nt === 'okta'          && <OktaConfig          config={config} set={set} str={str} num={num} />}
        {nt === 'zoom'          && <ZoomConfig          config={config} set={set} str={str} num={num} />}
        {nt === 'spotify'       && <SpotifyConfig       config={config} set={set} str={str} num={num} />}
        {nt === 'typeform'      && <TypeformConfig      config={config} set={set} str={str} num={num} />}
        {nt === 'webflow'       && <WebflowConfig       config={config} set={set} str={str} num={num} />}
        {nt === 'intercom'      && <IntercomConfig      config={config} set={set} str={str} num={num} />}
        {nt === 'pipedrive'     && <PipedriveConfig     config={config} set={set} str={str} num={num} />}
        {nt === 'trello'        && <TrelloConfig        config={config} set={set} str={str} num={num} />}
        {nt === 'monday'        && <MondayConfig        config={config} set={set} str={str} num={num} />}
        {nt === 'clickup'       && <ClickupConfig       config={config} set={set} str={str} num={num} />}
        {nt === 'amplitude'     && <AmplitudeConfig     config={config} set={set} str={str} num={num} />}
        {nt === 'mixpanel'      && <MixpanelConfig      config={config} set={set} str={str} num={num} />}
        {nt === 'segment'       && <SegmentConfig       config={config} set={set} str={str} num={num} />}
        {nt === 'sendgrid'      && <SendgridConfig      config={config} set={set} str={str} num={num} />}
        {nt === 'braintree'     && <BraintreeConfig     config={config} set={set} str={str} num={num} />}
        {nt === 'paypal'        && <PaypalConfig        config={config} set={set} str={str} num={num} />}
        {nt === 'razorpay'      && <RazorpayConfig      config={config} set={set} str={str} num={num} />}
        {nt === 'firebase'      && <FirebaseConfig      config={config} set={set} str={str} num={num} />}
        {nt === 'supabase'        && <SupabaseConfig        config={config} set={set} str={str} num={num} />}
        {nt === 'mailchimp'       && <MailchimpConfig       config={config} set={set} str={str} num={num} />}
        {nt === 'activecampaign'  && <ActivecampaignConfig  config={config} set={set} str={str} num={num} />}
        {nt === 'klaviyo'         && <KlaviyoConfig         config={config} set={set} str={str} num={num} />}
        {nt === 'resend'          && <ResendConfig          config={config} set={set} str={str} num={num} />}
        {nt === 'contentful'      && <ContentfulConfig      config={config} set={set} str={str} num={num} />}
        {nt === 'algolia'         && <AlgoliaConfig         config={config} set={set} str={str} num={num} />}
        {nt === 'postmark'        && <PostmarkConfig        config={config} set={set} str={str} num={num} />}
        {nt === 'vonage'          && <VonageConfig          config={config} set={set} str={str} num={num} />}
        {nt === 'telegram'        && <TelegramConfig        config={config} set={set} str={str} num={num} />}
        {nt === 'replicate'       && <ReplicateConfig       config={config} set={set} str={str} num={num} />}
        {nt === 'mistral'         && <MistralConfig         config={config} set={set} str={str} num={num} />}
        {nt === 'whatsapp'        && <WhatsappConfig        config={config} set={set} str={str} num={num} />}
        {nt === 'googledocs'      && <GoogledocsConfig      config={config} set={set} str={str} num={num} />}
        {nt === 'perplexity'      && <PerplexityConfig      config={config} set={set} str={str} num={num} />}
        {nt === 'cohere'          && <CohereConfig          config={config} set={set} str={str} num={num} />}
        {nt === 'googledrive'     && <GoogledriveConfig     config={config} set={set} str={str} num={num} />}
        {nt === 'woocommerce'     && <WoocommerceConfig     config={config} set={set} str={str} num={num} />}
        {nt === 'pinecone'        && <PineconeConfig        config={config} set={set} str={str} num={num} />}
        {nt === 'togetherai'      && <TogetheraiConfig      config={config} set={set} str={str} num={num} />}
        {nt === 'awss3'           && <Awss3Config           config={config} set={set} str={str} num={num} />}
        {nt === 'huggingface'     && <HuggingfaceConfig     config={config} set={set} str={str} num={num} />}
        {nt === 'groq'            && <GroqConfig            config={config} set={set} str={str} num={num} />}
        {nt === 'openrouter'      && <OpenrouterConfig      config={config} set={set} str={str} num={num} />}
        {nt === 'qdrant'          && <QdrantConfig          config={config} set={set} str={str} num={num} />}
        {nt === 'cloudinary'      && <CloudinaryConfig      config={config} set={set} str={str} num={num} />}
        {nt === 'gcal'            && <GcalConfig            config={config} set={set} str={str} num={num} />}
        {nt === 'docusign'        && <DocusignConfig        config={config} set={set} str={str} num={num} />}
        {nt === 'xero'            && <XeroConfig            config={config} set={set} str={str} num={num} />}
        {nt === 'calendly'        && <CalendlyConfig        config={config} set={set} str={str} num={num} />}
        {nt === 'apify'           && <ApifyConfig           config={config} set={set} str={str} num={num} />}
        {nt === 'ganalytics'      && <GanalyticsConfig      config={config} set={set} str={str} num={num} />}
        {nt === 'neon'            && <NeonConfig            config={config} set={set} str={str} num={num} />}
        {nt === 'copper'          && <CopperConfig          config={config} set={set} str={str} num={num} />}
        {nt === 'azure_openai'    && <AzureOpenaiConfig    config={config} set={set} str={str} num={num} />}
        {nt === 'grok'            && <GrokConfig            config={config} set={set} str={str} num={num} />}
        {nt === 'ollama'          && <OllamaConfig          config={config} set={set} str={str} num={num} />}
        {nt === 'weaviate'        && <WeaviateConfig        config={config} set={set} str={str} num={num} />}
        {nt === 'chroma'          && <ChromaConfig          config={config} set={set} str={str} num={num} />}
        {nt === 'mongodb'         && <MongodbConfig         config={config} set={set} str={str} num={num} />}
        {nt === 'clickhouse'      && <ClickhouseConfig      config={config} set={set} str={str} num={num} />}
        {nt === 'gcs'             && <GcsConfig             config={config} set={set} str={str} num={num} />}
        {nt === 'azure_blob'      && <AzureBlobConfig       config={config} set={set} str={str} num={num} />}
        {nt === 'hash'            && <HashConfig            config={config} set={set} str={str} num={num} />}
        {nt === 'jwt'             && <JwtConfig             config={config} set={set} str={str} num={num} />}
        {nt === 'vertex'          && <VertexConfig          config={config} set={set} str={str} num={num} />}
        {nt === 'sqs'             && <SqsConfig             config={config} set={set} str={str} num={num} />}
        {nt === 'sns'             && <SnsConfig             config={config} set={set} str={str} num={num} />}
        {nt === 'bedrock'         && <BedrockConfig         config={config} set={set} str={str} num={num} />}
        {nt === 'milvus'          && <MilvusConfig          config={config} set={set} str={str} num={num} />}
        {nt === 'kafka'           && <KafkaConfig           config={config} set={set} str={str} num={num} />}
        {nt === 'rabbitmq'        && <RabbitmqConfig        config={config} set={set} str={str} num={num} />}
        {nt === 'zip'             && <ZipConfig             config={config} set={set} str={str} num={num} />}
        {nt === 'image'           && <ImageConfig           config={config} set={set} str={str} num={num} />}
        {nt === 'pdf_extract'     && <PdfExtractConfig      config={config} set={set} str={str} num={num} />}
        {nt === 'ocr'             && <OcrConfig             config={config} set={set} str={str} num={num} />}
        {nt === 'feishu'          && <FeishuConfig          config={config} set={set} str={str} num={num} />}
        {nt === 'dingtalk'        && <DingtalkConfig        config={config} set={set} str={str} num={num} />}
        {nt === 'wecom'           && <WecomConfig           config={config} set={set} str={str} num={num} />}
        {nt === 'embedding'       && <EmbeddingConfig       config={config} set={set} str={str} num={num} />}
        {nt === 'reranker'        && <RerankerConfig        config={config} set={set} str={str} num={num} />}
        {nt === 'text_splitter'   && <TextSplitterConfig    config={config} set={set} str={str} num={num} />}
        {nt === 'structured_output' && <StructuredOutputConfig config={config} set={set} str={str} num={num} />}
        {nt === 'classifier'      && <ClassifierConfig      config={config} set={set} str={str} num={num} />}
        {nt === 'image_gen'       && <ImageGenConfig        config={config} set={set} str={str} num={num} />}
        {nt === 'speech_to_text'  && <SpeechToTextConfig    config={config} set={set} str={str} num={num} />}
        {nt === 'tts'             && <TtsConfig             config={config} set={set} str={str} num={num} />}
        {nt === 'html_extract'    && <HtmlExtractConfig     config={config} set={set} str={str} num={num} />}
        {nt === 'rss'             && <RssConfig             config={config} set={set} str={str} num={num} />}
        {nt === 'deepseek'        && <DeepseekConfig        config={config} set={set} str={str} num={num} />}
        {nt === 'qwen'            && <QwenConfig            config={config} set={set} str={str} num={num} />}
        {nt === 'zhipu'           && <ZhipuConfig           config={config} set={set} str={str} num={num} />}
        {nt === 'moonshot'        && <MoonshotConfig        config={config} set={set} str={str} num={num} />}
        {nt === 'doubao'          && <DoubaoConfig          config={config} set={set} str={str} num={num} />}
        {nt === 'minimax'         && <MinimaxConfig         config={config} set={set} str={str} num={num} />}
        {nt === 'ernie'           && <ErnieConfig           config={config} set={set} str={str} num={num} />}
        {nt === 'hunyuan'         && <HunyuanConfig         config={config} set={set} str={str} num={num} />}
        {nt !== 'trigger' && nt !== 'note' && nt !== 'approval' && nt !== 'fan_out' && nt !== 'fan_in' && (
          <AdvancedConfig config={config} set={set} str={str} num={num} />
        )}
        {upstreamNodes && upstreamNodes.length > 0 && <UpstreamVarsHint nodes={upstreamNodes} />}
        <RawConfigPreview config={config} />
      </div>
    </div>
  )
}


function AdvancedConfig({ config, set }: ConfigProps) {
  const [open, setOpen] = useState(false)
  const { t } = useLocale()
  const maxRetries = (config.max_retries as number | undefined) ?? 0
  const timeoutSecs = (config.timeout_secs as number | undefined) ?? 0
  const retryDelayMs = (config.retry_delay_ms as number | undefined) ?? 200
  const cacheTtlSecs = (config.cache_ttl_secs as number | undefined) ?? 0

  const hasSetting = maxRetries > 0 || timeoutSecs > 0 || retryDelayMs !== 200 || cacheTtlSecs > 0

  return (
    <div style={{ marginTop: 10, borderTop: '1px solid var(--border)', paddingTop: 8 }}>
      <button
        onClick={() => setOpen((o) => !o)}
        style={{ background: 'none', border: 'none', color: 'var(--muted)', cursor: 'pointer', fontSize: 11, padding: 0, display: 'flex', alignItems: 'center', gap: 4 }}
      >
        {open ? '▾' : '▸'} {t('node.advanced')} {hasSetting && <span style={{ color: 'var(--link)', fontSize: 10 }}>●</span>}
      </button>
      {open && (
        <div style={{ marginTop: 8, display: 'flex', flexDirection: 'column', gap: 8 }}>
          <div className="field" style={{ marginBottom: 0 }}>
            <label style={{ fontSize: 11 }}>{t('node.max.retries')} <span style={{ color: 'var(--muted)' }}>{t('node.max.retries.hint')}</span></label>
            <input
              type="number" min={0} max={5}
              value={maxRetries}
              onChange={(e) => set('max_retries', Math.max(0, Math.min(5, parseInt(e.target.value) || 0)))}
              style={{ fontSize: 12, width: 80 }}
            />
          </div>
          {maxRetries > 0 && (
            <div className="field" style={{ marginBottom: 0 }}>
              <label style={{ fontSize: 11 }}>{t('node.retry.delay')} <span style={{ color: 'var(--muted)' }}>{t('node.retry.delay.hint')}</span></label>
              <input
                type="number" min={50} max={10000}
                value={retryDelayMs}
                onChange={(e) => set('retry_delay_ms', Math.max(50, Math.min(10000, parseInt(e.target.value) || 200)))}
                style={{ fontSize: 12, width: 100 }}
              />
            </div>
          )}
          <div className="field" style={{ marginBottom: 0 }}>
            <label style={{ fontSize: 11 }}>{t('node.timeout')} <span style={{ color: 'var(--muted)' }}>{t('node.timeout.hint')}</span></label>
            <input
              type="number" min={0} max={3600}
              value={timeoutSecs}
              onChange={(e) => set('timeout_secs', Math.max(0, parseInt(e.target.value) || 0))}
              style={{ fontSize: 12, width: 100 }}
            />
          </div>
          <div className="field" style={{ marginBottom: 0 }}>
            <label style={{ fontSize: 11 }}>Cache TTL (s) <span style={{ color: 'var(--muted)' }}>(0 = off) — cache node output; same node+input reuses result</span></label>
            <input
              type="number" min={0} max={86400}
              value={cacheTtlSecs}
              onChange={(e) => set('cache_ttl_secs', Math.max(0, parseInt(e.target.value) || 0))}
              style={{ fontSize: 12, width: 100 }}
            />
          </div>
        </div>
      )}
    </div>
  )
}

function UpstreamVarsHint({ nodes }: { nodes: FlowNode[] }) {
  const [open, setOpen] = useState(false)
  const [copied, setCopied] = useState<string | null>(null)
  const { t } = useLocale()

  const copyVar = (v: string) => {
    navigator.clipboard.writeText(v).catch(() => {})
    setCopied(v)
    setTimeout(() => setCopied(null), 1200)
  }

  return (
    <div style={{ marginTop: 12, borderTop: '1px solid var(--border)', paddingTop: 8 }}>
      <button
        onClick={() => setOpen((o) => !o)}
        style={{ background: 'none', border: 'none', color: 'var(--muted)', cursor: 'pointer', fontSize: 11, padding: 0, display: 'flex', alignItems: 'center', gap: 4 }}
      >
        {open ? '▾' : '▸'} {t('node.vars')} ({nodes.length} {t('node.vars.upstream')})
      </button>
      {open && (
        <div style={{ marginTop: 6, padding: '6px 8px', background: 'var(--canvas-bg)', border: '1px solid var(--border)', borderRadius: 4 }}>
          <div style={{ marginBottom: 8 }}>
            <div style={{ fontSize: 10, fontWeight: 700, color: 'var(--muted)', marginBottom: 3, fontFamily: 'monospace' }}>ctx · execution metadata</div>
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: 3 }}>
              {(['ctx.execution_id', 'ctx.workflow_version_id'] as const).map((v) => {
                const full = `{{${v}}}`
                return (
                  <code
                    key={v}
                    title="Click to copy"
                    onClick={() => copyVar(full)}
                    style={{
                      fontSize: 10, padding: '1px 5px',
                      background: copied === full ? 'rgba(34,197,94,0.15)' : 'rgba(37,99,235,0.10)',
                      color: copied === full ? 'var(--success-text)' : 'var(--link)',
                      border: `1px solid ${copied === full ? 'rgba(34,197,94,0.3)' : 'rgba(37,99,235,0.25)'}`,
                      borderRadius: 3, cursor: 'pointer', fontFamily: 'monospace',
                    }}
                  >
                    {copied === full ? '✓' : full}
                  </code>
                )
              })}
            </div>
          </div>
          {nodes.map((n) => {
            const fields = NODE_OUTPUTS[n.data.nodeType] ?? []
            const label = n.data.config?.node_label as string | undefined
            return (
              <div key={n.id} style={{ marginBottom: 8 }}>
                <div style={{ fontSize: 10, fontWeight: 700, color: 'var(--muted)', marginBottom: 3, fontFamily: 'monospace' }}>
                  {label ? `${label} (${n.id})` : n.id} · {n.data.nodeType}
                </div>
                <div style={{ display: 'flex', flexWrap: 'wrap', gap: 3 }}>
                  {fields.map((field) => {
                    const v = `{{${n.id}.${field}}}`
                    return (
                      <code
                        key={field}
                        title="Click to copy"
                        onClick={() => copyVar(v)}
                        style={{
                          fontSize: 10, padding: '1px 5px',
                          background: copied === v ? 'rgba(34,197,94,0.15)' : 'rgba(37,99,235,0.10)',
                          color: copied === v ? 'var(--success-text)' : 'var(--link)',
                          border: `1px solid ${copied === v ? 'rgba(34,197,94,0.3)' : 'rgba(37,99,235,0.25)'}`,
                          borderRadius: 3, cursor: 'pointer', fontFamily: 'monospace',
                          transition: 'background 0.2s',
                        }}
                      >
                        {copied === v ? '✓' : v}
                      </code>
                    )
                  })}
                  {fields.length === 0 && (
                    <span style={{ fontSize: 10, color: 'var(--muted)', fontStyle: 'italic' }}>
                      (dynamic output — use <code style={{ fontSize: 10 }}>{`{{${n.id}.*}}`}</code>)
                    </span>
                  )}
                </div>
              </div>
            )
          })}
        </div>
      )}
    </div>
  )
}

function RawConfigPreview({ config }: { config: Record<string, unknown> }) {
  const [open, setOpen] = useState(false)
  const { t } = useLocale()
  const json = JSON.stringify(config, null, 2)
  return (
    <div style={{ marginTop: 12, borderTop: '1px solid var(--border)', paddingTop: 8 }}>
      <button
        onClick={() => setOpen((o) => !o)}
        style={{ background: 'none', border: 'none', color: 'var(--muted)', cursor: 'pointer', fontSize: 11, padding: 0, display: 'flex', alignItems: 'center', gap: 4 }}
      >
        {open ? '▾' : '▸'} {t('node.raw')}
      </button>
      {open && (
        <pre style={{
          marginTop: 6, padding: '8px 10px',
          background: 'var(--canvas-bg)', border: '1px solid var(--border)',
          borderRadius: 4, fontSize: 10, fontFamily: 'monospace',
          color: 'var(--muted)', whiteSpace: 'pre-wrap', wordBreak: 'break-all',
          maxHeight: 200, overflowY: 'auto', lineHeight: 1.5,
        }}>
          {json}
        </pre>
      )}
    </div>
  )
}


function NodeResultBox({ result, locale: _locale, t }: { result: NodeExecutionRecord; locale: string; t: (key: TranslationKey) => string }) {
  const prettyOutput = (() => {
    if (!result.output_json) return null
    try { return JSON.stringify(JSON.parse(result.output_json), null, 2) }
    catch { return result.output_json }
  })()

  return (
    <div className="exec-result-box">
      <div className="exec-result-box-header">
        <span className={`dot dot-${result.status}`} />
        <span>{t('node.result')}</span>
        <span className={`badge badge-${result.status}`} style={{ fontSize: 10, padding: '1px 5px', marginLeft: 'auto' }}>
          {result.status}
        </span>
      </div>
      {result.error && (
        <div className="exec-result-error">{result.error}</div>
      )}
      {prettyOutput && !result.error && (
        <pre className="exec-result-output">{prettyOutput}</pre>
      )}
      {!result.error && !prettyOutput && (
        <div style={{ padding: '8px 10px', fontSize: 11, color: 'var(--muted)' }}>{t('node.no.output')}</div>
      )}
    </div>
  )
}

