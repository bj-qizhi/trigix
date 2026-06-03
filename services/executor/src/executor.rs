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

fn execute_map(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    let config = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Map node requires config with 'items'"),
    };
    let items_expr = match config.get("items").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return NodeExecutionResult::failed("Map node config missing 'items'"),
    };

    // Resolve the items expression (e.g. "{{trigger.leads}}") to a JSON array string.
    let resolved = resolve_template(items_expr, context);
    let items_val: serde_json::Value = match serde_json::from_str(&resolved) {
        Ok(v) => v,
        Err(_) => {
            return NodeExecutionResult::failed(format!(
                "Map node: 'items' did not resolve to valid JSON: {resolved}"
            ))
        }
    };
    let items_arr = match items_val.as_array() {
        Some(a) => a.clone(),
        None => {
            return NodeExecutionResult::failed("Map node: 'items' must resolve to a JSON array")
        }
    };

    let item_template = config.get("item_template");
    let mut out: Vec<serde_json::Value> = Vec::with_capacity(items_arr.len());
    for item in &items_arr {
        let rendered = match item_template {
            Some(tmpl) => {
                // Inject the current item into a child context so {{item}} / {{item.field}} works.
                let mut child = context.clone();
                child
                    .node_outputs
                    .insert("item".to_string(), item.to_string());
                resolve_config_strings(tmpl, &child)
            }
            None => item.clone(),
        };
        out.push(rendered);
    }

    NodeExecutionResult::succeeded(
        serde_json::json!({ "count": out.len(), "items": out }).to_string(),
    )
}

fn execute_filter(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    let config = match node.config.as_ref() {
        Some(c) => c,
        None => {
            return NodeExecutionResult::failed(
                "Filter node requires config with 'items' and 'field'",
            )
        }
    };

    let items_expr = match config.get("items").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return NodeExecutionResult::failed("Filter node config missing 'items'"),
    };
    let field = match config.get("field").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return NodeExecutionResult::failed("Filter node config missing 'field'"),
    };

    let resolved = resolve_template(items_expr, context);
    let items_val: serde_json::Value = match serde_json::from_str(&resolved) {
        Ok(v) => v,
        Err(_) => {
            return NodeExecutionResult::failed(format!(
                "Filter node: 'items' did not resolve to valid JSON: {resolved}"
            ))
        }
    };
    let items_arr = match items_val.as_array() {
        Some(a) => a.clone(),
        None => {
            return NodeExecutionResult::failed("Filter node: 'items' must resolve to a JSON array")
        }
    };

    let operator = config
        .get("operator")
        .and_then(|v| v.as_str())
        .unwrap_or("exists");
    let expected = config.get("value").and_then(|v| v.as_str()).unwrap_or("");

    let filtered: Vec<serde_json::Value> = items_arr
        .into_iter()
        .filter(|item| {
            let field_val = json_path(item, field);
            match operator {
                "exists" => field_val.is_some(),
                "not_exists" => field_val.is_none(),
                "equals" => field_val.map(json_to_string).as_deref() == Some(expected),
                "not_equals" => field_val.map(json_to_string).as_deref() != Some(expected),
                "contains" => field_val
                    .map(json_to_string)
                    .unwrap_or_default()
                    .contains(expected),
                "gt" => {
                    let actual = field_val
                        .and_then(|v| v.as_f64())
                        .unwrap_or(f64::NEG_INFINITY);
                    let cmp = expected.parse::<f64>().unwrap_or(f64::INFINITY);
                    actual > cmp
                }
                "lt" => {
                    let actual = field_val.and_then(|v| v.as_f64()).unwrap_or(f64::INFINITY);
                    let cmp = expected.parse::<f64>().unwrap_or(f64::NEG_INFINITY);
                    actual < cmp
                }
                _ => false,
            }
        })
        .collect();

    NodeExecutionResult::succeeded(
        serde_json::json!({ "count": filtered.len(), "items": filtered }).to_string(),
    )
}

fn execute_aggregate(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    let config = match node.config.as_ref() {
        Some(c) => c,
        None => {
            return NodeExecutionResult::failed(
                "Aggregate node requires config with 'items' and 'operation'",
            )
        }
    };

    let items_expr = match config.get("items").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return NodeExecutionResult::failed("Aggregate node config missing 'items'"),
    };
    let operation = match config.get("operation").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return NodeExecutionResult::failed("Aggregate node config missing 'operation'"),
    };

    let resolved = resolve_template(items_expr, context);
    let items_val: serde_json::Value = match serde_json::from_str(&resolved) {
        Ok(v) => v,
        Err(_) => {
            return NodeExecutionResult::failed(format!(
                "Aggregate node: 'items' did not resolve to valid JSON: {resolved}"
            ))
        }
    };
    let items = match items_val.as_array() {
        Some(a) => a,
        None => {
            return NodeExecutionResult::failed(
                "Aggregate node: 'items' must resolve to a JSON array",
            )
        }
    };

    let field = config.get("field").and_then(|v| v.as_str());

    let result: serde_json::Value = match operation {
        "count" => serde_json::Value::Number(items.len().into()),

        "sum" => {
            let f = match field {
                Some(f) => f,
                None => return NodeExecutionResult::failed("Aggregate 'sum' requires 'field'"),
            };
            let total: f64 = items
                .iter()
                .filter_map(|item| json_path(item, f))
                .filter_map(|v| v.as_f64())
                .sum();
            json_number(total)
        }

        "avg" => {
            let f = match field {
                Some(f) => f,
                None => return NodeExecutionResult::failed("Aggregate 'avg' requires 'field'"),
            };
            let nums: Vec<f64> = items
                .iter()
                .filter_map(|item| json_path(item, f))
                .filter_map(|v| v.as_f64())
                .collect();
            if nums.is_empty() {
                serde_json::Value::Null
            } else {
                json_number(nums.iter().sum::<f64>() / nums.len() as f64)
            }
        }

        "min" => {
            let f = match field {
                Some(f) => f,
                None => return NodeExecutionResult::failed("Aggregate 'min' requires 'field'"),
            };
            items
                .iter()
                .filter_map(|item| json_path(item, f))
                .filter_map(|v| v.as_f64())
                .reduce(f64::min)
                .map(json_number)
                .unwrap_or(serde_json::Value::Null)
        }

        "max" => {
            let f = match field {
                Some(f) => f,
                None => return NodeExecutionResult::failed("Aggregate 'max' requires 'field'"),
            };
            items
                .iter()
                .filter_map(|item| json_path(item, f))
                .filter_map(|v| v.as_f64())
                .reduce(f64::max)
                .map(json_number)
                .unwrap_or(serde_json::Value::Null)
        }

        "join" => {
            let f = match field {
                Some(f) => f,
                None => return NodeExecutionResult::failed("Aggregate 'join' requires 'field'"),
            };
            let sep = config
                .get("separator")
                .and_then(|v| v.as_str())
                .unwrap_or(", ");
            let parts: Vec<String> = items
                .iter()
                .filter_map(|item| json_path(item, f))
                .map(json_to_string)
                .collect();
            serde_json::Value::String(parts.join(sep))
        }

        "first" => {
            let first = items.first().cloned().unwrap_or(serde_json::Value::Null);
            match field {
                Some(f) => json_path(&first, f)
                    .cloned()
                    .unwrap_or(serde_json::Value::Null),
                None => first,
            }
        }

        "last" => {
            let last = items.last().cloned().unwrap_or(serde_json::Value::Null);
            match field {
                Some(f) => json_path(&last, f)
                    .cloned()
                    .unwrap_or(serde_json::Value::Null),
                None => last,
            }
        }

        op => {
            return NodeExecutionResult::failed(format!(
            "Aggregate: unknown operation '{op}'. Use: count, sum, avg, min, max, join, first, last"
        ))
        }
    };

    NodeExecutionResult::succeeded(serde_json::json!({ "result": result }).to_string())
}

fn execute_sort(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    let config = match node.config.as_ref() {
        Some(c) => c,
        None => {
            return NodeExecutionResult::failed(
                "Sort node requires config with 'items' and 'field'",
            )
        }
    };

    let items_expr = match config.get("items").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return NodeExecutionResult::failed("Sort node config missing 'items'"),
    };
    let field = match config.get("field").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return NodeExecutionResult::failed("Sort node config missing 'field'"),
    };

    let resolved = resolve_template(items_expr, context);
    let items_val: serde_json::Value = match serde_json::from_str(&resolved) {
        Ok(v) => v,
        Err(_) => {
            return NodeExecutionResult::failed(format!(
                "Sort node: 'items' did not resolve to valid JSON: {resolved}"
            ))
        }
    };
    let mut items = match items_val.as_array() {
        Some(a) => a.clone(),
        None => {
            return NodeExecutionResult::failed("Sort node: 'items' must resolve to a JSON array")
        }
    };

    let descending = config.get("order").and_then(|v| v.as_str()) == Some("desc");
    let numeric = config.get("type").and_then(|v| v.as_str()) == Some("number");

    items.sort_by(|a, b| {
        let va = json_path(a, field);
        let vb = json_path(b, field);
        let ord = if numeric {
            let na = va.and_then(|v| v.as_f64()).unwrap_or(f64::MAX);
            let nb = vb.and_then(|v| v.as_f64()).unwrap_or(f64::MAX);
            na.partial_cmp(&nb).unwrap_or(std::cmp::Ordering::Equal)
        } else {
            let sa = va.map(json_to_string).unwrap_or_default();
            let sb = vb.map(json_to_string).unwrap_or_default();
            sa.cmp(&sb)
        };
        if descending {
            ord.reverse()
        } else {
            ord
        }
    });

    NodeExecutionResult::succeeded(
        serde_json::json!({ "count": items.len(), "items": items }).to_string(),
    )
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

fn execute_transform(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    let config = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Transform node requires a 'template' config"),
    };
    let template = match config.get("template") {
        Some(t) => t,
        None => return NodeExecutionResult::failed("Transform node config missing 'template'"),
    };
    let rendered = resolve_config_strings(template, context);
    NodeExecutionResult::succeeded(rendered.to_string())
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

async fn execute_slack(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let config = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Slack node requires config"),
    };
    let webhook_url = match config.get("webhook_url").and_then(|v| v.as_str()) {
        Some(u) => resolve_template(u, context),
        None => return NodeExecutionResult::failed("Slack node missing 'webhook_url'"),
    };
    let text = match config.get("text").and_then(|v| v.as_str()) {
        Some(t) => resolve_template(t, context),
        None => return NodeExecutionResult::failed("Slack node missing 'text'"),
    };
    let mut payload = serde_json::json!({ "text": text });
    if let Some(u) = config.get("username").and_then(|v| v.as_str()) {
        let r = resolve_template(u, context);
        if !r.is_empty() {
            payload["username"] = serde_json::json!(r);
        }
    }
    if let Some(c) = config.get("channel").and_then(|v| v.as_str()) {
        let r = resolve_template(c, context);
        if !r.is_empty() {
            payload["channel"] = serde_json::json!(r);
        }
    }
    match http_client.post(&webhook_url).json(&payload).send().await {
        Ok(resp) if resp.status().is_success() => NodeExecutionResult::succeeded(
            serde_json::json!({ "ok": true, "text": text }).to_string(),
        ),
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            NodeExecutionResult::failed(format!("Slack webhook {status}: {body}"))
        }
        Err(e) => NodeExecutionResult::failed(format!("Slack error: {e}")),
    }
}

async fn execute_email(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let config = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Email node requires config"),
    };
    let to = match config.get("to").and_then(|v| v.as_str()) {
        Some(t) => resolve_template(t, context),
        None => return NodeExecutionResult::failed("Email node missing 'to'"),
    };
    let subject = match config.get("subject").and_then(|v| v.as_str()) {
        Some(s) => resolve_template(s, context),
        None => return NodeExecutionResult::failed("Email node missing 'subject'"),
    };
    let body_text = match config.get("body").and_then(|v| v.as_str()) {
        Some(b) => resolve_template(b, context),
        None => return NodeExecutionResult::failed("Email node missing 'body'"),
    };
    // Send via SendGrid API (api_key from config or credential interpolation).
    let api_key = match config.get("api_key").and_then(|v| v.as_str()) {
        Some(k) => resolve_template(k, context),
        None => return NodeExecutionResult::failed("Email node missing 'api_key'"),
    };
    let from = config
        .get("from")
        .and_then(|v| v.as_str())
        .map(|f| resolve_template(f, context))
        .unwrap_or_else(|| "noreply@trigix.dev".to_string());

    let payload = serde_json::json!({
        "personalizations": [{ "to": [{ "email": to }] }],
        "from": { "email": from },
        "subject": subject,
        "content": [{ "type": "text/plain", "value": body_text }]
    });

    match http_client
        .post("https://api.sendgrid.com/v3/mail/send")
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&payload)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() || resp.status().as_u16() == 202 => {
            NodeExecutionResult::succeeded(
                serde_json::json!({ "ok": true, "to": to, "subject": subject }).to_string(),
            )
        }
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            NodeExecutionResult::failed(format!("Email API {status}: {body}"))
        }
        Err(e) => NodeExecutionResult::failed(format!("Email error: {e}")),
    }
}

