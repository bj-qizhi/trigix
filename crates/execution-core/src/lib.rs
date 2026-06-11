// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! Core execution contracts shared between the workflow runtime driver and the
//! node executor. This crate owns the abstractions (`NodeExecutor`, the
//! `ExecutionContext`/`NodeExecutionResult` types, and the `run_workflow`
//! driver loop) so both `executor` and `platform-rs` depend on the abstraction
//! rather than reaching into the executor's internals.

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use tracing::{info, info_span, Instrument};
use workflow_core::{GraphError, Node, NodeType, WorkflowGraph};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    Running,
    WaitingApproval,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeStatus {
    Running,
    Succeeded,
    Failed,
    Skipped,
}

pub trait NodeExecutor: Sync {
    fn execute<'a>(
        &'a self,
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
    /// When true, external HTTP/integration calls are skipped and mocked.
    pub dry_run: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeExecutionResult {
    pub status: NodeStatus,
    pub output_json: Option<String>,
    pub error: Option<String>,
    pub retry_count: u32,
}

impl NodeExecutionResult {
    pub fn succeeded(output_json: impl Into<String>) -> Self {
        Self {
            status: NodeStatus::Succeeded,
            output_json: Some(output_json.into()),
            error: None,
            retry_count: 0,
        }
    }

