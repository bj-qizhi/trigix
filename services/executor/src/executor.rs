use std::sync::Arc;

use serde::{Deserialize, Serialize};
use workflow_core::{Node, NodeType};
use rhai;

use crate::approval::ApprovalGate;
use crate::runtime::{ExecutionContext, NodeExecutionResult, NodeExecutor};

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
    let json_str = match root {
        "input" => Some(context.input_json.as_str()),
        node_id => context.node_outputs.get(node_id).map(|s| s.as_str()),
    };
    match (json_str, path) {
        (None, _) => String::new(),
        (Some(raw), None) => raw.to_string(),
        (Some(raw), Some(path)) => {
            let val: serde_json::Value = serde_json::from_str(raw).unwrap_or(serde_json::Value::Null);
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

fn resolve_config_strings(config: &serde_json::Value, context: &ExecutionContext) -> serde_json::Value {
    match config {
        serde_json::Value::String(s) => serde_json::Value::String(resolve_template(s, context)),
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), resolve_config_strings(v, context)))
                .collect(),
        ),
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(|v| resolve_config_strings(v, context)).collect())
        }
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
        &'a mut self,
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

            // Clone cheaply (reqwest::Client is Arc-backed; ai_runtime_base_url is a small String)
            let http_client = self.http_client.clone();
            let ai_base = self.ai_runtime_base_url.clone();

            let mut last = NodeExecutionResult::failed("Execution not started");
            for attempt in 0..=max_retries {
                last = dispatch_with_timeout(node, context, &http_client, ai_base.as_deref(), timeout_secs).await;
                if last.status == execution_core::NodeStatus::Succeeded {
                    return last;
                }
                if attempt < max_retries {
                    // Exponential backoff: 200ms, 400ms, 800ms, … capped at ~6.4s
                    let ms = 200u64 * (1u64 << attempt.min(5));
                    tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
                }
            }
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
        Some(secs) => {
            match tokio::time::timeout(std::time::Duration::from_secs(secs), fut).await {
                Ok(result) => result,
                Err(_) => {
                    NodeExecutionResult::failed(format!("Node timed out after {secs}s"))
                }
            }
        }
        None => fut.await,
    }
}

