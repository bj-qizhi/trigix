// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// Contact: managecode@gmail.com

use crate::runtime::ExecutionContext;
use workflow_core::Node;

// ── Template resolution ───────────────────────────────────────────────────────
//
// Syntax: {{expr}} where expr is one of:
//   input            → the raw input_json string
//   input.a.b        → field a.b inside input_json (dot-path)
//   node_id          → the raw output_json of that node
//   node_id.a.b      → field a.b inside that node's output_json

pub(crate) fn resolve_template(template: &str, context: &ExecutionContext) -> String {
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

pub(crate) fn resolve_expr(expr: &str, context: &ExecutionContext) -> String {
    let (root, path) = match expr.find('.') {
        Some(i) => (&expr[..i], Some(&expr[i + 1..])),
        None => (expr, None),
    };
    // ctx.* variables expose execution metadata.
    if root == "ctx" {
        return match path {
            Some("execution_id")      => context.execution_id.clone(),
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
            let val: serde_json::Value = serde_json::from_str(raw).unwrap_or(serde_json::Value::Null);
            json_path(&val, path)
                .map(json_to_string)
                .unwrap_or_default()
        }
    }
}

pub(crate) fn json_path<'a>(value: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
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

pub(crate) fn json_to_string(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

pub(crate) fn resolve_config_strings(config: &serde_json::Value, context: &ExecutionContext) -> serde_json::Value {
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

/// Convert f64 to a serde_json Number (integer if whole, float otherwise).
pub(crate) fn json_number(v: f64) -> serde_json::Value {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        serde_json::Value::Number((v as i64).into())
    } else {
        serde_json::Number::from_f64(v)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null)
    }
}

pub(crate) fn is_truthy(s: &str) -> bool {
    !matches!(s, "" | "false" | "null" | "0" | "[]" | "{}")
}

pub(crate) fn json_type_name(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::Null    => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_)  => "array",
        serde_json::Value::Object(_) => "object",
    }
}
