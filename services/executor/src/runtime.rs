use std::collections::{HashMap, HashSet};

use crate::scheduler::schedule;
use execution_core::{ExecutionStatus, NodeStatus};
use serde::{Deserialize, Serialize};
use workflow_core::{GraphError, Node, NodeType, WorkflowGraph};
use serde_json;

pub trait NodeExecutor {
    fn execute<'a>(
        &'a mut self,
        node: &'a Node,
        context: &'a ExecutionContext,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = NodeExecutionResult> + Send + 'a>>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionContext {
    pub execution_id: String,
    pub workflow_version_id: String,
    pub input_json: String,
    pub node_outputs: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeExecutionResult {
    pub status: NodeStatus,
    pub output_json: Option<String>,
    pub error: Option<String>,
}

impl NodeExecutionResult {
    pub fn succeeded(output_json: impl Into<String>) -> Self {
        Self {
            status: NodeStatus::Succeeded,
            output_json: Some(output_json.into()),
            error: None,
        }
    }

    pub fn failed(error: impl Into<String>) -> Self {
        Self {
            status: NodeStatus::Failed,
            output_json: None,
            error: Some(error.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionReport {
    pub execution_id: String,
    pub status: ExecutionStatus,
    pub node_results: Vec<NodeReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeReport {
    pub node_id: String,
    pub status: NodeStatus,
    pub output_json: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeError {
    InvalidGraph(GraphError),
    MissingNode(String),
}

pub async fn run_workflow(
    execution_id: impl Into<String>,
    graph: &WorkflowGraph,
    input_json: impl Into<String>,
    executor: &mut impl NodeExecutor,
) -> Result<ExecutionReport, RuntimeError> {
    let execution_id = execution_id.into();
    let order = schedule(graph).map_err(RuntimeError::InvalidGraph)?;
    let nodes_by_id: HashMap<&str, &Node> = graph
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();

    // Build incoming-edge index: node_id -> [(source_id, condition_label)]
    let mut incoming: HashMap<&str, Vec<(&str, Option<&str>)>> = HashMap::new();
    for node in &graph.nodes {
        incoming.entry(node.id.as_str()).or_default();
    }
    for edge in &graph.edges {
        incoming
            .entry(edge.target.as_str())
            .or_default()
            .push((edge.source.as_str(), edge.condition_label.as_deref()));
    }

    let mut context = ExecutionContext {
        execution_id: execution_id.clone(),
        workflow_version_id: graph.workflow_version_id.clone(),
        input_json: input_json.into(),
        node_outputs: HashMap::new(),
    };
    let mut node_results = Vec::with_capacity(order.len());
    let mut skipped: HashSet<String> = HashSet::new();
    let mut failed_nodes: HashSet<String> = HashSet::new();
    // condition node id -> bool result
    let mut condition_results: HashMap<String, bool> = HashMap::new();

    for node_id in order {
        let node = nodes_by_id
            .get(node_id.as_str())
            .ok_or_else(|| RuntimeError::MissingNode(node_id.clone()))?;

        // Determine if this node should be skipped due to inactive incoming edges.
        let edges_in = incoming.get(node_id.as_str()).map(Vec::as_slice).unwrap_or(&[]);
        let should_skip = !edges_in.is_empty() && edges_in.iter().all(|(src, label)| {
            if skipped.contains(*src) {
                return true; // source skipped → edge inactive
            }
            if failed_nodes.contains(*src) {
                // error edge from a failed node is ACTIVE; all others are inactive
                return label.map(|l| l != "error").unwrap_or(true);
            }
            if let Some(lbl) = label {
                if *lbl == "error" {
                    return true; // error edge from a successful node is inactive
                }
                let expected = *lbl == "true";
                return condition_results.get(*src).copied() != Some(expected);
            }
            false // unconditional edge from successful node → active
        });

        if should_skip {
            skipped.insert(node_id.clone());
            node_results.push(NodeReport {
                node_id: node_id.clone(),
                status: NodeStatus::Skipped,
                output_json: None,
                error: None,
            });
            continue;
        }

        // For FanIn nodes, inject the IDs of active incoming source nodes into config
        // so execute_fan_in can collect their outputs.
        let owned_fan_in: Option<Node> = if node.node_type == NodeType::FanIn {
            let sources: Vec<String> = edges_in
                .iter()
                .filter(|(src, _)| !skipped.contains(*src) && !failed_nodes.contains(*src))
                .map(|(src, _)| (*src).to_string())
                .collect();
            let mut config = node.config.clone().unwrap_or_else(|| serde_json::json!({}));
            config["_sources"] = serde_json::json!(sources);
            Some(Node {
                id: node.id.clone(),
                node_type: NodeType::FanIn,
                config: Some(config),
            })
        } else {
            None
        };
        let effective_node = owned_fan_in.as_ref().unwrap_or(node);

        let result = executor.execute(effective_node, &context).await;

        if result.status == NodeStatus::Succeeded {
            if let Some(output) = &result.output_json {
                context.node_outputs.insert(node_id.clone(), output.clone());
                // Record condition result for branch routing.
                if node.node_type == NodeType::Condition {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(output) {
                        if let Some(b) = v.get("result").and_then(|r| r.as_bool()) {
                            condition_results.insert(node_id.clone(), b);
                        }
                    }
                }
            }
        }

        node_results.push(NodeReport {
            node_id: node_id.clone(),
            status: result.status.clone(),
            output_json: result.output_json.clone(),
            error: result.error.clone(),
        });

        if result.status == NodeStatus::Failed {
            // Store error output so {{node_id.error}} is available to catch handlers.
            let error_msg = result.error.as_deref().unwrap_or("unknown error");
            context.node_outputs.insert(
                node_id.clone(),
                serde_json::json!({ "error": error_msg, "failed": true }).to_string(),
            );
            failed_nodes.insert(node_id.clone());

            // If no error edge exists, fail the workflow immediately.
            let has_error_route = graph.edges.iter().any(|e| {
                e.source == node_id && e.condition_label.as_deref() == Some("error")
            });
            if !has_error_route {
                return Ok(ExecutionReport {
                    execution_id,
                    status: ExecutionStatus::Failed,
                    node_results,
                });
            }
            // Error route exists — continue so the Catch node can handle it.
        }
    }

    Ok(ExecutionReport {
        execution_id,
        status: ExecutionStatus::Succeeded,
        node_results,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use workflow_core::{Edge, NodeType};

    #[tokio::test]
    async fn runs_single_node_workflow_to_success() {
        let graph = WorkflowGraph {
            workflow_version_id: "version-1".to_string(),
            nodes: vec![Node {
                id: "trigger".to_string(),
                node_type: NodeType::Trigger,
                config: None,
            }],
            edges: vec![],
        };
        let mut executor = EchoExecutor::default();

        let report = run_workflow("execution-1", &graph, "{}", &mut executor)
            .await
            .unwrap();

        assert_eq!(report.status, ExecutionStatus::Succeeded);
        assert_eq!(report.node_results.len(), 1);
        assert_eq!(report.node_results[0].node_id, "trigger");
        assert_eq!(executor.executed_nodes, vec!["trigger"]);
    }

    #[tokio::test]
    async fn passes_prior_node_outputs_through_context() {
        let graph = WorkflowGraph {
            workflow_version_id: "version-1".to_string(),
            nodes: vec![
                Node {
                    id: "trigger".to_string(),
                    node_type: NodeType::Trigger,
                    config: None,
                },
                Node {
                    id: "http".to_string(),
                    node_type: NodeType::Http,
                    config: None,
                },
            ],
            edges: vec![Edge {
                source: "trigger".to_string(),
                target: "http".to_string(),
                condition_label: None,
            }],
        };
        let mut executor = EchoExecutor::default();

        let report = run_workflow("execution-1", &graph, "{}", &mut executor)
            .await
            .unwrap();

        assert_eq!(report.status, ExecutionStatus::Succeeded);
        assert_eq!(
            executor.context_output_counts,
            vec![("trigger".to_string(), 0), ("http".to_string(), 1)]
        );
    }

    #[tokio::test]
    async fn stops_after_failed_node() {
        let graph = WorkflowGraph {
            workflow_version_id: "version-1".to_string(),
            nodes: vec![
                Node {
                    id: "trigger".to_string(),
                    node_type: NodeType::Trigger,
                    config: None,
                },
                Node {
                    id: "http".to_string(),
                    node_type: NodeType::Http,
                    config: None,
                },
            ],
            edges: vec![Edge {
                source: "trigger".to_string(),
                target: "http".to_string(),
                condition_label: None,
            }],
        };
        let mut executor = FailingExecutor {
            fail_node_id: "trigger".to_string(),
            executed_nodes: Vec::new(),
        };

        let report = run_workflow("execution-1", &graph, "{}", &mut executor)
            .await
            .unwrap();

        assert_eq!(report.status, ExecutionStatus::Failed);
        assert_eq!(report.node_results.len(), 1);
        assert_eq!(report.node_results[0].status, NodeStatus::Failed);
        assert_eq!(executor.executed_nodes, vec!["trigger"]);
    }

    // Graph: trigger -> condition -> (true: http_true, false: http_false)
    fn condition_graph() -> WorkflowGraph {
        WorkflowGraph {
            workflow_version_id: "version-1".to_string(),
            nodes: vec![
                Node { id: "trigger".to_string(),    node_type: NodeType::Trigger,   config: None },
                Node { id: "cond".to_string(),       node_type: NodeType::Condition, config: None },
                Node { id: "http_true".to_string(),  node_type: NodeType::Http,      config: None },
                Node { id: "http_false".to_string(), node_type: NodeType::Http,      config: None },
            ],
            edges: vec![
                Edge { source: "trigger".to_string(),    target: "cond".to_string(),       condition_label: None },
                Edge { source: "cond".to_string(),       target: "http_true".to_string(),  condition_label: Some("true".to_string()) },
                Edge { source: "cond".to_string(),       target: "http_false".to_string(), condition_label: Some("false".to_string()) },
            ],
        }
    }

    #[tokio::test]
    async fn condition_true_skips_false_branch() {
        let graph = condition_graph();
        // Executor returns {"result":true} for condition node, echo for others.
        let mut executor = ConditionExecutor { result: true };

        let report = run_workflow("exec-1", &graph, "{}", &mut executor).await.unwrap();

        assert_eq!(report.status, ExecutionStatus::Succeeded);
        let statuses: HashMap<&str, NodeStatus> = report
            .node_results.iter().map(|r| (r.node_id.as_str(), r.status.clone())).collect();
        assert_eq!(statuses["trigger"],    NodeStatus::Succeeded);
        assert_eq!(statuses["cond"],       NodeStatus::Succeeded);
        assert_eq!(statuses["http_true"],  NodeStatus::Succeeded);
        assert_eq!(statuses["http_false"], NodeStatus::Skipped);
    }

    #[tokio::test]
    async fn condition_false_skips_true_branch() {
        let graph = condition_graph();
        let mut executor = ConditionExecutor { result: false };

        let report = run_workflow("exec-2", &graph, "{}", &mut executor).await.unwrap();

        assert_eq!(report.status, ExecutionStatus::Succeeded);
        let statuses: HashMap<&str, NodeStatus> = report
            .node_results.iter().map(|r| (r.node_id.as_str(), r.status.clone())).collect();
        assert_eq!(statuses["trigger"],    NodeStatus::Succeeded);
        assert_eq!(statuses["cond"],       NodeStatus::Succeeded);
        assert_eq!(statuses["http_true"],  NodeStatus::Skipped);
        assert_eq!(statuses["http_false"], NodeStatus::Succeeded);
    }

    #[tokio::test]
    async fn map_node_runs_in_workflow() {
        // trigger → map: trigger outputs an array, map fans it out
        let graph = WorkflowGraph {
            workflow_version_id: "v1".to_string(),
            nodes: vec![
                Node { id: "trigger".to_string(), node_type: NodeType::Trigger, config: None },
                Node {
                    id: "map".to_string(),
                    node_type: NodeType::Map,
                    config: Some(serde_json::json!({ "items": "{{trigger.items}}" })),
                },
            ],
            edges: vec![Edge { source: "trigger".to_string(), target: "map".to_string(), condition_label: None }],
        };
        let mut executor = EchoExecutor::default();

        // Trigger returns input as-is; input has an "items" array.
        let report = run_workflow(
            "exec-map",
            &graph,
            r#"{"items":[1,2,3]}"#,
            &mut executor,
        )
        .await
        .unwrap();

        assert_eq!(report.status, ExecutionStatus::Succeeded);
        // map node was called
        assert!(executor.executed_nodes.contains(&"map".to_string()));
    }

    // Graph: trigger → fan_out → [branch_a, branch_b] → fan_in
    fn fan_out_in_graph() -> WorkflowGraph {
        WorkflowGraph {
            workflow_version_id: "v1".to_string(),
            nodes: vec![
                Node { id: "trigger".to_string(),  node_type: NodeType::Trigger, config: None },
                Node { id: "fan_out".to_string(),  node_type: NodeType::FanOut,  config: None },
                Node { id: "branch_a".to_string(), node_type: NodeType::Http,    config: None },
                Node { id: "branch_b".to_string(), node_type: NodeType::Agent,   config: None },
                Node { id: "fan_in".to_string(),   node_type: NodeType::FanIn,   config: None },
            ],
            edges: vec![
                Edge { source: "trigger".to_string(),  target: "fan_out".to_string(),  condition_label: None },
                Edge { source: "fan_out".to_string(),  target: "branch_a".to_string(), condition_label: None },
                Edge { source: "fan_out".to_string(),  target: "branch_b".to_string(), condition_label: None },
                Edge { source: "branch_a".to_string(), target: "fan_in".to_string(),   condition_label: None },
                Edge { source: "branch_b".to_string(), target: "fan_in".to_string(),   condition_label: None },
            ],
        }
    }

    #[tokio::test]
    async fn fan_out_in_all_nodes_succeed() {
        let graph = fan_out_in_graph();
        let mut executor = EchoExecutor::default();
        let report = run_workflow("exec-fan", &graph, "{}", &mut executor).await.unwrap();

        assert_eq!(report.status, ExecutionStatus::Succeeded);
        let statuses: HashMap<&str, NodeStatus> = report
            .node_results.iter().map(|r| (r.node_id.as_str(), r.status.clone())).collect();
        assert_eq!(statuses["trigger"],  NodeStatus::Succeeded);
        assert_eq!(statuses["fan_out"],  NodeStatus::Succeeded);
        assert_eq!(statuses["branch_a"], NodeStatus::Succeeded);
        assert_eq!(statuses["branch_b"], NodeStatus::Succeeded);
        assert_eq!(statuses["fan_in"],   NodeStatus::Succeeded);
        // All 5 nodes executed (EchoExecutor confirms via executed_nodes)
        assert_eq!(executor.executed_nodes.len(), 5);
    }

    #[tokio::test]
    async fn fan_in_sources_injected_from_active_branches() {
        // Verify _sources injection: use a FanInAwareExecutor that records the injected sources.
        let graph = fan_out_in_graph();
        let mut executor = FanInAwareExecutor::default();
        let report = run_workflow("exec-fan2", &graph, "{}", &mut executor).await.unwrap();
        assert_eq!(report.status, ExecutionStatus::Succeeded);
        // Both branch_a and branch_b should have been injected as sources
        let sources = &executor.fan_in_sources;
        assert!(sources.contains(&"branch_a".to_string()), "expected branch_a in sources: {sources:?}");
        assert!(sources.contains(&"branch_b".to_string()), "expected branch_b in sources: {sources:?}");
    }

    // Graph: trigger -> http (fails) --error--> catch -> agent
    fn error_routing_graph() -> WorkflowGraph {
        WorkflowGraph {
            workflow_version_id: "v1".to_string(),
            nodes: vec![
                Node { id: "trigger".to_string(), node_type: NodeType::Trigger, config: None },
                Node { id: "http".to_string(), node_type: NodeType::Http, config: None },
                Node { id: "catch".to_string(), node_type: NodeType::Catch, config: None },
                Node { id: "agent".to_string(), node_type: NodeType::Agent, config: None },
            ],
            edges: vec![
                Edge { source: "trigger".to_string(), target: "http".to_string(), condition_label: None },
                Edge { source: "http".to_string(), target: "catch".to_string(), condition_label: Some("error".to_string()) },
                Edge { source: "catch".to_string(), target: "agent".to_string(), condition_label: None },
            ],
        }
    }

    #[tokio::test]
    async fn error_edge_routes_to_catch_node() {
        let graph = error_routing_graph();
        let mut executor = FailingExecutor {
            fail_node_id: "http".to_string(),
            executed_nodes: Vec::new(),
        };

        let report = run_workflow("exec-err", &graph, "{}", &mut executor).await.unwrap();

        assert_eq!(report.status, ExecutionStatus::Succeeded);
        let statuses: HashMap<&str, NodeStatus> = report
            .node_results.iter().map(|r| (r.node_id.as_str(), r.status.clone())).collect();
        assert_eq!(statuses["trigger"], NodeStatus::Succeeded);
        assert_eq!(statuses["http"],    NodeStatus::Failed);
        assert_eq!(statuses["catch"],   NodeStatus::Succeeded);
        assert_eq!(statuses["agent"],   NodeStatus::Succeeded);
    }

    #[tokio::test]
    async fn no_error_edge_fails_workflow() {
        // Same graph but without the error edge — workflow should fail
        let graph = WorkflowGraph {
            workflow_version_id: "v1".to_string(),
            nodes: vec![
                Node { id: "trigger".to_string(), node_type: NodeType::Trigger, config: None },
                Node { id: "http".to_string(), node_type: NodeType::Http, config: None },
            ],
            edges: vec![
                Edge { source: "trigger".to_string(), target: "http".to_string(), condition_label: None },
            ],
        };
        let mut executor = FailingExecutor {
            fail_node_id: "http".to_string(),
            executed_nodes: Vec::new(),
        };

        let report = run_workflow("exec-noe", &graph, "{}", &mut executor).await.unwrap();
        assert_eq!(report.status, ExecutionStatus::Failed);
    }

    #[derive(Default)]
    struct FanInAwareExecutor {
        fan_in_sources: Vec<String>,
    }

    impl NodeExecutor for FanInAwareExecutor {
        fn execute<'a>(
            &'a mut self,
            node: &'a Node,
            _ctx: &'a ExecutionContext,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = NodeExecutionResult> + Send + 'a>> {
            if node.node_type == NodeType::FanIn {
                if let Some(sources) = node.config.as_ref()
                    .and_then(|c| c.get("_sources"))
                    .and_then(|v| v.as_array())
                {
                    self.fan_in_sources = sources.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect();
                }
            }
            let node_id = node.id.clone();
            Box::pin(async move {
                NodeExecutionResult::succeeded(format!("{{\"node_id\":\"{node_id}\"}}"))
            })
        }
    }

    struct ConditionExecutor { result: bool }

    impl NodeExecutor for ConditionExecutor {
        fn execute<'a>(
            &'a mut self,
            node: &'a Node,
            _ctx: &'a ExecutionContext,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = NodeExecutionResult> + Send + 'a>> {
            let result = self.result;
            let node_id = node.id.clone();
            let is_condition = node.node_type == NodeType::Condition;
            Box::pin(async move {
                if is_condition {
                    return NodeExecutionResult::succeeded(format!("{{\"result\":{result}}}"));
                }
                NodeExecutionResult::succeeded(format!("{{\"node_id\":\"{node_id}\"}}"))
            })
        }
    }

    #[derive(Default)]
    struct EchoExecutor {
        executed_nodes: Vec<String>,
        context_output_counts: Vec<(String, usize)>,
    }

    impl NodeExecutor for EchoExecutor {
        fn execute<'a>(
            &'a mut self,
            node: &'a Node,
            context: &'a ExecutionContext,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = NodeExecutionResult> + Send + 'a>> {
            self.executed_nodes.push(node.id.clone());
            self.context_output_counts.push((node.id.clone(), context.node_outputs.len()));
            let node_id = node.id.clone();
            Box::pin(async move {
                NodeExecutionResult::succeeded(format!("{{\"node_id\":\"{node_id}\"}}"))
            })
        }
    }

    struct FailingExecutor {
        fail_node_id: String,
        executed_nodes: Vec<String>,
    }

    impl NodeExecutor for FailingExecutor {
        fn execute<'a>(
            &'a mut self,
            node: &'a Node,
            _context: &'a ExecutionContext,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = NodeExecutionResult> + Send + 'a>> {
            self.executed_nodes.push(node.id.clone());
            let should_fail = node.id == self.fail_node_id;
            let node_id = node.id.clone();
            Box::pin(async move {
                if should_fail {
                    return NodeExecutionResult::failed("node failed");
                }
                NodeExecutionResult::succeeded(format!("{{\"node_id\":\"{node_id}\"}}"))
            })
        }
    }
}