async fn dispatch(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
    ai_runtime_base_url: Option<&str>,
) -> NodeExecutionResult {
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
            return NodeExecutionResult::failed(
                "Http node requires config with 'url' and 'method'",
            )
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

    match builder.send().await {
        Ok(response) => {
            let status = response.status();
            match response.text().await {
                Ok(body) => {
                    if status.is_success() {
                        let output = if serde_json::from_str::<serde_json::Value>(&body).is_ok() {
                            body
                        } else {
                            serde_json::json!({"body": body}).to_string()
                        };
                        NodeExecutionResult::succeeded(output)
                    } else {
                        NodeExecutionResult::failed(format!("HTTP {status}: {body}"))
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
            return NodeExecutionResult::failed(
                "Map node: 'items' must resolve to a JSON array",
            )
        }
    };

    let item_template = config.get("item_template");
    let mut out: Vec<serde_json::Value> = Vec::with_capacity(items_arr.len());
    for item in &items_arr {
        let rendered = match item_template {
            Some(tmpl) => {
                // Inject the current item into a child context so {{item}} / {{item.field}} works.
                let mut child = context.clone();
                child.node_outputs.insert("item".to_string(), item.to_string());
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
        None => return NodeExecutionResult::failed("Filter node requires config with 'items' and 'field'"),
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
        Err(_) => return NodeExecutionResult::failed(
            format!("Filter node: 'items' did not resolve to valid JSON: {resolved}")
        ),
    };
    let items_arr = match items_val.as_array() {
        Some(a) => a.clone(),
        None => return NodeExecutionResult::failed("Filter node: 'items' must resolve to a JSON array"),
    };

    let operator = config.get("operator").and_then(|v| v.as_str()).unwrap_or("exists");
    let expected = config.get("value").and_then(|v| v.as_str()).unwrap_or("");

    let filtered: Vec<serde_json::Value> = items_arr.into_iter().filter(|item| {
        let field_val = json_path(item, field);
        match operator {
            "exists"     => field_val.is_some(),
            "not_exists" => field_val.is_none(),
            "equals"     => field_val.map(json_to_string).as_deref() == Some(expected),
            "not_equals" => field_val.map(json_to_string).as_deref() != Some(expected),
            "contains"   => field_val.map(json_to_string).unwrap_or_default().contains(expected),
            "gt" => {
                let actual = field_val.and_then(|v| v.as_f64()).unwrap_or(f64::NEG_INFINITY);
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
    }).collect();

    NodeExecutionResult::succeeded(
        serde_json::json!({ "count": filtered.len(), "items": filtered }).to_string(),
    )
}

fn execute_aggregate(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    let config = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Aggregate node requires config with 'items' and 'operation'"),
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
        Err(_) => return NodeExecutionResult::failed(
            format!("Aggregate node: 'items' did not resolve to valid JSON: {resolved}")
        ),
    };
    let items = match items_val.as_array() {
        Some(a) => a,
        None => return NodeExecutionResult::failed("Aggregate node: 'items' must resolve to a JSON array"),
    };

    let field = config.get("field").and_then(|v| v.as_str());

    let result: serde_json::Value = match operation {
        "count" => serde_json::Value::Number(items.len().into()),

        "sum" => {
            let f = match field {
                Some(f) => f,
                None => return NodeExecutionResult::failed("Aggregate 'sum' requires 'field'"),
            };
            let total: f64 = items.iter()
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
            let nums: Vec<f64> = items.iter()
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
            items.iter()
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
            items.iter()
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
            let sep = config.get("separator").and_then(|v| v.as_str()).unwrap_or(", ");
            let parts: Vec<String> = items.iter()
                .filter_map(|item| json_path(item, f))
                .map(json_to_string)
                .collect();
            serde_json::Value::String(parts.join(sep))
        }

        "first" => {
            let first = items.first().cloned().unwrap_or(serde_json::Value::Null);
            match field {
                Some(f) => json_path(&first, f).cloned().unwrap_or(serde_json::Value::Null),
                None => first,
            }
        }

        "last" => {
            let last = items.last().cloned().unwrap_or(serde_json::Value::Null);
            match field {
                Some(f) => json_path(&last, f).cloned().unwrap_or(serde_json::Value::Null),
                None => last,
            }
        }

        op => return NodeExecutionResult::failed(format!(
            "Aggregate: unknown operation '{op}'. Use: count, sum, avg, min, max, join, first, last"
        )),
    };

    NodeExecutionResult::succeeded(serde_json::json!({ "result": result }).to_string())
}

fn execute_sort(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    let config = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Sort node requires config with 'items' and 'field'"),
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
        Err(_) => return NodeExecutionResult::failed(
            format!("Sort node: 'items' did not resolve to valid JSON: {resolved}")
        ),
    };
    let mut items = match items_val.as_array() {
        Some(a) => a.clone(),
        None => return NodeExecutionResult::failed("Sort node: 'items' must resolve to a JSON array"),
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
        if descending { ord.reverse() } else { ord }
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
    NodeExecutionResult::succeeded(
        serde_json::json!({ "waited_secs": seconds }).to_string(),
    )
}

async fn execute_sub_workflow(
    node: &Node,
    context: &ExecutionContext,
    ai_runtime_base_url: Option<&str>,
) -> NodeExecutionResult {
    let config = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("SubWorkflow node requires config with '_graph'"),
    };

    let sub_graph: workflow_core::WorkflowGraph = match config.get("_graph") {
        Some(g) => match serde_json::from_value(g.clone()) {
            Ok(graph) => graph,
            Err(e) => return NodeExecutionResult::failed(format!("SubWorkflow node: invalid '_graph': {e}")),
        },
        None => return NodeExecutionResult::failed("SubWorkflow node missing '_graph' — platform must inject it before execution"),
    };

    // Resolve input for sub-execution: if 'input_template' is set, render it; otherwise pass through
    let sub_input = match config.get("input_template") {
        Some(template) => resolve_config_strings(template, context).to_string(),
        None => context.input_json.clone(),
    };

    let sub_execution_id = format!("{}:sub:{}", context.execution_id, node.id);
    let mut sub_executor = DispatchingNodeExecutor::new(ai_runtime_base_url.map(str::to_owned));

    match crate::runtime::run_workflow(&sub_execution_id, &sub_graph, sub_input, &mut sub_executor).await {
        Ok(report) => {
            let last_output = report.node_results.iter().rev()
                .find(|r| r.status == execution_core::NodeStatus::Succeeded)
                .and_then(|r| r.output_json.as_deref());
            let output_val = match last_output {
                Some(s) => serde_json::from_str(s).unwrap_or(serde_json::Value::String(s.to_owned())),
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
        if !r.is_empty() { payload["username"] = serde_json::json!(r); }
    }
    if let Some(c) = config.get("channel").and_then(|v| v.as_str()) {
        let r = resolve_template(c, context);
        if !r.is_empty() { payload["channel"] = serde_json::json!(r); }
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
        .unwrap_or_else(|| "noreply@agentflow.dev".to_string());

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
    let message = config.get("message").and_then(|v| v.as_str()).unwrap_or("Assertion failed");
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
        if resolved.is_empty() { None } else { Some(resolved) }
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
        }
    }

    #[tokio::test]
    async fn trigger_returns_input() {
        let mut executor = DispatchingNodeExecutor::new(None);
        let node = Node {
            id: "trigger".to_string(),
            node_type: NodeType::Trigger,
            config: None,
        };
        let context = make_context(r#"{"lead_id":"lead-1"}"#);

        let result = executor.execute(&node, &context).await;

        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
        assert_eq!(result.output_json.as_deref(), Some(r#"{"lead_id":"lead-1"}"#));
    }

    #[tokio::test]
    async fn http_node_requires_config() {
        let mut executor = DispatchingNodeExecutor::new(None);
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
        let mut executor = DispatchingNodeExecutor::new(None);
        let node = Node {
            id: "agent".to_string(),
            node_type: NodeType::Agent,
            config: None,
        };
        let context = make_context("{}");

        let result = executor.execute(&node, &context).await;

        assert_eq!(result.status, execution_core::NodeStatus::Failed);
        assert!(result.error.as_deref().unwrap_or("").contains("AI_RUNTIME_BASE_URL"));
    }

    #[tokio::test]
    async fn condition_node_evaluates_field_presence() {
        let mut executor = DispatchingNodeExecutor::new(None);
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
        let mut executor = DispatchingNodeExecutor::new(None);
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
        let mut executor = DispatchingNodeExecutor::new(None);
        // Agent with max_retries:1 — will fail twice (no AI Runtime URL)
        let node = Node {
            id: "agent".to_string(),
            node_type: NodeType::Agent,
            config: Some(serde_json::json!({"max_retries": 1})),
        };
        let result = executor.execute(&node, &make_context("{}")).await;
        assert_eq!(result.status, execution_core::NodeStatus::Failed);
        // Error should be from the node, not from the retry wrapper
        assert!(result.error.as_deref().unwrap_or("").contains("AI_RUNTIME_BASE_URL"));
    }

    #[tokio::test]
    async fn timeout_config_does_not_break_fast_nodes() {
        let mut executor = DispatchingNodeExecutor::new(None);
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
        assert_eq!(resolve_template("id={{input.lead_id}}", &context), "id=lead-42");
        assert_eq!(resolve_template("{{input}}", &context), r#"{"lead_id":"lead-42","status":"active"}"#);
        assert_eq!(resolve_template("no template here", &context), "no template here");
    }

    #[test]
    fn template_resolver_replaces_node_output_field() {
        let mut context = make_context("{}");
        context.node_outputs.insert(
            "trigger".to_string(),
            r#"{"lead_id":"lead-99","name":"Alice"}"#.to_string(),
        );
        assert_eq!(resolve_template("Hello {{trigger.name}}", &context), "Hello Alice");
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
        let mut executor = DispatchingNodeExecutor::new(None);
        let mut context = make_context(r#"{"endpoint":"https://example.com/api"}"#);
        context.node_outputs.insert("trigger".to_string(), r#"{"id":"42"}"#.to_string());
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
        let mut executor = DispatchingNodeExecutor::new(None);
        let mut context = make_context("{}");
        context.node_outputs.insert(
            "trigger".to_string(),
            r#"{"status":"active"}"#.to_string(),
        );
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
        let mut executor = DispatchingNodeExecutor::new(None);
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
        let mut executor = DispatchingNodeExecutor::new(None);
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
        let mut executor = DispatchingNodeExecutor::new(None);
        let mut context = make_context(r#"{"user":"Alice"}"#);
        context.node_outputs.insert(
            "trigger".to_string(),
            r#"{"score":42}"#.to_string(),
        );
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
        let mut executor = DispatchingNodeExecutor::new(None);
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
        let mut executor = DispatchingNodeExecutor::new(None);
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
        let mut executor = DispatchingNodeExecutor::new(None);
        let context = make_context(
            r#"{"words":[{"v":"banana"},{"v":"apple"},{"v":"cherry"}]}"#,
        );
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
        let mut executor = DispatchingNodeExecutor::new(None);
        let context = make_context(
            r#"{"scores":[{"s":3},{"s":1},{"s":4},{"s":1},{"s":5}]}"#,
        );
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
        let mut executor = DispatchingNodeExecutor::new(None);
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
        let mut executor = DispatchingNodeExecutor::new(None);
        let context = make_context(r#"{"items":[1,2,3,4,5]}"#);
        let node = Node {
            id: "agg".to_string(),
            node_type: NodeType::Aggregate,
            config: Some(serde_json::json!({ "items": "{{input.items}}", "operation": "count" })),
        };
        let result = executor.execute(&node, &context).await;
        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
        let output: serde_json::Value = serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(output["result"], 5);
    }

    #[tokio::test]
    async fn aggregate_node_sum_and_avg() {
        let mut executor = DispatchingNodeExecutor::new(None);
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
        let sum_out: serde_json::Value = serde_json::from_str(sum_result.output_json.as_deref().unwrap()).unwrap();
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
        let avg_out: serde_json::Value = serde_json::from_str(avg_result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(avg_out["result"], 20);
    }

    #[tokio::test]
    async fn aggregate_node_join() {
        let mut executor = DispatchingNodeExecutor::new(None);
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
        let output: serde_json::Value = serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(output["result"], "rust | wasm | axum");
    }

    #[tokio::test]
    async fn aggregate_node_fails_with_unknown_operation() {
        let mut executor = DispatchingNodeExecutor::new(None);
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
        let mut executor = DispatchingNodeExecutor::new(None);
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
        let mut executor = DispatchingNodeExecutor::new(None);
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
        let mut executor = DispatchingNodeExecutor::new(None);
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
        let mut executor = DispatchingNodeExecutor::new(None);
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
        let mut executor = DispatchingNodeExecutor::new(None);
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
        let mut executor = DispatchingNodeExecutor::new(None);
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
        let mut executor = DispatchingNodeExecutor::new(None);
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
        let mut executor = DispatchingNodeExecutor::new(None);
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
        let mut executor = DispatchingNodeExecutor::new(None);
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
        let mut executor = DispatchingNodeExecutor::new(None);
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
        let mut executor = DispatchingNodeExecutor::new(None);
        let node = Node {
            id: "assert".to_string(),
            node_type: NodeType::Assert,
            config: Some(serde_json::json!({ "condition": "{{filter.count}}", "message": "Expected count" })),
        };
        let mut ctx = make_context(r#"{}"#);
        ctx.node_outputs.insert("filter".to_string(), r#"{"count": 5}"#.to_string());
        let result = executor.execute(&node, &ctx).await;
        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
        let out: serde_json::Value = serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(out["ok"], true);
    }

    #[tokio::test]
    async fn assert_node_fails_falsy_value() {
        let mut executor = DispatchingNodeExecutor::new(None);
        let node = Node {
            id: "assert".to_string(),
            node_type: NodeType::Assert,
            config: Some(serde_json::json!({ "condition": "{{filter.count}}", "message": "Count must be non-zero" })),
        };
        let mut ctx = make_context(r#"{}"#);
        ctx.node_outputs.insert("filter".to_string(), r#"{"count": 0}"#.to_string());
        let result = executor.execute(&node, &ctx).await;
        assert_eq!(result.status, execution_core::NodeStatus::Failed);
        assert_eq!(result.error.as_deref(), Some("Count must be non-zero"));
    }

    #[tokio::test]
    async fn assert_node_uses_default_message() {
        let mut executor = DispatchingNodeExecutor::new(None);
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
        let mut executor = DispatchingNodeExecutor::new(None);
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
        ctx.node_outputs.insert("prev".to_string(), r#"{"value": 1}"#.to_string());
        let result = executor.execute(&node, &ctx).await;
        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
        let out: serde_json::Value =
            serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(out["doubled"], 10);
        assert_eq!(out["ok"], true);
    }

    #[tokio::test]
    async fn code_node_accesses_nodes_map() {
        let mut executor = DispatchingNodeExecutor::new(None);
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
        let mut executor = DispatchingNodeExecutor::new(None);
        let node = Node {
            id: "code".to_string(),
            node_type: NodeType::Code,
            config: Some(serde_json::json!({ "script": "this is not valid rhai !!!" })),
        };
        let ctx = make_context(r#"{}"#);
        let result = executor.execute(&node, &ctx).await;
        assert_eq!(result.status, execution_core::NodeStatus::Failed);
        assert!(result.error.as_deref().unwrap_or("").starts_with("Code error:"));
    }

    #[tokio::test]
    async fn code_node_fails_without_script() {
        let mut executor = DispatchingNodeExecutor::new(None);
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
        let mut executor = DispatchingNodeExecutor::new(None);
        let node = Node {
            id: "slack".to_string(),
            node_type: NodeType::Slack,
            config: Some(serde_json::json!({ "text": "hello" })),
        };
        let result = executor.execute(&node, &make_context("{}")).await;
        assert_eq!(result.status, execution_core::NodeStatus::Failed);
        assert!(result.error.as_deref().unwrap_or("").contains("webhook_url"));
    }

    #[tokio::test]
    async fn slack_node_fails_without_text() {
        let mut executor = DispatchingNodeExecutor::new(None);
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
        let mut executor = DispatchingNodeExecutor::new(None);
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
        let mut executor = DispatchingNodeExecutor::new(None);
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
        let mut executor = DispatchingNodeExecutor::new(None);
        let node = Node { id: "fan_out".to_string(), node_type: NodeType::FanOut, config: None };
        let ctx = make_context(r#"{"user":"alice"}"#);
        let result = executor.execute(&node, &ctx).await;
        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
        let out: serde_json::Value = serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(out["ok"], true);
        assert_eq!(out["input"]["user"], "alice");
    }

    #[tokio::test]
    async fn fan_in_collects_sources() {
        let mut executor = DispatchingNodeExecutor::new(None);
        let node = Node {
            id: "fan_in".to_string(),
            node_type: NodeType::FanIn,
            config: Some(serde_json::json!({ "_sources": ["branch_a", "branch_b"] })),
        };
        let mut ctx = make_context(r#"{}"#);
        ctx.node_outputs.insert("branch_a".to_string(), r#"{"value": 1}"#.to_string());
        ctx.node_outputs.insert("branch_b".to_string(), r#"{"value": 2}"#.to_string());
        let result = executor.execute(&node, &ctx).await;
        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
        let out: serde_json::Value = serde_json::from_str(result.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(out["count"], 2);
        assert_eq!(out["results"].as_array().unwrap().len(), 2);
    }
}