    pub fn failed(error: impl Into<String>) -> Self {
        Self {
            status: NodeStatus::Failed,
            output_json: None,
            error: Some(error.into()),
            retry_count: 0,
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
    pub duration_ms: u64,
    /// Milliseconds from execution start to when this node began executing.
    #[serde(default)]
    pub started_at_ms: u64,
    #[serde(default)]
    pub retry_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeError {
    InvalidGraph(GraphError),
    MissingNode(String),
}

/// Callback invoked after each node completes (succeeded, failed, or skipped).
/// The implementation must be synchronous; use `tokio::spawn` for async side-effects.
pub trait NodeProgressCallback: Send + Sync {
    fn on_node_complete(&self, report: &NodeReport);
}

/// No-op implementation used when progress reporting is not needed.
pub struct NoopProgress;
impl NodeProgressCallback for NoopProgress {
    fn on_node_complete(&self, _: &NodeReport) {}
}

pub async fn run_workflow(
    execution_id: impl Into<String>,
    graph: &WorkflowGraph,
    input_json: impl Into<String>,
    executor: &impl NodeExecutor,
    dry_run: bool,
) -> Result<ExecutionReport, RuntimeError> {
    run_workflow_with_progress(
        execution_id,
        graph,
        input_json,
        executor,
        &NoopProgress,
        dry_run,
    )
    .await
}

pub async fn run_workflow_with_progress(
    execution_id: impl Into<String>,
    graph: &WorkflowGraph,
    input_json: impl Into<String>,
    executor: &impl NodeExecutor,
    progress: &impl NodeProgressCallback,
    dry_run: bool,
) -> Result<ExecutionReport, RuntimeError> {
    let execution_id = execution_id.into();
    let exec_start = Instant::now();
    info!(execution_id = %execution_id, dry_run, node_count = graph.nodes.len(), "workflow started");
    let levels = graph
        .topological_levels()
        .map_err(RuntimeError::InvalidGraph)?;
    let nodes_by_id: HashMap<&str, &Node> =
        graph.nodes.iter().map(|n| (n.id.as_str(), n)).collect();

    // incoming[node_id] = [(source_id, condition_label)]
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
        dry_run,
    };
    let mut node_results: Vec<NodeReport> = Vec::with_capacity(graph.nodes.len());
    let mut skipped: HashSet<String> = HashSet::new();
    let mut failed_nodes: HashSet<String> = HashSet::new();
    let mut condition_results: HashMap<String, bool> = HashMap::new();

    'levels: for level in &levels {
        // Determine which nodes in this level should run vs skip.
        let mut to_run: Vec<(String, Node)> = Vec::new();

        for node_id in level {
            let node = nodes_by_id
                .get(node_id.as_str())
                .ok_or_else(|| RuntimeError::MissingNode(node_id.clone()))?;
            let edges_in = incoming
                .get(node_id.as_str())
                .map(Vec::as_slice)
                .unwrap_or(&[]);

            let should_skip = !edges_in.is_empty()
                && edges_in.iter().all(|(src, label)| {
                    if skipped.contains(*src) {
                        return true;
                    }
                    if failed_nodes.contains(*src) {
                        return label.map(|l| l != "error").unwrap_or(true);
                    }
                    if let Some(lbl) = label {
                        if *lbl == "error" {
                            return true;
                        }
                        let expected = *lbl == "true";
                        return condition_results.get(*src).copied() != Some(expected);
                    }
                    false
                });

            if should_skip {
                skipped.insert(node_id.clone());
                let report = NodeReport {
                    node_id: node_id.clone(),
                    status: NodeStatus::Skipped,
                    output_json: None,
                    error: None,
                    duration_ms: 0,
                    started_at_ms: 0,
                    retry_count: 0,
                };
                progress.on_node_complete(&report);
                node_results.push(report);
                continue;
            }

            // For FanIn nodes, inject active incoming source IDs.
            let effective_node = if node.node_type == NodeType::FanIn {
                let sources: Vec<String> = edges_in
                    .iter()
                    .filter(|(src, _)| !skipped.contains(*src) && !failed_nodes.contains(*src))
                    .map(|(src, _)| (*src).to_string())
                    .collect();
                let mut config = node.config.clone().unwrap_or_else(|| serde_json::json!({}));
                config["_sources"] = serde_json::json!(sources);
                Node {
                    id: node.id.clone(),
                    node_type: NodeType::FanIn,
                    config: Some(config),
                }
            } else {
                (*node).clone()
            };

            to_run.push((node_id.clone(), effective_node));
        }

        if to_run.is_empty() {
            continue 'levels;
        }

        // Execute all nodes in this level concurrently.
        let futs: Vec<_> = to_run
            .iter()
            .map(|(node_id, node)| {
                let started_at_ms = exec_start.elapsed().as_millis() as u64;
                let start = Instant::now();
                let span = info_span!("node_execute", execution_id = %context.execution_id, node_id = %node_id, node_type = ?node.node_type);
                let fut = executor.execute(node, &context).instrument(span);
                (started_at_ms, start, fut)
            })
            .collect();

        // Poll all futures concurrently using futures::future::join_all.
        let level_results: Vec<(u64, Instant, NodeExecutionResult)> = {
            use futures::future::join_all;
            let starts: Vec<(u64, Instant)> = futs.iter().map(|(ms, s, _)| (*ms, *s)).collect();
            let futures_only: Vec<_> = futs.into_iter().map(|(_, _, f)| f).collect();
            let results = join_all(futures_only).await;
            starts
                .into_iter()
                .zip(results)
                .map(|((ms, s), r)| (ms, s, r))
                .collect()
        };

        // Process results and check for early-exit conditions.
        for ((node_id, node), (started_at_ms, start, result)) in
            to_run.iter().zip(level_results.iter())
        {
            let duration_ms = start.elapsed().as_millis() as u64;
            info!(execution_id = %context.execution_id, node_id = %node_id, status = ?result.status, duration_ms, "node complete");

            if result.status == NodeStatus::Succeeded {
                if let Some(output) = &result.output_json {
                    context.node_outputs.insert(node_id.clone(), output.clone());
                    if node.node_type == NodeType::Condition {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(output) {
                            if let Some(b) = v.get("result").and_then(|r| r.as_bool()) {
                                condition_results.insert(node_id.clone(), b);
                            }
                        }
                    }
                }
            }

            let node_report = NodeReport {
                node_id: node_id.clone(),
                status: result.status.clone(),
                output_json: result.output_json.clone(),
                error: result.error.clone(),
                duration_ms,
                started_at_ms: *started_at_ms,
                retry_count: result.retry_count,
            };
            progress.on_node_complete(&node_report);
            node_results.push(node_report);

            if result.status == NodeStatus::Failed {
                let error_msg = result.error.as_deref().unwrap_or("unknown error");
                context.node_outputs.insert(
                    node_id.clone(),
                    serde_json::json!({ "error": error_msg, "failed": true }).to_string(),
                );
                failed_nodes.insert(node_id.clone());

                let has_error_route = graph
                    .edges
                    .iter()
                    .any(|e| e.source == *node_id && e.condition_label.as_deref() == Some("error"));
                if !has_error_route {
                    // Fill remaining nodes in this level as skipped then bail.
                    for (remaining_id, _) in to_run
                        .iter()
                        .skip(to_run.iter().position(|(id, _)| id == node_id).unwrap_or(0) + 1)
                    {
                        let skipped_report = NodeReport {
                            node_id: remaining_id.clone(),
                            status: NodeStatus::Skipped,
                            output_json: None,
                            error: None,
                            duration_ms: 0,
                            started_at_ms: 0,
                            retry_count: 0,
                        };
                        progress.on_node_complete(&skipped_report);
                        node_results.push(skipped_report);
                    }
                    return Ok(ExecutionReport {
                        execution_id,
                        status: ExecutionStatus::Failed,
                        node_results,
                    });
                }
            }
        }
    }

    info!(execution_id = %execution_id, status = "succeeded", "workflow complete");
    Ok(ExecutionReport {
        execution_id,
        status: ExecutionStatus::Succeeded,
        node_results,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
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
            input_schema: vec![],
        };
        let executor = EchoExecutor::default();
        let report = run_workflow("exec-1", &graph, "{}", &executor, false)
            .await
            .unwrap();

        assert_eq!(report.status, ExecutionStatus::Succeeded);
        assert_eq!(report.node_results.len(), 1);
        assert_eq!(report.node_results[0].node_id, "trigger");
        assert_eq!(*executor.executed_nodes.lock().unwrap(), vec!["trigger"]);
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
            input_schema: vec![],
        };
        let executor = EchoExecutor::default();
        let report = run_workflow("exec-2", &graph, "{}", &executor, false)
            .await
            .unwrap();

        assert_eq!(report.status, ExecutionStatus::Succeeded);
        let counts = executor.context_output_counts.lock().unwrap();
        assert_eq!(
            *counts,
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
            input_schema: vec![],
        };
        let executor = FailingExecutor::new("trigger");
        let report = run_workflow("exec-3", &graph, "{}", &executor, false)
            .await
            .unwrap();

        assert_eq!(report.status, ExecutionStatus::Failed);
        assert_eq!(report.node_results.len(), 1);
        assert_eq!(report.node_results[0].status, NodeStatus::Failed);
        assert_eq!(*executor.executed_nodes.lock().unwrap(), vec!["trigger"]);
    }

