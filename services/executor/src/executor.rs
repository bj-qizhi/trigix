// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use hmac::Mac as _;
use lru::LruCache;
use rhai;
use serde::{Deserialize, Serialize};
use sha2::Digest as _;
use sqlx::Column as _;
use sqlx::Row as _;
use workflow_core::{Node, NodeType};

use crate::approval::ApprovalGate;
use crate::runtime::{ExecutionContext, NodeExecutionResult, NodeExecutor};

// Third-party integration nodes, grouped by domain.
mod nodes_ai_ext;
mod nodes_data_ext;
mod nodes_messaging_ext;
mod nodes_saas_commerce;
mod nodes_saas_crm;
mod nodes_saas_marketing;
mod nodes_saas_misc;
mod nodes_storage_ext;
use nodes_ai_ext::*;
use nodes_data_ext::*;
use nodes_messaging_ext::*;
use nodes_saas_commerce::*;
use nodes_saas_crm::*;
use nodes_saas_marketing::*;
use nodes_saas_misc::*;
use nodes_storage_ext::*;

// Chinese-vendor LLM nodes extracted into their own submodule.
mod nodes_cn_llm;
use nodes_cn_llm::*;

// Vector-store nodes (Weaviate / Chroma) over HTTP.
mod nodes_vector;
use nodes_vector::*;

// Database / warehouse nodes over HTTP (MongoDB Atlas Data API, ClickHouse).
mod nodes_db_queue;
use nodes_db_queue::*;

// Object-storage nodes over HTTP (Google Cloud Storage, Azure Blob).
mod nodes_storage;
use nodes_storage::*;

// Pure-compute crypto utility nodes (Hash/HMAC, JWT).
mod nodes_tools;
use nodes_tools::*;

// AWS nodes signed with SigV4 (SQS, SNS, Bedrock).
mod nodes_aws;
use nodes_aws::*;

// Message-broker nodes over HTTP (Kafka REST Proxy, RabbitMQ Management API).
mod nodes_messaging;
use nodes_messaging::*;

// Binary-processing nodes (zip, image, pdf, ocr).
mod nodes_binary;
use nodes_binary::*;

// Chinese enterprise collaboration nodes (Feishu, DingTalk, WeChat Work).
mod nodes_china;
use nodes_china::*;

// AI-native building blocks (embedding, reranker, splitter, structured output, …).
mod nodes_ai_blocks;
use nodes_ai_blocks::*;

// Core workflow primitives (HTML extract, RSS).
mod nodes_core;
use nodes_core::*;

// Data-warehouse / extra DB nodes (MySQL, Snowflake, BigQuery).
mod nodes_warehouse;
use nodes_warehouse::*;

// Network file / shell / mail clients (FTP, SFTP, SSH, IMAP).
mod nodes_ftp;
use nodes_ftp::*;
mod nodes_ssh;
use nodes_ssh::*;
mod nodes_mail;
use nodes_mail::*;

// SaaS integration nodes extracted into their own submodule.
mod nodes_integrations;
use nodes_integrations::*;

// Data-transform / utility nodes extracted into their own submodule.
mod nodes_transform;
use nodes_transform::*;

// Western LLM nodes (OpenAI / Gemini / Claude) extracted into their own submodule.
mod nodes_ai;
use nodes_ai::*;

// Community/third-party HTTP nodes (node SDK).
mod nodes_custom;
use nodes_custom::*;

// Global per-process node output cache: key → (cached_at, output_json)
type NodeCache = Arc<Mutex<LruCache<String, (Instant, String)>>>;
static NODE_CACHE: OnceLock<NodeCache> = OnceLock::new();

fn node_cache() -> &'static NodeCache {
    NODE_CACHE.get_or_init(|| {
        Arc::new(Mutex::new(LruCache::new(
            std::num::NonZeroUsize::new(1024).unwrap(),
        )))
    })
}

fn node_cache_key(node: &Node, context: &ExecutionContext) -> String {
    // Cache key includes node type + node id + config hash + execution input
    let config_str = node
        .config
        .as_ref()
        .map(|c| c.to_string())
        .unwrap_or_default();
    let raw = format!(
        "{:?}:{}:{}:{}",
        node.node_type, node.id, config_str, context.input_json
    );
    let hash = sha2::Sha256::digest(raw.as_bytes());
    hex::encode(hash)
}

// ── Template resolution ───────────────────────────────────────────────────────
//
// Syntax: {{expr}} where expr is one of:
//   input            → the raw input_json string
//   input.a.b        → field a.b inside input_json (dot-path)
//   node_id          → the raw output_json of that node
//   node_id.a.b      → field a.b inside that node's output_json

fn resolve_template(template: &str, context: &ExecutionContext) -> String {
    let mut result = String::new();
    let mut rest = template;
    while let Some(open) = rest.find("{{") {
        result.push_str(&rest[..open]);
        let after = &rest[open + 2..];
        if let Some(close) = after.find("}}") {
            let expr = after[..close].trim();
            result.push_str(&resolve_expr(expr, context));
            rest = &after[close + 2..];
        } else {
            result.push_str("{{");
            rest = after;
        }
    }
    result.push_str(rest);
    result
}

fn resolve_expr(expr: &str, context: &ExecutionContext) -> String {
    let (root, path) = match expr.find('.') {
        Some(i) => (&expr[..i], Some(&expr[i + 1..])),
        None => (expr, None),
    };
    // ctx.* variables expose execution metadata.
    if root == "ctx" {
        return match path {
            Some("execution_id") => context.execution_id.clone(),
            Some("workflow_version_id") => context.workflow_version_id.clone(),
            _ => String::new(),
        };
    }
    let json_str = match root {
        "input" => Some(context.input_json.as_str()),
        node_id => context.node_outputs.get(node_id).map(|s| s.as_str()),
    };
    match (json_str, path) {
        (None, _) => String::new(),
        (Some(raw), None) => raw.to_string(),
        (Some(raw), Some(path)) => {
            let val: serde_json::Value =
                serde_json::from_str(raw).unwrap_or(serde_json::Value::Null);
            json_path(&val, path)
                .map(json_to_string)
                .unwrap_or_default()
        }
    }
}

