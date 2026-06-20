// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com
//! Commerce / payments / e-sign integration nodes.

use super::*;
use crate::runtime::{ExecutionContext, NodeExecutionResult};
use workflow_core::Node;

pub(super) async fn execute_braintree(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);

    let merchant_id = match cfg.get("merchant_id").and_then(|v| v.as_str()) {
        Some(m) if !m.is_empty() => m.to_string(),
        _ => return NodeExecutionResult::failed("Braintree requires 'merchant_id'"),
    };
    let public_key = match cfg.get("public_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Braintree requires 'public_key'"),
    };
    let private_key = match cfg.get("private_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Braintree requires 'private_key'"),
    };
    let environment = cfg
        .get("environment")
        .and_then(|v| v.as_str())
        .unwrap_or("sandbox")
        .to_string();
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("Braintree requires 'endpoint'"),
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();

    use base64::Engine as _;
    let credentials = format!("{public_key}:{private_key}");
    let encoded = base64::engine::general_purpose::STANDARD.encode(credentials.as_bytes());

    let base_url = if environment == "production" {
        format!("https://api.braintreegateway.com/merchants/{merchant_id}")
    } else {
        format!("https://api.sandbox.braintreegateway.com/merchants/{merchant_id}")
    };
    let ep = if endpoint.starts_with('/') {
        endpoint.clone()
    } else {
        format!("/{endpoint}")
    };
    let url = format!("{base_url}{ep}");

    let mut req = http_client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("Authorization", format!("Basic {encoded}"))
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .header("Braintree-Version", "2019-01-01");

    if matches!(method.as_str(), "POST" | "PUT") {
        if let Some(body) = cfg.get("body") {
            req = req.json(body);
        }
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
            NodeExecutionResult::succeeded(
                serde_json::json!({ "status": status, "body": body }).to_string(),
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Braintree request error: {e}")),
    }
}

pub(super) async fn execute_paypal(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);

    let client_id = match cfg.get("client_id").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("PayPal requires 'client_id'"),
    };
    let client_secret = match cfg.get("client_secret").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return NodeExecutionResult::failed("PayPal requires 'client_secret'"),
    };
    let access_token = match cfg.get("access_token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => {
            // Obtain token via client_credentials grant
            let environment = cfg
                .get("environment")
                .and_then(|v| v.as_str())
                .unwrap_or("sandbox");
            let token_url = if environment == "live" {
                "https://api-m.paypal.com/v1/oauth2/token"
            } else {
                "https://api-m.sandbox.paypal.com/v1/oauth2/token"
            };
            use base64::Engine as _;
            let encoded = base64::engine::general_purpose::STANDARD
                .encode(format!("{client_id}:{client_secret}").as_bytes());
            match http_client
                .post(token_url)
                .header("Authorization", format!("Basic {encoded}"))
                .header("Content-Type", "application/x-www-form-urlencoded")
                .body("grant_type=client_credentials")
                .send()
                .await
            {
                Ok(resp) => {
                    let json: serde_json::Value = resp.json().await.unwrap_or_default();
                    match json.get("access_token").and_then(|v| v.as_str()) {
                        Some(t) => t.to_string(),
                        None => return NodeExecutionResult::failed("PayPal token exchange failed"),
                    }
                }
                Err(e) => {
                    return NodeExecutionResult::failed(format!("PayPal token exchange error: {e}"))
                }
            }
        }
    };

    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("PayPal requires 'endpoint'"),
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();
    let environment = cfg
        .get("environment")
        .and_then(|v| v.as_str())
        .unwrap_or("sandbox");
    let base = if environment == "live" {
        "https://api-m.paypal.com"
    } else {
        "https://api-m.sandbox.paypal.com"
    };
    let ep = if endpoint.starts_with('/') {
        endpoint.clone()
    } else {
        format!("/{endpoint}")
    };
    let url = format!("{base}{ep}");

    let mut req = http_client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("Authorization", format!("Bearer {access_token}"))
        .header("Content-Type", "application/json");

    if matches!(method.as_str(), "POST" | "PUT" | "PATCH") {
        if let Some(body) = cfg.get("body") {
            req = req.json(body);
        }
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
            NodeExecutionResult::succeeded(
                serde_json::json!({ "status": status, "body": body }).to_string(),
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("PayPal request error: {e}")),
    }
}