    fn condition_graph() -> WorkflowGraph {
        WorkflowGraph {
            workflow_version_id: "version-1".to_string(),
            nodes: vec![
                Node {
                    id: "trigger".to_string(),
                    node_type: NodeType::Trigger,
                    config: None,
                },
                Node {
                    id: "cond".to_string(),
                    node_type: NodeType::Condition,
                    config: None,
                },
                Node {
                    id: "http_true".to_string(),
                    node_type: NodeType::Http,
                    config: None,
                },
                Node {
                    id: "http_false".to_string(),
                    node_type: NodeType::Http,
                    config: None,
                },
            ],
            edges: vec![
                Edge {
                    source: "trigger".to_string(),
                    target: "cond".to_string(),
                    condition_label: None,
                },
                Edge {
                    source: "cond".to_string(),
                    target: "http_true".to_string(),
                    condition_label: Some("true".to_string()),
                },
                Edge {
                    source: "cond".to_string(),
                    target: "http_false".to_string(),
                    condition_label: Some("false".to_string()),
                },
            ],
            input_schema: vec![],
        }
    }

    #[tokio::test]
    async fn condition_true_skips_false_branch() {
        let executor = ConditionExecutor { result: true };
        let report = run_workflow("rt1", &condition_graph(), "{}", &executor, false)
            .await
            .unwrap();
        assert_eq!(report.status, ExecutionStatus::Succeeded);
        let statuses: HashMap<&str, NodeStatus> = report
            .node_results
            .iter()
            .map(|r| (r.node_id.as_str(), r.status.clone()))
            .collect();
        assert_eq!(statuses["http_true"], NodeStatus::Succeeded);
        assert_eq!(statuses["http_false"], NodeStatus::Skipped);
    }

    #[tokio::test]
    async fn condition_false_skips_true_branch() {
        let executor = ConditionExecutor { result: false };
        let report = run_workflow("rt2", &condition_graph(), "{}", &executor, false)
            .await
            .unwrap();
        assert_eq!(report.status, ExecutionStatus::Succeeded);
        let statuses: HashMap<&str, NodeStatus> = report
            .node_results
            .iter()
            .map(|r| (r.node_id.as_str(), r.status.clone()))
            .collect();
        assert_eq!(statuses["http_true"], NodeStatus::Skipped);
        assert_eq!(statuses["http_false"], NodeStatus::Succeeded);
    }