fn json_path<'a>(value: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
    let mut cur = value;
    for seg in path.split('.') {
        cur = match cur {
            serde_json::Value::Object(map) => map.get(seg)?,
            serde_json::Value::Array(arr) => arr.get(seg.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(cur)
}

fn json_to_string(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// Resolve a config value that may be a JSON-encoded string back into JSON.
///
/// `resolve_config_strings` renders every `{{...}}` substitution to a string, so
/// an upstream array or object (e.g. a list of vectors produced by a code node)
/// arrives at the consuming node as a string like `"[{...}]"`. Nodes that need
/// the real array/object call this to parse it back. Values that are not
/// JSON-looking strings (and non-string values) are returned unchanged.
fn json_array_or_parse(value: &serde_json::Value) -> serde_json::Value {
    if let serde_json::Value::String(s) = value {
        let t = s.trim();
        let looks_json =
            (t.starts_with('[') && t.ends_with(']')) || (t.starts_with('{') && t.ends_with('}'));
        if looks_json {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(t) {
                return parsed;
            }
        }
    }
    value.clone()
}

fn resolve_config_strings(
    config: &serde_json::Value,
    context: &ExecutionContext,
) -> serde_json::Value {
    match config {
        serde_json::Value::String(s) => serde_json::Value::String(resolve_template(s, context)),
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), resolve_config_strings(v, context)))
                .collect(),
        ),
        serde_json::Value::Array(arr) => serde_json::Value::Array(
            arr.iter()
                .map(|v| resolve_config_strings(v, context))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// Routes each node to the appropriate executor based on node type.
#[derive(Clone)]
pub struct DispatchingNodeExecutor {
    http_client: reqwest::Client,
    ai_runtime_base_url: Option<String>,
    approval_gate: Option<Arc<ApprovalGate>>,
}

impl DispatchingNodeExecutor {
    pub fn new(ai_runtime_base_url: Option<String>) -> Self {
        Self {
            http_client: reqwest::Client::new(),
            ai_runtime_base_url,
            approval_gate: None,
        }
    }

    pub fn with_approval_gate(mut self, gate: Arc<ApprovalGate>) -> Self {
        self.approval_gate = Some(gate);
        self
    }
}

impl NodeExecutor for DispatchingNodeExecutor {
    fn execute<'a>(
        &'a self,
        node: &'a Node,
        context: &'a ExecutionContext,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = NodeExecutionResult> + Send + 'a>> {
        Box::pin(async move {
            // Approval nodes bypass retry/timeout — they block until a human acts.
            if node.node_type == NodeType::Approval {
                return match &self.approval_gate {
                    Some(gate) => execute_approval(context, gate).await,
                    None => NodeExecutionResult::failed("Approval gate not configured"),
                };
            }

            // Wait nodes also bypass retry/timeout: they either sleep for a
            // duration or suspend until resumed via the (approval) resume gate.
            if node.node_type == NodeType::Wait {
                return execute_wait(node, context, self.approval_gate.as_deref()).await;
            }

            let max_retries = node_config_u64(node, "max_retries").unwrap_or(0).min(5) as u32;
            let timeout_secs = node_config_u64(node, "timeout_secs").filter(|&s| s > 0);
            let cache_ttl_secs = node_config_u64(node, "cache_ttl_secs").filter(|&s| s > 0);

            // Check node output cache if cache_ttl_secs is configured.
            if let Some(ttl) = cache_ttl_secs {
                let cache_key = node_cache_key(node, context);
                let hit = {
                    let mut cache = node_cache().lock().unwrap();
                    cache.get(&cache_key).and_then(|(cached_at, output)| {
                        if cached_at.elapsed() < Duration::from_secs(ttl) {
                            Some(output.clone())
                        } else {
                            None
                        }
                    })
                };
                if let Some(cached_output) = hit {
                    return NodeExecutionResult::succeeded(cached_output);
                }
            }

            // Clone cheaply (reqwest::Client is Arc-backed; ai_runtime_base_url is a small String)
            let http_client = self.http_client.clone();
            let ai_base = self.ai_runtime_base_url.clone();

            let mut last = NodeExecutionResult::failed("Execution not started");
            for attempt in 0..=max_retries {
                last = dispatch_with_timeout(
                    node,
                    context,
                    &http_client,
                    ai_base.as_deref(),
                    timeout_secs,
                )
                .await;
                if last.status == execution_core::NodeStatus::Succeeded {
                    last.retry_count = attempt;
                    // Store result in cache when cache_ttl_secs is configured.
                    if cache_ttl_secs.is_some() {
                        if let Some(output) = &last.output_json {
                            let cache_key = node_cache_key(node, context);
                            let mut cache = node_cache().lock().unwrap();
                            cache.put(cache_key, (Instant::now(), output.clone()));
                        }
                    }
                    return last;
                }
                if attempt < max_retries {
                    // Exponential backoff. Base is configurable via retry_delay_ms (default 200).
                    let base = node_config_u64(node, "retry_delay_ms")
                        .unwrap_or(200)
                        .min(10_000);
                    let ms = base * (1u64 << attempt.min(5));
                    tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
                }
            }
            last.retry_count = max_retries;
            last
        })
    }
}

fn node_config_u64(node: &Node, key: &str) -> Option<u64> {
    node.config.as_ref()?.get(key)?.as_u64()
}

/// Run a CPU-bound *synchronous* node executor on Tokio's blocking pool.
///
/// The runtime executes every node in a graph level concurrently on a single
/// async task (`join_all`), so a long interpreted-script (Rhai) or regex
/// evaluation that ran inline would stall the whole level — the I/O-bound
/// siblings can't make progress while one CPU-bound node hogs the worker.
/// Offloading to `spawn_blocking` keeps the async worker free. Node/context are
/// cloned because the blocking closure must be `'static`.
async fn run_blocking(
    node: &Node,
    context: &ExecutionContext,
    f: fn(&Node, &ExecutionContext) -> NodeExecutionResult,
) -> NodeExecutionResult {
    let node = node.clone();
    let context = context.clone();
    match tokio::task::spawn_blocking(move || f(&node, &context)).await {
        Ok(result) => result,
        Err(e) => NodeExecutionResult::failed(format!("blocking node task failed: {e}")),
    }
}

async fn dispatch_with_timeout(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
    ai_runtime_base_url: Option<&str>,
    timeout_secs: Option<u64>,
) -> NodeExecutionResult {
    let fut = dispatch(node, context, http_client, ai_runtime_base_url);
    match timeout_secs {
        Some(secs) => match tokio::time::timeout(std::time::Duration::from_secs(secs), fut).await {
            Ok(result) => result,
            Err(_) => NodeExecutionResult::failed(format!("Node timed out after {secs}s")),
        },
        None => fut.await,
    }
}

fn is_external_node(nt: &NodeType) -> bool {
    !matches!(
        nt,
        NodeType::Trigger
            | NodeType::Condition
            | NodeType::Approval
            | NodeType::Map
            | NodeType::Filter
            | NodeType::Aggregate
            | NodeType::Sort
            | NodeType::Transform
            | NodeType::Assert
            | NodeType::Catch
            | NodeType::FanOut
            | NodeType::FanIn
            | NodeType::Code
            | NodeType::Extract
            | NodeType::Merge
            | NodeType::Loop
            | NodeType::Split
            | NodeType::Join
            | NodeType::Switch
            | NodeType::Random
            | NodeType::Dedupe
            | NodeType::Regex
            | NodeType::Csv
            | NodeType::Rename
            | NodeType::Format
            | NodeType::Date
            | NodeType::Handlebars
            | NodeType::Math
            | NodeType::ArrayUtils
            | NodeType::Xml
            | NodeType::Yaml
            | NodeType::Crypto
            | NodeType::Note
            | NodeType::Validate
            | NodeType::Delay
    )
}

async fn dispatch(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
    ai_runtime_base_url: Option<&str>,
) -> NodeExecutionResult {
    if context.dry_run && is_external_node(&node.node_type) {
        return NodeExecutionResult::succeeded(
            serde_json::json!({"dry_run": true, "note": "external call skipped in dry-run mode"})
                .to_string(),
        );
    }
    match node.node_type {
        NodeType::Trigger => execute_trigger(context),
        NodeType::Http => execute_http(node, context, http_client).await,
        NodeType::Agent => execute_agent(node, context, http_client, ai_runtime_base_url).await,
        NodeType::Rag => execute_rag(node, context, http_client, ai_runtime_base_url).await,
        NodeType::RagIngest => {
            execute_rag_ingest(node, context, http_client, ai_runtime_base_url).await
        }
        NodeType::Custom => execute_custom(node, context, http_client).await,
        NodeType::Condition => execute_condition(node, context),
        NodeType::Map => execute_map(node, context),
        NodeType::Filter => execute_filter(node, context),
        NodeType::Aggregate => execute_aggregate(node, context),
        NodeType::Sort => execute_sort(node, context),
        NodeType::Transform => execute_transform(node, context),
        NodeType::Delay => execute_delay(node).await,
        NodeType::SubWorkflow => execute_sub_workflow(node, context, ai_runtime_base_url).await,
        NodeType::Assert => execute_assert(node, context),
        NodeType::Catch => execute_catch(node, context),
        NodeType::FanOut => execute_fan_out(context),
        NodeType::FanIn => execute_fan_in(node, context),
        NodeType::Code => run_blocking(node, context, execute_code).await,
        NodeType::Slack => execute_slack(node, context, http_client).await,
        NodeType::Email => execute_email(node, context, http_client).await,
        NodeType::Openai => execute_openai(node, context, http_client).await,
        NodeType::Gemini => execute_gemini(node, context, http_client).await,
        NodeType::Database => execute_database(node, context).await,
        NodeType::Extract => execute_extract(node, context),
        NodeType::Merge => execute_merge(node, context),
        NodeType::Loop => execute_loop(node, context),
        NodeType::Graphql => execute_graphql(node, context, http_client).await,
        NodeType::Validate => execute_validate(node, context),
        NodeType::Note => NodeExecutionResult::succeeded(serde_json::json!({}).to_string()),
        NodeType::Claude => execute_claude(node, context, http_client).await,
        NodeType::Split => execute_split(node, context),
        NodeType::Join => execute_join(node, context),
        NodeType::Switch => execute_switch(node, context),
        NodeType::Random => execute_random(node, context),
        NodeType::Dedupe => execute_dedupe(node, context),
        NodeType::Regex => run_blocking(node, context, execute_regex).await,
        NodeType::Csv => execute_csv(node, context),
        NodeType::Rename => execute_rename(node, context),
        NodeType::Format => execute_format(node, context),
        NodeType::Github => execute_github(node, context, http_client).await,
        NodeType::Webhook => execute_webhook_send(node, context, http_client).await,
        NodeType::Jira => execute_jira(node, context, http_client).await,
        NodeType::Notion => execute_notion(node, context, http_client).await,
        NodeType::Linear => execute_linear(node, context, http_client).await,
        NodeType::Airtable => execute_airtable(node, context, http_client).await,
        NodeType::ForEach => execute_for_each(node, context, ai_runtime_base_url).await,
        NodeType::Discord => execute_discord(node, context, http_client).await,
        NodeType::Teams => execute_teams(node, context, http_client).await,
        NodeType::Sheets => execute_sheets(node, context, http_client).await,
        NodeType::Xml => execute_xml(node, context),
        NodeType::Yaml => execute_yaml(node, context),
        NodeType::Twilio => execute_twilio(node, context, http_client).await,
        NodeType::Stripe => execute_stripe(node, context, http_client).await,
        NodeType::Crypto => execute_crypto(node, context),
        NodeType::Hubspot => execute_hubspot(node, context, http_client).await,
        NodeType::Date => execute_date(node, context),
        NodeType::Zendesk => execute_zendesk(node, context, http_client).await,
        NodeType::Redis => execute_redis(node, context).await,
        NodeType::Elasticsearch => execute_elasticsearch(node, context, http_client).await,
        NodeType::Pagerduty => execute_pagerduty(node, context, http_client).await,
        NodeType::Handlebars => execute_handlebars(node, context),
        NodeType::Math => run_blocking(node, context, execute_math).await,
        NodeType::ArrayUtils => execute_array_utils(node, context),
        NodeType::Shopify => execute_shopify(node, context, http_client).await,
        NodeType::Datadog => execute_datadog(node, context, http_client).await,
        NodeType::Salesforce => execute_salesforce(node, context, http_client).await,
        NodeType::Freshdesk => execute_freshdesk(node, context, http_client).await,
        NodeType::Mailgun => execute_mailgun(node, context, http_client).await,
        NodeType::Asana => execute_asana(node, context, http_client).await,
        NodeType::Servicenow => execute_servicenow(node, context, http_client).await,
        NodeType::Confluence => execute_confluence(node, context, http_client).await,
        NodeType::Bitbucket => execute_bitbucket(node, context, http_client).await,
        NodeType::AzureDevops => execute_azure_devops(node, context, http_client).await,
        NodeType::Twitch => execute_twitch(node, context, http_client).await,
        NodeType::Figma => execute_figma(node, context, http_client).await,
        NodeType::Dropbox => execute_dropbox(node, context, http_client).await,
        NodeType::Cloudflare => execute_cloudflare(node, context, http_client).await,
        NodeType::Box => execute_box(node, context, http_client).await,
        NodeType::Okta => execute_okta(node, context, http_client).await,
        NodeType::Zoom => execute_zoom(node, context, http_client).await,
        NodeType::Spotify => execute_spotify(node, context, http_client).await,
        NodeType::Typeform => execute_typeform(node, context, http_client).await,
        NodeType::Webflow => execute_webflow(node, context, http_client).await,
        NodeType::Intercom => execute_intercom(node, context, http_client).await,
        NodeType::Pipedrive => execute_pipedrive(node, context, http_client).await,
        NodeType::Trello => execute_trello(node, context, http_client).await,
        NodeType::Monday => execute_monday(node, context, http_client).await,
        NodeType::Clickup => execute_clickup(node, context, http_client).await,
        NodeType::Amplitude => execute_amplitude(node, context, http_client).await,
        NodeType::Mixpanel => execute_mixpanel(node, context, http_client).await,
        NodeType::Segment => execute_segment(node, context, http_client).await,
        NodeType::Sendgrid => execute_sendgrid(node, context, http_client).await,
        NodeType::Braintree => execute_braintree(node, context, http_client).await,
        NodeType::Paypal => execute_paypal(node, context, http_client).await,
        NodeType::Razorpay => execute_razorpay(node, context, http_client).await,
        NodeType::Firebase => execute_firebase(node, context, http_client).await,
        NodeType::Supabase => execute_supabase(node, context, http_client).await,
        NodeType::Mailchimp => execute_mailchimp(node, context, http_client).await,
        NodeType::Activecampaign => execute_activecampaign(node, context, http_client).await,
        NodeType::Klaviyo => execute_klaviyo(node, context, http_client).await,
        NodeType::Resend => execute_resend(node, context, http_client).await,
        NodeType::Contentful => execute_contentful(node, context, http_client).await,
        NodeType::Algolia => execute_algolia(node, context, http_client).await,
        NodeType::Postmark => execute_postmark(node, context, http_client).await,
        NodeType::Vonage => execute_vonage(node, context, http_client).await,
        NodeType::Telegram => execute_telegram(node, context, http_client).await,
        NodeType::Replicate => execute_replicate(node, context, http_client).await,
        NodeType::Mistral => execute_mistral(node, context, http_client).await,
        NodeType::Whatsapp => execute_whatsapp(node, context, http_client).await,
        NodeType::Googledocs => execute_googledocs(node, context, http_client).await,
        NodeType::Perplexity => execute_perplexity(node, context, http_client).await,
        NodeType::Cohere => execute_cohere(node, context, http_client).await,
        NodeType::Googledrive => execute_googledrive(node, context, http_client).await,
        NodeType::Woocommerce => execute_woocommerce(node, context, http_client).await,
        NodeType::Pinecone => execute_pinecone(node, context, http_client).await,
        NodeType::Togetherai => execute_togetherai(node, context, http_client).await,
        NodeType::Awss3 => execute_awss3(node, context, http_client).await,
        NodeType::Huggingface => execute_huggingface(node, context, http_client).await,
        NodeType::Groq => execute_groq(node, context, http_client).await,
        NodeType::Openrouter => execute_openrouter(node, context, http_client).await,
        NodeType::Qdrant => execute_qdrant(node, context, http_client).await,
        NodeType::Cloudinary => execute_cloudinary(node, context, http_client).await,
        NodeType::Gcal => execute_gcal(node, context, http_client).await,
        NodeType::Docusign => execute_docusign(node, context, http_client).await,
        NodeType::Xero => execute_xero(node, context, http_client).await,
        NodeType::Calendly => execute_calendly(node, context, http_client).await,
        NodeType::Apify => execute_apify(node, context, http_client).await,
        NodeType::Ganalytics => execute_ganalytics(node, context, http_client).await,
        NodeType::Neon => execute_neon(node, context, http_client).await,
        NodeType::Copper => execute_copper(node, context, http_client).await,
        NodeType::AzureOpenai => execute_azure_openai(node, context, http_client).await,
        NodeType::Grok => execute_grok(node, context, http_client).await,
        NodeType::Ollama => execute_ollama(node, context, http_client).await,
        NodeType::Weaviate => execute_weaviate(node, context, http_client).await,
        NodeType::Chroma => execute_chroma(node, context, http_client).await,
        NodeType::Mongodb => execute_mongodb(node, context, http_client).await,
        NodeType::Clickhouse => execute_clickhouse(node, context, http_client).await,
        NodeType::Gcs => execute_gcs(node, context, http_client).await,
        NodeType::AzureBlob => execute_azure_blob(node, context, http_client).await,
        NodeType::Hash => execute_hash(node, context).await,
        NodeType::Jwt => execute_jwt(node, context).await,
        NodeType::Vertex => execute_vertex(node, context, http_client).await,
        NodeType::Sqs => execute_sqs(node, context, http_client).await,
        NodeType::Sns => execute_sns(node, context, http_client).await,
        NodeType::Bedrock => execute_bedrock(node, context, http_client).await,
        NodeType::Milvus => execute_milvus(node, context, http_client).await,
        NodeType::Kafka => execute_kafka(node, context, http_client).await,
        NodeType::Rabbitmq => execute_rabbitmq(node, context, http_client).await,
        NodeType::Zip => execute_zip(node, context).await,
        NodeType::Image => execute_image(node, context).await,
        NodeType::PdfExtract => execute_pdf_extract(node, context).await,
        NodeType::Ocr => execute_ocr(node, context).await,
        NodeType::Feishu => execute_feishu(node, context, http_client).await,
        NodeType::Dingtalk => execute_dingtalk(node, context, http_client).await,
        NodeType::Wecom => execute_wecom(node, context, http_client).await,
        NodeType::Embedding => execute_embedding(node, context, http_client).await,
        NodeType::Reranker => execute_reranker(node, context, http_client).await,
        NodeType::TextSplitter => execute_text_splitter(node, context).await,
        NodeType::StructuredOutput => execute_structured_output(node, context, http_client).await,
        NodeType::Classifier => execute_classifier(node, context, http_client).await,
        NodeType::ImageGen => execute_image_gen(node, context, http_client).await,
        NodeType::VideoGen => execute_video_gen(node, context, http_client).await,
        NodeType::SpeechToText => execute_speech_to_text(node, context, http_client).await,
        NodeType::Tts => execute_tts(node, context, http_client).await,
        NodeType::HtmlExtract => execute_html_extract(node, context).await,
        NodeType::Rss => execute_rss(node, context, http_client).await,
        NodeType::Mysql => execute_mysql(node, context).await,
        NodeType::Snowflake => execute_snowflake(node, context, http_client).await,
        NodeType::Bigquery => execute_bigquery(node, context, http_client).await,
        NodeType::Sqlserver => execute_sqlserver(node, context).await,
        NodeType::Ftp => execute_ftp(node, context).await,
        NodeType::Sftp => execute_sftp(node, context).await,
        NodeType::Ssh => execute_ssh(node, context).await,
        NodeType::Imap => execute_imap(node, context).await,
        NodeType::Deepseek => execute_deepseek(node, context, http_client).await,
        NodeType::Qwen => execute_qwen(node, context, http_client).await,
        NodeType::Zhipu => execute_zhipu(node, context, http_client).await,
        NodeType::Moonshot => execute_moonshot(node, context, http_client).await,
        NodeType::Doubao => execute_doubao(node, context, http_client).await,
        NodeType::Minimax => execute_minimax(node, context, http_client).await,
        NodeType::Ernie => execute_ernie(node, context, http_client).await,
        NodeType::Hunyuan => execute_hunyuan(node, context, http_client).await,
        // Approval is handled before dispatch; reaching here means no gate was configured.
        NodeType::Approval => NodeExecutionResult::failed("Approval gate not configured"),
        // Wait is handled before dispatch; reaching here means duration mode with no gate.
        NodeType::Wait => execute_wait(node, context, None).await,
    }
}

async fn execute_approval(context: &ExecutionContext, gate: &ApprovalGate) -> NodeExecutionResult {
    let rx = gate.register(context.execution_id.clone()).await;
    match rx.await {
        Ok(true) => NodeExecutionResult::succeeded(r#"{"approved":true}"#.to_string()),
        Ok(false) => NodeExecutionResult::failed("Rejected by approver".to_string()),
        Err(_) => NodeExecutionResult::failed("Approval gate was closed".to_string()),
    }
}

// Wait node: pause the run either for a fixed time ("duration" mode — sleep
// `seconds`, or until an absolute RFC3339 `until`) or until an external resume
// signal arrives ("resume" mode — reuses the approval gate, resolved via the
// /v1/executions/{id}/approve endpoint).
async fn execute_wait(
    node: &Node,
    context: &ExecutionContext,
    gate: Option<&ApprovalGate>,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let mode = cfg
        .get("mode")
        .and_then(|v| v.as_str())
        .unwrap_or("duration");

    match mode {
        "resume" => match gate {
            Some(gate) => {
                let rx = gate.register(context.execution_id.clone()).await;
                match rx.await {
                    // Any resolution resumes the run (the signal itself is the event).
                    Ok(_) => NodeExecutionResult::succeeded(
                        serde_json::json!({ "resumed": true, "mode": "resume" }).to_string(),
                    ),
                    Err(_) => NodeExecutionResult::failed("Wait gate was closed".to_string()),
                }
            }
            None => NodeExecutionResult::failed("Wait resume mode requires the resume gate"),
        },
        _ => {
            // Duration mode: prefer an absolute `until`, else relative `seconds`.
            let secs = if let Some(until) = cfg.get("until").and_then(|v| v.as_str()) {
                match chrono::DateTime::parse_from_rfc3339(until) {
                    Ok(dt) => {
                        let delta = dt.with_timezone(&chrono::Utc) - chrono::Utc::now();
                        delta.num_seconds().max(0) as u64
                    }
                    Err(e) => {
                        return NodeExecutionResult::failed(format!(
                            "Wait 'until' must be RFC3339: {e}"
                        ))
                    }
                }
            } else {
                cfg.get("seconds").and_then(|v| v.as_u64()).unwrap_or(0)
            };
            // Guard against pathologically long holds (cap at 30 days).
            let secs = secs.min(60 * 60 * 24 * 30);
            tokio::time::sleep(Duration::from_secs(secs)).await;
            NodeExecutionResult::succeeded(
                serde_json::json!({ "resumed": true, "mode": "duration", "waited_secs": secs })
                    .to_string(),
            )
        }
    }
}

fn execute_trigger(context: &ExecutionContext) -> NodeExecutionResult {
    NodeExecutionResult::succeeded(context.input_json.clone())
}

async fn execute_http(
    node: &Node,
    context: &ExecutionContext,
    client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw_config = match node.config.as_ref() {
        Some(c) => c,
        None => {
            return NodeExecutionResult::failed("Http node requires config with 'url' and 'method'")
        }
    };

    // Resolve {{...}} templates in all string values before use
    let config = resolve_config_strings(raw_config, context);

    let url = match config.get("url").and_then(|v| v.as_str()) {
        Some(u) => u.to_string(),
        None => return NodeExecutionResult::failed("Http node config missing 'url'"),
    };

    let method = config
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();

    let mut builder = match method.as_str() {
        "GET" => client.get(&url),
        "POST" => client.post(&url),
        "PUT" => client.put(&url),
        "PATCH" => client.patch(&url),
        "DELETE" => client.delete(&url),
        m => return NodeExecutionResult::failed(format!("Unsupported HTTP method: {m}")),
    };

    // Auth handling: bearer token or OAuth2 client credentials
    match config.get("auth_type").and_then(|v| v.as_str()) {
        Some("bearer") => {
            if let Some(token) = config.get("auth_token").and_then(|v| v.as_str()) {
                if !token.is_empty() {
                    builder = builder.bearer_auth(token);
                }
            }
        }
        Some("oauth2") => {
            let client_id = config
                .get("oauth2_client_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let client_secret = config
                .get("oauth2_client_secret")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let token_url = match config.get("oauth2_token_url").and_then(|v| v.as_str()) {
                Some(u) if !u.is_empty() => u,
                _ => return NodeExecutionResult::failed("OAuth2 node missing 'oauth2_token_url'"),
            };
            let scope = config
                .get("oauth2_scope")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let mut params: Vec<(&str, &str)> = vec![
                ("grant_type", "client_credentials"),
                ("client_id", client_id),
                ("client_secret", client_secret),
            ];
            if !scope.is_empty() {
                params.push(("scope", scope));
            }

            let token_resp = match client.post(token_url).form(&params).send().await {
                Ok(r) => r,
                Err(e) => {
                    return NodeExecutionResult::failed(format!("OAuth2 token request failed: {e}"))
                }
            };
            let token_status = token_resp.status();
            let token_body = token_resp.text().await.unwrap_or_default();
            if !token_status.is_success() {
                return NodeExecutionResult::failed(format!(
                    "OAuth2 token endpoint {token_status}: {token_body}"
                ));
            }
            let token_json: serde_json::Value = match serde_json::from_str(&token_body) {
                Ok(v) => v,
                Err(e) => {
                    return NodeExecutionResult::failed(format!("OAuth2 token parse error: {e}"))
                }
            };
            match token_json.get("access_token").and_then(|v| v.as_str()) {
                Some(token) => builder = builder.bearer_auth(token),
                None => {
                    return NodeExecutionResult::failed("OAuth2 response missing 'access_token'")
                }
            }
        }
        _ => {}
    }

    if let Some(headers) = config.get("headers").and_then(|v| v.as_object()) {
        for (key, value) in headers {
            if let Some(val) = value.as_str() {
                if let (Ok(name), Ok(hval)) = (
                    reqwest::header::HeaderName::from_bytes(key.as_bytes()),
                    reqwest::header::HeaderValue::from_str(val),
                ) {
                    builder = builder.header(name, hval);
                }
            }
        }
    }

    if let Some(body) = config.get("body").and_then(|v| v.as_str()) {
        builder = builder
            .header("content-type", "application/json")
            .body(body.to_string());
    }

    let fail_on_error = config
        .get("fail_on_error")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    match builder.send().await {
        Ok(response) => {
            let status = response.status();
            let status_code = status.as_u16();
            let headers_obj: serde_json::Map<String, serde_json::Value> = response
                .headers()
                .iter()
                .map(|(k, v)| {
                    (
                        k.as_str().to_string(),
                        serde_json::json!(v.to_str().unwrap_or("")),
                    )
                })
                .collect();
            match response.text().await {
                Ok(body_text) => {
                    let body_val: serde_json::Value = serde_json::from_str(&body_text)
                        .unwrap_or(serde_json::Value::String(body_text.clone()));
                    let output = serde_json::json!({
                        "status": status_code,
                        "headers": headers_obj,
                        "body": body_val,
                    })
                    .to_string();
                    if status.is_success() || !fail_on_error {
                        NodeExecutionResult::succeeded(output)
                    } else {
                        NodeExecutionResult::failed(format!("HTTP {status_code}: {body_text}"))
                    }
                }
                Err(e) => NodeExecutionResult::failed(format!("Failed to read response: {e}")),
            }
        }
        Err(e) => NodeExecutionResult::failed(format!("HTTP request failed: {e}")),
    }
}

#[derive(Debug, Serialize)]
struct AgentNodeRequest {
    node_id: String,
    node_config: serde_json::Value,
    input_json: String,
    node_outputs: std::collections::HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct AgentNodeResponse {
    output_json: String,
}

async fn execute_agent(
    node: &Node,
    context: &ExecutionContext,
    client: &reqwest::Client,
    ai_runtime_base_url: Option<&str>,
) -> NodeExecutionResult {
    let base_url = match ai_runtime_base_url {
        Some(url) => url,
        None => {
            return NodeExecutionResult::failed(
                "Agent node requires AI_RUNTIME_BASE_URL to be configured",
            )
        }
    };

    let config = node
        .config
        .clone()
        .unwrap_or_else(|| serde_json::Value::Object(Default::default()));

    let endpoint = format!("{}/v1/nodes/agent", base_url.trim_end_matches('/'));

    let request = AgentNodeRequest {
        node_id: node.id.clone(),
        node_config: config,
        input_json: context.input_json.clone(),
        node_outputs: context.node_outputs.clone(),
    };

    match client.post(&endpoint).json(&request).send().await {
        Ok(response) => {
            if response.status().is_success() {
                match response.json::<AgentNodeResponse>().await {
                    Ok(payload) => NodeExecutionResult::succeeded(payload.output_json),
                    Err(e) => {
                        NodeExecutionResult::failed(format!("Failed to parse agent response: {e}"))
                    }
                }
            } else {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                NodeExecutionResult::failed(format!("AI Runtime returned {status}: {body}"))
            }
        }
        Err(e) => NodeExecutionResult::failed(format!("Failed to reach AI Runtime: {e}")),
    }
}

/// Convert f64 to a serde_json Number (integer if whole, float otherwise).
fn json_number(v: f64) -> serde_json::Value {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        serde_json::Value::Number((v as i64).into())
    } else {
        serde_json::Number::from_f64(v)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null)
    }
}

async fn execute_delay(node: &Node) -> NodeExecutionResult {
    let config = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Delay node requires config with 'seconds'"),
    };
    let seconds = match config.get("seconds").and_then(|v| v.as_u64()) {
        Some(s) => s.min(3600), // cap at 1 hour
        None => return NodeExecutionResult::failed("Delay node config missing 'seconds'"),
    };
    if seconds > 0 {
        tokio::time::sleep(std::time::Duration::from_secs(seconds)).await;
    }
    NodeExecutionResult::succeeded(serde_json::json!({ "waited_secs": seconds }).to_string())
}

async fn execute_sub_workflow(
    node: &Node,
    context: &ExecutionContext,
    ai_runtime_base_url: Option<&str>,
) -> NodeExecutionResult {
    let config = match node.config.as_ref() {
        Some(c) => c,
        None => {
            return NodeExecutionResult::failed("SubWorkflow node requires config with '_graph'")
        }
    };

    let sub_graph: workflow_core::WorkflowGraph = match config.get("_graph") {
        Some(g) => match serde_json::from_value(g.clone()) {
            Ok(graph) => graph,
            Err(e) => {
                return NodeExecutionResult::failed(format!(
                    "SubWorkflow node: invalid '_graph': {e}"
                ))
            }
        },
        None => {
            return NodeExecutionResult::failed(
                "SubWorkflow node missing '_graph' — platform must inject it before execution",
            )
        }
    };

    // Resolve input for sub-execution: if 'input_template' is set, render it; otherwise pass through
    let sub_input = match config.get("input_template") {
        Some(template) => resolve_config_strings(template, context).to_string(),
        None => context.input_json.clone(),
    };

    let sub_execution_id = format!("{}:sub:{}", context.execution_id, node.id);
    let sub_executor = DispatchingNodeExecutor::new(ai_runtime_base_url.map(str::to_owned));

    match crate::runtime::run_workflow(
        &sub_execution_id,
        &sub_graph,
        sub_input,
        &sub_executor,
        context.dry_run,
    )
    .await
    {
        Ok(report) => {
            let last_output = report
                .node_results
                .iter()
                .rev()
                .find(|r| r.status == execution_core::NodeStatus::Succeeded)
                .and_then(|r| r.output_json.as_deref());
            let output_val = match last_output {
                Some(s) => {
                    serde_json::from_str(s).unwrap_or(serde_json::Value::String(s.to_owned()))
                }
                None => serde_json::Value::Null,
            };
            let result_json = serde_json::json!({
                "status": format!("{:?}", report.status).to_lowercase(),
                "output": output_val,
            });
            if report.status == execution_core::ExecutionStatus::Succeeded {
                NodeExecutionResult::succeeded(result_json.to_string())
            } else {
                NodeExecutionResult::failed(format!("Sub-workflow failed: {result_json}"))
            }
        }
        Err(e) => NodeExecutionResult::failed(format!("SubWorkflow execution error: {e:?}")),
    }
}

fn is_truthy(s: &str) -> bool {
    !matches!(s, "" | "false" | "null" | "0" | "[]" | "{}")
}

async fn execute_database(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    let config = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Database node requires config"),
    };
    let url = match config.get("url").and_then(|v| v.as_str()) {
        Some(u) => resolve_template(u, context),
        None => return NodeExecutionResult::failed("Database node missing 'url'"),
    };
    let query_str = match config.get("query").and_then(|v| v.as_str()) {
        Some(q) => resolve_template(q, context),
        None => return NodeExecutionResult::failed("Database node missing 'query'"),
    };

    let pool = match sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&url)
        .await
    {
        Ok(p) => p,
        Err(e) => return NodeExecutionResult::failed(format!("DB connect error: {e}")),
    };

    let trimmed = query_str.trim().to_ascii_uppercase();
    let is_select = trimmed.starts_with("SELECT") || trimmed.starts_with("WITH");

    if is_select {
        match sqlx::query(&query_str).fetch_all(&pool).await {
            Ok(rows) => {
                let json_rows: Vec<serde_json::Value> = rows
                    .iter()
                    .map(|row| {
                        let mut obj = serde_json::Map::new();
                        for (i, col) in row.columns().iter().enumerate() {
                            let val: serde_json::Value = row
                                .try_get::<i64, _>(i)
                                .map(|v| serde_json::json!(v))
                                .or_else(|_| row.try_get::<f64, _>(i).map(|v| serde_json::json!(v)))
                                .or_else(|_| {
                                    row.try_get::<bool, _>(i).map(|v| serde_json::json!(v))
                                })
                                .or_else(|_| {
                                    row.try_get::<String, _>(i).map(|v| serde_json::json!(v))
                                })
                                .unwrap_or(serde_json::Value::Null);
                            obj.insert(col.name().to_string(), val);
                        }
                        serde_json::Value::Object(obj)
                    })
                    .collect();
                let count = json_rows.len();
                NodeExecutionResult::succeeded(
                    serde_json::json!({ "rows": json_rows, "count": count }).to_string(),
                )
            }
            Err(e) => NodeExecutionResult::failed(format!("DB query error: {e}")),
        }
    } else {
        match sqlx::query(&query_str).execute(&pool).await {
            Ok(result) => NodeExecutionResult::succeeded(
                serde_json::json!({ "rows_affected": result.rows_affected() }).to_string(),
            ),
            Err(e) => NodeExecutionResult::failed(format!("DB execute error: {e}")),
        }
    }
}

