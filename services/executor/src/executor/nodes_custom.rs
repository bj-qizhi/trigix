// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! Custom (community / third-party) nodes served over HTTP via the node SDK.
//!
//! A custom node's endpoint is resolved from the platform registry into the
//! node config as `_endpoint` at execution start; it may also be supplied
//! directly as `endpoint` (template-resolved). The executor POSTs the SDK
//! contract and returns the node's `output_json`.

use super::*;
use crate::runtime::{ExecutionContext, NodeExecutionResult};
use workflow_core::Node;

#[derive(serde::Serialize)]
struct CustomNodeRequest {
    node_id: String,
    config: serde_json::Value,
    input_json: String,
    node_outputs: std::collections::HashMap<String, String>,
}

#[derive(serde::Deserialize)]
struct CustomNodeResponse {
    output_json: String,
}

pub(super) async fn execute_custom(
    node: &Node,
    context: &ExecutionContext,
    client: &reqwest::Client,
) -> NodeExecutionResult {
    let cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Custom node requires config"),
    };

    // Endpoint is injected by the platform (`_endpoint`) from the registry, or
    // provided directly as `endpoint` (supports {{...}} templates).
    let endpoint = cfg
        .get("_endpoint")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            cfg.get("endpoint")
                .and_then(|v| v.as_str())
                .map(|s| resolve_template(s, context))
        });
    let endpoint = match endpoint {
        Some(e) if !e.is_empty() => e,
        _ => {
            return NodeExecutionResult::failed(
                "Custom node has no endpoint (unknown custom_node or missing endpoint)",
            )
        }
    };

    // The user-facing config passed to the node, with internal keys stripped.
    let mut node_config = cfg.clone();
    if let Some(obj) = node_config.as_object_mut() {
        obj.remove("_endpoint");
        obj.remove("custom_node");
    }

    let request = CustomNodeRequest {
        node_id: node.id.clone(),
        config: node_config,
        input_json: context.input_json.clone(),
        node_outputs: context.node_outputs.clone(),
    };

    match client.post(&endpoint).json(&request).send().await {
        Ok(response) => {
            if response.status().is_success() {
                match response.json::<CustomNodeResponse>().await {
                    Ok(payload) => NodeExecutionResult::succeeded(payload.output_json),
                    Err(e) => NodeExecutionResult::failed(format!(
                        "Custom node returned an invalid response: {e}"
                    )),
                }
            } else {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                NodeExecutionResult::failed(format!("Custom node returned {status}: {body}"))
            }
        }
        Err(e) => NodeExecutionResult::failed(format!("Failed to reach custom node: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use workflow_core::NodeType;

    fn ctx() -> ExecutionContext {
        ExecutionContext {
            execution_id: "e1".into(),
            workflow_version_id: "v1".into(),
            input_json: "{}".into(),
            node_outputs: Default::default(),
            dry_run: false,
        }
    }

    #[tokio::test]
    async fn fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "c1".into(),
            node_type: NodeType::Custom,
            config: Some(serde_json::json!({ "foo": "bar" })),
        };
        let r = execute_custom(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    #[tokio::test]
    async fn fails_without_config() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "c2".into(),
            node_type: NodeType::Custom,
            config: None,
        };
        let r = execute_custom(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("config"));
    }
}
