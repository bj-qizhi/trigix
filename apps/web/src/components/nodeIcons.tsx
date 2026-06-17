// Single source of truth for node iconography.
// Provider/integration nodes use their official brand mark (Simple Icons via
// react-icons/si); generic flow/data/AI nodes use a consistent Phosphor line
// set (react-icons/pi). Icons inherit `currentColor`, so the node header colour
// drives them — no emoji, one stroke language across the whole editor.
import type { IconType } from 'react-icons'
import {
  SiSlack, SiOpenai, SiGithub, SiGooglegemini, SiNotion, SiLinear, SiAirtable,
  SiDiscord, SiStripe, SiHubspot, SiZendesk, SiRedis, SiElasticsearch, SiPagerduty,
  SiShopify, SiDatadog, SiSalesforce, SiMailgun, SiAsana, SiConfluence, SiBitbucket,
  SiTwitch, SiFigma, SiDropbox, SiCloudflare, SiBox, SiOkta, SiZoom, SiSpotify,
  SiTypeform, SiWebflow, SiIntercom, SiTrello, SiClickup, SiMixpanel, SiSendgrid,
  SiBraintree, SiPaypal, SiRazorpay, SiFirebase, SiSupabase, SiMailchimp, SiResend,
  SiContentful, SiAlgolia, SiVonage, SiTelegram, SiReplicate, SiMistralai, SiWhatsapp,
  SiGoogledocs, SiPerplexity, SiGoogledrive, SiWoocommerce, SiHuggingface, SiCloudinary,
  SiGooglecalendar, SiXero, SiCalendly, SiGoogleanalytics, SiX, SiOllama, SiMongodb,
  SiClickhouse, SiGooglecloud, SiApachekafka, SiRabbitmq, SiJira, SiGooglesheets,
  SiAlibabadotcom, SiBaidu, SiClaude, SiSnowflake, SiGooglebigquery, SiMysql,
  SiAlibabacloud, SiWechat, SiMilvus, SiTwilio, SiGraphql,
} from 'react-icons/si'
import {
  PiLightning, PiGlobe, PiRobot, PiGitBranch, PiSealCheck, PiArrowsClockwise,
  PiFunnel, PiSigma, PiSortAscending, PiArrowsLeftRight, PiTimer, PiTreeStructure,
  PiCheckCircle, PiLifebuoy, PiArrowsSplit, PiArrowsMerge, PiCode, PiEnvelopeSimple,
  PiBooks, PiUploadSimple, PiPuzzlePiece, PiDatabase, PiArrowElbowDownRight, PiNote,
  PiGitFork, PiDiceFive, PiCopySimple, PiAsterisk, PiTable, PiPencilSimpleLine,
  PiTextAa, PiWebhooksLogo, PiRepeat, PiBracketsAngle, PiListDashes, PiLockKey,
  PiCalendarBlank, PiBracketsCurly, PiMathOperations, PiBracketsSquare, PiHeadset,
  PiTicket, PiInfinity, PiCalendarCheck, PiChartLine, PiShuffle, PiBroadcast,
  PiStack, PiBrain, PiMegaphone, PiEnvelope, PiEnvelopeOpen, PiTree, PiHandshake,
  PiHardDrives, PiCpu, PiGraph, PiSignature, PiBug, PiUsersThree, PiCloud, PiHash,
  PiKey, PiPaperPlaneTilt, PiVectorThree, PiSlidersHorizontal, PiScissors, PiTag,
  PiPaintBrush, PiMicrophone, PiSpeakerHigh, PiRss, PiFolderSimple, PiFolderLock,
  PiTerminal, PiTray, PiHourglass, PiMoon, PiChatCircleDots, PiFileZip, PiImage,
  PiFilePdf, PiScan, PiCircle,
} from 'react-icons/pi'
import type { NodeType } from '../types'