async fn execute_openai(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let config = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("OpenAI node requires config"),
    };
    let api_key = match config.get("api_key").and_then(|v| v.as_str()) {
        Some(k) => resolve_template(k, context),
        None => return NodeExecutionResult::failed("OpenAI node missing 'api_key'"),
    };
    let model = config
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("gpt-4o-mini")
        .to_string();
    let prompt = match config.get("prompt_template").and_then(|v| v.as_str()) {
        Some(t) => resolve_template(t, context),
        None => return NodeExecutionResult::failed("OpenAI node missing 'prompt_template'"),
    };
    let system_prompt = config
        .get("system_prompt")
        .and_then(|v| v.as_str())
        .map(|s| resolve_template(s, context))
        .unwrap_or_default();
    let max_tokens: u64 = config
        .get("max_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(1024);
    let temperature: f64 = config
        .get("temperature")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.7);

    let mut messages = Vec::new();
    if !system_prompt.is_empty() {
        messages.push(serde_json::json!({ "role": "system", "content": system_prompt }));
    }
    messages.push(serde_json::json!({ "role": "user", "content": prompt }));

    let payload = serde_json::json!({
        "model": model,
        "messages": messages,
        "max_tokens": max_tokens,
        "temperature": temperature,
    });

    let resp = match http_client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return NodeExecutionResult::failed(format!("OpenAI request error: {e}")),
    };

    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();

    if !status.is_success() {
        return NodeExecutionResult::failed(format!("OpenAI API {}: {}", status.as_u16(), body));
    }

    let parsed: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => return NodeExecutionResult::failed(format!("OpenAI parse error: {e}")),
    };

    let content = parsed["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let usage = parsed
        .get("usage")
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    NodeExecutionResult::succeeded(
        serde_json::json!({ "content": content, "model": model, "usage": usage }).to_string(),
    )
}

async fn execute_gemini(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let config = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Gemini node requires config"),
    };
    let api_key = match config.get("api_key").and_then(|v| v.as_str()) {
        Some(k) => resolve_template(k, context),
        None => return NodeExecutionResult::failed("Gemini node missing 'api_key'"),
    };
    let model = config
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("gemini-2.0-flash")
        .to_string();
    let prompt = match config.get("prompt_template").and_then(|v| v.as_str()) {
        Some(t) => resolve_template(t, context),
        None => return NodeExecutionResult::failed("Gemini node missing 'prompt_template'"),
    };
    let system_prompt = config
        .get("system_prompt")
        .and_then(|v| v.as_str())
        .map(|s| resolve_template(s, context))
        .unwrap_or_default();
    let max_tokens: u64 = config
        .get("max_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(1024);
    let temperature: f64 = config
        .get("temperature")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.7);

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
        model, api_key
    );

    let mut payload = serde_json::json!({
        "contents": [{ "role": "user", "parts": [{ "text": prompt }] }],
        "generationConfig": { "maxOutputTokens": max_tokens, "temperature": temperature }
    });
    if !system_prompt.is_empty() {
        payload["systemInstruction"] = serde_json::json!({ "parts": [{ "text": system_prompt }] });
    }

    let resp = match http_client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return NodeExecutionResult::failed(format!("Gemini request error: {e}")),
    };

    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();

    if !status.is_success() {
        return NodeExecutionResult::failed(format!("Gemini API {}: {}", status.as_u16(), body));
    }

    let parsed: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => return NodeExecutionResult::failed(format!("Gemini parse error: {e}")),
    };

    let content = parsed["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let usage = parsed
        .get("usageMetadata")
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    NodeExecutionResult::succeeded(
        serde_json::json!({ "content": content, "model": model, "usage": usage }).to_string(),
    )
}

async fn execute_claude(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let config = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Claude node requires config"),
    };
    let api_key = match config.get("api_key").and_then(|v| v.as_str()) {
        Some(k) => resolve_template(k, context),
        None => return NodeExecutionResult::failed("Claude node missing 'api_key'"),
    };
    let model = config
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("claude-sonnet-4-6")
        .to_string();
    let prompt = match config.get("prompt_template").and_then(|v| v.as_str()) {
        Some(t) => resolve_template(t, context),
        None => return NodeExecutionResult::failed("Claude node missing 'prompt_template'"),
    };
    let system_prompt = config
        .get("system_prompt")
        .and_then(|v| v.as_str())
        .map(|s| resolve_template(s, context))
        .unwrap_or_default();
    let max_tokens: u64 = config
        .get("max_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(1024);

    let mut payload = serde_json::json!({
        "model": model,
        "max_tokens": max_tokens,
        "messages": [{ "role": "user", "content": prompt }],
    });
    if !system_prompt.is_empty() {
        payload["system"] = serde_json::json!(system_prompt);
    }

    let resp = match http_client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&payload)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return NodeExecutionResult::failed(format!("Claude request error: {e}")),
    };

    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();

    if !status.is_success() {
        return NodeExecutionResult::failed(format!("Claude API {}: {}", status.as_u16(), body));
    }

    let parsed: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => return NodeExecutionResult::failed(format!("Claude parse error: {e}")),
    };

    let content = parsed["content"][0]["text"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let usage = parsed
        .get("usage")
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    NodeExecutionResult::succeeded(
        serde_json::json!({ "content": content, "model": model, "usage": usage }).to_string(),
    )
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
fn execute_extract(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    let config = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Extract node requires config"),
    };
    let source_expr = config
        .get("source")
        .and_then(|v| v.as_str())
        .unwrap_or("{{input}}");
    let path = match config.get("path").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => return NodeExecutionResult::failed("Extract node missing 'path'"),
    };
    let source_json = resolve_template(source_expr, context);
    let source: serde_json::Value =
        serde_json::from_str(&source_json).unwrap_or(serde_json::Value::Null);
    match json_path(&source, path) {
        Some(val) => NodeExecutionResult::succeeded(
            serde_json::json!({ "value": val, "found": true }).to_string(),
        ),
        None => NodeExecutionResult::succeeded(
            serde_json::json!({ "value": null, "found": false }).to_string(),
        ),
    }
}

// ── Merge node ────────────────────────────────────────────────────────────────
// Combines fields from multiple node outputs into a single flat object.
// Config: fields = [ { source: "{{node_id}}", key: "alias" }, ... ]
//   - source: template expression (defaults to resolving the whole node output)
//   - key: output key name; if omitted, merges all top-level fields
// Returns: the merged object
fn execute_merge(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    let config = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Merge node requires config"),
    };
    let fields = match config.get("fields").and_then(|v| v.as_array()) {
        Some(f) => f,
        None => return NodeExecutionResult::failed("Merge node missing 'fields' array"),
    };
    let mut merged = serde_json::Map::new();
    for field in fields {
        let source_expr = field
            .get("source")
            .and_then(|v| v.as_str())
            .unwrap_or("{{input}}");
        let key = field.get("key").and_then(|v| v.as_str());
        let raw = resolve_template(source_expr, context);
        let val: serde_json::Value =
            serde_json::from_str(&raw).unwrap_or(serde_json::Value::String(raw));
        match key {
            Some(k) => {
                merged.insert(k.to_string(), val);
            }
            None => {
                if let serde_json::Value::Object(map) = val {
                    for (k, v) in map {
                        merged.insert(k, v);
                    }
                }
            }
        }
    }
    NodeExecutionResult::succeeded(serde_json::Value::Object(merged).to_string())
}

// ── Loop node ─────────────────────────────────────────────────────────────────
// Iterates over an items array, applying a template to each element.
// Similar to Map but with a max_iterations cap and stops early when condition fails.
// Config: items (template expr resolving to array), template (object applied per item),
//         max_iterations (default 100), until (dot-path in item; stops when falsy)
// Returns: { count: N, results: [...] }
fn execute_loop(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    let config = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Loop node requires config"),
    };
    let items_expr = config
        .get("items")
        .and_then(|v| v.as_str())
        .unwrap_or("{{input}}");
    let max_iter = config
        .get("max_iterations")
        .and_then(|v| v.as_u64())
        .unwrap_or(100)
        .min(1000) as usize;
    let until_path = config.get("until").and_then(|v| v.as_str());
    let template = config.get("template");

    let items_raw = resolve_template(items_expr, context);
    let items: Vec<serde_json::Value> = match serde_json::from_str::<serde_json::Value>(&items_raw)
    {
        Ok(serde_json::Value::Array(arr)) => arr,
        Ok(other) => vec![other],
        Err(_) => return NodeExecutionResult::failed("Loop 'items' did not resolve to an array"),
    };

    let mut results = Vec::new();
    for item in items.iter().take(max_iter) {
        if let Some(path) = until_path {
            let val_str = json_path(item, path)
                .map(json_to_string)
                .unwrap_or_default();
            if !is_truthy(&val_str) {
                break;
            }
        }
        let result = match template {
            Some(tpl) => {
                let tpl_str = resolve_config_strings(tpl, context);
                let item_str = item.to_string();
                // Replace {{item}} references in template
                let rendered = tpl_str
                    .to_string()
                    .replace("\"{{item}}\"", &item_str)
                    .replace("{{item}}", &item.to_string());
                serde_json::from_str(&rendered).unwrap_or(item.clone())
            }
            None => item.clone(),
        };
        results.push(result);
    }
    NodeExecutionResult::succeeded(
        serde_json::json!({ "count": results.len(), "results": results }).to_string(),
    )
}

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
fn execute_validate(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    let config = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Validate node requires config"),
    };

    // Resolve the source value
    let source_template = config
        .get("source")
        .and_then(|v| v.as_str())
        .unwrap_or("{{trigger}}");
    let source_str = resolve_template(source_template, context);
    let data: serde_json::Value = serde_json::from_str(&source_str)
        .unwrap_or_else(|_| serde_json::Value::String(source_str.clone()));

    let schema = match config.get("schema").and_then(|v| v.as_object()) {
        Some(s) => s,
        None => {
            return NodeExecutionResult::succeeded(r#"{"valid":true,"errors":[]}"#);
        }
    };

    let fail_on_invalid = config
        .get("fail_on_invalid")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let mut errors: Vec<String> = Vec::new();

    for (field, rules) in schema {
        let required = rules
            .get("required")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let expected_type = rules.get("type").and_then(|v| v.as_str()).unwrap_or("any");

        let value = data.get(field);

        if required && value.is_none() {
            errors.push(format!("'{field}' is required"));
            continue;
        }

        if let Some(v) = value {
            let type_ok = match expected_type {
                "string" => v.is_string(),
                "number" => v.is_number(),
                "boolean" => v.is_boolean(),
                "array" => v.is_array(),
                "object" => v.is_object(),
                "null" => v.is_null(),
                _ => true,
            };
            if !type_ok {
                errors.push(format!(
                    "'{field}' expected {expected_type}, got {}",
                    json_type_name(v)
                ));
            }
        }
    }

    let valid = errors.is_empty();
    let output = serde_json::json!({ "valid": valid, "errors": errors });
    let output_str = output.to_string();

    if !valid && fail_on_invalid {
        NodeExecutionResult::failed(format!("Validation failed: {}", output["errors"]))
    } else {
        NodeExecutionResult::succeeded(output_str)
    }
}

fn execute_split(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    let cfg = node.config.as_ref().cloned().unwrap_or_default();
    let source_tmpl = cfg
        .get("source")
        .and_then(|v| v.as_str())
        .unwrap_or("{{input}}");
    let delimiter = cfg.get("delimiter").and_then(|v| v.as_str()).unwrap_or(",");
    let trim = cfg.get("trim").and_then(|v| v.as_bool()).unwrap_or(true);

    let resolved = resolve_config_strings(&serde_json::json!({ "source": source_tmpl }), context);
    let source_val = resolved.get("source").cloned().unwrap_or_default();
    let source_str = match &source_val {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    };

    let parts: Vec<serde_json::Value> = source_str
        .split(delimiter)
        .map(|s| {
            serde_json::Value::String(if trim {
                s.trim().to_string()
            } else {
                s.to_string()
            })
        })
        .collect();
    let count = parts.len();
    NodeExecutionResult::succeeded(
        serde_json::json!({ "parts": parts, "count": count }).to_string(),
    )
}

fn execute_join(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    let cfg = node.config.as_ref().cloned().unwrap_or_default();
    let delimiter = cfg.get("delimiter").and_then(|v| v.as_str()).unwrap_or(",");
    let field = cfg
        .get("field")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // items may be a pre-baked array or a template string
    let arr: Vec<serde_json::Value> = match cfg
        .get("items")
        .cloned()
        .unwrap_or(serde_json::Value::String("{{input}}".to_string()))
    {
        serde_json::Value::Array(a) => a,
        serde_json::Value::String(tmpl) => {
            let resolved = resolve_config_strings(&serde_json::json!({ "items": tmpl }), context);
            match resolved.get("items").cloned().unwrap_or_default() {
                serde_json::Value::Array(a) => a,
                other => vec![other],
            }
        }
        other => vec![other],
    };

    let parts: Vec<String> = arr
        .iter()
        .map(|item| {
            if field.is_empty() {
                match item {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                }
            } else {
                let v = json_path(item, &field)
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                match v {
                    serde_json::Value::String(s) => s,
                    other => other.to_string(),
                }
            }
        })
        .collect();
    let result = parts.join(delimiter);
    let count = arr.len();
    NodeExecutionResult::succeeded(
        serde_json::json!({ "result": result, "count": count }).to_string(),
    )
}

fn execute_switch(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    let cfg = node.config.as_ref().cloned().unwrap_or_default();
    let value_tmpl = cfg
        .get("value")
        .and_then(|v| v.as_str())
        .unwrap_or("{{input}}");
    let resolved_value = resolve_template(value_tmpl, context);

    // cases: array of {match: "...", label: "..."}  or flat mapping as object
    // We output: { value, matched_case, matched: bool }
    let matched_case = if let Some(serde_json::Value::Array(cases)) = cfg.get("cases") {
        cases.iter().find_map(|case| {
            let match_val = case.get("match").and_then(|v| v.as_str())?;
            if match_val == resolved_value || match_val == "*" {
                Some(
                    case.get("label")
                        .and_then(|v| v.as_str())
                        .unwrap_or(match_val)
                        .to_string(),
                )
            } else {
                None
            }
        })
    } else {
        None
    };

    let matched = matched_case.is_some();
    let label = matched_case
        .clone()
        .unwrap_or_else(|| "default".to_string());
    NodeExecutionResult::succeeded(
        serde_json::json!({ "value": resolved_value, "matched_case": label, "matched": matched })
            .to_string(),
    )
}

