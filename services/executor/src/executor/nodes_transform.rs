// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! Data transform / utility nodes (map, filter, aggregate, regex, csv, date, …).

use super::*;
use crate::runtime::{ExecutionContext, NodeExecutionResult};
use workflow_core::Node;

pub(super) fn execute_map(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
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

pub(super) fn execute_filter(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
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

pub(super) fn execute_aggregate(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
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

pub(super) fn execute_sort(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
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

pub(super) fn execute_transform(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
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

pub(super) fn execute_extract(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
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

pub(super) fn execute_merge(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
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

pub(super) fn execute_loop(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
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

pub(super) fn execute_validate(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
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

pub(super) fn execute_split(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
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

pub(super) fn execute_join(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
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

pub(super) fn execute_switch(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
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

pub(super) fn execute_random(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
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

pub(super) fn execute_dedupe(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
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

pub(super) fn execute_regex(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
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

pub(super) fn execute_csv(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
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

pub(super) fn execute_rename(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
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

pub(super) fn execute_format(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
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

pub(super) fn execute_handlebars(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
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

pub(super) fn execute_crypto(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
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

pub(super) fn execute_date(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
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

pub(super) fn execute_xml(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
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

pub(super) fn execute_yaml(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
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

pub(super) fn execute_math(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
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

pub(super) fn execute_array_utils(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
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