// ── Slice 295: Razorpay ────────────────────────────────────────────────────────

pub(super) async fn execute_razorpay(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);

    let key_id = match cfg.get("key_id").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Razorpay requires 'key_id'"),
    };
    let key_secret = match cfg.get("key_secret").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return NodeExecutionResult::failed("Razorpay requires 'key_secret'"),
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => return NodeExecutionResult::failed("Razorpay requires 'endpoint'"),
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();
    let ep = if endpoint.starts_with('/') {
        endpoint.clone()
    } else {
        format!("/{endpoint}")
    };
    let url = format!("https://api.razorpay.com/v1{ep}");

    use base64::Engine as _;
    let encoded = base64::engine::general_purpose::STANDARD
        .encode(format!("{key_id}:{key_secret}").as_bytes());

    let mut req = http_client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("Authorization", format!("Basic {encoded}"))
        .header("Content-Type", "application/json");

    if matches!(method.as_str(), "POST" | "PUT" | "PATCH") {
        if let Some(body) = cfg.get("body") {
            req = req.json(body);
        }
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
            NodeExecutionResult::succeeded(
                serde_json::json!({ "status": status, "body": body }).to_string(),
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Razorpay request error: {e}")),
    }
}

// ── Slice 296: Firebase ────────────────────────────────────────────────────────

// ── Slice 317: WooCommerce ─────────────────────────────────────────────────────
pub(super) async fn execute_woocommerce(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let consumer_key = match cfg.get("consumer_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("WooCommerce requires 'consumer_key'"),
    };
    let consumer_secret = match cfg.get("consumer_secret").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return NodeExecutionResult::failed("WooCommerce requires 'consumer_secret'"),
    };
    let site_url = match cfg.get("site_url").and_then(|v| v.as_str()) {
        Some(u) if !u.is_empty() => u.trim_end_matches('/').to_string(),
        _ => return NodeExecutionResult::failed("WooCommerce requires 'site_url'"),
    };
    let endpoint = cfg
        .get("endpoint")
        .and_then(|v| v.as_str())
        .unwrap_or("/wp-json/wc/v3/products")
        .to_string();
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();
    let url = format!("{}{}", site_url, endpoint);

    use base64::Engine as _;
    let encoded = base64::engine::general_purpose::STANDARD
        .encode(format!("{consumer_key}:{consumer_secret}").as_bytes());

    let mut req = match method.as_str() {
        "POST" => http_client.post(&url),
        "PUT" => http_client.put(&url),
        "DELETE" => http_client.delete(&url),
        "PATCH" => http_client.patch(&url),
        _ => http_client.get(&url),
    };
    req = req
        .header("Authorization", format!("Basic {encoded}"))
        .header("Content-Type", "application/json");
    if let Some(body) = cfg.get("body") {
        req = req.json(body);
    }
    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
            NodeExecutionResult::succeeded(
                serde_json::json!({ "status": status, "body": body }).to_string(),
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("WooCommerce error: {e}")),
    }
}

