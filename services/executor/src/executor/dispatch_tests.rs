// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! Tests for the executor engine core (dispatch, shared helpers and the
//! built-in node implementations). Extracted from executor.rs to keep that
//! file focused on the ~1.7k lines of production engine code.

#[cfg(test)]
mod tests {
    use super::super::*;
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
    async fn grok_and_ollama_nodes_require_config() {
        let executor = DispatchingNodeExecutor::new(None);
        let context = make_context("{}");
        for nt in [
            NodeType::Grok,
            NodeType::Ollama,
            NodeType::AzureOpenai,
            NodeType::Vertex,
        ] {
            let node = Node {
                id: "llm".to_string(),
                node_type: nt,
                config: None,
            };
            let result = executor.execute(&node, &context).await;
            assert_eq!(result.status, execution_core::NodeStatus::Failed);
        }
    }

    #[tokio::test]
    async fn wait_duration_zero_returns_immediately() {
        let node = Node {
            id: "w".to_string(),
            node_type: NodeType::Wait,
            config: Some(serde_json::json!({ "mode": "duration", "seconds": 0 })),
        };
        let r = execute_wait(&node, &make_context("{}"), None).await;
        assert_eq!(r.status, execution_core::NodeStatus::Succeeded);
    }

    #[tokio::test]
    async fn wait_resume_without_gate_fails() {
        let node = Node {
            id: "w".to_string(),
            node_type: NodeType::Wait,
            config: Some(serde_json::json!({ "mode": "resume" })),
        };
        let r = execute_wait(&node, &make_context("{}"), None).await;
        assert_eq!(r.status, execution_core::NodeStatus::Failed);
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

    async fn run_condition(config: serde_json::Value, input: &str) -> bool {
        let executor = DispatchingNodeExecutor::new(None);
        let node = Node {
            id: "condition".to_string(),
            node_type: NodeType::Condition,
            config: Some(config),
        };
        let result = executor.execute(&node, &make_context(input)).await;
        assert_eq!(result.status, execution_core::NodeStatus::Succeeded);
        serde_json::from_str::<serde_json::Value>(result.output_json.as_deref().unwrap()).unwrap()
            ["result"]
            .as_bool()
            .unwrap()
    }

    #[tokio::test]
    async fn condition_numeric_operators() {
        // gt / lt / gte / lte on a value looked up in input_json.
        assert!(
            run_condition(
                serde_json::json!({ "field": "amount", "operator": "gt", "value": "100" }),
                r#"{"amount":150}"#
            )
            .await
        );
        assert!(
            !run_condition(
                serde_json::json!({ "field": "amount", "operator": "gt", "value": "100" }),
                r#"{"amount":50}"#
            )
            .await
        );
        assert!(
            run_condition(
                serde_json::json!({ "field": "amount", "operator": "lte", "value": "50" }),
                r#"{"amount":50}"#
            )
            .await
        );
    }

    #[tokio::test]
    async fn condition_operators_equals_contains_exists() {
        assert!(
            run_condition(
                serde_json::json!({ "field": "type", "operator": "equals", "value": "purchase" }),
                r#"{"type":"purchase"}"#
            )
            .await
        );
        assert!(
            run_condition(
                serde_json::json!({ "field": "msg", "operator": "contains", "value": "error" }),
                r#"{"msg":"fatal error here"}"#
            )
            .await
        );
        assert!(
            run_condition(
                serde_json::json!({ "field": "id", "operator": "exists" }),
                r#"{"id":"x"}"#
            )
            .await
        );
        assert!(
            !run_condition(
                serde_json::json!({ "field": "id", "operator": "exists" }),
                r#"{"other":1}"#
            )
            .await
        );
    }

    #[tokio::test]
    async fn condition_source_with_dotpath() {
        // `source` resolves a JSON object, then `field` is a dot-path into it —
        // the form the bundled templates use.
        let cfg = serde_json::json!({ "source": "{{input}}", "field": "order.total", "operator": "gte", "value": "100" });
        assert!(run_condition(cfg, r#"{"order":{"total":120}}"#).await);
    }

    #[test]
    fn json_array_or_parse_handles_stringified_and_plain() {
        // A JSON array that arrived as a string (the template engine stringifies
        // every {{...}} substitution) is parsed back into a real array, with
        // inner float arrays preserved.
        let stringified = serde_json::Value::String(
            r#"[{"id":"d-0","values":[0.1,0.2],"metadata":{"text":"a"}}]"#.to_string(),
        );
        let parsed = json_array_or_parse(&stringified);
        assert!(parsed.is_array());
        let first = &parsed[0];
        assert_eq!(first["id"], "d-0");
        assert!(first["values"].is_array());
        assert_eq!(first["values"][1], 0.2);

        // Whitespace around a JSON object is tolerated.
        let obj = serde_json::Value::String("  {\"a\":1}  ".to_string());
        assert_eq!(json_array_or_parse(&obj)["a"], 1);

        // A plain (non-JSON) string is returned unchanged.
        let plain = serde_json::Value::String("hello".to_string());
        assert_eq!(json_array_or_parse(&plain), plain);

        // A malformed array string is left as-is rather than silently dropped.
        let broken = serde_json::Value::String("[1,2".to_string());
        assert_eq!(json_array_or_parse(&broken), broken);

        // Already-structured values pass through untouched.
        let arr = serde_json::json!([1, 2, 3]);
        assert_eq!(json_array_or_parse(&arr), arr);
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
        let _node = Node {
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
            assert!((10.0..=20.0).contains(&v), "value {v} out of range");
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
        let _node = Node {
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
    use super::super::*;
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
            config: Some(serde_json::json!({ "operation": "round", "a": 5.6789, "precision": 2 })),
        };
        let r = execute_math(&node, &ctx());
        let out: serde_json::Value =
            serde_json::from_str(r.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(out["result"], 5.68);
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
    use super::super::*;
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
    use super::super::*;
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
    use super::super::*;

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