    #[tokio::test]
    async fn map_node_runs_in_workflow() {
        let graph = WorkflowGraph {
            workflow_version_id: "v1".to_string(),
            nodes: vec![
                Node {
                    id: "trigger".to_string(),
                    node_type: NodeType::Trigger,
                    config: None,
                },
                Node {
                    id: "map".to_string(),
                    node_type: NodeType::Map,
                    config: Some(serde_json::json!({ "items": "{{trigger.items}}" })),
                },
            ],
            edges: vec![Edge {
                source: "trigger".to_string(),
                target: "map".to_string(),
                condition_label: None,
            }],
            input_schema: vec![],
        };
        let executor = EchoExecutor::default();
        let report = run_workflow("exec-4", &graph, r#"{"items":[1,2,3]}"#, &executor, false)
            .await
            .unwrap();
        assert_eq!(report.status, ExecutionStatus::Succeeded);
        assert!(executor
            .executed_nodes
            .lock()
            .unwrap()
            .contains(&"map".to_string()));
    }

    fn fan_out_in_graph() -> WorkflowGraph {
        WorkflowGraph {
            workflow_version_id: "v1".to_string(),
            nodes: vec![
                Node {
                    id: "trigger".to_string(),
                    node_type: NodeType::Trigger,
                    config: None,
                },
                Node {
                    id: "fan_out".to_string(),
                    node_type: NodeType::FanOut,
                    config: None,
                },
                Node {
                    id: "branch_a".to_string(),
                    node_type: NodeType::Http,
                    config: None,
                },
                Node {
                    id: "branch_b".to_string(),
                    node_type: NodeType::Agent,
                    config: None,
                },
                Node {
                    id: "fan_in".to_string(),
                    node_type: NodeType::FanIn,
                    config: None,
                },
            ],
            edges: vec![
                Edge {
                    source: "trigger".to_string(),
                    target: "fan_out".to_string(),
                    condition_label: None,
                },
                Edge {
                    source: "fan_out".to_string(),
                    target: "branch_a".to_string(),
                    condition_label: None,
                },
                Edge {
                    source: "fan_out".to_string(),
                    target: "branch_b".to_string(),
                    condition_label: None,
                },
                Edge {
                    source: "branch_a".to_string(),
                    target: "fan_in".to_string(),
                    condition_label: None,
                },
                Edge {
                    source: "branch_b".to_string(),
                    target: "fan_in".to_string(),
                    condition_label: None,
                },
            ],
            input_schema: vec![],
        }
    }

    #[tokio::test]
    async fn fan_out_in_all_nodes_succeed() {
        let executor = EchoExecutor::default();
        let report = run_workflow("rt3", &fan_out_in_graph(), "{}", &executor, false)
            .await
            .unwrap();
        assert_eq!(report.status, ExecutionStatus::Succeeded);
        let statuses: HashMap<&str, NodeStatus> = report
            .node_results
            .iter()
            .map(|r| (r.node_id.as_str(), r.status.clone()))
            .collect();
        assert_eq!(statuses["trigger"], NodeStatus::Succeeded);
        assert_eq!(statuses["fan_out"], NodeStatus::Succeeded);
        assert_eq!(statuses["branch_a"], NodeStatus::Succeeded);
        assert_eq!(statuses["branch_b"], NodeStatus::Succeeded);
        assert_eq!(statuses["fan_in"], NodeStatus::Succeeded);
        assert_eq!(executor.executed_nodes.lock().unwrap().len(), 5);
    }

    #[tokio::test]
    async fn fan_in_sources_injected_from_active_branches() {
        let executor = FanInAwareExecutor::default();
        let report = run_workflow("rt4", &fan_out_in_graph(), "{}", &executor, false)
            .await
            .unwrap();
        assert_eq!(report.status, ExecutionStatus::Succeeded);
        let sources = executor.fan_in_sources.lock().unwrap();
        assert!(
            sources.contains(&"branch_a".to_string()),
            "expected branch_a in sources: {sources:?}"
        );
        assert!(
            sources.contains(&"branch_b".to_string()),
            "expected branch_b in sources: {sources:?}"
        );
    }

    fn error_routing_graph() -> WorkflowGraph {
        WorkflowGraph {
            workflow_version_id: "v1".to_string(),
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
                Node {
                    id: "catch".to_string(),
                    node_type: NodeType::Catch,
                    config: None,
                },
                Node {
                    id: "agent".to_string(),
                    node_type: NodeType::Agent,
                    config: None,
                },
            ],
            edges: vec![
                Edge {
                    source: "trigger".to_string(),
                    target: "http".to_string(),
                    condition_label: None,
                },
                Edge {
                    source: "http".to_string(),
                    target: "catch".to_string(),
                    condition_label: Some("error".to_string()),
                },
                Edge {
                    source: "catch".to_string(),
                    target: "agent".to_string(),
                    condition_label: None,
                },
            ],
            input_schema: vec![],
        }
    }

