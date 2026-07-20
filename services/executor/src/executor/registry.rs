use workflow_core::NodeType;

/// Runtime policy resolved by the Node registry before handler dispatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NodeRuntimeKind {
    Local,
    External,
    Approval,
    Wait,
}

/// Single metadata boundary for Node execution policy. Handler implementations
/// remain split by domain modules; retry, dry-run and suspension decisions no
/// longer duplicate ad-hoc Node lists across the runtime.
pub(super) fn runtime_kind(node_type: &NodeType) -> NodeRuntimeKind {
    match node_type {
        NodeType::Approval => NodeRuntimeKind::Approval,
        NodeType::Wait => NodeRuntimeKind::Wait,
        NodeType::Trigger
        | NodeType::Condition
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
        | NodeType::Hash
        | NodeType::Jwt
        | NodeType::TextSplitter
        | NodeType::HtmlExtract
        | NodeType::Zip
        | NodeType::Image
        | NodeType::PdfExtract
        | NodeType::Ocr => NodeRuntimeKind::Local,
        _ => NodeRuntimeKind::External,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_classifies_control_and_side_effect_nodes() {
        assert_eq!(runtime_kind(&NodeType::Approval), NodeRuntimeKind::Approval);
        assert_eq!(runtime_kind(&NodeType::Wait), NodeRuntimeKind::Wait);
        assert_eq!(runtime_kind(&NodeType::Transform), NodeRuntimeKind::Local);
        assert_eq!(runtime_kind(&NodeType::Slack), NodeRuntimeKind::External);
    }
}
