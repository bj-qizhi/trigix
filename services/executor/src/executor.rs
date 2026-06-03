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

// Late-stage integrations (slices 274+) live in a real submodule (executor/late.rs)
// instead of being textually `include!`d into this file.
mod late;
use late::*;

// Chinese-vendor LLM nodes extracted into their own submodule.
mod nodes_cn_llm;
use nodes_cn_llm::*;

// SaaS integration nodes extracted into their own submodule.
mod nodes_integrations;
use nodes_integrations::*;

// Data-transform / utility nodes extracted into their own submodule.
mod nodes_transform;
use nodes_transform::*;

// Western LLM nodes (OpenAI / Gemini / Claude) extracted into their own submodule.
mod nodes_ai;
use nodes_ai::*;

// Global per-process node output cache: key → (cached_at, output_json)
static NODE_CACHE: OnceLock<Arc<Mutex<LruCache<String, (Instant, String)>>>> = OnceLock::new();

fn node_cache() -> &'static Arc<Mutex<LruCache<String, (Instant, String)>>> {
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
        NodeType::Code => execute_code(node, context),
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
        NodeType::Regex => execute_regex(node, context),
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
        NodeType::Math => execute_math(node, context),
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

fn execute_condition(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    let config = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Condition node requires config with 'field'"),
    };

    let field_raw = match config.get("field").and_then(|v| v.as_str()) {
        Some(f) => f,
        None => return NodeExecutionResult::failed("Condition node config missing 'field'"),
    };

    // If field is a template expression (e.g. "{{trigger.status}}"), resolve it to the
    // actual value to compare. Otherwise look the field up as a key in input_json.
    let check_value: Option<String> = if field_raw.contains("{{") {
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

    let equals_raw = config.get("equals").and_then(|v| v.as_str());
    let result = match equals_raw {
        Some(expected) => {
            let expected_resolved = resolve_template(expected, context);
            check_value.as_deref() == Some(expected_resolved.as_str())
        }
        None => check_value.is_some(),
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

/// Validates a JSON value against a simple schema definition.
/// Config: `source` (template resolving to JSON), `schema` (JSON object with field definitions),
///         `fail_on_invalid` (bool, default true).
/// Schema format: `{ "field_name": { "type": "string"|"number"|"boolean"|"array"|"object", "required": true } }`
/// Returns: `{ valid: bool, errors: [...] }`

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

/// GitHub REST API node.
/// config: token (required), method (GET/POST/PATCH/DELETE, default GET),
///         endpoint (required, e.g. "/repos/{owner}/{repo}/issues"),
///         body (optional JSON template), base_url (default https://api.github.com)

/// Outbound webhook send node — send an HTTP POST to an arbitrary URL.
/// config: url (required), headers (optional object), body_template (optional JSON template)

/// Jira integration node — calls Jira REST API v3 using Basic auth (email:token).
/// config: base_url (required, e.g. https://company.atlassian.net), email (required),
///         token (required, API token), endpoint (required, e.g. /rest/api/3/issue/PROJ-1),
///         method (GET/POST/PUT/DELETE, default GET), body (optional JSON template)

/// Notion integration node — calls Notion REST API v1 using Bearer token.
/// config: token (required, Notion integration token), endpoint (required, e.g. /v1/pages),
///         method (GET/POST/PATCH/DELETE, default GET), body (optional JSON template)

/// Linear integration node — calls Linear GraphQL API using Bearer token (API key).
/// config: token (required), query (required, GraphQL query string),
///         variables (optional JSON object template)

/// Airtable integration node — calls Airtable REST API using Bearer token (personal access token).
/// config: token (required), base_id (required), table (required),
///         method (GET/POST/PATCH/DELETE, default GET), record_id (optional for single-record ops),
///         body (optional JSON template for writes), filter_formula (optional for GET list)

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

/// Discord notification node — sends a message to a Discord channel via an incoming webhook.
/// config: webhook_url (required), content (required, message text template),
///         username (optional override), avatar_url (optional)

/// Microsoft Teams notification node — sends an Adaptive Card message via an incoming webhook.
/// config: webhook_url (required), title (optional), text (required, message body template),
///         color (optional hex, e.g. "#0078D4")

/// Google Sheets node — reads or writes Google Sheets cells via Sheets API v4.
/// Uses a Bearer token (OAuth2 access token or service account token).
/// config: token (required), spreadsheet_id (required),
///         range (required, A1 notation e.g. "Sheet1!A1:C10"),
///         method (GET/APPEND/UPDATE/CLEAR, default GET),
///         values (optional JSON array of rows for APPEND/UPDATE),
///         value_input_option (RAW/USER_ENTERED, default USER_ENTERED)

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

#[cfg(test)]
mod tests {
    use super::*;
    use workflow_core::NodeType;

    fn make_context(input_json: &str) -> ExecutionContext {
        ExecutionContext {
            execution_id: "exec-1".to_string(),
            workflow_version_id: "ver-1".to_string(),
            input_json: input_json.to_string(),
            node_outputs: Default::default(),
            dry_run: false,
        }
    }

    #[tokio::test]
    async fn trigger_returns_input() {
        let executor = DispatchingNodeExecutor::new(None);
        let node = Node {
            id: "trigger".to_string(),
            node_type: NodeType::Trigger,
            config: None,
        };
        let context = make_context(r#"{"lead_id":"lead-1"}"#);

        let result = executor.execute(&node, &context).await;

        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
        assert_eq!(
            result.output_json.as_deref(),
            Some(r#"{"lead_id":"lead-1"}"#)
        );
    }

    #[tokio::test]
    async fn http_node_requires_config() {
        let executor = DispatchingNodeExecutor::new(None);
        let node = Node {
            id: "http".to_string(),
            node_type: NodeType::Http,
            config: None,
        };
        let context = make_context("{}");

        let result = executor.execute(&node, &context).await;

        assert_eq!(result.status, execution_core::NodeStatus::Failed);
    }

    #[tokio::test]
    async fn agent_node_fails_without_runtime_url() {
        let executor = DispatchingNodeExecutor::new(None);
        let node = Node {
            id: "agent".to_string(),
            node_type: NodeType::Agent,
            config: None,
        };
        let context = make_context("{}");

        let result = executor.execute(&node, &context).await;

        assert_eq!(result.status, execution_core::NodeStatus::Failed);
        assert!(result
            .error
            .as_deref()
            .unwrap_or("")
            .contains("AI_RUNTIME_BASE_URL"));
    }

    #[tokio::test]
    async fn condition_node_evaluates_field_presence() {
        let executor = DispatchingNodeExecutor::new(None);
        let node = Node {
            id: "condition".to_string(),
            node_type: NodeType::Condition,
            config: Some(serde_json::json!({ "field": "status" })),
        };
        let context = make_context(r#"{"status":"active"}"#);

        let result = executor.execute(&node, &context).await;

        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
        let output: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(output["result"], true);
    }

    #[test]
    fn node_config_u64_extracts_values() {
        let node = Node {
            id: "n".to_string(),
            node_type: NodeType::Http,
            config: Some(serde_json::json!({"max_retries": 3, "timeout_secs": 30})),
        };
        assert_eq!(node_config_u64(&node, "max_retries"), Some(3));
        assert_eq!(node_config_u64(&node, "timeout_secs"), Some(30));
        assert_eq!(node_config_u64(&node, "missing"), None);
    }

    #[tokio::test]
    async fn max_retries_zero_succeeds_on_first_attempt() {
        let executor = DispatchingNodeExecutor::new(None);
        let node = Node {
            id: "trigger".to_string(),
            node_type: NodeType::Trigger,
            config: Some(serde_json::json!({"max_retries": 0})),
        };
        let result = executor.execute(&node, &make_context("{}")).await;
        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
    }

    #[tokio::test]
    async fn failing_node_retried_and_still_fails() {
        let executor = DispatchingNodeExecutor::new(None);
        // Agent with max_retries:1 — will fail twice (no AI Runtime URL)
        let node = Node {
            id: "agent".to_string(),
            node_type: NodeType::Agent,
            config: Some(serde_json::json!({"max_retries": 1})),
        };
        let result = executor.execute(&node, &make_context("{}")).await;
        assert_eq!(result.status, execution_core::NodeStatus::Failed);
        // Error should be from the node, not from the retry wrapper
        assert!(result
            .error
            .as_deref()
            .unwrap_or("")
            .contains("AI_RUNTIME_BASE_URL"));
    }

    #[tokio::test]
    async fn timeout_config_does_not_break_fast_nodes() {
        let executor = DispatchingNodeExecutor::new(None);
        let node = Node {
            id: "trigger".to_string(),
            node_type: NodeType::Trigger,
            config: Some(serde_json::json!({"timeout_secs": 30})),
        };
        let result = executor.execute(&node, &make_context(r#"{"k":"v"}"#)).await;
        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
    }

    #[test]
    fn template_resolver_replaces_input_field() {
        let context = make_context(r#"{"lead_id":"lead-42","status":"active"}"#);
        assert_eq!(
            resolve_template("id={{input.lead_id}}", &context),
            "id=lead-42"
        );
        assert_eq!(
            resolve_template("{{input}}", &context),
            r#"{"lead_id":"lead-42","status":"active"}"#
        );
        assert_eq!(
            resolve_template("no template here", &context),
            "no template here"
        );
    }

    #[test]
    fn template_resolver_replaces_node_output_field() {
        let mut context = make_context("{}");
        context.node_outputs.insert(
            "trigger".to_string(),
            r#"{"lead_id":"lead-99","name":"Alice"}"#.to_string(),
        );
        assert_eq!(
            resolve_template("Hello {{trigger.name}}", &context),
            "Hello Alice"
        );
        assert_eq!(resolve_template("{{trigger.lead_id}}", &context), "lead-99");
    }

    #[test]
    fn template_resolver_handles_missing_keys_gracefully() {
        let context = make_context(r#"{"a":1}"#);
        assert_eq!(resolve_template("{{input.missing}}", &context), "");
        assert_eq!(resolve_template("{{unknown_node.field}}", &context), "");
    }

    #[tokio::test]
    async fn http_node_resolves_url_template() {
        let executor = DispatchingNodeExecutor::new(None);
        let mut context = make_context(r#"{"endpoint":"https://example.com/api"}"#);
        context
            .node_outputs
            .insert("trigger".to_string(), r#"{"id":"42"}"#.to_string());
        let node = Node {
            id: "http".to_string(),
            node_type: NodeType::Http,
            // URL and body use templates — will fail with real HTTP but shows template resolved
            config: Some(serde_json::json!({
                "url": "{{input.endpoint}}/items/{{trigger.id}}",
                "method": "GET"
            })),
        };
        let result = executor.execute(&node, &context).await;
        // Should fail (no server), not fail because template unresolved
        if let Some(err) = &result.error {
            assert!(!err.contains("{{"), "Template was not resolved: {err}");
        }
    }

    #[tokio::test]
    async fn condition_node_uses_template_in_field() {
        let executor = DispatchingNodeExecutor::new(None);
        let mut context = make_context("{}");
        context
            .node_outputs
            .insert("trigger".to_string(), r#"{"status":"active"}"#.to_string());
        let node = Node {
            id: "condition".to_string(),
            node_type: NodeType::Condition,
            config: Some(serde_json::json!({
                "field": "{{trigger.status}}",
                "equals": "active"
            })),
        };
        let result = executor.execute(&node, &context).await;
        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
        let output: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(output["result"], true);
    }

    #[tokio::test]
    async fn delay_node_zero_seconds_completes_immediately() {
        let executor = DispatchingNodeExecutor::new(None);
        let context = make_context("{}");
        let node = Node {
            id: "delay".to_string(),
            node_type: NodeType::Delay,
            config: Some(serde_json::json!({ "seconds": 0 })),
        };
        let result = executor.execute(&node, &context).await;
        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
        let output: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(output["waited_secs"], 0);
    }

    #[tokio::test]
    async fn delay_node_fails_without_seconds_config() {
        let executor = DispatchingNodeExecutor::new(None);
        let context = make_context("{}");
        let node = Node {
            id: "delay".to_string(),
            node_type: NodeType::Delay,
            config: Some(serde_json::json!({ "label": "wait" })),
        };
        let result = executor.execute(&node, &context).await;
        assert_eq!(result.status, execution_core::NodeStatus::Failed);
        assert!(result.error.as_deref().unwrap_or("").contains("seconds"));
    }

    #[tokio::test]
    async fn transform_node_renders_template() {
        let executor = DispatchingNodeExecutor::new(None);
        let mut context = make_context(r#"{"user":"Alice"}"#);
        context
            .node_outputs
            .insert("trigger".to_string(), r#"{"score":42}"#.to_string());
        let node = Node {
            id: "transform".to_string(),
            node_type: NodeType::Transform,
            config: Some(serde_json::json!({
                "template": { "name": "{{input.user}}", "score": "{{trigger.score}}" }
            })),
        };
        let result = executor.execute(&node, &context).await;
        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
        let output: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(output["name"], "Alice");
        assert_eq!(output["score"], "42");
    }

    #[tokio::test]
    async fn transform_node_passes_through_scalar_template() {
        let executor = DispatchingNodeExecutor::new(None);
        let context = make_context(r#"{"msg":"hello"}"#);
        let node = Node {
            id: "transform".to_string(),
            node_type: NodeType::Transform,
            config: Some(serde_json::json!({ "template": "{{input.msg}}" })),
        };
        let result = executor.execute(&node, &context).await;
        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
        assert_eq!(result.output_json.as_deref(), Some("\"hello\""));
    }

    #[tokio::test]
    async fn transform_node_fails_without_template() {
        let executor = DispatchingNodeExecutor::new(None);
        let context = make_context("{}");
        let node = Node {
            id: "transform".to_string(),
            node_type: NodeType::Transform,
            config: Some(serde_json::json!({ "other_key": "irrelevant" })),
        };
        let result = executor.execute(&node, &context).await;
        assert_eq!(result.status, execution_core::NodeStatus::Failed);
        assert!(result.error.as_deref().unwrap_or("").contains("template"));
    }

    #[tokio::test]
    async fn sort_node_ascending_string() {
        let executor = DispatchingNodeExecutor::new(None);
        let context = make_context(r#"{"words":[{"v":"banana"},{"v":"apple"},{"v":"cherry"}]}"#);
        let node = Node {
            id: "sort".to_string(),
            node_type: NodeType::Sort,
            config: Some(serde_json::json!({
                "items": "{{input.words}}",
                "field": "v",
                "order": "asc"
            })),
        };
        let result = executor.execute(&node, &context).await;
        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
        let output: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(output["count"], 3);
        assert_eq!(output["items"][0]["v"], "apple");
        assert_eq!(output["items"][1]["v"], "banana");
        assert_eq!(output["items"][2]["v"], "cherry");
    }

    #[tokio::test]
    async fn sort_node_descending_numeric() {
        let executor = DispatchingNodeExecutor::new(None);
        let context = make_context(r#"{"scores":[{"s":3},{"s":1},{"s":4},{"s":1},{"s":5}]}"#);
        let node = Node {
            id: "sort".to_string(),
            node_type: NodeType::Sort,
            config: Some(serde_json::json!({
                "items": "{{input.scores}}",
                "field": "s",
                "order": "desc",
                "type": "number"
            })),
        };
        let result = executor.execute(&node, &context).await;
        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
        let output: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(output["count"], 5);
        assert_eq!(output["items"][0]["s"], 5);
        assert_eq!(output["items"][1]["s"], 4);
    }

    #[tokio::test]
    async fn sort_node_fails_when_items_not_array() {
        let executor = DispatchingNodeExecutor::new(None);
        let context = make_context(r#"{"name":"Alice"}"#);
        let node = Node {
            id: "sort".to_string(),
            node_type: NodeType::Sort,
            config: Some(serde_json::json!({
                "items": "{{input}}",
                "field": "name"
            })),
        };
        let result = executor.execute(&node, &context).await;
        assert_eq!(result.status, execution_core::NodeStatus::Failed);
        assert!(result.error.as_deref().unwrap_or("").contains("array"));
    }

    #[tokio::test]
    async fn aggregate_node_count() {
        let executor = DispatchingNodeExecutor::new(None);
        let context = make_context(r#"{"items":[1,2,3,4,5]}"#);
        let node = Node {
            id: "agg".to_string(),
            node_type: NodeType::Aggregate,
            config: Some(serde_json::json!({ "items": "{{input.items}}", "operation": "count" })),
        };
        let result = executor.execute(&node, &context).await;
        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
        let output: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(output["result"], 5);
    }

    #[tokio::test]
    async fn aggregate_node_sum_and_avg() {
        let executor = DispatchingNodeExecutor::new(None);
        let context = make_context(r#"{"rows":[{"v":10},{"v":20},{"v":30}]}"#);
        // sum
        let sum_node = Node {
            id: "agg".to_string(),
            node_type: NodeType::Aggregate,
            config: Some(serde_json::json!({
                "items": "{{input.rows}}", "operation": "sum", "field": "v"
            })),
        };
        let sum_result = executor.execute(&sum_node, &context).await;
        assert_eq!(sum_result.status, execution_core::NodeStatus::Succeeded);
        let sum_out: serde_json::Value =
            serde_json::from_str(sum_result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(sum_out["result"], 60);

        // avg
        let avg_node = Node {
            id: "agg".to_string(),
            node_type: NodeType::Aggregate,
            config: Some(serde_json::json!({
                "items": "{{input.rows}}", "operation": "avg", "field": "v"
            })),
        };
        let avg_result = executor.execute(&avg_node, &context).await;
        let avg_out: serde_json::Value =
            serde_json::from_str(avg_result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(avg_out["result"], 20);
    }

    #[tokio::test]
    async fn aggregate_node_join() {
        let executor = DispatchingNodeExecutor::new(None);
        let context = make_context(r#"{"tags":[{"name":"rust"},{"name":"wasm"},{"name":"axum"}]}"#);
        let node = Node {
            id: "agg".to_string(),
            node_type: NodeType::Aggregate,
            config: Some(serde_json::json!({
                "items": "{{input.tags}}",
                "operation": "join",
                "field": "name",
                "separator": " | "
            })),
        };
        let result = executor.execute(&node, &context).await;
        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
        let output: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(output["result"], "rust | wasm | axum");
    }

    #[tokio::test]
    async fn aggregate_node_fails_with_unknown_operation() {
        let executor = DispatchingNodeExecutor::new(None);
        let context = make_context(r#"{"items":[1,2,3]}"#);
        let node = Node {
            id: "agg".to_string(),
            node_type: NodeType::Aggregate,
            config: Some(serde_json::json!({ "items": "{{input.items}}", "operation": "product" })),
        };
        let result = executor.execute(&node, &context).await;
        assert_eq!(result.status, execution_core::NodeStatus::Failed);
        assert!(result.error.as_deref().unwrap_or("").contains("product"));
    }

    #[tokio::test]
    async fn filter_node_keeps_matching_items() {
        let executor = DispatchingNodeExecutor::new(None);
        let mut context = make_context("{}");
        context.node_outputs.insert(
            "trigger".to_string(),
            r#"{"users":[{"name":"Alice","active":true},{"name":"Bob","active":false},{"name":"Carol","active":true}]}"#.to_string(),
        );
        let node = Node {
            id: "filter".to_string(),
            node_type: NodeType::Filter,
            config: Some(serde_json::json!({
                "items": "{{trigger.users}}",
                "field": "active",
                "operator": "equals",
                "value": "true"
            })),
        };
        let result = executor.execute(&node, &context).await;
        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
        let output: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(output["count"], 2);
        assert_eq!(output["items"][0]["name"], "Alice");
        assert_eq!(output["items"][1]["name"], "Carol");
    }

    #[tokio::test]
    async fn filter_node_exists_operator() {
        let executor = DispatchingNodeExecutor::new(None);
        let context = make_context(r#"{"items":[{"score":10},{"label":"x"},{"score":5}]}"#);
        let node = Node {
            id: "filter".to_string(),
            node_type: NodeType::Filter,
            config: Some(serde_json::json!({
                "items": "{{input.items}}",
                "field": "score",
                "operator": "exists"
            })),
        };
        let result = executor.execute(&node, &context).await;
        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
        let output: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(output["count"], 2);
    }

    #[tokio::test]
    async fn filter_node_gt_operator() {
        let executor = DispatchingNodeExecutor::new(None);
        let context = make_context(r#"{"scores":[{"v":3},{"v":7},{"v":5},{"v":10}]}"#);
        let node = Node {
            id: "filter".to_string(),
            node_type: NodeType::Filter,
            config: Some(serde_json::json!({
                "items": "{{input.scores}}",
                "field": "v",
                "operator": "gt",
                "value": "5"
            })),
        };
        let result = executor.execute(&node, &context).await;
        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
        let output: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(output["count"], 2);
        assert_eq!(output["items"][0]["v"], 7);
        assert_eq!(output["items"][1]["v"], 10);
    }

    #[tokio::test]
    async fn filter_node_fails_when_items_not_array() {
        let executor = DispatchingNodeExecutor::new(None);
        let context = make_context(r#"{"name":"Alice"}"#);
        let node = Node {
            id: "filter".to_string(),
            node_type: NodeType::Filter,
            config: Some(serde_json::json!({
                "items": "{{input}}",
                "field": "name",
                "operator": "exists"
            })),
        };
        let result = executor.execute(&node, &context).await;
        assert_eq!(result.status, execution_core::NodeStatus::Failed);
        assert!(result.error.as_deref().unwrap_or("").contains("array"));
    }

    #[tokio::test]
    async fn map_node_fans_out_array_passthrough() {
        let executor = DispatchingNodeExecutor::new(None);
        let mut context = make_context("{}");
        context.node_outputs.insert(
            "trigger".to_string(),
            r#"{"leads":[{"name":"Alice"},{"name":"Bob"}]}"#.to_string(),
        );
        let node = Node {
            id: "map".to_string(),
            node_type: NodeType::Map,
            config: Some(serde_json::json!({ "items": "{{trigger.leads}}" })),
        };
        let result = executor.execute(&node, &context).await;
        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
        let output: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(output["count"], 2);
        assert_eq!(output["items"][0]["name"], "Alice");
        assert_eq!(output["items"][1]["name"], "Bob");
    }

    #[tokio::test]
    async fn map_node_applies_item_template() {
        let executor = DispatchingNodeExecutor::new(None);
        let mut context = make_context("{}");
        context.node_outputs.insert(
            "trigger".to_string(),
            r#"{"leads":[{"name":"Alice","email":"alice@x.com"},{"name":"Bob","email":"bob@x.com"}]}"#.to_string(),
        );
        let node = Node {
            id: "map".to_string(),
            node_type: NodeType::Map,
            config: Some(serde_json::json!({
                "items": "{{trigger.leads}}",
                "item_template": { "label": "{{item.name}}", "contact": "{{item.email}}" }
            })),
        };
        let result = executor.execute(&node, &context).await;
        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
        let output: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(output["count"], 2);
        assert_eq!(output["items"][0]["label"], "Alice");
        assert_eq!(output["items"][0]["contact"], "alice@x.com");
        assert_eq!(output["items"][1]["label"], "Bob");
    }

    #[tokio::test]
    async fn sub_workflow_node_fails_without_graph_config() {
        let executor = DispatchingNodeExecutor::new(None);
        let context = make_context("{}");
        let node = Node {
            id: "sub".to_string(),
            node_type: NodeType::SubWorkflow,
            config: Some(serde_json::json!({ "workflow_id": "wf-1" })), // missing _graph
        };
        let result = executor.execute(&node, &context).await;
        assert_eq!(result.status, execution_core::NodeStatus::Failed);
        assert!(result.error.as_deref().unwrap_or("").contains("_graph"));
    }

    #[tokio::test]
    async fn sub_workflow_node_runs_embedded_graph() {
        let executor = DispatchingNodeExecutor::new(None);
        let context = make_context(r#"{"value":42}"#);
        let sub_graph = serde_json::json!({
            "workflow_version_id": "sub-v1",
            "nodes": [{ "id": "trigger", "type": "trigger" }],
            "edges": []
        });
        let node = Node {
            id: "sub".to_string(),
            node_type: NodeType::SubWorkflow,
            config: Some(serde_json::json!({
                "workflow_id": "wf-sub",
                "_graph": sub_graph
            })),
        };
        let result = executor.execute(&node, &context).await;
        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
        let output: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(output["status"], "succeeded");
        // trigger node echoes input, so sub-workflow output is the input JSON
        assert_eq!(output["output"]["value"], 42);
    }

    #[tokio::test]
    async fn map_node_fails_when_items_not_array() {
        let executor = DispatchingNodeExecutor::new(None);
        // {{input}} resolves to a JSON object — valid JSON but not an array
        let context = make_context(r#"{"name":"Alice"}"#);
        let node = Node {
            id: "map".to_string(),
            node_type: NodeType::Map,
            config: Some(serde_json::json!({ "items": "{{input}}" })),
        };
        let result = executor.execute(&node, &context).await;
        assert_eq!(result.status, execution_core::NodeStatus::Failed);
        assert!(result.error.as_deref().unwrap_or("").contains("array"));
    }

    #[tokio::test]
    async fn condition_node_evaluates_field_equals() {
        let executor = DispatchingNodeExecutor::new(None);
        let node = Node {
            id: "condition".to_string(),
            node_type: NodeType::Condition,
            config: Some(serde_json::json!({ "field": "status", "equals": "active" })),
        };

        let context_match = make_context(r#"{"status":"active"}"#);
        let result_match = executor.execute(&node, &context_match).await;
        let output_match: serde_json::Value =
            serde_json::from_str(result_match.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(output_match["result"], true);

        let context_no_match = make_context(r#"{"status":"inactive"}"#);
        let result_no_match = executor.execute(&node, &context_no_match).await;
        let output_no_match: serde_json::Value =
            serde_json::from_str(result_no_match.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(output_no_match["result"], false);
    }

    #[tokio::test]
    async fn assert_node_passes_truthy_value() {
        let executor = DispatchingNodeExecutor::new(None);
        let node = Node {
            id: "assert".to_string(),
            node_type: NodeType::Assert,
            config: Some(
                serde_json::json!({ "condition": "{{filter.count}}", "message": "Expected count" }),
            ),
        };
        let mut ctx = make_context(r#"{}"#);
        ctx.node_outputs
            .insert("filter".to_string(), r#"{"count": 5}"#.to_string());
        let result = executor.execute(&node, &ctx).await;
        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
        let out: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(out["ok"], true);
    }

    #[tokio::test]
    async fn assert_node_fails_falsy_value() {
        let executor = DispatchingNodeExecutor::new(None);
        let node = Node {
            id: "assert".to_string(),
            node_type: NodeType::Assert,
            config: Some(
                serde_json::json!({ "condition": "{{filter.count}}", "message": "Count must be non-zero" }),
            ),
        };
        let mut ctx = make_context(r#"{}"#);
        ctx.node_outputs
            .insert("filter".to_string(), r#"{"count": 0}"#.to_string());
        let result = executor.execute(&node, &ctx).await;
        assert_eq!(result.status, execution_core::NodeStatus::Failed);
        assert_eq!(result.error.as_deref(), Some("Count must be non-zero"));
    }

    #[tokio::test]
    async fn assert_node_uses_default_message() {
        let executor = DispatchingNodeExecutor::new(None);
        let node = Node {
            id: "assert".to_string(),
            node_type: NodeType::Assert,
            config: Some(serde_json::json!({ "condition": "{{some_node.missing_field}}" })),
        };
        let ctx = make_context(r#"{}"#);
        let result = executor.execute(&node, &ctx).await;
        assert_eq!(result.status, execution_core::NodeStatus::Failed);
        assert_eq!(result.error.as_deref(), Some("Assertion failed"));
    }

    #[tokio::test]
    async fn code_node_executes_rhai_script() {
        let executor = DispatchingNodeExecutor::new(None);
        let node = Node {
            id: "code".to_string(),
            node_type: NodeType::Code,
            config: Some(serde_json::json!({
                "script": r#"
                    let n = input["count"];
                    #{ doubled: n * 2, ok: true }
                "#
            })),
        };
        let mut ctx = make_context(r#"{"count": 5}"#);
        ctx.node_outputs
            .insert("prev".to_string(), r#"{"value": 1}"#.to_string());
        let result = executor.execute(&node, &ctx).await;
        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
        let out: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(out["doubled"], 10);
        assert_eq!(out["ok"], true);
    }

    #[tokio::test]
    async fn code_node_accesses_nodes_map() {
        let executor = DispatchingNodeExecutor::new(None);
        let node = Node {
            id: "code".to_string(),
            node_type: NodeType::Code,
            config: Some(serde_json::json!({
                "script": r#"nodes["http"]["status"]"#
            })),
        };
        let mut ctx = make_context(r#"{}"#);
        ctx.node_outputs
            .insert("http".to_string(), r#"{"status": 200}"#.to_string());
        let result = executor.execute(&node, &ctx).await;
        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
        let out: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(out, 200);
    }

    #[tokio::test]
    async fn code_node_fails_on_script_error() {
        let executor = DispatchingNodeExecutor::new(None);
        let node = Node {
            id: "code".to_string(),
            node_type: NodeType::Code,
            config: Some(serde_json::json!({ "script": "this is not valid rhai !!!" })),
        };
        let ctx = make_context(r#"{}"#);
        let result = executor.execute(&node, &ctx).await;
        assert_eq!(result.status, execution_core::NodeStatus::Failed);
        assert!(result
            .error
            .as_deref()
            .unwrap_or("")
            .starts_with("Code error:"));
    }

    #[tokio::test]
    async fn code_node_fails_without_script() {
        let executor = DispatchingNodeExecutor::new(None);
        let node = Node {
            id: "code".to_string(),
            node_type: NodeType::Code,
            config: None,
        };
        let ctx = make_context(r#"{}"#);
        let result = executor.execute(&node, &ctx).await;
        assert_eq!(result.status, execution_core::NodeStatus::Failed);
    }

    #[tokio::test]
    async fn slack_node_fails_without_webhook_url() {
        let executor = DispatchingNodeExecutor::new(None);
        let node = Node {
            id: "slack".to_string(),
            node_type: NodeType::Slack,
            config: Some(serde_json::json!({ "text": "hello" })),
        };
        let result = executor.execute(&node, &make_context("{}")).await;
        assert_eq!(result.status, execution_core::NodeStatus::Failed);
        assert!(result
            .error
            .as_deref()
            .unwrap_or("")
            .contains("webhook_url"));
    }

    #[tokio::test]
    async fn slack_node_fails_without_text() {
        let executor = DispatchingNodeExecutor::new(None);
        let node = Node {
            id: "slack".to_string(),
            node_type: NodeType::Slack,
            config: Some(serde_json::json!({ "webhook_url": "https://hooks.slack.com/fake" })),
        };
        let result = executor.execute(&node, &make_context("{}")).await;
        assert_eq!(result.status, execution_core::NodeStatus::Failed);
        assert!(result.error.as_deref().unwrap_or("").contains("text"));
    }

    #[tokio::test]
    async fn email_node_fails_without_required_fields() {
        let executor = DispatchingNodeExecutor::new(None);
        let node = Node {
            id: "email".to_string(),
            node_type: NodeType::Email,
            config: Some(serde_json::json!({ "to": "user@example.com" })),
        };
        let result = executor.execute(&node, &make_context("{}")).await;
        assert_eq!(result.status, execution_core::NodeStatus::Failed);
        // Missing subject
        assert!(result.error.as_deref().unwrap_or("").contains("subject"));
    }

    #[tokio::test]
    async fn email_node_fails_without_config() {
        let executor = DispatchingNodeExecutor::new(None);
        let node = Node {
            id: "email".to_string(),
            node_type: NodeType::Email,
            config: None,
        };
        let result = executor.execute(&node, &make_context("{}")).await;
        assert_eq!(result.status, execution_core::NodeStatus::Failed);
    }

    #[tokio::test]
    async fn fan_out_passes_input_through() {
        let executor = DispatchingNodeExecutor::new(None);
        let node = Node {
            id: "fan_out".to_string(),
            node_type: NodeType::FanOut,
            config: None,
        };
        let ctx = make_context(r#"{"user":"alice"}"#);
        let result = executor.execute(&node, &ctx).await;
        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
        let out: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(out["ok"], true);
        assert_eq!(out["input"]["user"], "alice");
    }

    #[tokio::test]
    async fn fan_in_collects_sources() {
        let executor = DispatchingNodeExecutor::new(None);
        let node = Node {
            id: "fan_in".to_string(),
            node_type: NodeType::FanIn,
            config: Some(serde_json::json!({ "_sources": ["branch_a", "branch_b"] })),
        };
        let mut ctx = make_context(r#"{}"#);
        ctx.node_outputs
            .insert("branch_a".to_string(), r#"{"value": 1}"#.to_string());
        ctx.node_outputs
            .insert("branch_b".to_string(), r#"{"value": 2}"#.to_string());
        let result = executor.execute(&node, &ctx).await;
        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
        let out: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(out["count"], 2);
        assert_eq!(out["results"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn extract_node_returns_value() {
        let executor = DispatchingNodeExecutor::new(None);
        let node = Node {
            id: "extract".to_string(),
            node_type: NodeType::Extract,
            config: Some(serde_json::json!({ "source": "{{input}}", "path": "user.email" })),
        };
        let ctx = make_context(r#"{"user":{"email":"alice@example.com"}}"#);
        let result = executor.execute(&node, &ctx).await;
        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
        let out: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(out["value"], "alice@example.com");
        assert_eq!(out["found"], true);
    }

    #[tokio::test]
    async fn extract_node_missing_path() {
        let executor = DispatchingNodeExecutor::new(None);
        let node = Node {
            id: "extract".to_string(),
            node_type: NodeType::Extract,
            config: Some(serde_json::json!({ "source": "{{input}}", "path": "missing.key" })),
        };
        let ctx = make_context(r#"{"name":"Bob"}"#);
        let result = executor.execute(&node, &ctx).await;
        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
        let out: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(out["found"], false);
        assert_eq!(out["value"], serde_json::Value::Null);
    }

    #[tokio::test]
    async fn merge_node_combines_fields() {
        let executor = DispatchingNodeExecutor::new(None);
        let node = Node {
            id: "merge".to_string(),
            node_type: NodeType::Merge,
            config: Some(serde_json::json!({
                "fields": [
                    { "source": "{{input}}", "key": "from_input" },
                    { "source": "{\"extra\": 42}", "key": "extra_obj" }
                ]
            })),
        };
        let ctx = make_context(r#"{"name":"Alice"}"#);
        let result = executor.execute(&node, &ctx).await;
        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
        let out: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(out["from_input"]["name"], "Alice");
        assert_eq!(out["extra_obj"]["extra"], 42);
    }

    #[tokio::test]
    async fn loop_node_iterates_array() {
        let executor = DispatchingNodeExecutor::new(None);
        let node = Node {
            id: "loop".to_string(),
            node_type: NodeType::Loop,
            config: Some(serde_json::json!({ "items": "{{input}}", "max_iterations": 5 })),
        };
        let ctx = make_context(r#"[1,2,3]"#);
        let result = executor.execute(&node, &ctx).await;
        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
        let out: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(out["count"], 3);
        assert_eq!(out["results"][0], 1);
    }

    #[tokio::test]
    async fn loop_node_respects_max_iterations_cap() {
        let executor = DispatchingNodeExecutor::new(None);
        let node = Node {
            id: "loop".to_string(),
            node_type: NodeType::Loop,
            config: Some(serde_json::json!({ "items": "{{input}}", "max_iterations": 2 })),
        };
        // 5 items but cap = 2
        let ctx = make_context(r#"[10,20,30,40,50]"#);
        let result = executor.execute(&node, &ctx).await;
        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
        let out: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(out["count"], 2);
    }

    #[tokio::test]
    async fn extract_node_finds_nested_value() {
        let executor = DispatchingNodeExecutor::new(None);
        let node = Node {
            id: "ex".to_string(),
            node_type: NodeType::Extract,
            config: Some(serde_json::json!({ "source": "{{input}}", "path": "user.name" })),
        };
        let ctx = make_context(r#"{"user":{"name":"Alice","age":30}}"#);
        let result = executor.execute(&node, &ctx).await;
        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
        let out: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(out["value"], "Alice");
        assert_eq!(out["found"], true);
    }

    #[tokio::test]
    async fn extract_node_reports_not_found() {
        let executor = DispatchingNodeExecutor::new(None);
        let node = Node {
            id: "ex2".to_string(),
            node_type: NodeType::Extract,
            config: Some(serde_json::json!({ "source": "{{input}}", "path": "missing.key" })),
        };
        let ctx = make_context(r#"{"foo": 1}"#);
        let result = executor.execute(&node, &ctx).await;
        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
        let out: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(out["found"], false);
    }

    #[tokio::test]
    async fn claude_node_fails_without_config() {
        let executor = DispatchingNodeExecutor::new(None);
        let node = Node {
            id: "claude".to_string(),
            node_type: NodeType::Claude,
            config: None,
        };
        let ctx = make_context("{}");
        let result = executor.execute(&node, &ctx).await;
        assert_eq!(result.status, execution_core::NodeStatus::Failed);
        assert!(result.error.as_deref().unwrap_or("").contains("config"));
    }

    #[tokio::test]
    async fn claude_node_fails_without_api_key() {
        let executor = DispatchingNodeExecutor::new(None);
        let node = Node {
            id: "claude".to_string(),
            node_type: NodeType::Claude,
            config: Some(serde_json::json!({ "prompt_template": "Hello" })),
        };
        let ctx = make_context("{}");
        let result = executor.execute(&node, &ctx).await;
        assert_eq!(result.status, execution_core::NodeStatus::Failed);
        assert!(result.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn claude_node_fails_without_prompt() {
        let executor = DispatchingNodeExecutor::new(None);
        let node = Node {
            id: "claude".to_string(),
            node_type: NodeType::Claude,
            config: Some(serde_json::json!({ "api_key": "sk-test" })),
        };
        let ctx = make_context("{}");
        let result = executor.execute(&node, &ctx).await;
        assert_eq!(result.status, execution_core::NodeStatus::Failed);
        assert!(result
            .error
            .as_deref()
            .unwrap_or("")
            .contains("prompt_template"));
    }

    #[test]
    fn split_node_splits_by_comma() {
        let node = Node {
            id: "split1".to_string(),
            node_type: NodeType::Split,
            config: Some(serde_json::json!({ "source": "a, b, c", "delimiter": "," })),
        };
        let ctx = make_context("{}");
        let result = execute_split(&node, &ctx);
        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
        let out: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(out["count"], 3);
        assert_eq!(out["parts"][0], "a");
        assert_eq!(out["parts"][1], "b");
        assert_eq!(out["parts"][2], "c");
    }

    #[test]
    fn split_node_no_trim_preserves_spaces() {
        let node = Node {
            id: "split2".to_string(),
            node_type: NodeType::Split,
            config: Some(serde_json::json!({ "source": "a , b", "delimiter": ",", "trim": false })),
        };
        let ctx = make_context("{}");
        let result = execute_split(&node, &ctx);
        let out: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        // trim=false preserves leading/trailing whitespace
        assert_eq!(out["parts"][0], "a ");
        assert_eq!(out["parts"][1], " b");
        assert_eq!(out["count"], 2);
    }

    #[test]
    fn rename_node_renames_keys() {
        let node = Node {
            id: "rn1".to_string(),
            node_type: NodeType::Rename,
            config: Some(serde_json::json!({
                "source": {"first_name": "Alice", "last_name": "Smith"},
                "mappings": [{"from": "first_name", "to": "name"}, {"from": "last_name", "to": "surname"}]
            })),
        };
        let ctx = make_context("{}");
        let result = execute_rename(&node, &ctx);
        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
        let out: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(out["name"], "Alice");
        assert_eq!(out["surname"], "Smith");
        assert!(out.get("first_name").is_none());
    }

    #[test]
    fn format_node_uppercase() {
        let node = Node {
            id: "fmt1".to_string(),
            node_type: NodeType::Format,
            config: Some(serde_json::json!({ "source": "hello world", "operation": "uppercase" })),
        };
        let ctx = make_context("{}");
        let result = execute_format(&node, &ctx);
        let out: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(out["result"], "HELLO WORLD");
    }

    #[test]
    fn format_node_truncate() {
        let node = Node {
            id: "fmt2".to_string(),
            node_type: NodeType::Format,
            config: Some(
                serde_json::json!({ "source": "Hello World", "operation": "truncate", "max_length": 5, "suffix": "..." }),
            ),
        };
        let ctx = make_context("{}");
        let result = execute_format(&node, &ctx);
        let out: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(out["result"], "Hello...");
    }

    #[test]
    fn dedupe_node_removes_duplicates_by_field() {
        let node = Node {
            id: "dd1".to_string(),
            node_type: NodeType::Dedupe,
            config: Some(serde_json::json!({ "field": "id" })),
        };
        let mut ctx = make_context("{}");
        ctx.node_outputs.insert("src".to_string(), "{}".to_string());
        // items passed inline via template-resolved array
        let node2 = Node {
            id: "dd2".to_string(),
            node_type: NodeType::Dedupe,
            config: Some(serde_json::json!({
                "items": [{"id":"a","v":1},{"id":"b","v":2},{"id":"a","v":3}],
                "field": "id"
            })),
        };
        let ctx2 = make_context("{}");
        let result = execute_dedupe(&node2, &ctx2);
        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
        let out: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(out["count"], 2);
        assert_eq!(out["removed_count"], 1);
    }

    #[test]
    fn csv_node_parses_with_header() {
        let node = Node {
            id: "csv1".to_string(),
            node_type: NodeType::Csv,
            config: Some(serde_json::json!({ "source": "name,age\nAlice,30\nBob,25" })),
        };
        let ctx = make_context("{}");
        let result = execute_csv(&node, &ctx);
        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
        let out: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(out["count"], 2);
        assert_eq!(out["rows"][0]["name"], "Alice");
        assert_eq!(out["rows"][1]["age"], "25");
        assert_eq!(out["headers"][0], "name");
    }

    #[test]
    fn regex_node_matches_substring() {
        let node = Node {
            id: "re1".to_string(),
            node_type: NodeType::Regex,
            config: Some(serde_json::json!({ "source": "hello world", "pattern": "world" })),
        };
        let ctx = make_context("{}");
        let result = execute_regex(&node, &ctx);
        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
        let out: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(out["matched"], true);
        assert_eq!(out["full_match"], "world");
    }

    #[test]
    fn regex_node_no_match() {
        let node = Node {
            id: "re2".to_string(),
            node_type: NodeType::Regex,
            config: Some(serde_json::json!({ "source": "hello world", "pattern": "xyz" })),
        };
        let ctx = make_context("{}");
        let result = execute_regex(&node, &ctx);
        let out: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(out["matched"], false);
    }

    #[test]
    fn random_node_generates_number_in_range() {
        let node = Node {
            id: "rnd1".to_string(),
            node_type: NodeType::Random,
            config: Some(serde_json::json!({ "type": "number", "min": 10.0, "max": 20.0 })),
        };
        let ctx = make_context("{}");
        for _ in 0..20 {
            let result = execute_random(&node, &ctx);
            assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
            let out: serde_json::Value =
                serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
            let v = out["value"].as_f64().unwrap();
            assert!(v >= 10.0 && v <= 20.0, "value {v} out of range");
        }
    }

    #[test]
    fn random_node_pick_from_items() {
        let node = Node {
            id: "rnd2".to_string(),
            node_type: NodeType::Random,
            config: Some(serde_json::json!({ "type": "pick", "items": ["x", "y", "z"] })),
        };
        let ctx = make_context("{}");
        for _ in 0..10 {
            let result = execute_random(&node, &ctx);
            let out: serde_json::Value =
                serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
            let v = out["value"].as_str().unwrap();
            assert!(["x", "y", "z"].contains(&v));
        }
    }

    #[test]
    fn switch_node_matches_case() {
        let node = Node {
            id: "sw1".to_string(),
            node_type: NodeType::Switch,
            config: Some(serde_json::json!({
                "value": "approved",
                "cases": [
                    { "match": "approved", "label": "approve" },
                    { "match": "rejected", "label": "reject" },
                    { "match": "*", "label": "default" }
                ]
            })),
        };
        let ctx = make_context("{}");
        let result = execute_switch(&node, &ctx);
        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
        let out: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(out["matched_case"], "approve");
        assert_eq!(out["matched"], true);
        assert_eq!(out["value"], "approved");
    }

    #[test]
    fn switch_node_falls_through_to_wildcard() {
        let node = Node {
            id: "sw2".to_string(),
            node_type: NodeType::Switch,
            config: Some(serde_json::json!({
                "value": "unknown",
                "cases": [
                    { "match": "approved", "label": "approve" },
                    { "match": "*", "label": "default" }
                ]
            })),
        };
        let ctx = make_context("{}");
        let result = execute_switch(&node, &ctx);
        let out: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(out["matched_case"], "default");
        assert_eq!(out["matched"], true);
    }

    #[test]
    fn join_node_joins_array() {
        let node = Node {
            id: "join1".to_string(),
            node_type: NodeType::Join,
            config: Some(serde_json::json!({ "delimiter": "-" })),
        };
        let mut ctx = make_context("{}");
        ctx.node_outputs.insert(
            "upstream".to_string(),
            r#"{"parts":["x","y","z"]}"#.to_string(),
        );
        // Use explicit items template referencing the input parts
        let node2 = Node {
            id: "join2".to_string(),
            node_type: NodeType::Join,
            config: Some(serde_json::json!({ "items": ["hello", "world"], "delimiter": " " })),
        };
        let result = execute_join(&node2, &ctx);
        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
        let out: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(out["result"], "hello world");
        assert_eq!(out["count"], 2);
    }

    #[tokio::test]
    async fn github_node_fails_without_config() {
        let node = Node {
            id: "gh1".to_string(),
            node_type: NodeType::Github,
            config: None,
        };
        let ctx = make_context("{}");
        let client = reqwest::Client::new();
        let result = execute_github(&node, &ctx, &client).await;
        assert_eq!(result.status, execution_core::NodeStatus::Failed);
        assert!(result.error.as_deref().unwrap_or("").contains("config"));
    }

    #[tokio::test]
    async fn github_node_fails_without_token() {
        let node = Node {
            id: "gh2".to_string(),
            node_type: NodeType::Github,
            config: Some(serde_json::json!({ "endpoint": "/repos/owner/repo" })),
        };
        let ctx = make_context("{}");
        let client = reqwest::Client::new();
        let result = execute_github(&node, &ctx, &client).await;
        assert_eq!(result.status, execution_core::NodeStatus::Failed);
        assert!(result.error.as_deref().unwrap_or("").contains("token"));
    }

    #[tokio::test]
    async fn github_node_fails_without_endpoint() {
        let node = Node {
            id: "gh3".to_string(),
            node_type: NodeType::Github,
            config: Some(serde_json::json!({ "token": "ghp_test" })),
        };
        let ctx = make_context("{}");
        let client = reqwest::Client::new();
        let result = execute_github(&node, &ctx, &client).await;
        assert_eq!(result.status, execution_core::NodeStatus::Failed);
        assert!(result.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    #[tokio::test]
    async fn webhook_send_node_fails_without_url() {
        let node = Node {
            id: "wh1".to_string(),
            node_type: NodeType::Webhook,
            config: Some(serde_json::json!({})),
        };
        let ctx = make_context("{}");
        let client = reqwest::Client::new();
        let result = execute_webhook_send(&node, &ctx, &client).await;
        assert_eq!(result.status, execution_core::NodeStatus::Failed);
        assert!(result.error.as_deref().unwrap_or("").contains("url"));
    }

    #[tokio::test]
    async fn webhook_send_node_fails_without_config() {
        let node = Node {
            id: "wh2".to_string(),
            node_type: NodeType::Webhook,
            config: None,
        };
        let ctx = make_context("{}");
        let client = reqwest::Client::new();
        let result = execute_webhook_send(&node, &ctx, &client).await;
        assert_eq!(result.status, execution_core::NodeStatus::Failed);
    }

    #[test]
    fn ctx_variables_resolve_in_transform() {
        let ctx = ExecutionContext {
            execution_id: "exec-abc123".to_string(),
            workflow_version_id: "ver-xyz789".to_string(),
            input_json: "{}".to_string(),
            node_outputs: Default::default(),
            dry_run: false,
        };
        let node = Node {
            id: "t1".to_string(),
            node_type: NodeType::Transform,
            config: Some(serde_json::json!({
                "template": {
                    "exec": "{{ctx.execution_id}}",
                    "ver":  "{{ctx.workflow_version_id}}"
                }
            })),
        };
        let result = execute_transform(&node, &ctx);
        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
        let out: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(out["exec"], "exec-abc123");
        assert_eq!(out["ver"], "ver-xyz789");
    }

    #[tokio::test]
    async fn jira_node_fails_without_config() {
        let client = reqwest::Client::new();
        let ctx = make_context("{}");
        let node = Node {
            id: "j1".to_string(),
            node_type: NodeType::Jira,
            config: None,
        };
        let result = execute_jira(&node, &ctx, &client).await;
        assert_eq!(result.status, execution_core::NodeStatus::Failed);
    }

    #[tokio::test]
    async fn jira_node_fails_without_base_url() {
        let client = reqwest::Client::new();
        let ctx = make_context("{}");
        let node = Node {
            id: "j1".to_string(),
            node_type: NodeType::Jira,
            config: Some(
                serde_json::json!({ "email": "a@b.com", "token": "t", "endpoint": "/rest/api/3/issue" }),
            ),
        };
        let result = execute_jira(&node, &ctx, &client).await;
        assert!(result.error.as_deref().unwrap_or("").contains("base_url"));
    }

    #[tokio::test]
    async fn jira_node_fails_without_token() {
        let client = reqwest::Client::new();
        let ctx = make_context("{}");
        let node = Node {
            id: "j1".to_string(),
            node_type: NodeType::Jira,
            config: Some(
                serde_json::json!({ "base_url": "https://x.atlassian.net", "email": "a@b.com", "endpoint": "/rest/api/3/issue" }),
            ),
        };
        let result = execute_jira(&node, &ctx, &client).await;
        assert!(result.error.as_deref().unwrap_or("").contains("token"));
    }

    #[tokio::test]
    async fn notion_node_fails_without_config() {
        let client = reqwest::Client::new();
        let ctx = make_context("{}");
        let node = Node {
            id: "n1".to_string(),
            node_type: NodeType::Notion,
            config: None,
        };
        let result = execute_notion(&node, &ctx, &client).await;
        assert_eq!(result.status, execution_core::NodeStatus::Failed);
    }

    #[tokio::test]
    async fn notion_node_fails_without_token() {
        let client = reqwest::Client::new();
        let ctx = make_context("{}");
        let node = Node {
            id: "n1".to_string(),
            node_type: NodeType::Notion,
            config: Some(serde_json::json!({ "endpoint": "/v1/pages" })),
        };
        let result = execute_notion(&node, &ctx, &client).await;
        assert!(result.error.as_deref().unwrap_or("").contains("token"));
    }

    #[tokio::test]
    async fn linear_node_fails_without_config() {
        let client = reqwest::Client::new();
        let ctx = make_context("{}");
        let node = Node {
            id: "lin1".to_string(),
            node_type: NodeType::Linear,
            config: None,
        };
        let result = execute_linear(&node, &ctx, &client).await;
        assert_eq!(result.status, execution_core::NodeStatus::Failed);
    }

    #[tokio::test]
    async fn linear_node_fails_without_query() {
        let client = reqwest::Client::new();
        let ctx = make_context("{}");
        let node = Node {
            id: "lin1".to_string(),
            node_type: NodeType::Linear,
            config: Some(serde_json::json!({ "token": "tok" })),
        };
        let result = execute_linear(&node, &ctx, &client).await;
        assert!(result.error.as_deref().unwrap_or("").contains("query"));
    }

    #[tokio::test]
    async fn airtable_node_fails_without_token() {
        let client = reqwest::Client::new();
        let ctx = make_context("{}");
        let node = Node {
            id: "at1".to_string(),
            node_type: NodeType::Airtable,
            config: Some(serde_json::json!({ "base_id": "appXXX", "table": "Tasks" })),
        };
        let result = execute_airtable(&node, &ctx, &client).await;
        assert!(result.error.as_deref().unwrap_or("").contains("token"));
    }

    #[tokio::test]
    async fn airtable_node_fails_without_base_id() {
        let client = reqwest::Client::new();
        let ctx = make_context("{}");
        let node = Node {
            id: "at1".to_string(),
            node_type: NodeType::Airtable,
            config: Some(serde_json::json!({ "token": "tok", "table": "Tasks" })),
        };
        let result = execute_airtable(&node, &ctx, &client).await;
        assert!(result.error.as_deref().unwrap_or("").contains("base_id"));
    }

    #[tokio::test]
    async fn for_each_fails_without_graph() {
        let ctx = make_context(r#"{"items": [1, 2, 3]}"#);
        let node = Node {
            id: "fe1".to_string(),
            node_type: NodeType::ForEach,
            config: Some(serde_json::json!({ "items": "{{input.items}}" })),
        };
        let result = execute_for_each(&node, &ctx, None).await;
        assert!(result.error.as_deref().unwrap_or("").contains("_graph"));
    }

    #[tokio::test]
    async fn for_each_empty_items_returns_empty_results() {
        let ctx = make_context(r#"{"items": []}"#);
        let node = Node {
            id: "fe1".to_string(),
            node_type: NodeType::ForEach,
            config: Some(serde_json::json!({ "items": "{{input.items}}" })),
        };
        let result = execute_for_each(&node, &ctx, None).await;
        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
        let out: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(out["total"], 0);
        assert_eq!(out["results"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn for_each_items_not_array_fails() {
        let ctx = make_context(r#"{"items": "not-an-array"}"#);
        let node = Node {
            id: "fe1".to_string(),
            node_type: NodeType::ForEach,
            config: Some(serde_json::json!({ "items": "{{input.items}}" })),
        };
        let result = execute_for_each(&node, &ctx, None).await;
        assert_eq!(result.status, execution_core::NodeStatus::Failed);
    }

    #[tokio::test]
    async fn discord_node_fails_without_webhook_url() {
        let client = reqwest::Client::new();
        let ctx = make_context("{}");
        let node = Node {
            id: "d1".to_string(),
            node_type: NodeType::Discord,
            config: Some(serde_json::json!({ "content": "hello" })),
        };
        let result = execute_discord(&node, &ctx, &client).await;
        assert!(result
            .error
            .as_deref()
            .unwrap_or("")
            .contains("webhook_url"));
    }

    #[tokio::test]
    async fn discord_node_fails_without_content() {
        let client = reqwest::Client::new();
        let ctx = make_context("{}");
        let node = Node {
            id: "d1".to_string(),
            node_type: NodeType::Discord,
            config: Some(
                serde_json::json!({ "webhook_url": "https://discord.com/api/webhooks/x/y" }),
            ),
        };
        let result = execute_discord(&node, &ctx, &client).await;
        assert!(result.error.as_deref().unwrap_or("").contains("content"));
    }

    #[tokio::test]
    async fn teams_node_fails_without_text() {
        let client = reqwest::Client::new();
        let ctx = make_context("{}");
        let node = Node {
            id: "t1".to_string(),
            node_type: NodeType::Teams,
            config: Some(
                serde_json::json!({ "webhook_url": "https://outlook.office.com/webhook/xxx" }),
            ),
        };
        let result = execute_teams(&node, &ctx, &client).await;
        assert!(result.error.as_deref().unwrap_or("").contains("text"));
    }

    #[tokio::test]
    async fn sheets_node_fails_without_spreadsheet_id() {
        let client = reqwest::Client::new();
        let ctx = make_context("{}");
        let node = Node {
            id: "s1".to_string(),
            node_type: NodeType::Sheets,
            config: Some(serde_json::json!({ "token": "tok", "range": "Sheet1!A1:B10" })),
        };
        let result = execute_sheets(&node, &ctx, &client).await;
        assert!(result
            .error
            .as_deref()
            .unwrap_or("")
            .contains("spreadsheet_id"));
    }

    #[tokio::test]
    async fn sheets_node_fails_without_token() {
        let client = reqwest::Client::new();
        let ctx = make_context("{}");
        let node = Node {
            id: "s1".to_string(),
            node_type: NodeType::Sheets,
            config: Some(
                serde_json::json!({ "spreadsheet_id": "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgVE2upms", "range": "Sheet1!A1:B10" }),
            ),
        };
        let result = execute_sheets(&node, &ctx, &client).await;
        assert!(result.error.as_deref().unwrap_or("").contains("token"));
    }

    #[test]
    fn yaml_parse_node_succeeds() {
        let ctx = make_context("{}");
        let node = Node {
            id: "y1".to_string(),
            node_type: NodeType::Yaml,
            config: Some(serde_json::json!({ "source": "name: Alice\nage: 30" })),
        };
        let result = execute_yaml(&node, &ctx);
        assert!(result.output_json.is_some());
        let out: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(out["data"]["name"], "Alice");
        assert_eq!(out["data"]["age"], 30);
    }

    #[test]
    fn yaml_serialize_node_succeeds() {
        let ctx = make_context(r#"{"val": {"key": "hello"}}"#);
        let node = Node {
            id: "y2".to_string(),
            node_type: NodeType::Yaml,
            config: Some(serde_json::json!({ "mode": "serialize", "source": "{{input.val}}" })),
        };
        let result = execute_yaml(&node, &ctx);
        assert!(result.output_json.is_some());
        let out: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert!(out["yaml"].as_str().unwrap_or("").contains("hello"));
    }

    #[test]
    fn yaml_node_fails_without_source() {
        let ctx = make_context("{}");
        let node = Node {
            id: "y3".to_string(),
            node_type: NodeType::Yaml,
            config: Some(serde_json::json!({})),
        };
        let result = execute_yaml(&node, &ctx);
        assert!(result.error.as_deref().unwrap_or("").contains("source"));
    }

    #[test]
    fn xml_parse_node_fails_without_source() {
        let ctx = make_context("{}");
        let node = Node {
            id: "x1".to_string(),
            node_type: NodeType::Xml,
            config: Some(serde_json::json!({})),
        };
        let result = execute_xml(&node, &ctx);
        assert!(result.error.as_deref().unwrap_or("").contains("source"));
    }

    #[tokio::test]
    async fn twilio_node_fails_without_account_sid() {
        let client = reqwest::Client::new();
        let ctx = make_context("{}");
        let node = Node {
            id: "tw1".to_string(),
            node_type: NodeType::Twilio,
            config: Some(
                serde_json::json!({ "auth_token": "tok", "to": "+1555", "from": "+1666", "body": "hi" }),
            ),
        };
        let result = execute_twilio(&node, &ctx, &client).await;
        assert!(result
            .error
            .as_deref()
            .unwrap_or("")
            .contains("account_sid"));
    }

    #[tokio::test]
    async fn stripe_node_fails_without_api_key() {
        let client = reqwest::Client::new();
        let ctx = make_context("{}");
        let node = Node {
            id: "st1".to_string(),
            node_type: NodeType::Stripe,
            config: Some(serde_json::json!({ "endpoint": "/customers" })),
        };
        let result = execute_stripe(&node, &ctx, &client).await;
        assert!(result.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn stripe_node_fails_without_endpoint() {
        let client = reqwest::Client::new();
        let ctx = make_context("{}");
        let node = Node {
            id: "st2".to_string(),
            node_type: NodeType::Stripe,
            config: Some(serde_json::json!({ "api_key": "sk_test_xxx" })),
        };
        let result = execute_stripe(&node, &ctx, &client).await;
        assert!(result.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    #[test]
    fn crypto_sha256_produces_hex() {
        let ctx = make_context("{}");
        let node = Node {
            id: "c1".to_string(),
            node_type: NodeType::Crypto,
            config: Some(serde_json::json!({ "operation": "sha256", "source": "hello" })),
        };
        let result = execute_crypto(&node, &ctx);
        assert!(result.output_json.is_some());
        let out: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        // SHA256("hello") = 2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824
        assert_eq!(
            out["result"],
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn crypto_base64_encode_decode_roundtrip() {
        let ctx = make_context("{}");
        let node_enc = Node {
            id: "c2".to_string(),
            node_type: NodeType::Crypto,
            config: Some(
                serde_json::json!({ "operation": "base64_encode", "source": "hello world" }),
            ),
        };
        let enc = execute_crypto(&node_enc, &ctx);
        let encoded =
            serde_json::from_str::<serde_json::Value>(enc.output_json.as_deref().unwrap()).unwrap()
                ["result"]
                .as_str()
                .unwrap()
                .to_string();
        let node_dec = Node {
            id: "c3".to_string(),
            node_type: NodeType::Crypto,
            config: Some(serde_json::json!({ "operation": "base64_decode", "source": encoded })),
        };
        let dec = execute_crypto(&node_dec, &ctx);
        let out: serde_json::Value =
            serde_json::from_str(dec.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(out["result"], "hello world");
    }

    #[test]
    fn crypto_random_hex_returns_hex() {
        let ctx = make_context("{}");
        let node = Node {
            id: "c4".to_string(),
            node_type: NodeType::Crypto,
            config: Some(serde_json::json!({ "operation": "random_hex", "length": 16 })),
        };
        let result = execute_crypto(&node, &ctx);
        let out: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        let hex_str = out["result"].as_str().unwrap();
        assert_eq!(hex_str.len(), 32); // 16 bytes = 32 hex chars
        assert!(hex_str.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn date_now_returns_unix_and_iso() {
        let ctx = make_context("{}");
        let node = Node {
            id: "d1".to_string(),
            node_type: NodeType::Date,
            config: Some(serde_json::json!({ "operation": "now" })),
        };
        let result = execute_date(&node, &ctx);
        assert!(result.output_json.is_some());
        let out: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert!(out["unix"].as_i64().unwrap_or(0) > 0);
        assert!(out["iso"].as_str().unwrap_or("").contains("T"));
    }

    #[test]
    fn date_add_hours_works() {
        let ctx = make_context("{}");
        let node = Node {
            id: "d2".to_string(),
            node_type: NodeType::Date,
            config: Some(serde_json::json!({
                "operation": "add",
                "source": "1704067200",
                "amount": 2,
                "unit": "hours"
            })),
        };
        let result = execute_date(&node, &ctx);
        let out: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        // 1704067200 + 7200 = 1704074400
        assert_eq!(out["unix"].as_i64().unwrap(), 1704074400);
    }

    #[tokio::test]
    async fn hubspot_node_fails_without_token() {
        let client = reqwest::Client::new();
        let ctx = make_context("{}");
        let node = Node {
            id: "hs1".to_string(),
            node_type: NodeType::Hubspot,
            config: Some(serde_json::json!({ "endpoint": "/crm/v3/objects/contacts" })),
        };
        let result = execute_hubspot(&node, &ctx, &client).await;
        assert!(result.error.as_deref().unwrap_or("").contains("token"));
    }

    #[tokio::test]
    async fn zendesk_node_fails_without_subdomain() {
        let client = reqwest::Client::new();
        let ctx = make_context("{}");
        let node = Node {
            id: "z1".to_string(),
            node_type: NodeType::Zendesk,
            config: Some(serde_json::json!({ "token": "tok", "endpoint": "/tickets.json" })),
        };
        let result = execute_zendesk(&node, &ctx, &client).await;
        assert!(result.error.as_deref().unwrap_or("").contains("subdomain"));
    }

    #[tokio::test]
    async fn zendesk_node_fails_without_token() {
        let client = reqwest::Client::new();
        let ctx = make_context("{}");
        let node = Node {
            id: "z2".to_string(),
            node_type: NodeType::Zendesk,
            config: Some(
                serde_json::json!({ "subdomain": "mycompany", "endpoint": "/tickets.json" }),
            ),
        };
        let result = execute_zendesk(&node, &ctx, &client).await;
        assert!(result.error.as_deref().unwrap_or("").contains("token"));
    }

    #[tokio::test]
    async fn redis_node_fails_without_url() {
        let ctx = make_context("{}");
        let node = Node {
            id: "r1".to_string(),
            node_type: NodeType::Redis,
            config: Some(serde_json::json!({ "operation": "get", "key": "test_key" })),
        };
        let result = execute_redis(&node, &ctx).await;
        assert!(result.error.as_deref().unwrap_or("").contains("url"));
    }

    #[tokio::test]
    async fn redis_node_connection_error_on_bad_url() {
        let ctx = make_context("{}");
        let node = Node {
            id: "r2".to_string(),
            node_type: NodeType::Redis,
            config: Some(serde_json::json!({
                "url": "redis://127.0.0.1:1",
                "operation": "ping"
            })),
        };
        let result = execute_redis(&node, &ctx).await;
        // Should fail with connect error (no Redis running on port 1)
        assert!(result.error.is_some());
    }

    #[tokio::test]
    async fn elasticsearch_node_fails_without_url() {
        let client = reqwest::Client::new();
        let ctx = make_context("{}");
        let node = Node {
            id: "es1".to_string(),
            node_type: NodeType::Elasticsearch,
            config: Some(serde_json::json!({ "endpoint": "/_search" })),
        };
        let result = execute_elasticsearch(&node, &ctx, &client).await;
        assert!(result.error.as_deref().unwrap_or("").contains("url"));
    }

    #[tokio::test]
    async fn pagerduty_node_fails_without_routing_key() {
        let client = reqwest::Client::new();
        let ctx = make_context("{}");
        let node = Node {
            id: "pd1".to_string(),
            node_type: NodeType::Pagerduty,
            config: Some(serde_json::json!({ "summary": "Test alert" })),
        };
        let result = execute_pagerduty(&node, &ctx, &client).await;
        assert!(result
            .error
            .as_deref()
            .unwrap_or("")
            .contains("routing_key"));
    }

    #[test]
    fn handlebars_renders_template_with_data() {
        let ctx = make_context(r#"{"name": "Alice", "count": 3}"#);
        let node = Node {
            id: "hb1".to_string(),
            node_type: NodeType::Handlebars,
            config: Some(serde_json::json!({
                "template": "Hello, {{name}}! You have {{count}} items.",
                "data": "{{input}}"
            })),
        };
        let result = execute_handlebars(&node, &ctx);
        assert!(result.output_json.is_some());
        let out: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(out["result"], "Hello, Alice! You have 3 items.");
    }

    #[test]
    fn handlebars_renders_each_block() {
        let ctx = make_context(r#"{"items": ["a", "b", "c"]}"#);
        let node = Node {
            id: "hb2".to_string(),
            node_type: NodeType::Handlebars,
            config: Some(serde_json::json!({
                "template": "{{#each items}}{{this}},{{/each}}",
                "data": "{{input}}"
            })),
        };
        let result = execute_handlebars(&node, &ctx);
        let out: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(out["result"], "a,b,c,");
    }

    #[test]
    fn handlebars_fails_without_template() {
        let ctx = make_context("{}");
        let node = Node {
            id: "hb3".to_string(),
            node_type: NodeType::Handlebars,
            config: Some(serde_json::json!({ "data": "{}" })),
        };
        let result = execute_handlebars(&node, &ctx);
        assert!(result.error.as_deref().unwrap_or("").contains("template"));
    }
}

// ── Slice 262: Math ───────────────────────────────────────────────────────────

// ── Slice 263: ArrayUtils ─────────────────────────────────────────────────────

// ── Slice 264: Shopify ────────────────────────────────────────────────────────

// ── Slice 265: Datadog ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests_262_265 {
    use super::*;
    use workflow_core::{Node, NodeType};

    fn ctx() -> ExecutionContext {
        ExecutionContext {
            execution_id: "e1".into(),
            workflow_version_id: "v1".into(),
            input_json: "{}".into(),
            node_outputs: Default::default(),
            dry_run: false,
        }
    }

    // ── Math ──────────────────────────────────────────────────────────────────

    #[test]
    fn math_add() {
        let node = Node {
            id: "m1".into(),
            node_type: NodeType::Math,
            config: Some(serde_json::json!({ "operation": "add", "a": 3, "b": 4 })),
        };
        let r = execute_math(&node, &ctx());
        let out: serde_json::Value =
            serde_json::from_str(r.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(out["result"], 7.0);
    }

    #[test]
    fn math_round_precision() {
        let node = Node {
            id: "m2".into(),
            node_type: NodeType::Math,
            config: Some(serde_json::json!({ "operation": "round", "a": 3.14159, "precision": 2 })),
        };
        let r = execute_math(&node, &ctx());
        let out: serde_json::Value =
            serde_json::from_str(r.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(out["result"], 3.14);
    }

    #[test]
    fn math_sum_array() {
        let node = Node {
            id: "m3".into(),
            node_type: NodeType::Math,
            config: Some(serde_json::json!({ "operation": "sum", "items": [1, 2, 3, 4] })),
        };
        let r = execute_math(&node, &ctx());
        let out: serde_json::Value =
            serde_json::from_str(r.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(out["result"], 10.0);
    }

    #[test]
    fn math_fails_without_config() {
        let node = Node {
            id: "m4".into(),
            node_type: NodeType::Math,
            config: None,
        };
        let r = execute_math(&node, &ctx());
        assert!(r.error.is_some());
    }

    // ── ArrayUtils ────────────────────────────────────────────────────────────

    #[test]
    fn array_utils_chunk() {
        let node = Node {
            id: "a1".into(),
            node_type: NodeType::ArrayUtils,
            config: Some(
                serde_json::json!({ "operation": "chunk", "source": [1,2,3,4,5], "size": 2 }),
            ),
        };
        let r = execute_array_utils(&node, &ctx());
        let out: serde_json::Value =
            serde_json::from_str(r.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(out["count"], 3);
    }

    #[test]
    fn array_utils_pluck() {
        let node = Node {
            id: "a2".into(),
            node_type: NodeType::ArrayUtils,
            config: Some(serde_json::json!({
                "operation": "pluck",
                "source": [{"name":"a"},{"name":"b"}],
                "field": "name"
            })),
        };
        let r = execute_array_utils(&node, &ctx());
        let out: serde_json::Value =
            serde_json::from_str(r.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(out["items"], serde_json::json!(["a", "b"]));
    }

    #[test]
    fn array_utils_range() {
        let node = Node {
            id: "a3".into(),
            node_type: NodeType::ArrayUtils,
            config: Some(
                serde_json::json!({ "operation": "range", "start": 0, "end": 5, "step": 1 }),
            ),
        };
        let r = execute_array_utils(&node, &ctx());
        let out: serde_json::Value =
            serde_json::from_str(r.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(out["count"], 5);
    }

    #[test]
    fn array_utils_fails_without_config() {
        let node = Node {
            id: "a4".into(),
            node_type: NodeType::ArrayUtils,
            config: None,
        };
        let r = execute_array_utils(&node, &ctx());
        assert!(r.error.is_some());
    }

    // ── Shopify (config validation only) ─────────────────────────────────────

    #[tokio::test]
    async fn shopify_fails_without_shop() {
        let client = reqwest::Client::new();
        let node = Node {
            id: "s1".into(),
            node_type: NodeType::Shopify,
            config: Some(serde_json::json!({ "token": "tok" })),
        };
        let r = execute_shopify(&node, &ctx(), &client).await;
        assert!(r.error.as_deref().unwrap_or("").contains("shop"));
    }

    #[tokio::test]
    async fn shopify_fails_without_token() {
        let client = reqwest::Client::new();
        let node = Node {
            id: "s2".into(),
            node_type: NodeType::Shopify,
            config: Some(serde_json::json!({ "shop": "mystore" })),
        };
        let r = execute_shopify(&node, &ctx(), &client).await;
        assert!(r.error.as_deref().unwrap_or("").contains("token"));
    }

    // ── Datadog (config validation only) ─────────────────────────────────────

    #[tokio::test]
    async fn datadog_fails_without_api_key() {
        let client = reqwest::Client::new();
        let node = Node {
            id: "d1".into(),
            node_type: NodeType::Datadog,
            config: Some(serde_json::json!({ "endpoint": "/api/v1/validate" })),
        };
        let r = execute_datadog(&node, &ctx(), &client).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn datadog_fails_without_endpoint() {
        let client = reqwest::Client::new();
        let node = Node {
            id: "d2".into(),
            node_type: NodeType::Datadog,
            config: Some(serde_json::json!({ "api_key": "abc123" })),
        };
        let r = execute_datadog(&node, &ctx(), &client).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }
}

// ── Slice 266: Salesforce ─────────────────────────────────────────────────────

// ── Slice 267: Freshdesk ──────────────────────────────────────────────────────

// ── Slice 268: Mailgun ────────────────────────────────────────────────────────

// ── Slice 269: Asana ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests_266_269 {
    use super::*;
    use workflow_core::{Node, NodeType};

    fn ctx() -> ExecutionContext {
        ExecutionContext {
            execution_id: "e1".into(),
            workflow_version_id: "v1".into(),
            input_json: "{}".into(),
            node_outputs: Default::default(),
            dry_run: false,
        }
    }

    // ── Salesforce ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn salesforce_fails_without_token() {
        let client = reqwest::Client::new();
        let node = Node {
            id: "sf1".into(),
            node_type: NodeType::Salesforce,
            config: Some(serde_json::json!({ "instance_url": "https://myorg.salesforce.com" })),
        };
        let r = execute_salesforce(&node, &ctx(), &client).await;
        assert!(r.error.as_deref().unwrap_or("").contains("token"));
    }

    #[tokio::test]
    async fn salesforce_fails_without_instance_url() {
        let client = reqwest::Client::new();
        let node = Node {
            id: "sf2".into(),
            node_type: NodeType::Salesforce,
            config: Some(serde_json::json!({ "token": "Bearer abc" })),
        };
        let r = execute_salesforce(&node, &ctx(), &client).await;
        assert!(r.error.as_deref().unwrap_or("").contains("instance_url"));
    }

    // ── Freshdesk ─────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn freshdesk_fails_without_api_key() {
        let client = reqwest::Client::new();
        let node = Node {
            id: "fd1".into(),
            node_type: NodeType::Freshdesk,
            config: Some(
                serde_json::json!({ "domain": "co.freshdesk.com", "endpoint": "/tickets" }),
            ),
        };
        let r = execute_freshdesk(&node, &ctx(), &client).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn freshdesk_fails_without_domain() {
        let client = reqwest::Client::new();
        let node = Node {
            id: "fd2".into(),
            node_type: NodeType::Freshdesk,
            config: Some(serde_json::json!({ "api_key": "abc", "endpoint": "/tickets" })),
        };
        let r = execute_freshdesk(&node, &ctx(), &client).await;
        assert!(r.error.as_deref().unwrap_or("").contains("domain"));
    }

    // ── Mailgun ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn mailgun_fails_without_api_key() {
        let client = reqwest::Client::new();
        let node = Node {
            id: "mg1".into(),
            node_type: NodeType::Mailgun,
            config: Some(serde_json::json!({ "domain": "mg.example.com", "to": "a@b.com" })),
        };
        let r = execute_mailgun(&node, &ctx(), &client).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn mailgun_fails_without_to() {
        let client = reqwest::Client::new();
        let node = Node {
            id: "mg2".into(),
            node_type: NodeType::Mailgun,
            config: Some(serde_json::json!({ "api_key": "key-abc", "domain": "mg.example.com" })),
        };
        let r = execute_mailgun(&node, &ctx(), &client).await;
        assert!(r.error.as_deref().unwrap_or("").contains("to"));
    }

    // ── Asana ─────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn asana_fails_without_token() {
        let client = reqwest::Client::new();
        let node = Node {
            id: "as1".into(),
            node_type: NodeType::Asana,
            config: Some(serde_json::json!({ "endpoint": "/tasks" })),
        };
        let r = execute_asana(&node, &ctx(), &client).await;
        assert!(r.error.as_deref().unwrap_or("").contains("token"));
    }

    #[tokio::test]
    async fn asana_fails_without_endpoint() {
        let client = reqwest::Client::new();
        let node = Node {
            id: "as2".into(),
            node_type: NodeType::Asana,
            config: Some(serde_json::json!({ "token": "1/abc" })),
        };
        let r = execute_asana(&node, &ctx(), &client).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }
}

// ── Slice 270: ServiceNow ─────────────────────────────────────────────────────

// ── Slice 271: Confluence ─────────────────────────────────────────────────────

// ── Slice 272: Bitbucket ──────────────────────────────────────────────────────

// ── Slice 273: Azure DevOps ───────────────────────────────────────────────────

#[cfg(test)]
mod tests_270_273 {
    use super::*;
    use workflow_core::{Node, NodeType};

    fn ctx() -> ExecutionContext {
        ExecutionContext {
            execution_id: "e1".into(),
            workflow_version_id: "v1".into(),
            input_json: "{}".into(),
            node_outputs: Default::default(),
            dry_run: false,
        }
    }

    // ── ServiceNow ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn servicenow_fails_without_instance() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "sn1".into(),
            node_type: NodeType::Servicenow,
            config: Some(serde_json::json!({ "username": "admin", "password": "pwd" })),
        };
        let r = execute_servicenow(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("instance"));
    }

    #[tokio::test]
    async fn servicenow_fails_without_credentials() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "sn2".into(),
            node_type: NodeType::Servicenow,
            config: Some(serde_json::json!({ "instance": "myco.service-now.com" })),
        };
        let r = execute_servicenow(&n, &ctx(), &c).await;
        assert!(r.error.is_some());
    }

    // ── Confluence ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn confluence_fails_without_base_url() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "cf1".into(),
            node_type: NodeType::Confluence,
            config: Some(serde_json::json!({ "token": "tok", "endpoint": "/rest/api/content" })),
        };
        let r = execute_confluence(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("base_url"));
    }

    #[tokio::test]
    async fn confluence_fails_without_auth() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "cf2".into(),
            node_type: NodeType::Confluence,
            config: Some(serde_json::json!({
                "base_url": "https://myco.atlassian.net/wiki",
                "endpoint": "/rest/api/content"
            })),
        };
        let r = execute_confluence(&n, &ctx(), &c).await;
        assert!(r.error.is_some());
    }

    // ── Bitbucket ─────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn bitbucket_fails_without_username() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "bb1".into(),
            node_type: NodeType::Bitbucket,
            config: Some(
                serde_json::json!({ "app_password": "pwd", "endpoint": "/repositories/ws/repo" }),
            ),
        };
        let r = execute_bitbucket(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("username"));
    }

    #[tokio::test]
    async fn bitbucket_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "bb2".into(),
            node_type: NodeType::Bitbucket,
            config: Some(serde_json::json!({ "username": "user", "app_password": "pwd" })),
        };
        let r = execute_bitbucket(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    // ── Azure DevOps ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn azure_devops_fails_without_pat() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "az1".into(),
            node_type: NodeType::AzureDevops,
            config: Some(
                serde_json::json!({ "organization": "myorg", "endpoint": "/build/builds" }),
            ),
        };
        let r = execute_azure_devops(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("pat"));
    }

    #[tokio::test]
    async fn azure_devops_fails_without_organization() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "az2".into(),
            node_type: NodeType::AzureDevops,
            config: Some(serde_json::json!({ "pat": "abc123", "endpoint": "/build/builds" })),
        };
        let r = execute_azure_devops(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("organization"));
    }
}

#[cfg(test)]
mod cn_llm_tests {
    use super::*;

    fn make_node(node_type: NodeType, config: serde_json::Value) -> Node {
        Node {
            id: "n1".into(),
            node_type,
            config: Some(config),
        }
    }

    fn ctx() -> ExecutionContext {
        ExecutionContext {
            execution_id: "e1".into(),
            workflow_version_id: "v1".into(),
            input_json: "{}".into(),
            node_outputs: std::collections::HashMap::new(),
            dry_run: false,
        }
    }

    #[tokio::test]
    async fn deepseek_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = make_node(
            NodeType::Deepseek,
            serde_json::json!({ "prompt_template": "hi" }),
        );
        let r = execute_deepseek(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn deepseek_fails_without_prompt() {
        let c = reqwest::Client::new();
        let n = make_node(NodeType::Deepseek, serde_json::json!({ "api_key": "sk-x" }));
        let r = execute_deepseek(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("prompt_template"));
    }

    #[tokio::test]
    async fn qwen_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = make_node(
            NodeType::Qwen,
            serde_json::json!({ "prompt_template": "hi" }),
        );
        let r = execute_qwen(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn zhipu_fails_without_prompt() {
        let c = reqwest::Client::new();
        let n = make_node(NodeType::Zhipu, serde_json::json!({ "api_key": "sk-x" }));
        let r = execute_zhipu(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("prompt_template"));
    }

    #[tokio::test]
    async fn moonshot_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = make_node(
            NodeType::Moonshot,
            serde_json::json!({ "prompt_template": "hi" }),
        );
        let r = execute_moonshot(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn doubao_fails_without_endpoint_id() {
        let c = reqwest::Client::new();
        let n = make_node(
            NodeType::Doubao,
            serde_json::json!({ "api_key": "sk-x", "prompt_template": "hi" }),
        );
        let r = execute_doubao(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint_id"));
    }

    #[tokio::test]
    async fn minimax_fails_without_group_id() {
        let c = reqwest::Client::new();
        let n = make_node(
            NodeType::Minimax,
            serde_json::json!({ "api_key": "sk-x", "prompt_template": "hi" }),
        );
        let r = execute_minimax(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("group_id"));
    }

    #[tokio::test]
    async fn ernie_fails_without_secret_key() {
        let c = reqwest::Client::new();
        let n = make_node(
            NodeType::Ernie,
            serde_json::json!({ "api_key": "sk-x", "prompt_template": "hi" }),
        );
        let r = execute_ernie(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("secret_key"));
    }

    #[tokio::test]
    async fn hunyuan_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = make_node(
            NodeType::Hunyuan,
            serde_json::json!({ "prompt_template": "hi" }),
        );
        let r = execute_hunyuan(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn deepseek_no_config_returns_error() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "n1".into(),
            node_type: NodeType::Deepseek,
            config: None,
        };
        let r = execute_deepseek(&n, &ctx(), &c).await;
        assert!(r.error.is_some());
    }
}