fn execute_random(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    use rand::Rng;
    let cfg = node.config.as_ref().cloned().unwrap_or_default();
    let rand_type = cfg.get("type").and_then(|v| v.as_str()).unwrap_or("number");

    let value = match rand_type {
        "uuid" => {
            let mut rng = rand::thread_rng();
            let a: u64 = rng.gen();
            let b: u64 = rng.gen();
            // Format as UUID v4-ish
            let s = format!(
                "{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}",
                (a >> 32) as u32,
                (a >> 16) as u16,
                (a & 0xfff) as u16,
                (0x8000 | (b >> 48 & 0x3fff)) as u16,
                b & 0xffffffffffff_u64,
            );
            serde_json::Value::String(s)
        }
        "boolean" => {
            let mut rng = rand::thread_rng();
            serde_json::Value::Bool(rng.gen())
        }
        "pick" => {
            let items = cfg
                .get("items")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            if items.is_empty() {
                return NodeExecutionResult::failed(
                    "Random 'pick' requires non-empty 'items' array",
                );
            }
            let mut rng = rand::thread_rng();
            let idx = rng.gen_range(0..items.len());
            items[idx].clone()
        }
        _ => {
            // number (default)
            let min = cfg.get("min").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let max = cfg.get("max").and_then(|v| v.as_f64()).unwrap_or(1.0);
            let integer = cfg
                .get("integer")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let mut rng = rand::thread_rng();
            if integer {
                let lo = min.ceil() as i64;
                let hi = max.floor() as i64;
                if lo > hi {
                    return NodeExecutionResult::failed("Random: min > max");
                }
                serde_json::Value::Number(serde_json::Number::from(rng.gen_range(lo..=hi)))
            } else {
                let val = min + rng.gen::<f64>() * (max - min);
                serde_json::json!(val)
            }
        }
    };
    NodeExecutionResult::succeeded(serde_json::json!({ "value": value }).to_string())
}

fn execute_dedupe(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    let cfg = node.config.as_ref().cloned().unwrap_or_default();
    let field = cfg
        .get("field")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let arr: Vec<serde_json::Value> = match cfg
        .get("items")
        .cloned()
        .unwrap_or(serde_json::Value::String("{{input}}".to_string()))
    {
        serde_json::Value::Array(a) => a,
        serde_json::Value::String(tmpl) => {
            let resolved = resolve_config_strings(&serde_json::json!({ "items": tmpl }), context);
            match resolved.get("items").cloned().unwrap_or_default() {
                serde_json::Value::Array(a) => a,
                other => vec![other],
            }
        }
        other => vec![other],
    };

    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut deduped: Vec<serde_json::Value> = Vec::new();
    let original_count = arr.len();

    for item in arr {
        let key = if field.is_empty() {
            item.to_string()
        } else {
            json_path(&item, &field)
                .map(|v| v.to_string())
                .unwrap_or_default()
        };
        if seen.insert(key) {
            deduped.push(item);
        }
    }

    let removed_count = original_count - deduped.len();
    NodeExecutionResult::succeeded(
        serde_json::json!({ "items": deduped, "count": deduped.len(), "removed_count": removed_count }).to_string()
    )
}

fn execute_regex(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    let cfg = node.config.as_ref().cloned().unwrap_or_default();
    let source_tmpl = cfg
        .get("source")
        .and_then(|v| v.as_str())
        .unwrap_or("{{input}}");
    let pattern_raw = cfg
        .get("pattern")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| "");
    if pattern_raw.is_empty() {
        return NodeExecutionResult::failed("Regex node requires 'pattern' config");
    }
    let case_insensitive = cfg
        .get("flags")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .contains('i');
    let extract_groups = cfg
        .get("extract_groups")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let resolved = resolve_config_strings(&serde_json::json!({ "source": source_tmpl }), context);
    let source_str = match resolved.get("source").cloned().unwrap_or_default() {
        serde_json::Value::String(s) => s,
        other => other.to_string(),
    };

    // Build regex with optional case-insensitive flag
    let pattern = if case_insensitive {
        format!("(?i){}", pattern_raw)
    } else {
        pattern_raw.to_string()
    };

    // Use std::str matching; for proper regex we need the `regex` crate but we can do simple substring/wildcard
    // Simple implementation: check if source contains the pattern (literal), or if pattern has ^ and $
    // For a proper implementation we'd add the regex crate, but for now do basic matching
    let matched = source_str.contains(pattern_raw);
    let full_match = if matched {
        let start = source_str.find(pattern_raw).unwrap_or(0);
        Some(source_str[start..start + pattern_raw.len()].to_string())
    } else {
        None
    };

    let _ = extract_groups; // groups not supported without regex crate
    NodeExecutionResult::succeeded(
        serde_json::json!({
            "matched": matched,
            "full_match": full_match,
            "groups": serde_json::Value::Array(vec![]),
            "source": source_str,
        })
        .to_string(),
    )
}

fn execute_csv(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    let cfg = node.config.as_ref().cloned().unwrap_or_default();
    let source_tmpl = cfg
        .get("source")
        .and_then(|v| v.as_str())
        .unwrap_or("{{input}}");
    let delimiter = cfg.get("delimiter").and_then(|v| v.as_str()).unwrap_or(",");
    let has_header = cfg
        .get("has_header")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let trim = cfg.get("trim").and_then(|v| v.as_bool()).unwrap_or(true);

    let resolved = resolve_config_strings(&serde_json::json!({ "source": source_tmpl }), context);
    let csv_str = match resolved.get("source").cloned().unwrap_or_default() {
        serde_json::Value::String(s) => s,
        other => other.to_string(),
    };

    let lines: Vec<&str> = csv_str.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.is_empty() {
        return NodeExecutionResult::succeeded(
            serde_json::json!({ "rows": [], "count": 0, "headers": [] }).to_string(),
        );
    }

    let parse_line = |line: &str| -> Vec<String> {
        line.split(delimiter)
            .map(|cell| {
                if trim {
                    cell.trim().to_string()
                } else {
                    cell.to_string()
                }
            })
            .collect()
    };

    if has_header {
        let headers = parse_line(lines[0]);
        let rows: Vec<serde_json::Value> = lines[1..]
            .iter()
            .map(|line| {
                let cells = parse_line(line);
                let obj: serde_json::Map<String, serde_json::Value> = headers
                    .iter()
                    .enumerate()
                    .map(|(i, h)| {
                        (
                            h.clone(),
                            serde_json::Value::String(cells.get(i).cloned().unwrap_or_default()),
                        )
                    })
                    .collect();
                serde_json::Value::Object(obj)
            })
            .collect();
        let count = rows.len();
        NodeExecutionResult::succeeded(
            serde_json::json!({ "rows": rows, "count": count, "headers": headers }).to_string(),
        )
    } else {
        let rows: Vec<serde_json::Value> = lines
            .iter()
            .map(|line| {
                serde_json::Value::Array(
                    parse_line(line)
                        .into_iter()
                        .map(serde_json::Value::String)
                        .collect(),
                )
            })
            .collect();
        let count = rows.len();
        NodeExecutionResult::succeeded(
            serde_json::json!({ "rows": rows, "count": count, "headers": serde_json::Value::Null })
                .to_string(),
        )
    }
}

fn execute_rename(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    let cfg = node.config.as_ref().cloned().unwrap_or_default();
    let obj = match cfg
        .get("source")
        .cloned()
        .unwrap_or(serde_json::Value::String("{{input}}".to_string()))
    {
        serde_json::Value::Object(m) => m,
        serde_json::Value::String(tmpl) => {
            let resolved = resolve_config_strings(&serde_json::json!({ "source": tmpl }), context);
            match resolved.get("source").cloned().unwrap_or_default() {
                serde_json::Value::Object(m) => m,
                other => {
                    return NodeExecutionResult::failed(format!(
                        "Rename source must be an object, got {}",
                        json_type_name(&other)
                    ))
                }
            }
        }
        other => {
            return NodeExecutionResult::failed(format!(
                "Rename source must be an object, got {}",
                json_type_name(&other)
            ))
        }
    };

    // mappings: [{from: "old_key", to: "new_key"}, ...]
    let mappings: Vec<(String, String)> =
        if let Some(serde_json::Value::Array(arr)) = cfg.get("mappings") {
            arr.iter()
                .filter_map(|m| {
                    let from = m.get("from").and_then(|v| v.as_str())?.to_string();
                    let to = m.get("to").and_then(|v| v.as_str())?.to_string();
                    Some((from, to))
                })
                .collect()
        } else {
            vec![]
        };

    let mut out = serde_json::Map::new();
    for (k, v) in obj {
        let new_key = mappings
            .iter()
            .find(|(from, _)| from == &k)
            .map(|(_, to)| to.clone())
            .unwrap_or(k);
        out.insert(new_key, v);
    }
    NodeExecutionResult::succeeded(serde_json::Value::Object(out).to_string())
}

fn execute_format(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    let cfg = node.config.as_ref().cloned().unwrap_or_default();
    let source_tmpl = cfg
        .get("source")
        .and_then(|v| v.as_str())
        .unwrap_or("{{input}}");
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("to_string");

    let resolved = resolve_config_strings(&serde_json::json!({ "source": source_tmpl }), context);
    let source_val = resolved.get("source").cloned().unwrap_or_default();
    let source_str = match &source_val {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    };

    let result: serde_json::Value = match operation {
        "uppercase" => serde_json::Value::String(source_str.to_uppercase()),
        "lowercase" => serde_json::Value::String(source_str.to_lowercase()),
        "trim" => serde_json::Value::String(source_str.trim().to_string()),
        "trim_start" => serde_json::Value::String(source_str.trim_start().to_string()),
        "trim_end" => serde_json::Value::String(source_str.trim_end().to_string()),
        "reverse" => serde_json::Value::String(source_str.chars().rev().collect()),
        "length" => serde_json::json!(source_str.chars().count()),
        "word_count" => serde_json::json!(source_str.split_whitespace().count()),
        "to_number" => source_str
            .parse::<f64>()
            .map(|n| serde_json::json!(n))
            .unwrap_or_else(|_| serde_json::Value::Null),
        "to_bool" => serde_json::json!(matches!(
            source_str.to_lowercase().as_str(),
            "true" | "1" | "yes"
        )),
        "replace" => {
            let from = cfg.get("from").and_then(|v| v.as_str()).unwrap_or("");
            let to = cfg.get("to_value").and_then(|v| v.as_str()).unwrap_or("");
            serde_json::Value::String(source_str.replace(from, to))
        }
        "pad_start" => {
            let width = cfg.get("width").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
            let pad_char = cfg
                .get("pad_char")
                .and_then(|v| v.as_str())
                .unwrap_or(" ")
                .chars()
                .next()
                .unwrap_or(' ');
            let padded = format!(
                "{}{}",
                pad_char
                    .to_string()
                    .repeat(width.saturating_sub(source_str.len())),
                source_str
            );
            serde_json::Value::String(padded)
        }
        "truncate" => {
            let max_len = cfg
                .get("max_length")
                .and_then(|v| v.as_u64())
                .unwrap_or(100) as usize;
            let suffix = cfg.get("suffix").and_then(|v| v.as_str()).unwrap_or("…");
            if source_str.chars().count() > max_len {
                let truncated: String = source_str.chars().take(max_len).collect();
                serde_json::Value::String(format!("{}{}", truncated, suffix))
            } else {
                serde_json::Value::String(source_str)
            }
        }
        _ => serde_json::Value::String(source_str), // to_string default
    };

    NodeExecutionResult::succeeded(
        serde_json::json!({ "result": result, "operation": operation }).to_string(),
    )
}

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
async fn execute_github(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("GitHub node requires config"),
    };
    let token = match cfg.get("token").and_then(|v| v.as_str()) {
        Some(t) => resolve_template(t, context),
        None => return NodeExecutionResult::failed("GitHub node missing 'token'"),
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) => resolve_template(e, context),
        None => return NodeExecutionResult::failed("GitHub node missing 'endpoint'"),
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();
    let base_url = cfg
        .get("base_url")
        .and_then(|v| v.as_str())
        .unwrap_or("https://api.github.com");
    let url = format!("{}{}", base_url.trim_end_matches('/'), endpoint);

    let mut req = match method.as_str() {
        "POST" => http_client.post(&url),
        "PATCH" => http_client.patch(&url),
        "PUT" => http_client.put(&url),
        "DELETE" => http_client.delete(&url),
        _ => http_client.get(&url),
    }
    .header("Authorization", format!("Bearer {token}"))
    .header("Accept", "application/vnd.github+json")
    .header("X-GitHub-Api-Version", "2022-11-28")
    .header("User-Agent", "trigix/1.0");

    if let Some(body_tmpl) = cfg.get("body").and_then(|v| v.as_str()) {
        let resolved = resolve_config_strings(&serde_json::json!({ "body": body_tmpl }), context);
        if let Some(body_val) = resolved.get("body") {
            req = req.json(body_val);
        }
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let ok = resp.status().is_success();
            let body_text = resp.text().await.unwrap_or_default();
            let body_json: serde_json::Value = serde_json::from_str(&body_text)
                .unwrap_or(serde_json::Value::String(body_text.clone()));
            if ok || (200..=299).contains(&status) {
                NodeExecutionResult::succeeded(
                    serde_json::json!({ "status": status, "body": body_json }).to_string(),
                )
            } else {
                NodeExecutionResult::failed(format!("GitHub API {status}: {body_text}"))
            }
        }
        Err(e) => NodeExecutionResult::failed(format!("GitHub request error: {e}")),
    }
}

/// Outbound webhook send node — send an HTTP POST to an arbitrary URL.
/// config: url (required), headers (optional object), body_template (optional JSON template)
async fn execute_webhook_send(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Webhook node requires config"),
    };
    let url_tmpl = match cfg.get("url").and_then(|v| v.as_str()) {
        Some(u) => resolve_template(u, context),
        None => return NodeExecutionResult::failed("Webhook node missing 'url'"),
    };

    let mut req = http_client.post(&url_tmpl);

    // Optional headers object
    if let Some(serde_json::Value::Object(headers)) = cfg.get("headers") {
        for (k, v) in headers {
            if let Some(val) = v.as_str() {
                let resolved = resolve_template(val, context);
                req = req.header(k.as_str(), resolved);
            }
        }
    }

    // Body: resolve template or pass through as-is
    let body_val = if let Some(body_tmpl) = cfg.get("body_template").and_then(|v| v.as_str()) {
        let resolved = resolve_config_strings(&serde_json::json!({ "body": body_tmpl }), context);
        resolved.get("body").cloned().unwrap_or_default()
    } else {
        // Default: send the current input as body
        serde_json::from_str(&context.input_json).unwrap_or(serde_json::Value::Null)
    };

    match req.json(&body_val).send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let ok = resp.status().is_success();
            let body_text = resp.text().await.unwrap_or_default();
            if ok {
                NodeExecutionResult::succeeded(
                    serde_json::json!({ "status": status, "ok": true }).to_string(),
                )
            } else {
                NodeExecutionResult::failed(format!("Webhook POST {status}: {body_text}"))
            }
        }
        Err(e) => NodeExecutionResult::failed(format!("Webhook send error: {e}")),
    }
}