fn execute_code(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    let script = match node
        .config
        .as_ref()
        .and_then(|c| c.get("script").and_then(|v| v.as_str()))
    {
        Some(s) => s,
        None => return NodeExecutionResult::failed("Code node requires 'script' config"),
    };

    // Resolve {{...}} template expressions inside the script before execution.
    let resolved_script = resolve_template(script, context);

    let mut engine = rhai::Engine::new();
    engine.set_max_operations(100_000);
    engine.set_max_string_size(1_000_000);
    // Disable file/module loading for sandboxing.
    engine.set_module_resolver(rhai::module_resolvers::DummyModuleResolver::new());

    let mut scope = rhai::Scope::new();

    // Expose `input` as a parsed Rhai map.
    let input_val: serde_json::Value =
        serde_json::from_str(&context.input_json).unwrap_or(serde_json::Value::Null);
    if let Ok(dyn_input) = rhai::serde::to_dynamic(input_val) {
        scope.push("input", dyn_input);
    }

    // Expose `nodes` map: nodes["node_id"]["field"].
    let mut nodes_map = rhai::Map::new();
    for (node_id, output_json) in &context.node_outputs {
        let val: serde_json::Value =
            serde_json::from_str(output_json).unwrap_or(serde_json::Value::Null);
        if let Ok(d) = rhai::serde::to_dynamic(val) {
            nodes_map.insert(node_id.clone().into(), d);
        }
    }
    scope.push("nodes", rhai::Dynamic::from(nodes_map));

    match engine.eval_with_scope::<rhai::Dynamic>(&mut scope, &resolved_script) {
        Ok(result) => match rhai::serde::from_dynamic::<serde_json::Value>(&result) {
            Ok(json_val) => NodeExecutionResult::succeeded(json_val.to_string()),
            Err(e) => NodeExecutionResult::failed(format!("Code result not serializable: {e}")),
        },
        Err(e) => NodeExecutionResult::failed(format!("Code error: {e}")),
    }
}