    #[tokio::test]
    async fn error_edge_routes_to_catch_node() {
        let executor = FailingExecutor::new("http");
        let report = run_workflow("rt5", &error_routing_graph(), "{}", &executor, false)
            .await
            .unwrap();
        assert_eq!(report.status, ExecutionStatus::Succeeded);
        let statuses: HashMap<&str, NodeStatus> = report
            .node_results
            .iter()
            .map(|r| (r.node_id.as_str(), r.status.clone()))
            .collect();
        assert_eq!(statuses["trigger"], NodeStatus::Succeeded);
        assert_eq!(statuses["http"], NodeStatus::Failed);
        assert_eq!(statuses["catch"], NodeStatus::Succeeded);
        assert_eq!(statuses["agent"], NodeStatus::Succeeded);
    }

    #[tokio::test]
    async fn no_error_edge_fails_workflow() {
        let graph = WorkflowGraph {
            workflow_version_id: "v1".to_string(),
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
            input_schema: vec![],
        };
        let executor = FailingExecutor::new("http");
        let report = run_workflow("exec-5", &graph, "{}", &executor, false)
            .await
            .unwrap();
        assert_eq!(report.status, ExecutionStatus::Failed);
    }

    // ── Test executor helpers ──────────────────────────────────────────────

    #[derive(Default)]
    struct FanInAwareExecutor {
        fan_in_sources: Mutex<Vec<String>>,
    }

    impl NodeExecutor for FanInAwareExecutor {
        fn execute<'a>(
            &'a self,
            node: &'a Node,
            _ctx: &'a ExecutionContext,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = NodeExecutionResult> + Send + 'a>>
        {
            if node.node_type == NodeType::FanIn {
                if let Some(sources) = node
                    .config
                    .as_ref()
                    .and_then(|c| c.get("_sources"))
                    .and_then(|v| v.as_array())
                {
                    *self.fan_in_sources.lock().unwrap() = sources
                        .iter()
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

    struct ConditionExecutor {
        result: bool,
    }

    impl NodeExecutor for ConditionExecutor {
        fn execute<'a>(
            &'a self,
            node: &'a Node,
            _ctx: &'a ExecutionContext,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = NodeExecutionResult> + Send + 'a>>
        {
            let result = self.result;
            let node_id = node.id.clone();
            let is_condition = node.node_type == NodeType::Condition;
            Box::pin(async move {
                if is_condition {
                    NodeExecutionResult::succeeded(format!("{{\"result\":{result}}}"))
                } else {
                    NodeExecutionResult::succeeded(format!("{{\"node_id\":\"{node_id}\"}}"))
                }
            })
        }
    }

    #[derive(Default)]
    struct EchoExecutor {
        executed_nodes: Mutex<Vec<String>>,
        context_output_counts: Mutex<Vec<(String, usize)>>,
    }

    impl NodeExecutor for EchoExecutor {
        fn execute<'a>(
            &'a self,
            node: &'a Node,
            context: &'a ExecutionContext,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = NodeExecutionResult> + Send + 'a>>
        {
            self.executed_nodes.lock().unwrap().push(node.id.clone());
            self.context_output_counts
                .lock()
                .unwrap()
                .push((node.id.clone(), context.node_outputs.len()));
            let node_id = node.id.clone();
            Box::pin(async move {
                NodeExecutionResult::succeeded(format!("{{\"node_id\":\"{node_id}\"}}"))
            })
        }
    }

    struct FailingExecutor {
        fail_node_id: String,
        executed_nodes: Mutex<Vec<String>>,
    }

    impl FailingExecutor {
        fn new(fail: &str) -> Self {
            Self {
                fail_node_id: fail.to_string(),
                executed_nodes: Mutex::new(Vec::new()),
            }
        }
    }

    impl NodeExecutor for FailingExecutor {
        fn execute<'a>(
            &'a self,
            node: &'a Node,
            _ctx: &'a ExecutionContext,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = NodeExecutionResult> + Send + 'a>>
        {
            self.executed_nodes.lock().unwrap().push(node.id.clone());
            let should_fail = node.id == self.fail_node_id;
            let node_id = node.id.clone();
            Box::pin(async move {
                if should_fail {
                    NodeExecutionResult::failed("node failed")
                } else {
                    NodeExecutionResult::succeeded(format!("{{\"node_id\":\"{node_id}\"}}"))
                }
            })
        }
    }
}