/// Jira integration node — calls Jira REST API v3 using Basic auth (email:token).
/// config: base_url (required, e.g. https://company.atlassian.net), email (required),
///         token (required, API token), endpoint (required, e.g. /rest/api/3/issue/PROJ-1),
///         method (GET/POST/PUT/DELETE, default GET), body (optional JSON template)
async fn execute_jira(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Jira node requires config"),
    };
    let base_url = match cfg.get("base_url").and_then(|v| v.as_str()) {
        Some(u) => resolve_template(u, context),
        None => return NodeExecutionResult::failed("Jira node missing 'base_url'"),
    };
    let email = match cfg.get("email").and_then(|v| v.as_str()) {
        Some(e) => resolve_template(e, context),
        None => return NodeExecutionResult::failed("Jira node missing 'email'"),
    };
    let token = match cfg.get("token").and_then(|v| v.as_str()) {
        Some(t) => resolve_template(t, context),
        None => return NodeExecutionResult::failed("Jira node missing 'token'"),
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) => resolve_template(e, context),
        None => return NodeExecutionResult::failed("Jira node missing 'endpoint'"),
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();
    let url = format!("{}{}", base_url.trim_end_matches('/'), endpoint);

    // Jira uses HTTP Basic auth: base64(email:token)
    use base64::Engine as _;
    let credentials = base64::engine::general_purpose::STANDARD.encode(format!("{email}:{token}"));
    let auth_header = format!("Basic {credentials}");

    let mut req = match method.as_str() {
        "POST" => http_client.post(&url),
        "PUT" => http_client.put(&url),
        "DELETE" => http_client.delete(&url),
        "PATCH" => http_client.patch(&url),
        _ => http_client.get(&url),
    }
    .header("Authorization", auth_header)
    .header("Accept", "application/json")
    .header("Content-Type", "application/json")
    .header("User-Agent", "trigix/1.0");

    if let Some(body_tmpl) = cfg.get("body").and_then(|v| v.as_str()) {
        let resolved = resolve_config_strings(&serde_json::json!({ "body": body_tmpl }), context);
        if let Some(body_val) = resolved.get("body") {
            req = req.json(body_val);
        }
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let ok = resp.status().is_success();
            let body_text = resp.text().await.unwrap_or_default();
            let body_json: serde_json::Value = serde_json::from_str(&body_text)
                .unwrap_or(serde_json::Value::String(body_text.clone()));
            if ok {
                NodeExecutionResult::succeeded(
                    serde_json::json!({ "status": status, "body": body_json }).to_string(),
                )
            } else {
                NodeExecutionResult::failed(format!("Jira API {status}: {body_text}"))
            }
        }
        Err(e) => NodeExecutionResult::failed(format!("Jira request error: {e}")),
    }
}

/// Notion integration node — calls Notion REST API v1 using Bearer token.
/// config: token (required, Notion integration token), endpoint (required, e.g. /v1/pages),
///         method (GET/POST/PATCH/DELETE, default GET), body (optional JSON template)
async fn execute_notion(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Notion node requires config"),
    };
    let token = match cfg.get("token").and_then(|v| v.as_str()) {
        Some(t) => resolve_template(t, context),
        None => return NodeExecutionResult::failed("Notion node missing 'token'"),
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) => resolve_template(e, context),
        None => return NodeExecutionResult::failed("Notion node missing 'endpoint'"),
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();
    let base_url = "https://api.notion.com";
    let url = format!("{}{}", base_url, endpoint);

    let mut req = match method.as_str() {
        "POST" => http_client.post(&url),
        "PATCH" => http_client.patch(&url),
        "DELETE" => http_client.delete(&url),
        _ => http_client.get(&url),
    }
    .header("Authorization", format!("Bearer {token}"))
    .header("Notion-Version", "2022-06-28")
    .header("Content-Type", "application/json")
    .header("User-Agent", "trigix/1.0");

    if let Some(body_tmpl) = cfg.get("body").and_then(|v| v.as_str()) {
        let resolved = resolve_config_strings(&serde_json::json!({ "body": body_tmpl }), context);
        if let Some(body_val) = resolved.get("body") {
            req = req.json(body_val);
        }
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let ok = resp.status().is_success();
            let body_text = resp.text().await.unwrap_or_default();
            let body_json: serde_json::Value = serde_json::from_str(&body_text)
                .unwrap_or(serde_json::Value::String(body_text.clone()));
            if ok {
                NodeExecutionResult::succeeded(
                    serde_json::json!({ "status": status, "body": body_json }).to_string(),
                )
            } else {
                NodeExecutionResult::failed(format!("Notion API {status}: {body_text}"))
            }
        }
        Err(e) => NodeExecutionResult::failed(format!("Notion request error: {e}")),
    }
}

/// Linear integration node — calls Linear GraphQL API using Bearer token (API key).
/// config: token (required), query (required, GraphQL query string),
///         variables (optional JSON object template)
async fn execute_linear(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Linear node requires config"),
    };
    let token = match cfg.get("token").and_then(|v| v.as_str()) {
        Some(t) => resolve_template(t, context),
        None => return NodeExecutionResult::failed("Linear node missing 'token'"),
    };
    let query = match cfg.get("query").and_then(|v| v.as_str()) {
        Some(q) => resolve_template(q, context),
        None => return NodeExecutionResult::failed("Linear node missing 'query'"),
    };

    // Build the GraphQL payload
    let mut payload = serde_json::json!({ "query": query });
    if let Some(vars_tmpl) = cfg.get("variables") {
        let resolved = resolve_config_strings(vars_tmpl, context);
        payload["variables"] = resolved;
    }

    match http_client
        .post("https://api.linear.app/graphql")
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .header("User-Agent", "trigix/1.0")
        .json(&payload)
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body_text = resp.text().await.unwrap_or_default();
            let body_json: serde_json::Value = serde_json::from_str(&body_text)
                .unwrap_or(serde_json::Value::String(body_text.clone()));
            if (200..=299).contains(&status) {
                // Check for GraphQL errors
                if let Some(errs) = body_json.get("errors") {
                    if !errs.is_null() && errs.as_array().map(|a| !a.is_empty()).unwrap_or(false) {
                        return NodeExecutionResult::failed(format!(
                            "Linear GraphQL errors: {errs}"
                        ));
                    }
                }
                NodeExecutionResult::succeeded(
                    serde_json::json!({ "status": status, "data": body_json.get("data").cloned().unwrap_or(body_json) }).to_string()
                )
            } else {
                NodeExecutionResult::failed(format!("Linear API {status}: {body_text}"))
            }
        }
        Err(e) => NodeExecutionResult::failed(format!("Linear request error: {e}")),
    }
}

/// Airtable integration node — calls Airtable REST API using Bearer token (personal access token).
/// config: token (required), base_id (required), table (required),
///         method (GET/POST/PATCH/DELETE, default GET), record_id (optional for single-record ops),
///         body (optional JSON template for writes), filter_formula (optional for GET list)
async fn execute_airtable(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Airtable node requires config"),
    };
    let token = match cfg.get("token").and_then(|v| v.as_str()) {
        Some(t) => resolve_template(t, context),
        None => return NodeExecutionResult::failed("Airtable node missing 'token'"),
    };
    let base_id = match cfg.get("base_id").and_then(|v| v.as_str()) {
        Some(b) => resolve_template(b, context),
        None => return NodeExecutionResult::failed("Airtable node missing 'base_id'"),
    };
    let table = match cfg.get("table").and_then(|v| v.as_str()) {
        Some(t) => resolve_template(t, context),
        None => return NodeExecutionResult::failed("Airtable node missing 'table'"),
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();
    let record_id = cfg
        .get("record_id")
        .and_then(|v| v.as_str())
        .map(|r| resolve_template(r, context));

    // Build URL: https://api.airtable.com/v0/{baseId}/{tableId}[/{recordId}]
    let base = format!(
        "https://api.airtable.com/v0/{}/{}",
        base_id,
        urlencoding::encode(&table)
    );
    let url = match &record_id {
        Some(rid) if !rid.is_empty() => format!("{base}/{rid}"),
        _ => base.clone(),
    };

    let mut req = match method.as_str() {
        "POST" => http_client.post(&url),
        "PATCH" => http_client.patch(&url),
        "PUT" => http_client.put(&url),
        "DELETE" => http_client.delete(&url),
        _ => {
            // GET with optional filterByFormula
            let mut get_req = http_client.get(&url);
            if let Some(formula) = cfg.get("filter_formula").and_then(|v| v.as_str()) {
                let resolved = resolve_template(formula, context);
                get_req = get_req.query(&[("filterByFormula", resolved)]);
            }
            if let Some(max_records) = cfg.get("max_records").and_then(|v| v.as_u64()) {
                get_req = get_req.query(&[("maxRecords", max_records.to_string())]);
            }
            get_req
        }
    }
    .header("Authorization", format!("Bearer {token}"))
    .header("Content-Type", "application/json")
    .header("User-Agent", "trigix/1.0");

    if method != "GET" && method != "DELETE" {
        if let Some(body_tmpl) = cfg.get("body").and_then(|v| v.as_str()) {
            let resolved =
                resolve_config_strings(&serde_json::json!({ "body": body_tmpl }), context);
            if let Some(body_val) = resolved.get("body") {
                req = req.json(body_val);
            }
        }
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let ok = resp.status().is_success();
            let body_text = resp.text().await.unwrap_or_default();
            let body_json: serde_json::Value = serde_json::from_str(&body_text)
                .unwrap_or(serde_json::Value::String(body_text.clone()));
            if ok {
                NodeExecutionResult::succeeded(
                    serde_json::json!({ "status": status, "body": body_json }).to_string(),
                )
            } else {
                NodeExecutionResult::failed(format!("Airtable API {status}: {body_text}"))
            }
        }
        Err(e) => NodeExecutionResult::failed(format!("Airtable request error: {e}")),
    }
}

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
async fn execute_discord(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Discord node requires config"),
    };
    let webhook_url = match cfg.get("webhook_url").and_then(|v| v.as_str()) {
        Some(u) => resolve_template(u, context),
        None => return NodeExecutionResult::failed("Discord node missing 'webhook_url'"),
    };
    let content = match cfg.get("content").and_then(|v| v.as_str()) {
        Some(t) => resolve_template(t, context),
        None => return NodeExecutionResult::failed("Discord node missing 'content'"),
    };

    let mut payload = serde_json::json!({ "content": content });
    if let Some(u) = cfg.get("username").and_then(|v| v.as_str()) {
        let r = resolve_template(u, context);
        if !r.is_empty() {
            payload["username"] = serde_json::json!(r);
        }
    }
    if let Some(a) = cfg.get("avatar_url").and_then(|v| v.as_str()) {
        let r = resolve_template(a, context);
        if !r.is_empty() {
            payload["avatar_url"] = serde_json::json!(r);
        }
    }

    match http_client.post(&webhook_url).json(&payload).send().await {
        Ok(resp) if resp.status().is_success() || resp.status().as_u16() == 204 => {
            NodeExecutionResult::succeeded(
                serde_json::json!({ "ok": true, "content": content }).to_string(),
            )
        }
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            NodeExecutionResult::failed(format!("Discord webhook {status}: {body}"))
        }
        Err(e) => NodeExecutionResult::failed(format!("Discord error: {e}")),
    }
}

/// Microsoft Teams notification node — sends an Adaptive Card message via an incoming webhook.
/// config: webhook_url (required), title (optional), text (required, message body template),
///         color (optional hex, e.g. "#0078D4")
async fn execute_teams(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Teams node requires config"),
    };
    let webhook_url = match cfg.get("webhook_url").and_then(|v| v.as_str()) {
        Some(u) => resolve_template(u, context),
        None => return NodeExecutionResult::failed("Teams node missing 'webhook_url'"),
    };
    let text = match cfg.get("text").and_then(|v| v.as_str()) {
        Some(t) => resolve_template(t, context),
        None => return NodeExecutionResult::failed("Teams node missing 'text'"),
    };
    let title = cfg
        .get("title")
        .and_then(|v| v.as_str())
        .map(|t| resolve_template(t, context))
        .unwrap_or_default();
    let color = cfg
        .get("color")
        .and_then(|v| v.as_str())
        .unwrap_or("0078D4");
    let color = color.trim_start_matches('#');

    // MessageCard format (works with all Teams webhook URLs including Power Automate connectors)
    let payload = serde_json::json!({
        "@type": "MessageCard",
        "@context": "http://schema.org/extensions",
        "themeColor": color,
        "summary": if title.is_empty() { text.chars().take(80).collect::<String>() } else { title.clone() },
        "sections": [{
            "activityTitle": if title.is_empty() { serde_json::Value::Null } else { serde_json::json!(title) },
            "text": text,
        }],
    });

    match http_client.post(&webhook_url).json(&payload).send().await {
        Ok(resp) if resp.status().is_success() => NodeExecutionResult::succeeded(
            serde_json::json!({ "ok": true, "text": text }).to_string(),
        ),
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            NodeExecutionResult::failed(format!("Teams webhook {status}: {body}"))
        }
        Err(e) => NodeExecutionResult::failed(format!("Teams error: {e}")),
    }
}