pub(super) async fn execute_docusign(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let access_token = match cfg.get("access_token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("DocuSign requires 'access_token'"),
    };
    let account_id = match cfg.get("account_id").and_then(|v| v.as_str()) {
        Some(a) if !a.is_empty() => a.to_string(),
        _ => return NodeExecutionResult::failed("DocuSign requires 'account_id'"),
    };
    let base_url = cfg
        .get("base_url")
        .and_then(|v| v.as_str())
        .unwrap_or("https://demo.docusign.net/restapi");
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("list_envelopes")
        .to_string();
    let auth = format!("Bearer {access_token}");

    match operation.as_str() {
        "list_envelopes" => {
            let from_date = cfg
                .get("from_date")
                .and_then(|v| v.as_str())
                .unwrap_or("2024-01-01");
            let url =
                format!("{base_url}/v2.1/accounts/{account_id}/envelopes?from_date={from_date}");
            match http_client
                .get(&url)
                .header("Authorization", &auth)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value =
                        resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string(),
                    )
                }
                Err(e) => {
                    NodeExecutionResult::failed(format!("DocuSign list_envelopes error: {e}"))
                }
            }
        }
        "get_envelope" => {
            let envelope_id = match cfg.get("envelope_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => {
                    return NodeExecutionResult::failed(
                        "DocuSign get_envelope requires 'envelope_id'",
                    )
                }
            };
            let url = format!("{base_url}/v2.1/accounts/{account_id}/envelopes/{envelope_id}");
            match http_client
                .get(&url)
                .header("Authorization", &auth)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value =
                        resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string(),
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("DocuSign get_envelope error: {e}")),
            }
        }
        "create_envelope" => {
            let body = cfg
                .get("body")
                .cloned()
                .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
            let url = format!("{base_url}/v2.1/accounts/{account_id}/envelopes");
            match http_client
                .post(&url)
                .header("Authorization", &auth)
                .json(&body)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value =
                        resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string(),
                    )
                }
                Err(e) => {
                    NodeExecutionResult::failed(format!("DocuSign create_envelope error: {e}"))
                }
            }
        }
        "void_envelope" => {
            let envelope_id = match cfg.get("envelope_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => {
                    return NodeExecutionResult::failed(
                        "DocuSign void_envelope requires 'envelope_id'",
                    )
                }
            };
            let reason = cfg
                .get("void_reason")
                .and_then(|v| v.as_str())
                .unwrap_or("Voided via workflow");
            let body = serde_json::json!({ "status": "voided", "voidedReason": reason });
            let url = format!("{base_url}/v2.1/accounts/{account_id}/envelopes/{envelope_id}");
            match http_client
                .put(&url)
                .header("Authorization", &auth)
                .json(&body)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value =
                        resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string(),
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("DocuSign void_envelope error: {e}")),
            }
        }
        other => NodeExecutionResult::failed(format!("DocuSign unknown operation '{other}'")),
    }
}