fn execute_fan_out(context: &ExecutionContext) -> NodeExecutionResult {
    // Pass the current input through to all outgoing branches.
    let input: serde_json::Value =
        serde_json::from_str(&context.input_json).unwrap_or(serde_json::Value::Null);
    NodeExecutionResult::succeeded(serde_json::json!({ "ok": true, "input": input }).to_string())
}

fn execute_fan_in(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    // _sources is injected by run_workflow before dispatch.
    let sources: Vec<String> = node
        .config
        .as_ref()
        .and_then(|c| c.get("_sources"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let results: Vec<serde_json::Value> = sources
        .iter()
        .filter_map(|src| {
            context
                .node_outputs
                .get(src)
                .and_then(|out| serde_json::from_str(out).ok())
        })
        .collect();

    let count = results.len();
    NodeExecutionResult::succeeded(
        serde_json::json!({ "results": results, "count": count }).to_string(),
    )
}

fn execute_catch(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    // Collect error messages from any upstream node that has {"failed": true} in its output.
    let source_hint = node
        .config
        .as_ref()
        .and_then(|c| c.get("source").and_then(|v| v.as_str()))
        .unwrap_or("");

    let error_msg = if !source_hint.is_empty() {
        // Explicit source configured: read {{source.error}}.
        let key = format!("{{{{{}.error}}}}", source_hint);
        resolve_template(&key, context)
    } else {
        // Auto-detect: find the first upstream node output that has "failed: true".
        context
            .node_outputs
            .values()
            .find_map(|out| {
                let v: serde_json::Value = serde_json::from_str(out).ok()?;
                if v.get("failed").and_then(|f| f.as_bool()).unwrap_or(false) {
                    v.get("error").and_then(|e| e.as_str()).map(String::from)
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "unknown error".to_string())
    };

    NodeExecutionResult::succeeded(
        serde_json::json!({ "caught": true, "error": error_msg }).to_string(),
    )
}

fn execute_assert(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    let config = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Assert node requires config with 'condition'"),
    };
    let condition_expr = match config.get("condition").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return NodeExecutionResult::failed("Assert node config missing 'condition'"),
    };
    let message = config
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or("Assertion failed");
    let resolved = resolve_template(condition_expr, context);
    if is_truthy(&resolved) {
        NodeExecutionResult::succeeded(serde_json::json!({ "ok": true }).to_string())
    } else {
        NodeExecutionResult::failed(message)
    }
}

/// Evaluate a comparison operator. `actual` is the resolved field value (None if
/// the field was absent); `expected` is the resolved comparison value.
fn eval_condition_op(op: &str, actual: Option<&str>, expected: &str) -> bool {
    let a = actual.unwrap_or("");
    match op {
        "exists" => actual.is_some(),
        "not_exists" => actual.is_none(),
        "equals" => a == expected,
        "not_equals" => a != expected,
        "contains" => a.contains(expected),
        "not_contains" => !a.contains(expected),
        "gt" | "lt" | "gte" | "lte" => {
            match (a.trim().parse::<f64>(), expected.trim().parse::<f64>()) {
                (Ok(x), Ok(y)) => match op {
                    "gt" => x > y,
                    "lt" => x < y,
                    "gte" => x >= y,
                    _ => x <= y,
                },
                _ => false,
            }
        }
        _ => false,
    }
}

fn execute_condition(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    let config = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Condition node requires config with 'field'"),
    };

    let field_raw = match config.get("field").and_then(|v| v.as_str()) {
        Some(f) => f,
        None => return NodeExecutionResult::failed("Condition node config missing 'field'"),
    };

    // Resolve the value to test. Priority:
    //  1. `source` template (an upstream node or input), then `field` as a dot-path into it
    //  2. `field` itself as a template expression (e.g. "{{node.value}}")
    //  3. `field` as a key looked up in input_json
    let check_value: Option<String> =
        if let Some(source) = config.get("source").and_then(|v| v.as_str()) {
            let resolved = resolve_template(source, context);
            let json: serde_json::Value =
                serde_json::from_str(&resolved).unwrap_or(serde_json::Value::Null);
            json_path(&json, field_raw).map(json_to_string)
        } else if field_raw.contains("{{") {
            let resolved = resolve_template(field_raw, context);
            if resolved.is_empty() {
                None
            } else {
                Some(resolved)
            }
        } else {
            let input: serde_json::Value = match serde_json::from_str(&context.input_json) {
                Ok(v) => v,
                Err(_) => {
                    return NodeExecutionResult::failed("Condition node could not parse input_json")
                }
            };
            json_path(&input, field_raw).map(json_to_string)
        };

    // Determine the result. Priority: operator+value, then equals, then existence.
    let result = if let Some(op) = config.get("operator").and_then(|v| v.as_str()) {
        let expected = config
            .get("value")
            .and_then(|v| v.as_str())
            .map(|v| resolve_template(v, context))
            .unwrap_or_default();
        eval_condition_op(op, check_value.as_deref(), &expected)
    } else if let Some(expected) = config.get("equals").and_then(|v| v.as_str()) {
        let expected_resolved = resolve_template(expected, context);
        check_value.as_deref() == Some(expected_resolved.as_str())
    } else {
        check_value.is_some()
    };

    NodeExecutionResult::succeeded(
        serde_json::json!({ "result": result, "field": field_raw }).to_string(),
    )
}

// ── Extract node ─────────────────────────────────────────────────────────────
// Extracts a value from a JSON source using a dot-path expression.
// Config: source (template expression), path (dot-path like "data.users.0.email")
// Returns: { "value": <extracted>, "found": bool }

// ── Merge node ────────────────────────────────────────────────────────────────
// Combines fields from multiple node outputs into a single flat object.
// Config: fields = [ { source: "{{node_id}}", key: "alias" }, ... ]
//   - source: template expression (defaults to resolving the whole node output)
//   - key: output key name; if omitted, merges all top-level fields
// Returns: the merged object

// ── Loop node ─────────────────────────────────────────────────────────────────
// Iterates over an items array, applying a template to each element.
// Similar to Map but with a max_iterations cap and stops early when condition fails.
// Config: items (template expr resolving to array), template (object applied per item),
//         max_iterations (default 100), until (dot-path in item; stops when falsy)
// Returns: { count: N, results: [...] }

// ── GraphQL node ──────────────────────────────────────────────────────────────
// Sends a GraphQL query or mutation to an endpoint.
// Config: url, query (GraphQL document string), variables (JSON object template),
//         headers (JSON object), auth_type (none/bearer), bearer_token
// Returns: { data: ..., errors: [...] } from the GraphQL response
async fn execute_graphql(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let config = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("GraphQL node requires config"),
    };
    let url = match config.get("url").and_then(|v| v.as_str()) {
        Some(u) => resolve_template(u, context),
        None => return NodeExecutionResult::failed("GraphQL node missing 'url'"),
    };
    let query = match config.get("query").and_then(|v| v.as_str()) {
        Some(q) => resolve_template(q, context),
        None => return NodeExecutionResult::failed("GraphQL node missing 'query'"),
    };

    // Resolve variables template
    let variables: serde_json::Value = if let Some(vars) = config.get("variables") {
        let vars_str = resolve_config_strings(vars, context).to_string();
        serde_json::from_str(&vars_str).unwrap_or(serde_json::Value::Object(Default::default()))
    } else {
        serde_json::Value::Object(Default::default())
    };

    let body = serde_json::json!({ "query": query, "variables": variables });

    let mut builder = http_client.post(&url).json(&body);

    // Auth
    if let Some(token) = config.get("bearer_token").and_then(|v| v.as_str()) {
        let t = resolve_template(token, context);
        if !t.is_empty() {
            builder = builder.bearer_auth(t);
        }
    }

    // Custom headers
    if let Some(headers) = config.get("headers").and_then(|v| v.as_object()) {
        for (k, v) in headers {
            if let Some(val) = v.as_str() {
                let resolved = resolve_template(val, context);
                if let Ok(name) = reqwest::header::HeaderName::from_bytes(k.as_bytes()) {
                    if let Ok(hval) = reqwest::header::HeaderValue::from_str(&resolved) {
                        builder = builder.header(name, hval);
                    }
                }
            }
        }
    }

    match builder.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            match resp.json::<serde_json::Value>().await {
                Ok(json) => {
                    let has_errors = json.get("errors").map(|e| !e.is_null()).unwrap_or(false);
                    if has_errors {
                        NodeExecutionResult::failed(format!(
                            "GraphQL errors: {}",
                            json.get("errors").unwrap_or(&serde_json::Value::Null)
                        ))
                    } else {
                        NodeExecutionResult::succeeded(json.to_string())
                    }
                }
                Err(e) => NodeExecutionResult::failed(format!(
                    "GraphQL response parse error ({status}): {e}"
                )),
            }
        }
        Err(e) => NodeExecutionResult::failed(format!("GraphQL request failed: {e}")),
    }
}

