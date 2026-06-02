// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use workflow_core::{GraphError, WorkflowGraph};

pub fn schedule(graph: &WorkflowGraph) -> Result<Vec<String>, GraphError> {
    graph.validate()?;
    graph.topological_order()
}

#[cfg(test)]
mod tests {
    use super::*;
    use workflow_core::{Edge, Node, NodeType};

    #[test]
    fn schedules_nodes_in_dependency_order() {
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

        assert_eq!(
            schedule(&graph),
            Ok(vec!["trigger".to_string(), "http".to_string()])
        );
    }
}