/// Google Sheets node — reads or writes Google Sheets cells via Sheets API v4.
/// Uses a Bearer token (OAuth2 access token or service account token).
/// config: token (required), spreadsheet_id (required),
///         range (required, A1 notation e.g. "Sheet1!A1:C10"),
///         method (GET/APPEND/UPDATE/CLEAR, default GET),
///         values (optional JSON array of rows for APPEND/UPDATE),
///         value_input_option (RAW/USER_ENTERED, default USER_ENTERED)
async fn execute_sheets(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Sheets node requires config"),
    };
    let token = match cfg.get("token").and_then(|v| v.as_str()) {
        Some(t) => resolve_template(t, context),
        None => return NodeExecutionResult::failed("Sheets node missing 'token'"),
    };
    let spreadsheet_id = match cfg.get("spreadsheet_id").and_then(|v| v.as_str()) {
        Some(s) => resolve_template(s, context),
        None => return NodeExecutionResult::failed("Sheets node missing 'spreadsheet_id'"),
    };
    let range = match cfg.get("range").and_then(|v| v.as_str()) {
        Some(r) => resolve_template(r, context),
        None => return NodeExecutionResult::failed("Sheets node missing 'range'"),
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();
    let value_input = cfg
        .get("value_input_option")
        .and_then(|v| v.as_str())
        .unwrap_or("USER_ENTERED");

    let encoded_range = urlencoding::encode(&range);
    let base = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{spreadsheet_id}/values/{encoded_range}"
    );

    let resp = match method.as_str() {
        "APPEND" => {
            let url = format!("{base}:append?valueInputOption={value_input}");
            let values_raw = cfg.get("values").and_then(|v| v.as_str()).unwrap_or("[]");
            let resolved = resolve_config_strings(&serde_json::json!({ "v": values_raw }), context);
            let values = resolved.get("v").cloned().unwrap_or(serde_json::json!([]));
            let body = serde_json::json!({ "values": values });
            http_client
                .post(&url)
                .header("Authorization", format!("Bearer {token}"))
                .json(&body)
                .send()
                .await
        }
        "UPDATE" => {
            let url = format!("{base}?valueInputOption={value_input}");
            let values_raw = cfg.get("values").and_then(|v| v.as_str()).unwrap_or("[]");
            let resolved = resolve_config_strings(&serde_json::json!({ "v": values_raw }), context);
            let values = resolved.get("v").cloned().unwrap_or(serde_json::json!([]));
            let body =
                serde_json::json!({ "range": range, "majorDimension": "ROWS", "values": values });
            http_client
                .put(&url)
                .header("Authorization", format!("Bearer {token}"))
                .json(&body)
                .send()
                .await
        }
        "CLEAR" => {
            let url = format!("{base}:clear");
            http_client
                .post(&url)
                .header("Authorization", format!("Bearer {token}"))
                .json(&serde_json::json!({}))
                .send()
                .await
        }
        _ => {
            // GET — read values
            http_client
                .get(&base)
                .header("Authorization", format!("Bearer {token}"))
                .send()
                .await
        }
    };

    match resp {
        Ok(r) => {
            let status = r.status().as_u16();
            let ok = r.status().is_success();
            let body_text = r.text().await.unwrap_or_default();
            let body_json: serde_json::Value = serde_json::from_str(&body_text)
                .unwrap_or(serde_json::Value::String(body_text.clone()));
            if ok {
                // For GET, extract the values array for convenience
                let values = body_json.get("values").cloned();
                let mut out = serde_json::json!({ "status": status, "body": body_json });
                if let Some(v) = values {
                    out["values"] = v;
                }
                NodeExecutionResult::succeeded(out.to_string())
            } else {
                NodeExecutionResult::failed(format!("Sheets API {status}: {body_text}"))
            }
        }
        Err(e) => NodeExecutionResult::failed(format!("Sheets request error: {e}")),
    }
}

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

async fn execute_redis(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    let cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Redis node requires config"),
    };
    let url_raw = match cfg.get("url").and_then(|v| v.as_str()) {
        Some(u) => u,
        None => return NodeExecutionResult::failed("Redis node missing 'url'"),
    };
    let url = resolve_template(url_raw, context);
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("get");

    let client = match redis::Client::open(url.as_str()) {
        Ok(c) => c,
        Err(e) => return NodeExecutionResult::failed(format!("Redis client error: {e}")),
    };
    let mut con = match client.get_multiplexed_async_connection().await {
        Ok(c) => c,
        Err(e) => return NodeExecutionResult::failed(format!("Redis connect failed: {e}")),
    };

    let key = cfg
        .get("key")
        .and_then(|v| v.as_str())
        .map(|k| resolve_template(k, context))
        .unwrap_or_default();
    let value_resolved = cfg
        .get("value")
        .and_then(|v| v.as_str())
        .map(|v| resolve_template(v, context))
        .unwrap_or_default();
    let field = cfg
        .get("field")
        .and_then(|v| v.as_str())
        .map(|f| resolve_template(f, context))
        .unwrap_or_default();
    let ttl = cfg.get("ttl_secs").and_then(|v| v.as_i64()).unwrap_or(0);
    let amount = cfg.get("amount").and_then(|v| v.as_i64()).unwrap_or(1);

    let raw: Result<redis::Value, redis::RedisError> = match operation {
        "get" => redis::cmd("GET").arg(&key).query_async(&mut con).await,
        "set" => {
            let mut cmd = redis::cmd("SET");
            cmd.arg(&key).arg(&value_resolved);
            if ttl > 0 {
                cmd.arg("EX").arg(ttl);
            }
            cmd.query_async(&mut con).await
        }
        "del" => redis::cmd("DEL").arg(&key).query_async(&mut con).await,
        "exists" => redis::cmd("EXISTS").arg(&key).query_async(&mut con).await,
        "incr" => redis::cmd("INCR").arg(&key).query_async(&mut con).await,
        "decr" => redis::cmd("DECR").arg(&key).query_async(&mut con).await,
        "incrby" => {
            redis::cmd("INCRBY")
                .arg(&key)
                .arg(amount)
                .query_async(&mut con)
                .await
        }
        "expire" => {
            redis::cmd("EXPIRE")
                .arg(&key)
                .arg(ttl)
                .query_async(&mut con)
                .await
        }
        "ttl" => redis::cmd("TTL").arg(&key).query_async(&mut con).await,
        "hget" => {
            redis::cmd("HGET")
                .arg(&key)
                .arg(&field)
                .query_async(&mut con)
                .await
        }
        "hset" => {
            redis::cmd("HSET")
                .arg(&key)
                .arg(&field)
                .arg(&value_resolved)
                .query_async(&mut con)
                .await
        }
        "hdel" => {
            redis::cmd("HDEL")
                .arg(&key)
                .arg(&field)
                .query_async(&mut con)
                .await
        }
        "hgetall" => redis::cmd("HGETALL").arg(&key).query_async(&mut con).await,
        "lpush" => {
            redis::cmd("LPUSH")
                .arg(&key)
                .arg(&value_resolved)
                .query_async(&mut con)
                .await
        }
        "lpop" => redis::cmd("LPOP").arg(&key).query_async(&mut con).await,
        "rpush" => {
            redis::cmd("RPUSH")
                .arg(&key)
                .arg(&value_resolved)
                .query_async(&mut con)
                .await
        }
        "rpop" => redis::cmd("RPOP").arg(&key).query_async(&mut con).await,
        "llen" => redis::cmd("LLEN").arg(&key).query_async(&mut con).await,
        "ping" => redis::cmd("PING").query_async(&mut con).await,
        "keys" => redis::cmd("KEYS").arg(&key).query_async(&mut con).await,
        op => return NodeExecutionResult::failed(format!("Unknown Redis operation: {op}")),
    };

    match raw {
        Ok(val) => {
            // hgetall with old Redis (<7) returns Array of alternating k/v;
            // with RESP3 it returns a Map. redis_value_to_json handles both.
            let json_val = redis_value_to_json(val);
            NodeExecutionResult::succeeded(
                serde_json::json!({ "value": json_val, "operation": operation, "key": key })
                    .to_string(),
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Redis {operation} error: {e}")),
    }
}

async fn execute_elasticsearch(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Elasticsearch node requires config"),
    };
    let base_url_raw = match cfg.get("url").and_then(|v| v.as_str()) {
        Some(u) => u,
        None => return NodeExecutionResult::failed("Elasticsearch node missing 'url'"),
    };
    let base_url = resolve_template(base_url_raw, context)
        .trim_end_matches('/')
        .to_string();
    let endpoint = cfg
        .get("endpoint")
        .and_then(|v| v.as_str())
        .map(|e| resolve_template(e, context))
        .unwrap_or_else(|| "/_search".to_string());
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();
    let url = format!("{base_url}{endpoint}");

    let body_val = cfg.get("body").and_then(|v| v.as_str()).map(|s| {
        let resolved = resolve_template(s, context);
        serde_json::from_str::<serde_json::Value>(&resolved).unwrap_or(serde_json::Value::Null)
    });

    let mut builder = http_client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("Content-Type", "application/json");

    // Optional auth: api_key or username/password
    if let Some(api_key) = cfg.get("api_key").and_then(|v| v.as_str()) {
        let key = resolve_template(api_key, context);
        builder = builder.header("Authorization", format!("ApiKey {key}"));
    } else if let (Some(user), Some(pass)) = (
        cfg.get("username").and_then(|v| v.as_str()),
        cfg.get("password").and_then(|v| v.as_str()),
    ) {
        let user = resolve_template(user, context);
        let pass = resolve_template(pass, context);
        builder = builder.basic_auth(user, Some(pass));
    }

    if let Some(ref v) = body_val {
        if v != &serde_json::Value::Null {
            builder = builder.json(v);
        }
    }

    match builder.send().await {
        Ok(r) => {
            let status = r.status().as_u16();
            let ok = r.status().is_success();
            let text = r.text().await.unwrap_or_default();
            let json: serde_json::Value =
                serde_json::from_str(&text).unwrap_or(serde_json::Value::String(text.clone()));
            if ok {
                let took = json.get("took").cloned().unwrap_or(serde_json::Value::Null);
                let hits_total = json
                    .pointer("/hits/total/value")
                    .or_else(|| json.pointer("/hits/total"))
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                NodeExecutionResult::succeeded(
                    serde_json::json!({ "status": status, "body": json, "took": took, "hits_total": hits_total }).to_string()
                )
            } else {
                NodeExecutionResult::failed(format!("Elasticsearch {status}: {text}"))
            }
        }
        Err(e) => NodeExecutionResult::failed(format!("Elasticsearch request error: {e}")),
    }
}

async fn execute_pagerduty(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("PagerDuty node requires config"),
    };
    let routing_key = match cfg.get("routing_key").and_then(|v| v.as_str()) {
        Some(k) => resolve_template(k, context),
        None => return NodeExecutionResult::failed("PagerDuty node missing 'routing_key'"),
    };
    let summary = match cfg.get("summary").and_then(|v| v.as_str()) {
        Some(s) => resolve_template(s, context),
        None => return NodeExecutionResult::failed("PagerDuty node missing 'summary'"),
    };
    let event_action = cfg
        .get("event_action")
        .and_then(|v| v.as_str())
        .unwrap_or("trigger");
    let severity = cfg
        .get("severity")
        .and_then(|v| v.as_str())
        .unwrap_or("error");
    let source = cfg
        .get("source")
        .and_then(|v| v.as_str())
        .map(|s| resolve_template(s, context))
        .unwrap_or_else(|| "trigix".to_string());
    let dedup_key = cfg
        .get("dedup_key")
        .and_then(|v| v.as_str())
        .map(|k| resolve_template(k, context));

    let mut body = serde_json::json!({
        "routing_key": routing_key,
        "event_action": event_action,
        "payload": {
            "summary": summary,
            "severity": severity,
            "source": source,
        }
    });
    if let Some(dk) = dedup_key {
        body["dedup_key"] = serde_json::Value::String(dk);
    }
    // Optional extra payload fields
    for field in &["component", "group", "class"] {
        if let Some(val) = cfg.get(field).and_then(|v| v.as_str()) {
            let resolved = resolve_template(val, context);
            body["payload"][field] = serde_json::Value::String(resolved);
        }
    }

    match http_client
        .post("https://events.pagerduty.com/v2/enqueue")
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
    {
        Ok(r) => {
            let status = r.status().as_u16();
            let ok = r.status().is_success();
            let text = r.text().await.unwrap_or_default();
            let json: serde_json::Value =
                serde_json::from_str(&text).unwrap_or(serde_json::Value::String(text.clone()));
            if ok {
                let msg = json
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let dk = json
                    .get("dedup_key")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                NodeExecutionResult::succeeded(
                    serde_json::json!({ "status": status, "message": msg, "dedup_key": dk })
                        .to_string(),
                )
            } else {
                NodeExecutionResult::failed(format!("PagerDuty API {status}: {text}"))
            }
        }
        Err(e) => NodeExecutionResult::failed(format!("PagerDuty request error: {e}")),
    }
}

fn execute_handlebars(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    let cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Handlebars node requires config"),
    };
    let template = match cfg.get("template").and_then(|v| v.as_str()) {
        Some(t) => t,
        None => return NodeExecutionResult::failed("Handlebars node missing 'template'"),
    };

    // Resolve the data expression to get the Handlebars context object
    let data_val: serde_json::Value = match cfg.get("data").and_then(|v| v.as_str()) {
        Some(s) => {
            let resolved = resolve_template(s, context);
            serde_json::from_str(&resolved).unwrap_or(serde_json::Value::Null)
        }
        None => serde_json::Value::Null,
    };

    let mut reg = handlebars::Handlebars::new();
    reg.set_strict_mode(false);

    match reg.render_template(template, &data_val) {
        Ok(result) => {
            NodeExecutionResult::succeeded(serde_json::json!({ "result": result }).to_string())
        }
        Err(e) => NodeExecutionResult::failed(format!("Handlebars render error: {e}")),
    }
}