pub(super) async fn execute_xero(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let access_token = match cfg.get("access_token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Xero requires 'access_token'"),
    };
    let tenant_id = match cfg.get("tenant_id").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Xero requires 'tenant_id'"),
    };
    let endpoint = cfg
        .get("endpoint")
        .and_then(|v| v.as_str())
        .unwrap_or("/Contacts");
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();
    let url = format!("https://api.xero.com/api.xro/2.0{endpoint}");
    let auth = format!("Bearer {access_token}");

    let mut req = match method.as_str() {
        "POST" => http_client.post(&url),
        "PUT" => http_client.put(&url),
        "DELETE" => http_client.delete(&url),
        _ => http_client.get(&url),
    };
    req = req
        .header("Authorization", &auth)
        .header("Xero-Tenant-Id", &tenant_id)
        .header("Accept", "application/json");
    if let Some(body) = cfg.get("body") {
        if !matches!(method.as_str(), "GET" | "DELETE") {
            req = req.json(body);
        }
    }
    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
            NodeExecutionResult::succeeded(
                serde_json::json!({ "status": status, "body": body }).to_string(),
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Xero error: {e}")),
    }
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
    async fn braintree_fails_without_merchant_id() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "bt1".into(),
            node_type: NodeType::Braintree,
            config: Some(
                serde_json::json!({ "public_key": "pk", "private_key": "prk", "endpoint": "/transactions" }),
            ),
        };
        let r = execute_braintree(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("merchant_id"));
    }

    #[tokio::test]
    async fn braintree_fails_without_public_key() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "bt2".into(),
            node_type: NodeType::Braintree,
            config: Some(
                serde_json::json!({ "merchant_id": "mid", "private_key": "prk", "endpoint": "/transactions" }),
            ),
        };
        let r = execute_braintree(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("public_key"));
    }

    #[tokio::test]
    async fn braintree_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "bt3".into(),
            node_type: NodeType::Braintree,
            config: Some(
                serde_json::json!({ "merchant_id": "mid", "public_key": "pk", "private_key": "prk" }),
            ),
        };
        let r = execute_braintree(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    #[tokio::test]
    async fn paypal_fails_without_client_id() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "pp1".into(),
            node_type: NodeType::Paypal,
            config: Some(
                serde_json::json!({ "client_secret": "sec", "endpoint": "/v2/checkout/orders" }),
            ),
        };
        let r = execute_paypal(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("client_id"));
    }

    #[tokio::test]
    async fn paypal_fails_without_client_secret() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "pp2".into(),
            node_type: NodeType::Paypal,
            config: Some(
                serde_json::json!({ "client_id": "cid", "endpoint": "/v2/checkout/orders" }),
            ),
        };
        let r = execute_paypal(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("client_secret"));
    }

    #[tokio::test]
    async fn paypal_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "pp3".into(),
            node_type: NodeType::Paypal,
            config: Some(
                serde_json::json!({ "client_id": "cid", "client_secret": "sec", "access_token": "tok" }),
            ),
        };
        let r = execute_paypal(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    // ── Razorpay ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn razorpay_fails_without_key_id() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "rp1".into(),
            node_type: NodeType::Razorpay,
            config: Some(serde_json::json!({ "key_secret": "sec", "endpoint": "/orders" })),
        };
        let r = execute_razorpay(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("key_id"));
    }

    #[tokio::test]
    async fn razorpay_fails_without_key_secret() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "rp2".into(),
            node_type: NodeType::Razorpay,
            config: Some(serde_json::json!({ "key_id": "rzp_test_abc", "endpoint": "/orders" })),
        };
        let r = execute_razorpay(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("key_secret"));
    }

    #[tokio::test]
    async fn razorpay_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "rp3".into(),
            node_type: NodeType::Razorpay,
            config: Some(serde_json::json!({ "key_id": "rzp_test_abc", "key_secret": "sec" })),
        };
        let r = execute_razorpay(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    // ── Firebase ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn shopify_fails_without_shop() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "s1".into(),
            node_type: NodeType::Shopify,
            config: Some(serde_json::json!({"token":"shpat_test"})),
        };
        let r = execute_shopify(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("shop"));
    }

    #[tokio::test]
    async fn shopify_fails_without_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "s2".into(),
            node_type: NodeType::Shopify,
            config: Some(serde_json::json!({"shop":"test.myshopify.com"})),
        };
        let r = execute_shopify(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("token"));
    }

    // ── Discord ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn woocommerce_fails_without_consumer_key() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "w1".into(),
            node_type: NodeType::Woocommerce,
            config: Some(
                serde_json::json!({"consumer_secret":"sec","site_url":"https://shop.example.com"}),
            ),
        };
        let r = execute_woocommerce(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("consumer_key"));
    }

    #[tokio::test]
    async fn woocommerce_fails_without_consumer_secret() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "w2".into(),
            node_type: NodeType::Woocommerce,
            config: Some(
                serde_json::json!({"consumer_key":"ck_test","site_url":"https://shop.example.com"}),
            ),
        };
        let r = execute_woocommerce(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("consumer_secret"));
    }

    #[tokio::test]
    async fn woocommerce_fails_without_site_url() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "w3".into(),
            node_type: NodeType::Woocommerce,
            config: Some(serde_json::json!({"consumer_key":"ck_test","consumer_secret":"cs_test"})),
        };
        let r = execute_woocommerce(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("site_url"));
    }

    #[tokio::test]
    async fn docusign_fails_without_access_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "d1".into(),
            node_type: NodeType::Docusign,
            config: Some(serde_json::json!({"account_id":"abc"})),
        };
        let r = execute_docusign(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("access_token"));
    }

    #[tokio::test]
    async fn docusign_fails_without_account_id() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "d2".into(),
            node_type: NodeType::Docusign,
            config: Some(serde_json::json!({"access_token":"tok"})),
        };
        let r = execute_docusign(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("account_id"));
    }

    #[tokio::test]
    async fn docusign_get_envelope_fails_without_id() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "d3".into(),
            node_type: NodeType::Docusign,
            config: Some(
                serde_json::json!({"access_token":"tok","account_id":"acc","operation":"get_envelope"}),
            ),
        };
        let r = execute_docusign(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("envelope_id"));
    }

    // ── Xero ──────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn xero_fails_without_access_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "x1".into(),
            node_type: NodeType::Xero,
            config: Some(serde_json::json!({"tenant_id":"tid"})),
        };
        let r = execute_xero(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("access_token"));
    }

    #[tokio::test]
    async fn xero_fails_without_tenant_id() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "x2".into(),
            node_type: NodeType::Xero,
            config: Some(serde_json::json!({"access_token":"tok"})),
        };
        let r = execute_xero(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("tenant_id"));
    }

    // ── Calendly ──────────────────────────────────────────────────────────────
}