// Validates a JSON value against a simple schema definition.
// Config: `source` (template resolving to JSON), `schema` (JSON object with field definitions),
//         `fail_on_invalid` (bool, default true).
// Schema format: `{ "field_name": { "type": "string"|"number"|"boolean"|"array"|"object", "required": true } }`
// Returns: `{ valid: bool, errors: [...] }`

fn json_type_name(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

// GitHub REST API node.
// config: token (required), method (GET/POST/PATCH/DELETE, default GET),
//         endpoint (required, e.g. "/repos/{owner}/{repo}/issues"),
//         body (optional JSON template), base_url (default https://api.github.com)

// Outbound webhook send node — send an HTTP POST to an arbitrary URL.
// config: url (required), headers (optional object), body_template (optional JSON template)

// Jira integration node — calls Jira REST API v3 using Basic auth (email:token).
// config: base_url (required, e.g. https://company.atlassian.net), email (required),
//         token (required, API token), endpoint (required, e.g. /rest/api/3/issue/PROJ-1),
//         method (GET/POST/PUT/DELETE, default GET), body (optional JSON template)

// Notion integration node — calls Notion REST API v1 using Bearer token.
// config: token (required, Notion integration token), endpoint (required, e.g. /v1/pages),
//         method (GET/POST/PATCH/DELETE, default GET), body (optional JSON template)

// Linear integration node — calls Linear GraphQL API using Bearer token (API key).
// config: token (required), query (required, GraphQL query string),
//         variables (optional JSON object template)

// Airtable integration node — calls Airtable REST API using Bearer token (personal access token).
// config: token (required), base_id (required), table (required),
//         method (GET/POST/PATCH/DELETE, default GET), record_id (optional for single-record ops),
//         body (optional JSON template for writes), filter_formula (optional for GET list)

/// ForEach node — runs a sub-workflow for each item in an array in parallel.
/// config: items (template expression resolving to array, required),
///         _graph (injected by platform — same mechanism as SubWorkflow),
///         input_key (optional: key to set per item, default "item"),
///         max_concurrency (optional cap on parallel runners, default 10)
/// Returns: { results: [{status, output, item}], succeeded, failed, total }
async fn execute_for_each(
    node: &Node,
    context: &ExecutionContext,
    ai_runtime_base_url: Option<&str>,
) -> NodeExecutionResult {
    let cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("ForEach node requires config"),
    };

    // Resolve the items array
    let items_tmpl = cfg
        .get("items")
        .and_then(|v| v.as_str())
        .unwrap_or("{{input}}");
    let items_str = resolve_template(items_tmpl, context);
    let items: Vec<serde_json::Value> = match serde_json::from_str(&items_str) {
        Ok(serde_json::Value::Array(arr)) => arr,
        _ => {
            return NodeExecutionResult::failed(format!(
                "ForEach 'items' must resolve to a JSON array, got: {items_str}"
            ))
        }
    };

    if items.is_empty() {
        return NodeExecutionResult::succeeded(
            serde_json::json!({ "results": [], "succeeded": 0, "failed": 0, "total": 0 })
                .to_string(),
        );
    }

    // Get the sub-graph (injected by platform, same as SubWorkflow)
    let sub_graph: workflow_core::WorkflowGraph =
        match cfg.get("_graph") {
            Some(g) => match serde_json::from_value(g.clone()) {
                Ok(graph) => graph,
                Err(e) => {
                    return NodeExecutionResult::failed(format!("ForEach: invalid '_graph': {e}"))
                }
            },
            None => return NodeExecutionResult::failed(
                "ForEach node missing '_graph' — set 'workflow_id' and the platform will inject it",
            ),
        };

    let input_key = cfg
        .get("input_key")
        .and_then(|v| v.as_str())
        .unwrap_or("item");
    let max_concurrency = cfg
        .get("max_concurrency")
        .and_then(|v| v.as_u64())
        .unwrap_or(10) as usize;
    let total = items.len();
    let ai_base = ai_runtime_base_url.map(str::to_owned);

    // Process items in batches of max_concurrency
    let mut all_results: Vec<serde_json::Value> = Vec::with_capacity(total);
    for chunk in items.chunks(max_concurrency) {
        let tasks: Vec<_> = chunk
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let sub_graph = sub_graph.clone();
                let item = item.clone();
                let ai_base = ai_base.clone();
                let exec_id = format!("{}:foreach:{}:{i}", context.execution_id, node.id);
                let input_key = input_key.to_owned();
                async move {
                    let item_input = serde_json::json!({ input_key: item }).to_string();
                    let executor = DispatchingNodeExecutor::new(ai_base);
                    match crate::runtime::run_workflow(
                        &exec_id,
                        &sub_graph,
                        item_input,
                        &executor,
                        context.dry_run,
                    )
                    .await
                    {
                        Ok(report) => {
                            let last_output = report
                                .node_results
                                .iter()
                                .rev()
                                .find(|r| r.status == execution_core::NodeStatus::Succeeded)
                                .and_then(|r| r.output_json.as_deref());
                            let output = match last_output {
                                Some(s) => serde_json::from_str(s)
                                    .unwrap_or(serde_json::Value::String(s.to_owned())),
                                None => serde_json::Value::Null,
                            };
                            let ok = report.status == execution_core::ExecutionStatus::Succeeded;
                            serde_json::json!({
                                "status": if ok { "succeeded" } else { "failed" },
                                "output": output,
                                "item": item,
                            })
                        }
                        Err(e) => serde_json::json!({
                            "status": "failed",
                            "error": format!("{e:?}"),
                            "item": item,
                        }),
                    }
                }
            })
            .collect();

        let batch: Vec<serde_json::Value> = futures::future::join_all(tasks).await;
        all_results.extend(batch);
    }

    let succeeded = all_results
        .iter()
        .filter(|r| r["status"] == "succeeded")
        .count();
    let failed = total - succeeded;

    NodeExecutionResult::succeeded(
        serde_json::json!({
            "results": all_results,
            "succeeded": succeeded,
            "failed": failed,
            "total": total,
        })
        .to_string(),
    )
}