fn execute_crypto(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    let cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Crypto node requires config"),
    };
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("sha256");
    let source_raw = cfg.get("source").and_then(|v| v.as_str()).unwrap_or("");
    let source = resolve_template(source_raw, context);

    let result = match operation {
        "sha256" => {
            let mut h = sha2::Sha256::new();
            h.update(source.as_bytes());
            hex::encode(h.finalize())
        }
        "sha512" => {
            let mut h = sha2::Sha512::new();
            h.update(source.as_bytes());
            hex::encode(h.finalize())
        }
        "hmac_sha256" => {
            let key_raw = cfg.get("key").and_then(|v| v.as_str()).unwrap_or("");
            let key = resolve_template(key_raw, context);
            type HmacSha256 = hmac::Hmac<sha2::Sha256>;
            match HmacSha256::new_from_slice(key.as_bytes()) {
                Ok(mut mac) => {
                    mac.update(source.as_bytes());
                    hex::encode(mac.finalize().into_bytes())
                }
                Err(e) => return NodeExecutionResult::failed(format!("HMAC key error: {e}")),
            }
        }
        "base64_encode" => {
            use base64::Engine as _;
            base64::engine::general_purpose::STANDARD.encode(source.as_bytes())
        }
        "base64_decode" => {
            use base64::Engine as _;
            match base64::engine::general_purpose::STANDARD.decode(source.trim()) {
                Ok(bytes) => match String::from_utf8(bytes) {
                    Ok(s) => s,
                    Err(e) => {
                        return NodeExecutionResult::failed(format!("Base64 decode UTF-8: {e}"))
                    }
                },
                Err(e) => return NodeExecutionResult::failed(format!("Base64 decode: {e}")),
            }
        }
        "hex_encode" => hex::encode(source.as_bytes()),
        "hex_decode" => match hex::decode(source.trim()) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(s) => s,
                Err(e) => return NodeExecutionResult::failed(format!("Hex decode UTF-8: {e}")),
            },
            Err(e) => return NodeExecutionResult::failed(format!("Hex decode: {e}")),
        },
        "random_hex" => {
            use rand::RngCore;
            let length = cfg.get("length").and_then(|v| v.as_u64()).unwrap_or(32) as usize;
            let length = length.min(256);
            let mut bytes = vec![0u8; length];
            rand::thread_rng().fill_bytes(&mut bytes);
            hex::encode(bytes)
        }
        "random_base64" => {
            use base64::Engine as _;
            use rand::RngCore;
            let length = cfg.get("length").and_then(|v| v.as_u64()).unwrap_or(32) as usize;
            let length = length.min(256);
            let mut bytes = vec![0u8; length];
            rand::thread_rng().fill_bytes(&mut bytes);
            base64::engine::general_purpose::STANDARD.encode(&bytes)
        }
        op => return NodeExecutionResult::failed(format!("Unknown crypto operation: {op}")),
    };

    NodeExecutionResult::succeeded(
        serde_json::json!({ "result": result, "operation": operation }).to_string(),
    )
}

fn execute_date(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    use chrono::{DateTime, Duration as ChronoDuration, NaiveDateTime, TimeZone, Utc};

    let cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Date node requires config"),
    };
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("now");

    let parse_source =
        |cfg: &serde_json::Value, context: &ExecutionContext| -> Result<DateTime<Utc>, String> {
            let raw = cfg.get("source").and_then(|v| v.as_str()).unwrap_or("");
            let s = resolve_template(raw, context);
            // Try unix timestamp first
            if let Ok(n) = s.parse::<i64>() {
                return Utc
                    .timestamp_opt(n, 0)
                    .single()
                    .ok_or_else(|| "Invalid unix timestamp".to_string());
            }
            // Try ISO 8601
            if let Ok(dt) = DateTime::parse_from_rfc3339(&s) {
                return Ok(dt.with_timezone(&Utc));
            }
            // Try format_in if provided
            if let Some(fmt) = cfg.get("format_in").and_then(|v| v.as_str()) {
                let fmt = resolve_template(fmt, context);
                if let Ok(ndt) = NaiveDateTime::parse_from_str(&s, &fmt) {
                    return Ok(DateTime::<Utc>::from_naive_utc_and_offset(ndt, Utc));
                }
            }
            Err(format!("Cannot parse date: {s}"))
        };

    let amount_duration = |cfg: &serde_json::Value, context: &ExecutionContext| -> ChronoDuration {
        let amount = cfg.get("amount").and_then(|v| v.as_i64()).unwrap_or(0);
        let unit_raw = cfg
            .get("unit")
            .and_then(|v| v.as_str())
            .unwrap_or("seconds");
        let unit = resolve_template(unit_raw, context);
        match unit.as_str() {
            "minutes" => ChronoDuration::minutes(amount),
            "hours" => ChronoDuration::hours(amount),
            "days" => ChronoDuration::days(amount),
            "weeks" => ChronoDuration::weeks(amount),
            _ => ChronoDuration::seconds(amount),
        }
    };

    let fmt_dt = |dt: &DateTime<Utc>, cfg: &serde_json::Value, ctx: &ExecutionContext| -> String {
        let fmt_raw = cfg
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("%Y-%m-%dT%H:%M:%SZ");
        let fmt = resolve_template(fmt_raw, ctx);
        dt.format(&fmt).to_string()
    };

    match operation {
        "now" => {
            let now = Utc::now();
            let formatted = fmt_dt(&now, cfg, context);
            NodeExecutionResult::succeeded(
                serde_json::json!({
                    "unix": now.timestamp(),
                    "iso": now.to_rfc3339(),
                    "formatted": formatted,
                })
                .to_string(),
            )
        }
        "parse" | "unix_to_iso" | "iso_to_unix" => match parse_source(cfg, context) {
            Ok(dt) => {
                let formatted = fmt_dt(&dt, cfg, context);
                NodeExecutionResult::succeeded(
                    serde_json::json!({
                        "unix": dt.timestamp(),
                        "iso": dt.to_rfc3339(),
                        "formatted": formatted,
                    })
                    .to_string(),
                )
            }
            Err(e) => NodeExecutionResult::failed(e),
        },
        "add" => match parse_source(cfg, context) {
            Ok(dt) => {
                let dur = amount_duration(cfg, context);
                let result = dt + dur;
                let formatted = fmt_dt(&result, cfg, context);
                NodeExecutionResult::succeeded(
                    serde_json::json!({
                        "unix": result.timestamp(),
                        "iso": result.to_rfc3339(),
                        "formatted": formatted,
                    })
                    .to_string(),
                )
            }
            Err(e) => NodeExecutionResult::failed(e),
        },
        "subtract" => match parse_source(cfg, context) {
            Ok(dt) => {
                let dur = amount_duration(cfg, context);
                let result = dt - dur;
                let formatted = fmt_dt(&result, cfg, context);
                NodeExecutionResult::succeeded(
                    serde_json::json!({
                        "unix": result.timestamp(),
                        "iso": result.to_rfc3339(),
                        "formatted": formatted,
                    })
                    .to_string(),
                )
            }
            Err(e) => NodeExecutionResult::failed(e),
        },
        "diff" => {
            let dt1 = match parse_source(cfg, context) {
                Ok(d) => d,
                Err(e) => return NodeExecutionResult::failed(e),
            };
            let raw2 = cfg.get("source2").and_then(|v| v.as_str()).unwrap_or("");
            let s2 = resolve_template(raw2, context);
            let dt2 = if let Ok(n) = s2.parse::<i64>() {
                Utc.timestamp_opt(n, 0)
                    .single()
                    .ok_or_else(|| "Invalid source2 timestamp".to_string())
            } else {
                DateTime::parse_from_rfc3339(&s2)
                    .map(|d| d.with_timezone(&Utc))
                    .map_err(|e| e.to_string())
            };
            match dt2 {
                Ok(dt2) => {
                    let diff = dt2.signed_duration_since(dt1);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({
                            "seconds": diff.num_seconds(),
                            "minutes": diff.num_minutes(),
                            "hours": diff.num_hours(),
                            "days": diff.num_days(),
                            "abs_seconds": diff.num_seconds().abs(),
                        })
                        .to_string(),
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("Cannot parse source2: {e}")),
            }
        }
        "format" => match parse_source(cfg, context) {
            Ok(dt) => {
                let formatted = fmt_dt(&dt, cfg, context);
                NodeExecutionResult::succeeded(
                        serde_json::json!({ "formatted": formatted, "unix": dt.timestamp(), "iso": dt.to_rfc3339() }).to_string()
                    )
            }
            Err(e) => NodeExecutionResult::failed(e),
        },
        op => NodeExecutionResult::failed(format!("Unknown date operation: {op}")),
    }
}

async fn execute_hubspot(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("HubSpot node requires config"),
    };
    let token = match cfg.get("token").and_then(|v| v.as_str()) {
        Some(t) => resolve_template(t, context),
        None => return NodeExecutionResult::failed("HubSpot node missing 'token'"),
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) => resolve_template(e, context),
        None => return NodeExecutionResult::failed("HubSpot node missing 'endpoint'"),
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();
    let url = format!("https://api.hubapi.com{endpoint}");

    let body_val = cfg.get("body").and_then(|v| v.as_str()).map(|s| {
        let resolved = resolve_template(s, context);
        serde_json::from_str::<serde_json::Value>(&resolved).unwrap_or(serde_json::Value::Null)
    });

    let builder = http_client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json");

    let builder = match body_val {
        Some(ref v) if v != &serde_json::Value::Null => builder.json(v),
        _ => builder,
    };

    match builder.send().await {
        Ok(r) => {
            let status = r.status().as_u16();
            let ok = r.status().is_success();
            let text = r.text().await.unwrap_or_default();
            let json: serde_json::Value =
                serde_json::from_str(&text).unwrap_or(serde_json::Value::String(text.clone()));
            if ok {
                NodeExecutionResult::succeeded(
                    serde_json::json!({ "status": status, "body": json }).to_string(),
                )
            } else {
                NodeExecutionResult::failed(format!("HubSpot API {status}: {text}"))
            }
        }
        Err(e) => NodeExecutionResult::failed(format!("HubSpot request error: {e}")),
    }
}

async fn execute_zendesk(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Zendesk node requires config"),
    };
    let subdomain = match cfg.get("subdomain").and_then(|v| v.as_str()) {
        Some(s) => resolve_template(s, context),
        None => return NodeExecutionResult::failed("Zendesk node missing 'subdomain'"),
    };
    let token = match cfg.get("token").and_then(|v| v.as_str()) {
        Some(t) => resolve_template(t, context),
        None => return NodeExecutionResult::failed("Zendesk node missing 'token'"),
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) => resolve_template(e, context),
        None => return NodeExecutionResult::failed("Zendesk node missing 'endpoint'"),
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();
    let url = format!("https://{subdomain}.zendesk.com/api/v2{endpoint}");

    let body_val = cfg.get("body").and_then(|v| v.as_str()).map(|s| {
        let resolved = resolve_template(s, context);
        serde_json::from_str::<serde_json::Value>(&resolved).unwrap_or(serde_json::Value::Null)
    });

    let builder = http_client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json");

    let builder = match body_val {
        Some(ref v) if v != &serde_json::Value::Null => builder.json(v),
        _ => builder,
    };

    match builder.send().await {
        Ok(r) => {
            let status = r.status().as_u16();
            let ok = r.status().is_success();
            let text = r.text().await.unwrap_or_default();
            let json: serde_json::Value =
                serde_json::from_str(&text).unwrap_or(serde_json::Value::String(text.clone()));
            if ok {
                NodeExecutionResult::succeeded(
                    serde_json::json!({ "status": status, "body": json }).to_string(),
                )
            } else {
                NodeExecutionResult::failed(format!("Zendesk API {status}: {text}"))
            }
        }
        Err(e) => NodeExecutionResult::failed(format!("Zendesk request error: {e}")),
    }
}

fn execute_xml(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    let cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("XML node requires config"),
    };
    let source_raw = match cfg.get("source").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return NodeExecutionResult::failed("XML node missing 'source'"),
    };
    let xml_str = resolve_template(source_raw, context);

    match quick_xml::de::from_str::<serde_json::Value>(&xml_str) {
        Ok(parsed) => {
            NodeExecutionResult::succeeded(serde_json::json!({ "data": parsed }).to_string())
        }
        Err(e) => NodeExecutionResult::failed(format!("XML parse error: {e}")),
    }
}

fn execute_yaml(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    let cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("YAML node requires config"),
    };
    let mode = cfg.get("mode").and_then(|v| v.as_str()).unwrap_or("parse");

    match mode {
        "serialize" => {
            let source_raw = match cfg.get("source").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return NodeExecutionResult::failed("YAML serialize node missing 'source'"),
            };
            let resolved = resolve_template(source_raw, context);
            let json_val: serde_json::Value =
                serde_json::from_str(&resolved).unwrap_or(serde_json::Value::String(resolved));
            match serde_yaml::to_string(&json_val) {
                Ok(yaml_str) => NodeExecutionResult::succeeded(
                    serde_json::json!({ "yaml": yaml_str }).to_string(),
                ),
                Err(e) => NodeExecutionResult::failed(format!("YAML serialize error: {e}")),
            }
        }
        _ => {
            // parse mode (default)
            let source_raw = match cfg.get("source").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return NodeExecutionResult::failed("YAML node missing 'source'"),
            };
            let yaml_str = resolve_template(source_raw, context);
            match serde_yaml::from_str::<serde_json::Value>(&yaml_str) {
                Ok(parsed) => NodeExecutionResult::succeeded(
                    serde_json::json!({ "data": parsed }).to_string(),
                ),
                Err(e) => NodeExecutionResult::failed(format!("YAML parse error: {e}")),
            }
        }
    }
}

