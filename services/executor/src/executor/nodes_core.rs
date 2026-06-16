// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! Core workflow primitives that fill gaps versus general-purpose automation
//! tools: HTML extraction (CSS selectors) and RSS/Atom feed reading.

use super::*;
use crate::runtime::{ExecutionContext, NodeExecutionResult};
use workflow_core::Node;

// ── HTML extract (CSS selectors) ──────────────────────────────────────────────
pub(super) async fn execute_html_extract(
    node: &Node,
    context: &ExecutionContext,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let html = match cfg.get("html").and_then(|v| v.as_str()) {
        Some(h) if !h.is_empty() => h.to_string(),
        _ => return NodeExecutionResult::failed("HTML extract requires 'html'"),
    };
    let selector_str = match cfg.get("selector").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return NodeExecutionResult::failed("HTML extract requires 'selector' (CSS)"),
    };
    let mode = cfg
        .get("extract")
        .and_then(|v| v.as_str())
        .unwrap_or("text")
        .to_string();
    let attr = cfg
        .get("attr")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let document = scraper::Html::parse_document(&html);
    let selector = match scraper::Selector::parse(&selector_str) {
        Ok(s) => s,
        Err(e) => return NodeExecutionResult::failed(format!("HTML extract bad selector: {e:?}")),
    };

    let mut matches: Vec<serde_json::Value> = Vec::new();
    for el in document.select(&selector) {
        let value = match mode.as_str() {
            "text" => el.text().collect::<Vec<_>>().join("").trim().to_string(),
            "html" => el.inner_html(),
            "attr" => {
                if attr.is_empty() {
                    return NodeExecutionResult::failed(
                        "HTML extract mode 'attr' requires 'attr' (attribute name)",
                    );
                }
                el.value().attr(&attr).unwrap_or("").to_string()
            }
            other => {
                return NodeExecutionResult::failed(format!(
                    "HTML extract unknown extract mode '{other}' (text/html/attr)"
                ))
            }
        };
        matches.push(serde_json::Value::String(value));
    }

    let first = matches.first().cloned().unwrap_or(serde_json::Value::Null);
    NodeExecutionResult::succeeded(
        serde_json::json!({ "matches": matches, "count": matches.len(), "first": first })
            .to_string(),
    )
}

// ── RSS / Atom feed read ──────────────────────────────────────────────────────
pub(super) async fn execute_rss(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let url = match cfg.get("url").and_then(|v| v.as_str()) {
        Some(u) if !u.is_empty() => u.to_string(),
        _ => return NodeExecutionResult::failed("RSS requires 'url'"),
    };
    let limit = cfg.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;

    let bytes = match http_client.get(&url).send().await {
        Ok(resp) => {
            if !resp.status().is_success() {
                let code = resp.status().as_u16();
                return NodeExecutionResult::failed(format!("RSS fetch HTTP {code}"));
            }
            match resp.bytes().await {
                Ok(b) => b,
                Err(e) => return NodeExecutionResult::failed(format!("RSS read error: {e}")),
            }
        }
        Err(e) => return NodeExecutionResult::failed(format!("RSS request error: {e}")),
    };

    // feed-rs parses RSS 2.0 / RSS 1.0 / Atom / JSON Feed uniformly.
    let feed = match feed_rs::parser::parse(bytes.as_ref()) {
        Ok(f) => f,
        Err(e) => return NodeExecutionResult::failed(format!("RSS parse error: {e}")),
    };
    let items: Vec<serde_json::Value> = feed
        .entries
        .iter()
        .take(limit)
        .map(|e| {
            let title = e
                .title
                .as_ref()
                .map(|t| t.content.clone())
                .unwrap_or_default();
            let link = e.links.first().map(|l| l.href.clone()).unwrap_or_default();
            let summary = e
                .summary
                .as_ref()
                .map(|s| s.content.clone())
                .unwrap_or_default();
            let published = e
                .published
                .or(e.updated)
                .map(|d| d.to_rfc3339())
                .unwrap_or_default();
            serde_json::json!({
                "id": e.id,
                "title": title,
                "link": link,
                "summary": summary,
                "published": published,
            })
        })
        .collect();

    let feed_title = feed
        .title
        .as_ref()
        .map(|t| t.content.clone())
        .unwrap_or_default();
    NodeExecutionResult::succeeded(
        serde_json::json!({ "feed_title": feed_title, "items": items, "count": items.len() })
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
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

    #[tokio::test]
    async fn html_extract_text_and_attr() {
        let html = r#"<ul><li class="x"><a href="/a">One</a></li><li class="x"><a href="/b">Two</a></li></ul>"#;
        let n = Node {
            id: "h1".into(),
            node_type: NodeType::HtmlExtract,
            config: Some(serde_json::json!({"html":html,"selector":"li.x a","extract":"text"})),
        };
        let out: serde_json::Value = serde_json::from_str(
            execute_html_extract(&n, &ctx())
                .await
                .output_json
                .as_deref()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(out["count"], 2);
        assert_eq!(out["matches"][0], "One");
        assert_eq!(out["first"], "One");

        let n2 = Node {
            id: "h2".into(),
            node_type: NodeType::HtmlExtract,
            config: Some(
                serde_json::json!({"html":html,"selector":"li.x a","extract":"attr","attr":"href"}),
            ),
        };
        let out2: serde_json::Value = serde_json::from_str(
            execute_html_extract(&n2, &ctx())
                .await
                .output_json
                .as_deref()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(out2["matches"][1], "/b");
    }

    #[tokio::test]
    async fn html_extract_requires_selector() {
        let n = Node {
            id: "h3".into(),
            node_type: NodeType::HtmlExtract,
            config: Some(serde_json::json!({"html":"<p>x</p>"})),
        };
        let r = execute_html_extract(&n, &ctx()).await;
        assert!(r.error.as_deref().unwrap_or("").contains("selector"));
    }

    #[tokio::test]
    async fn rss_requires_url() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "r1".into(),
            node_type: NodeType::Rss,
            config: Some(serde_json::json!({"limit":5})),
        };
        let r = execute_rss(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("url"));
    }
}
