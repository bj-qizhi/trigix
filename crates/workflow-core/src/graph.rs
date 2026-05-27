use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowGraph {
    pub workflow_version_id: String,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: NodeType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edge {
    pub source: String,
    pub target: String,
    /// For edges leaving a condition node: "true" or "false".
    /// Omitted for unconditional edges.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition_label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    Trigger,
    Http,
    Agent,
    Condition,
    Approval,
    Map,
    Filter,
    Aggregate,
    Sort,
    Transform,
    Delay,
    SubWorkflow,
    Assert,
    Catch,
    FanOut,
    FanIn,
    Code,
}

impl WorkflowGraph {
    pub fn validate(&self) -> Result<(), GraphError> {
        if self.workflow_version_id.is_empty() {
            return Err(GraphError::MissingWorkflowVersion);
        }
        if self.nodes.is_empty() {
            return Err(GraphError::EmptyGraph);
        }

        let mut node_ids = HashSet::new();
        for node in &self.nodes {
            if node.id.is_empty() {
                return Err(GraphError::EmptyNodeId);
            }
            if !node_ids.insert(node.id.as_str()) {
                return Err(GraphError::DuplicateNode(node.id.clone()));
            }
        }

        for edge in &self.edges {
            if !node_ids.contains(edge.source.as_str()) {
                return Err(GraphError::UnknownEdgeSource(edge.source.clone()));
            }
            if !node_ids.contains(edge.target.as_str()) {
                return Err(GraphError::UnknownEdgeTarget(edge.target.clone()));
            }
        }

        self.topological_order().map(|_| ())
    }

    pub fn topological_order(&self) -> Result<Vec<String>, GraphError> {
        let mut indegree: HashMap<&str, usize> = self
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), 0usize))
            .collect();
        let mut outgoing: HashMap<&str, Vec<&str>> = HashMap::new();

        for edge in &self.edges {
            let Some(target_degree) = indegree.get_mut(edge.target.as_str()) else {
                return Err(GraphError::UnknownEdgeTarget(edge.target.clone()));
            };
            *target_degree += 1;
            outgoing
                .entry(edge.source.as_str())
                .or_default()
                .push(edge.target.as_str());
        }

        let mut ready: Vec<&str> = indegree
            .iter()
            .filter_map(|(node_id, degree)| (*degree == 0).then_some(*node_id))
            .collect();
        ready.sort_unstable();

        let mut ordered = Vec::with_capacity(self.nodes.len());
        while let Some(node_id) = ready.pop() {
            ordered.push(node_id.to_string());

            for target in outgoing.get(node_id).into_iter().flatten() {
                let degree = indegree
                    .get_mut(target)
                    .ok_or_else(|| GraphError::UnknownEdgeTarget((*target).to_string()))?;
                *degree -= 1;
                if *degree == 0 {
                    ready.push(target);
                    ready.sort_unstable();
                }
            }
        }

        if ordered.len() != self.nodes.len() {
            return Err(GraphError::CycleDetected);
        }

        Ok(ordered)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphError {
    MissingWorkflowVersion,
    EmptyGraph,
    EmptyNodeId,
    DuplicateNode(String),
    UnknownEdgeSource(String),
    UnknownEdgeTarget(String),
    CycleDetected,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_single_node_graph() {
        let graph = WorkflowGraph {
            workflow_version_id: "version-1".to_string(),
            nodes: vec![Node {
                id: "trigger".to_string(),
                node_type: NodeType::Trigger,
                config: None,
            }],
            edges: vec![],
        };

        assert_eq!(graph.validate(), Ok(()));
        assert_eq!(graph.topological_order(), Ok(vec!["trigger".to_string()]));
    }

    #[test]
    fn rejects_empty_graph() {
        let graph = WorkflowGraph {
            workflow_version_id: "version-1".to_string(),
            nodes: vec![],
            edges: vec![],
        };

        assert_eq!(graph.validate(), Err(GraphError::EmptyGraph));
    }

    #[test]
    fn rejects_unknown_edge_target() {
        let graph = WorkflowGraph {
            workflow_version_id: "version-1".to_string(),
            nodes: vec![Node {
                id: "trigger".to_string(),
                node_type: NodeType::Trigger,
                config: None,
            }],
            edges: vec![Edge {
                source: "trigger".to_string(),
                target: "missing".to_string(),
                condition_label: None,
            }],
        };

        assert_eq!(
            graph.validate(),
            Err(GraphError::UnknownEdgeTarget("missing".to_string()))
        );
    }

    #[test]
    fn rejects_cycles() {
        let graph = WorkflowGraph {
            workflow_version_id: "version-1".to_string(),
            nodes: vec![
                Node {
                    id: "a".to_string(),
                    node_type: NodeType::Http,
                    config: None,
                },
                Node {
                    id: "b".to_string(),
                    node_type: NodeType::Agent,
                    config: None,
                },
            ],
            edges: vec![
                Edge {
                    source: "a".to_string(),
                    target: "b".to_string(),
                    condition_label: None,
                },
                Edge {
                    source: "b".to_string(),
                    target: "a".to_string(),
                    condition_label: None,
                },
            ],
        };

        assert_eq!(graph.validate(), Err(GraphError::CycleDetected));
    }
}