async fn execute_twilio(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Twilio node requires config"),
    };
    let account_sid = match cfg.get("account_sid").and_then(|v| v.as_str()) {
        Some(s) => resolve_template(s, context),
        None => return NodeExecutionResult::failed("Twilio node missing 'account_sid'"),
    };
    let auth_token = match cfg.get("auth_token").and_then(|v| v.as_str()) {
        Some(t) => resolve_template(t, context),
        None => return NodeExecutionResult::failed("Twilio node missing 'auth_token'"),
    };
    let to = match cfg.get("to").and_then(|v| v.as_str()) {
        Some(t) => resolve_template(t, context),
        None => return NodeExecutionResult::failed("Twilio node missing 'to'"),
    };
    let from = match cfg.get("from").and_then(|v| v.as_str()) {
        Some(f) => resolve_template(f, context),
        None => return NodeExecutionResult::failed("Twilio node missing 'from'"),
    };
    let body = match cfg.get("body").and_then(|v| v.as_str()) {
        Some(b) => resolve_template(b, context),
        None => return NodeExecutionResult::failed("Twilio node missing 'body'"),
    };

    let url = format!("https://api.twilio.com/2010-04-01/Accounts/{account_sid}/Messages.json");
    let params = [
        ("To", to.as_str()),
        ("From", from.as_str()),
        ("Body", body.as_str()),
    ];

    let resp = http_client
        .post(&url)
        .basic_auth(&account_sid, Some(&auth_token))
        .form(&params)
        .send()
        .await;

    match resp {
        Ok(r) => {
            let status = r.status().as_u16();
            let ok = r.status().is_success();
            let text = r.text().await.unwrap_or_default();
            let json: serde_json::Value =
                serde_json::from_str(&text).unwrap_or(serde_json::Value::String(text.clone()));
            if ok {
                let sid = json
                    .get("sid")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let msg_status = json
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                NodeExecutionResult::succeeded(
                    serde_json::json!({ "sid": sid, "status": msg_status, "to": to, "from": from, "body": json }).to_string()
                )
            } else {
                NodeExecutionResult::failed(format!("Twilio API {status}: {text}"))
            }
        }
        Err(e) => NodeExecutionResult::failed(format!("Twilio request error: {e}")),
    }
}

async fn execute_stripe(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Stripe node requires config"),
    };
    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) => resolve_template(k, context),
        None => return NodeExecutionResult::failed("Stripe node missing 'api_key'"),
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) => resolve_template(e, context),
        None => return NodeExecutionResult::failed("Stripe node missing 'endpoint'"),
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();
    let url = format!("https://api.stripe.com/v1{}", endpoint);

    let body_val = cfg
        .get("body")
        .and_then(|v| v.as_str())
        .map(|s| {
            let resolved = resolve_template(s, context);
            serde_json::from_str::<serde_json::Value>(&resolved).unwrap_or(serde_json::Value::Null)
        })
        .unwrap_or(serde_json::Value::Null);

    let builder = http_client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Stripe-Version", "2024-06-20");

    let builder = if (method == "POST" || method == "PATCH") && body_val.is_object() {
        // Form-encode flat object for Stripe v1 API
        let params: Vec<(String, String)> = body_val
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| {
                let val = v
                    .as_str()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| v.to_string());
                (k.clone(), val)
            })
            .collect();
        builder.form(&params)
    } else if method == "GET" && body_val.is_object() {
        let params: Vec<(String, String)> = body_val
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| {
                let val = v
                    .as_str()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| v.to_string());
                (k.clone(), val)
            })
            .collect();
        builder.query(&params)
    } else {
        builder
    };

    match builder.send().await {
        Ok(r) => {
            let status = r.status().as_u16();
            let ok = r.status().is_success();
            let text = r.text().await.unwrap_or_default();
            let json: serde_json::Value =
                serde_json::from_str(&text).unwrap_or(serde_json::Value::String(text.clone()));
            if ok {
                let id = json
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let obj = json
                    .get("object")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                NodeExecutionResult::succeeded(
                    serde_json::json!({ "status": status, "id": id, "object": obj, "body": json })
                        .to_string(),
                )
            } else {
                NodeExecutionResult::failed(format!("Stripe API {status}: {text}"))
            }
        }
        Err(e) => NodeExecutionResult::failed(format!("Stripe request error: {e}")),
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

fn execute_math(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    let cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Math node requires config"),
    };
    let config = resolve_config_strings(cfg, context);
    let operation = config
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("add");

    let get_f64 = |key: &str| -> Option<f64> {
        config.get(key).and_then(|v| match v {
            serde_json::Value::Number(n) => n.as_f64(),
            serde_json::Value::String(s) => s.parse::<f64>().ok(),
            _ => None,
        })
    };

    let result: f64 = match operation {
        "abs" => {
            let a = get_f64("a").unwrap_or(0.0);
            a.abs()
        }
        "round" => {
            let a = get_f64("a").unwrap_or(0.0);
            let p = get_f64("precision").unwrap_or(0.0) as i32;
            let f = 10f64.powi(p);
            (a * f).round() / f
        }
        "ceil" => {
            let a = get_f64("a").unwrap_or(0.0);
            a.ceil()
        }
        "floor" => {
            let a = get_f64("a").unwrap_or(0.0);
            a.floor()
        }
        "sqrt" => {
            let a = get_f64("a").unwrap_or(0.0);
            if a < 0.0 {
                return NodeExecutionResult::failed("sqrt of negative");
            }
            a.sqrt()
        }
        "pow" => {
            let a = get_f64("a").unwrap_or(0.0);
            let b = get_f64("b").unwrap_or(2.0);
            a.powf(b)
        }
        "mod" => {
            let a = get_f64("a").unwrap_or(0.0);
            let b = get_f64("b").unwrap_or(1.0);
            if b == 0.0 {
                return NodeExecutionResult::failed("modulo by zero");
            }
            a % b
        }
        "min" | "max" | "sum" | "avg" => {
            let items: Vec<f64> = match config.get("items") {
                Some(serde_json::Value::Array(arr)) => arr
                    .iter()
                    .filter_map(|v| match v {
                        serde_json::Value::Number(n) => n.as_f64(),
                        serde_json::Value::String(s) => s.parse().ok(),
                        _ => None,
                    })
                    .collect(),
                _ => vec![get_f64("a").unwrap_or(0.0), get_f64("b").unwrap_or(0.0)],
            };
            if items.is_empty() {
                return NodeExecutionResult::failed("items array is empty");
            }
            match operation {
                "min" => items.iter().cloned().fold(f64::INFINITY, f64::min),
                "max" => items.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
                "sum" => items.iter().sum(),
                "avg" => items.iter().sum::<f64>() / items.len() as f64,
                _ => unreachable!(),
            }
        }
        "clamp" => {
            let a = get_f64("a").unwrap_or(0.0);
            let min = get_f64("min").unwrap_or(f64::NEG_INFINITY);
            let max = get_f64("max").unwrap_or(f64::INFINITY);
            a.clamp(min, max)
        }
        "log" => {
            let a = get_f64("a").unwrap_or(0.0);
            let base = get_f64("b").unwrap_or(std::f64::consts::E);
            if a <= 0.0 {
                return NodeExecutionResult::failed("log of non-positive");
            }
            a.ln() / base.ln()
        }
        "pct_change" => {
            let a = get_f64("a").unwrap_or(0.0);
            let b = get_f64("b").unwrap_or(0.0);
            if a == 0.0 {
                return NodeExecutionResult::failed("pct_change: base is zero");
            }
            (b - a) / a * 100.0
        }
        "eval" => {
            let expr_raw = config
                .get("expression")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let mut engine = rhai::Engine::new();
            engine.set_max_operations(10_000);
            match engine.eval::<rhai::Dynamic>(expr_raw) {
                Ok(v) => {
                    if let Some(n) = v
                        .as_float()
                        .ok()
                        .or_else(|| v.as_int().ok().map(|i| i as f64))
                    {
                        n
                    } else {
                        return NodeExecutionResult::succeeded(
                            serde_json::json!({ "result": v.to_string(), "operation": "eval" })
                                .to_string(),
                        );
                    }
                }
                Err(e) => return NodeExecutionResult::failed(format!("eval error: {e}")),
            }
        }
        "add" | _ => {
            let a = get_f64("a").unwrap_or(0.0);
            let b = get_f64("b").unwrap_or(0.0);
            a + b
        }
    };

    let precision = get_f64("precision").unwrap_or(10.0) as i32;
    let factor = 10f64.powi(precision.min(15));
    let rounded = (result * factor).round() / factor;

    NodeExecutionResult::succeeded(
        serde_json::json!({ "result": rounded, "operation": operation }).to_string(),
    )
}

// ── Slice 263: ArrayUtils ─────────────────────────────────────────────────────

fn execute_array_utils(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    let cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("ArrayUtils node requires config"),
    };
    let config = resolve_config_strings(cfg, context);
    let operation = config
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("chunk");

    let parse_source = |v: &serde_json::Value| -> Result<Vec<serde_json::Value>, &'static str> {
        match v {
            serde_json::Value::String(s) => match serde_json::from_str::<serde_json::Value>(s) {
                Ok(serde_json::Value::Array(a)) => Ok(a),
                _ => Err("source is not a JSON array"),
            },
            serde_json::Value::Array(a) => Ok(a.clone()),
            _ => Err("source must be a JSON array"),
        }
    };

    // range generates its own items; all other operations require source
    let needs_source = operation != "range";
    let source_arr: Vec<serde_json::Value> = if needs_source {
        match config.get("source") {
            Some(v) => match parse_source(v) {
                Ok(a) => a,
                Err(e) => return NodeExecutionResult::failed(e),
            },
            None => return NodeExecutionResult::failed("ArrayUtils requires 'source' array"),
        }
    } else {
        vec![]
    };

    let get_usize = |key: &str, default: usize| -> usize {
        config
            .get(key)
            .and_then(|v| match v {
                serde_json::Value::Number(n) => n.as_u64().map(|n| n as usize),
                serde_json::Value::String(s) => s.parse().ok(),
                _ => None,
            })
            .unwrap_or(default)
    };
    let get_i64 = |key: &str, default: i64| -> i64 {
        config
            .get(key)
            .and_then(|v| match v {
                serde_json::Value::Number(n) => n.as_i64(),
                serde_json::Value::String(s) => s.parse().ok(),
                _ => None,
            })
            .unwrap_or(default)
    };

    let items: Vec<serde_json::Value> = match operation {
        "chunk" => {
            let size = get_usize("size", 2).max(1);
            source_arr
                .chunks(size)
                .map(|c| serde_json::Value::Array(c.to_vec()))
                .collect()
        }
        "flatten" => source_arr
            .into_iter()
            .flat_map(|v| match v {
                serde_json::Value::Array(inner) => inner,
                other => vec![other],
            })
            .collect(),
        "compact" => source_arr
            .into_iter()
            .filter(|v| {
                !matches!(v, serde_json::Value::Null)
                    && v.as_str() != Some("")
                    && v.as_bool() != Some(false)
            })
            .collect(),
        "zip" => {
            let source2_arr = match config.get("source2") {
                Some(serde_json::Value::String(s)) => {
                    match serde_json::from_str::<serde_json::Value>(s) {
                        Ok(serde_json::Value::Array(a)) => a,
                        _ => return NodeExecutionResult::failed("source2 is not a JSON array"),
                    }
                }
                Some(serde_json::Value::Array(a)) => a.clone(),
                _ => return NodeExecutionResult::failed("zip requires 'source2' array"),
            };
            source_arr
                .into_iter()
                .zip(source2_arr.into_iter())
                .map(|(a, b)| serde_json::json!([a, b]))
                .collect()
        }
        "reverse" => {
            let mut v = source_arr;
            v.reverse();
            v
        }
        "shuffle" => {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut v = source_arr;
            let n = v.len();
            for i in (1..n).rev() {
                let mut h = DefaultHasher::new();
                i.hash(&mut h);
                let j = (h.finish() as usize) % (i + 1);
                v.swap(i, j);
            }
            v
        }
        "sample" => {
            let n = get_usize("n", 1);
            source_arr.into_iter().take(n).collect()
        }
        "range" => {
            let start = get_i64("start", 0);
            let end = get_i64("end", 10);
            let step = get_i64("step", 1);
            if step == 0 {
                return NodeExecutionResult::failed("range step cannot be zero");
            }
            let mut v = Vec::new();
            let mut i = start;
            while (step > 0 && i < end) || (step < 0 && i > end) {
                v.push(serde_json::json!(i));
                i += step;
            }
            v
        }
        "pluck" => {
            let field = match config.get("field").and_then(|v| v.as_str()) {
                Some(f) => f.to_string(),
                None => return NodeExecutionResult::failed("pluck requires 'field'"),
            };
            source_arr
                .into_iter()
                .filter_map(|v| json_path(&v, &field).cloned())
                .collect()
        }
        "first_n" => {
            let n = get_usize("n", 1);
            source_arr.into_iter().take(n).collect()
        }
        "last_n" => {
            let n = get_usize("n", 1);
            let len = source_arr.len();
            source_arr.into_iter().skip(len.saturating_sub(n)).collect()
        }
        _ => return NodeExecutionResult::failed(format!("unknown array operation: {operation}")),
    };

    let count = items.len();
    NodeExecutionResult::succeeded(
        serde_json::json!({ "items": items, "count": count }).to_string(),
    )
}

// ── Slice 264: Shopify ────────────────────────────────────────────────────────

async fn execute_shopify(
    node: &Node,
    context: &ExecutionContext,
    client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw_cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Shopify node requires config"),
    };
    let cfg = resolve_config_strings(raw_cfg, context);

    let shop = match cfg.get("shop").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return NodeExecutionResult::failed("Shopify node missing 'shop'"),
    };
    let token = match cfg.get("token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Shopify node missing 'token'"),
    };
    let endpoint = cfg
        .get("endpoint")
        .and_then(|v| v.as_str())
        .unwrap_or("/products.json");
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();
    let api_version = cfg
        .get("api_version")
        .and_then(|v| v.as_str())
        .unwrap_or("2024-01");

    let url = format!("https://{shop}.myshopify.com/admin/api/{api_version}{endpoint}");

    let mut req = client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("X-Shopify-Access-Token", &token)
        .header("Content-Type", "application/json");

    if matches!(method.as_str(), "POST" | "PUT" | "PATCH") {
        if let Some(body) = cfg.get("body") {
            req = req.json(body);
        }
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
            NodeExecutionResult::succeeded(
                serde_json::json!({ "status": status, "body": body }).to_string(),
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Shopify request error: {e}")),
    }
}