export const NODE_ICON: Record<NodeType, IconType> = {
  // ── core flow ──
  trigger: PiLightning, http: PiGlobe, agent: PiRobot, condition: PiGitBranch,
  approval: PiSealCheck, map: PiArrowsClockwise, filter: PiFunnel, aggregate: PiSigma,
  sort: PiSortAscending, transform: PiArrowsLeftRight, delay: PiTimer,
  sub_workflow: PiTreeStructure, assert: PiCheckCircle, catch: PiLifebuoy,
  fan_out: PiArrowsSplit, fan_in: PiArrowsMerge, code: PiCode, merge: PiArrowsMerge,
  loop: PiArrowsClockwise, for_each: PiRepeat, split: PiArrowsSplit, join: PiArrowsMerge,
  switch: PiGitFork, random: PiDiceFive, dedupe: PiCopySimple, regex: PiAsterisk,
  extract: PiArrowElbowDownRight, validate: PiCheckCircle, note: PiNote,
  rename: PiPencilSimpleLine, format: PiTextAa, webhook: PiWebhooksLogo,
  // ── data / transform ──
  csv: PiTable, xml: PiBracketsAngle, yaml: PiListDashes, handlebars: PiBracketsCurly,
  math: PiMathOperations, array_utils: PiBracketsSquare, date: PiCalendarBlank,
  crypto: PiLockKey, hash: PiHash, jwt: PiKey, zip: PiFileZip, image: PiImage,
  pdf_extract: PiFilePdf, ocr: PiScan, database: PiDatabase, sqlserver: PiDatabase,
  mysql: SiMysql, snowflake: SiSnowflake, bigquery: SiGooglebigquery, neon: PiDatabase,
  // ── AI / RAG ──
  openai: SiOpenai, gemini: SiGooglegemini, claude: SiClaude, grok: SiX,
  mistral: SiMistralai, perplexity: SiPerplexity, cohere: PiBrain, ollama: SiOllama,
  huggingface: SiHuggingface, groq: PiCpu, openrouter: PiShuffle, togetherai: PiHandshake,
  replicate: SiReplicate, deepseek: PiBrain, qwen: SiAlibabadotcom, zhipu: PiBrain,
  moonshot: PiMoon, doubao: PiChatCircleDots, minimax: PiBrain, ernie: SiBaidu,
  hunyuan: PiBrain, azure_openai: PiBrain, vertex: SiGooglecloud, bedrock: PiBrain,
  rag: PiBooks, rag_ingest: PiUploadSimple, embedding: PiVectorThree,
  reranker: PiSlidersHorizontal, text_splitter: PiScissors, structured_output: PiBracketsCurly,
  classifier: PiTag, image_gen: PiPaintBrush, speech_to_text: PiMicrophone, tts: PiSpeakerHigh,
  // ── vector stores ──
  pinecone: PiTree, qdrant: PiGraph, weaviate: PiGraph, chroma: PiGraph, milvus: SiMilvus,
  // ── messaging / collab ──
  slack: SiSlack, discord: SiDiscord, teams: PiUsersThree, telegram: SiTelegram,
  whatsapp: SiWhatsapp, email: PiEnvelopeSimple, intercom: SiIntercom, zoom: SiZoom,
  feishu: PiPaperPlaneTilt, dingtalk: SiAlibabacloud, wecom: SiWechat,
  // ── dev / project ──
  github: SiGithub, bitbucket: SiBitbucket, jira: SiJira, linear: SiLinear, airtable: SiAirtable,
  confluence: SiConfluence, notion: SiNotion, asana: SiAsana, trello: SiTrello,
  clickup: SiClickup, monday: PiCalendarCheck, figma: SiFigma, azure_devops: PiInfinity,
  // ── storage / cloud ──
  awss3: PiHardDrives, gcs: SiGooglecloud, azure_blob: PiCloud, dropbox: SiDropbox,
  box: SiBox, googledrive: SiGoogledrive, googledocs: SiGoogledocs, sheets: SiGooglesheets,
  cloudinary: SiCloudinary, cloudflare: SiCloudflare, ftp: PiFolderSimple,
  sftp: PiFolderLock, ssh: PiTerminal, imap: PiTray,
  // ── data infra ──
  redis: SiRedis, mongodb: SiMongodb, clickhouse: SiClickhouse, elasticsearch: SiElasticsearch,
  kafka: SiApachekafka, rabbitmq: SiRabbitmq, sqs: PiStack, sns: PiBroadcast,
  graphql: SiGraphql, algolia: SiAlgolia,
  // ── commerce / billing ──
  stripe: SiStripe, paypal: SiPaypal, braintree: SiBraintree, razorpay: SiRazorpay,
  shopify: SiShopify, woocommerce: SiWoocommerce, xero: SiXero,
  // ── crm / marketing / support ──
  hubspot: SiHubspot, salesforce: SiSalesforce, pipedrive: PiFunnel, copper: PiFunnel,
  zendesk: SiZendesk, freshdesk: PiHeadset, servicenow: PiTicket, mailchimp: SiMailchimp,
  sendgrid: SiSendgrid, mailgun: SiMailgun, resend: SiResend, postmark: PiEnvelopeOpen,
  activecampaign: PiMegaphone, klaviyo: PiEnvelope, twilio: SiTwilio, vonage: SiVonage,
  // ── analytics / ops ──
  amplitude: PiChartLine, mixpanel: SiMixpanel, segment: PiShuffle, ganalytics: SiGoogleanalytics,
  datadog: SiDatadog, pagerduty: SiPagerduty, apify: PiBug,
  // ── productivity / misc ──
  twitch: SiTwitch, spotify: SiSpotify, typeform: SiTypeform, webflow: SiWebflow,
  contentful: SiContentful, supabase: SiSupabase, firebase: SiFirebase,
  okta: SiOkta, calendly: SiCalendly, gcal: SiGooglecalendar, docusign: PiSignature,
  rss: PiRss, html_extract: PiBracketsAngle, wait: PiHourglass, custom: PiPuzzlePiece,
}

export function NodeIcon({ type, size = 16, className, style }: {
  type: NodeType
  size?: number
  className?: string
  style?: React.CSSProperties
}) {
  const Icon = NODE_ICON[type] ?? PiCircle
  return <Icon size={size} className={className} style={style} aria-hidden />
}