// Discord notification node — sends a message to a Discord channel via an incoming webhook.
// config: webhook_url (required), content (required, message text template),
//         username (optional override), avatar_url (optional)

// Microsoft Teams notification node — sends an Adaptive Card message via an incoming webhook.
// config: webhook_url (required), title (optional), text (required, message body template),
//         color (optional hex, e.g. "#0078D4")

// Google Sheets node — reads or writes Google Sheets cells via Sheets API v4.
// Uses a Bearer token (OAuth2 access token or service account token).
// config: token (required), spreadsheet_id (required),
//         range (required, A1 notation e.g. "Sheet1!A1:C10"),
//         method (GET/APPEND/UPDATE/CLEAR, default GET),
//         values (optional JSON array of rows for APPEND/UPDATE),
//         value_input_option (RAW/USER_ENTERED, default USER_ENTERED)

fn redis_value_to_json(val: redis::Value) -> serde_json::Value {
    match val {
        redis::Value::Nil => serde_json::Value::Null,
        redis::Value::Int(n) => serde_json::json!(n),
        redis::Value::Double(f) => serde_json::json!(f),
        redis::Value::Boolean(b) => serde_json::json!(b),
        redis::Value::BulkString(bytes) => {
            let s = String::from_utf8_lossy(&bytes).into_owned();
            serde_json::from_str::<serde_json::Value>(&s).unwrap_or(serde_json::Value::String(s))
        }
        redis::Value::Array(vals) | redis::Value::Set(vals) => {
            serde_json::Value::Array(vals.into_iter().map(redis_value_to_json).collect())
        }
        redis::Value::Map(pairs) => {
            let mut map = serde_json::Map::new();
            for (k, v) in pairs {
                let key_str = match k {
                    redis::Value::BulkString(b) => String::from_utf8_lossy(&b).into_owned(),
                    redis::Value::SimpleString(s) => s,
                    other => redis_value_to_json(other).to_string(),
                };
                map.insert(key_str, redis_value_to_json(v));
            }
            serde_json::Value::Object(map)
        }
        redis::Value::SimpleString(s) => serde_json::Value::String(s),
        redis::Value::Okay => serde_json::Value::String("OK".to_string()),
        _ => serde_json::Value::Null,
    }
}

// Shared by late-stage integration nodes (URL query encoding).
fn urlencoding_simple(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            ' ' => "+".to_string(),
            c => format!("%{:02X}", c as u32),
        })
        .collect()
}

#[cfg(test)]
mod dispatch_tests;