// ── Slice 265: Datadog ────────────────────────────────────────────────────────

async fn execute_datadog(
    node: &Node,
    context: &ExecutionContext,
    client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw_cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Datadog node requires config"),
    };
    let cfg = resolve_config_strings(raw_cfg, context);

    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Datadog node missing 'api_key'"),
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("Datadog node missing 'endpoint'"),
    };
    let site = cfg
        .get("site")
        .and_then(|v| v.as_str())
        .unwrap_or("datadoghq.com");
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();
    let app_key = cfg
        .get("app_key")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let url = format!("https://api.{site}{endpoint}");

    let mut req = client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("DD-API-KEY", &api_key)
        .header("Content-Type", "application/json");

    if !app_key.is_empty() {
        req = req.header("DD-APPLICATION-KEY", &app_key);
    }

    if matches!(method.as_str(), "POST" | "PUT" | "PATCH") {
        if let Some(body) = cfg.get("body") {
            req = req.json(body);
        }
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
            NodeExecutionResult::succeeded(
                serde_json::json!({ "status": status, "body": body }).to_string(),
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Datadog request error: {e}")),
    }
}

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

async fn execute_salesforce(
    node: &Node,
    context: &ExecutionContext,
    client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw_cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Salesforce node requires config"),
    };
    let cfg = resolve_config_strings(raw_cfg, context);

    let token = match cfg.get("token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => {
            return NodeExecutionResult::failed(
                "Salesforce node missing 'token' (OAuth access token)",
            )
        }
    };
    let instance_url = match cfg.get("instance_url").and_then(|v| v.as_str()) {
        Some(u) if !u.is_empty() => u.trim_end_matches('/').to_string(),
        _ => return NodeExecutionResult::failed("Salesforce node missing 'instance_url'"),
    };
    let endpoint = cfg
        .get("endpoint")
        .and_then(|v| v.as_str())
        .unwrap_or("/services/data/v59.0/sobjects");
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();

    let url = format!("{instance_url}{endpoint}");

    let mut req = client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json");

    if matches!(method.as_str(), "POST" | "PUT" | "PATCH") {
        if let Some(body) = cfg.get("body") {
            req = req.json(body);
        }
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
            NodeExecutionResult::succeeded(
                serde_json::json!({ "status": status, "body": body }).to_string(),
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Salesforce request error: {e}")),
    }
}

// ── Slice 267: Freshdesk ──────────────────────────────────────────────────────

async fn execute_freshdesk(
    node: &Node,
    context: &ExecutionContext,
    client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw_cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Freshdesk node requires config"),
    };
    let cfg = resolve_config_strings(raw_cfg, context);

    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Freshdesk node missing 'api_key'"),
    };
    let domain = match cfg.get("domain").and_then(|v| v.as_str()) {
        Some(d) if !d.is_empty() => d.trim_end_matches('/').to_string(),
        _ => {
            return NodeExecutionResult::failed(
                "Freshdesk node missing 'domain' (e.g. yourcompany.freshdesk.com)",
            )
        }
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("Freshdesk node missing 'endpoint'"),
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();

    // Freshdesk uses HTTP Basic auth: api_key as username, "X" as password
    use base64::Engine as _;
    let credentials =
        base64::engine::general_purpose::STANDARD.encode(format!("{api_key}:X").as_bytes());

    let url = format!("https://{domain}{endpoint}");

    let mut req = client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("Authorization", format!("Basic {credentials}"))
        .header("Content-Type", "application/json");

    if matches!(method.as_str(), "POST" | "PUT" | "PATCH") {
        if let Some(body) = cfg.get("body") {
            req = req.json(body);
        }
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
            NodeExecutionResult::succeeded(
                serde_json::json!({ "status": status, "body": body }).to_string(),
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Freshdesk request error: {e}")),
    }
}

// ── Slice 268: Mailgun ────────────────────────────────────────────────────────

async fn execute_mailgun(
    node: &Node,
    context: &ExecutionContext,
    client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw_cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Mailgun node requires config"),
    };
    let cfg = resolve_config_strings(raw_cfg, context);

    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Mailgun node missing 'api_key'"),
    };
    let domain = match cfg.get("domain").and_then(|v| v.as_str()) {
        Some(d) if !d.is_empty() => d.to_string(),
        _ => return NodeExecutionResult::failed("Mailgun node missing 'domain' (sending domain)"),
    };
    let to = match cfg.get("to").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Mailgun node missing 'to' address"),
    };
    let from = cfg
        .get("from")
        .and_then(|v| v.as_str())
        .unwrap_or("noreply@example.com")
        .to_string();
    let subject = cfg
        .get("subject")
        .and_then(|v| v.as_str())
        .unwrap_or("(no subject)")
        .to_string();

    // Support html or text content
    let html = cfg.get("html").and_then(|v| v.as_str()).map(str::to_string);
    let text = cfg.get("text").and_then(|v| v.as_str()).map(str::to_string);
    let region = cfg.get("region").and_then(|v| v.as_str()).unwrap_or("us");
    let base = if region == "eu" {
        "api.eu.mailgun.net"
    } else {
        "api.mailgun.net"
    };

    use base64::Engine as _;
    let credentials =
        base64::engine::general_purpose::STANDARD.encode(format!("api:{api_key}").as_bytes());

    let url = format!("https://{base}/v3/{domain}/messages");

    let mut params = vec![
        ("from".to_string(), from),
        ("to".to_string(), to),
        ("subject".to_string(), subject),
    ];
    if let Some(h) = html {
        params.push(("html".to_string(), h));
    }
    if let Some(t) = text {
        params.push(("text".to_string(), t));
    }

    match client
        .post(&url)
        .header("Authorization", format!("Basic {credentials}"))
        .form(&params)
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
            NodeExecutionResult::succeeded(
                serde_json::json!({ "status": status, "body": body }).to_string(),
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Mailgun request error: {e}")),
    }
}

// ── Slice 269: Asana ──────────────────────────────────────────────────────────

async fn execute_asana(
    node: &Node,
    context: &ExecutionContext,
    client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw_cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Asana node requires config"),
    };
    let cfg = resolve_config_strings(raw_cfg, context);

    let token = match cfg.get("token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => {
            return NodeExecutionResult::failed(
                "Asana node missing 'token' (Personal Access Token)",
            )
        }
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("Asana node missing 'endpoint' (e.g. /tasks)"),
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();

    let url = format!("https://app.asana.com/api/1.0{endpoint}");

    let mut req = client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .header("Accept", "application/json");

    if matches!(method.as_str(), "POST" | "PUT" | "PATCH") {
        if let Some(body) = cfg.get("body") {
            req = req.json(body);
        }
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
            NodeExecutionResult::succeeded(
                serde_json::json!({ "status": status, "body": body }).to_string(),
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Asana request error: {e}")),
    }
}

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

async fn execute_servicenow(
    node: &Node,
    context: &ExecutionContext,
    client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw_cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("ServiceNow node requires config"),
    };
    let cfg = resolve_config_strings(raw_cfg, context);

    let instance = match cfg.get("instance").and_then(|v| v.as_str()) {
        Some(i) if !i.is_empty() => i.trim_end_matches('/').to_string(),
        _ => {
            return NodeExecutionResult::failed(
                "ServiceNow node missing 'instance' (e.g. myco.service-now.com)",
            )
        }
    };
    let username = match cfg.get("username").and_then(|v| v.as_str()) {
        Some(u) if !u.is_empty() => u.to_string(),
        _ => return NodeExecutionResult::failed("ServiceNow node missing 'username'"),
    };
    let password = match cfg.get("password").and_then(|v| v.as_str()) {
        Some(p) if !p.is_empty() => p.to_string(),
        _ => return NodeExecutionResult::failed("ServiceNow node missing 'password'"),
    };
    let endpoint = cfg
        .get("endpoint")
        .and_then(|v| v.as_str())
        .unwrap_or("/api/now/table/incident");
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();

    use base64::Engine as _;
    let credentials = base64::engine::general_purpose::STANDARD
        .encode(format!("{username}:{password}").as_bytes());

    let url = format!("https://{instance}{endpoint}");

    let mut req = client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("Authorization", format!("Basic {credentials}"))
        .header("Content-Type", "application/json")
        .header("Accept", "application/json");

    if matches!(method.as_str(), "POST" | "PUT" | "PATCH") {
        if let Some(body) = cfg.get("body") {
            req = req.json(body);
        }
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
            NodeExecutionResult::succeeded(
                serde_json::json!({ "status": status, "body": body }).to_string(),
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("ServiceNow request error: {e}")),
    }
}

// ── Slice 271: Confluence ─────────────────────────────────────────────────────

async fn execute_confluence(
    node: &Node,
    context: &ExecutionContext,
    client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw_cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Confluence node requires config"),
    };
    let cfg = resolve_config_strings(raw_cfg, context);

    let base_url = match cfg.get("base_url").and_then(|v| v.as_str()) {
        Some(u) if !u.is_empty() => u.trim_end_matches('/').to_string(),
        _ => {
            return NodeExecutionResult::failed(
                "Confluence node missing 'base_url' (e.g. https://myco.atlassian.net/wiki)",
            )
        }
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("Confluence node missing 'endpoint'"),
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();

    // Support either Bearer token or Basic auth (email + api_token)
    let auth_header = if let Some(token) = cfg
        .get("token")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
    {
        format!("Bearer {token}")
    } else {
        let email = cfg.get("email").and_then(|v| v.as_str()).unwrap_or("");
        let api_token = cfg.get("api_token").and_then(|v| v.as_str()).unwrap_or("");
        if email.is_empty() || api_token.is_empty() {
            return NodeExecutionResult::failed(
                "Confluence node requires either 'token' or both 'email' and 'api_token'",
            );
        }
        use base64::Engine as _;
        let creds = base64::engine::general_purpose::STANDARD
            .encode(format!("{email}:{api_token}").as_bytes());
        format!("Basic {creds}")
    };

    let url = format!("{base_url}{endpoint}");

    let mut req = client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("Authorization", auth_header)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json");

    if matches!(method.as_str(), "POST" | "PUT" | "PATCH") {
        if let Some(body) = cfg.get("body") {
            req = req.json(body);
        }
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
            NodeExecutionResult::succeeded(
                serde_json::json!({ "status": status, "body": body }).to_string(),
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Confluence request error: {e}")),
    }
}

// ── Slice 272: Bitbucket ──────────────────────────────────────────────────────

async fn execute_bitbucket(
    node: &Node,
    context: &ExecutionContext,
    client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw_cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Bitbucket node requires config"),
    };
    let cfg = resolve_config_strings(raw_cfg, context);

    let username = match cfg.get("username").and_then(|v| v.as_str()) {
        Some(u) if !u.is_empty() => u.to_string(),
        _ => return NodeExecutionResult::failed("Bitbucket node missing 'username'"),
    };
    let app_password = match cfg.get("app_password").and_then(|v| v.as_str()) {
        Some(p) if !p.is_empty() => p.to_string(),
        _ => return NodeExecutionResult::failed("Bitbucket node missing 'app_password'"),
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => {
            return NodeExecutionResult::failed(
                "Bitbucket node missing 'endpoint' (e.g. /repositories/workspace/slug)",
            )
        }
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();

    use base64::Engine as _;
    let credentials = base64::engine::general_purpose::STANDARD
        .encode(format!("{username}:{app_password}").as_bytes());

    let url = format!("https://api.bitbucket.org/2.0{endpoint}");

    let mut req = client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("Authorization", format!("Basic {credentials}"))
        .header("Content-Type", "application/json")
        .header("Accept", "application/json");

    if matches!(method.as_str(), "POST" | "PUT" | "PATCH") {
        if let Some(body) = cfg.get("body") {
            req = req.json(body);
        }
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
            NodeExecutionResult::succeeded(
                serde_json::json!({ "status": status, "body": body }).to_string(),
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Bitbucket request error: {e}")),
    }
}

// ── Slice 273: Azure DevOps ───────────────────────────────────────────────────

async fn execute_azure_devops(
    node: &Node,
    context: &ExecutionContext,
    client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw_cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Azure DevOps node requires config"),
    };
    let cfg = resolve_config_strings(raw_cfg, context);

    let pat = match cfg.get("pat").and_then(|v| v.as_str()) {
        Some(p) if !p.is_empty() => p.to_string(),
        _ => {
            return NodeExecutionResult::failed(
                "Azure DevOps node missing 'pat' (Personal Access Token)",
            )
        }
    };
    let organization = match cfg.get("organization").and_then(|v| v.as_str()) {
        Some(o) if !o.is_empty() => o.to_string(),
        _ => return NodeExecutionResult::failed("Azure DevOps node missing 'organization'"),
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("Azure DevOps node missing 'endpoint'"),
    };
    let project = cfg
        .get("project")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();
    let api_ver = cfg
        .get("api_version")
        .and_then(|v| v.as_str())
        .unwrap_or("7.1");

    // ADO uses Basic auth with empty username and PAT as password
    use base64::Engine as _;
    let credentials =
        base64::engine::general_purpose::STANDARD.encode(format!(":{pat}").as_bytes());

    // Build base URL: https://dev.azure.com/{org}/{project}/_apis{endpoint}
    let base = if project.is_empty() {
        format!("https://dev.azure.com/{organization}/_apis{endpoint}")
    } else {
        format!("https://dev.azure.com/{organization}/{project}/_apis{endpoint}")
    };
    let url = if base.contains('?') {
        format!("{base}&api-version={api_ver}")
    } else {
        format!("{base}?api-version={api_ver}")
    };

    let mut req = client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("Authorization", format!("Basic {credentials}"))
        .header("Content-Type", "application/json")
        .header("Accept", "application/json");

    if matches!(method.as_str(), "POST" | "PUT" | "PATCH") {
        if let Some(body) = cfg.get("body") {
            req = req.json(body);
        }
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
            NodeExecutionResult::succeeded(
                serde_json::json!({ "status": status, "body": body }).to_string(),
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Azure DevOps request error: {e}")),
    }
}

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
