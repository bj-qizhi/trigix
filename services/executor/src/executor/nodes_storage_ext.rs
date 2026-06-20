// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com
//! Object storage / file integration nodes (+ AWS SigV4 signing).

use super::*;
use crate::runtime::{ExecutionContext, NodeExecutionResult};
use workflow_core::Node;

pub(super) async fn execute_dropbox(
    node: &Node,
    context: &ExecutionContext,
    client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw_cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Dropbox node requires config"),
    };
    let cfg = resolve_config_strings(raw_cfg, context);

    let token = match cfg.get("token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => {
            return NodeExecutionResult::failed(
                "Dropbox node missing 'token' (OAuth2 access token)",
            )
        }
    };
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("list_folder");

    // Different Dropbox API endpoints per operation
    let (host, path, arg): (&str, &str, serde_json::Value) = match operation {
        "list_folder" => {
            let folder = cfg.get("path").and_then(|v| v.as_str()).unwrap_or("");
            (
                "api.dropboxapi.com",
                "/2/files/list_folder",
                serde_json::json!({ "path": folder, "recursive": false }),
            )
        }
        "get_metadata" => {
            let p = match cfg.get("path").and_then(|v| v.as_str()) {
                Some(p) if !p.is_empty() => p.to_string(),
                _ => return NodeExecutionResult::failed("Dropbox get_metadata requires 'path'"),
            };
            (
                "api.dropboxapi.com",
                "/2/files/get_metadata",
                serde_json::json!({ "path": p }),
            )
        }
        "delete" => {
            let p = match cfg.get("path").and_then(|v| v.as_str()) {
                Some(p) if !p.is_empty() => p.to_string(),
                _ => return NodeExecutionResult::failed("Dropbox delete requires 'path'"),
            };
            (
                "api.dropboxapi.com",
                "/2/files/delete_v2",
                serde_json::json!({ "path": p }),
            )
        }
        "create_folder" => {
            let p = match cfg.get("path").and_then(|v| v.as_str()) {
                Some(p) if !p.is_empty() => p.to_string(),
                _ => return NodeExecutionResult::failed("Dropbox create_folder requires 'path'"),
            };
            (
                "api.dropboxapi.com",
                "/2/files/create_folder_v2",
                serde_json::json!({ "path": p, "autorename": false }),
            )
        }
        "search" => {
            let q = match cfg.get("query").and_then(|v| v.as_str()) {
                Some(q) if !q.is_empty() => q.to_string(),
                _ => return NodeExecutionResult::failed("Dropbox search requires 'query'"),
            };
            (
                "api.dropboxapi.com",
                "/2/files/search_v2",
                serde_json::json!({ "query": q }),
            )
        }
        _ => return NodeExecutionResult::failed(format!("Unknown Dropbox operation: {operation}")),
    };

    let url = format!("https://{host}{path}");

    match client
        .post(&url)
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .json(&arg)
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
            NodeExecutionResult::succeeded(
                serde_json::json!({ "status": status, "body": body, "operation": operation })
                    .to_string(),
            )
        }
        Err(e) => NodeExecutionResult::failed(format!("Dropbox request error: {e}")),
    }
}

// ── Slice 277: Cloudflare ─────────────────────────────────────────────────────

pub(super) async fn execute_box(
    node: &Node,
    context: &ExecutionContext,
    client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw_cfg = match node.config.as_ref() {
        Some(c) => c,
        None => return NodeExecutionResult::failed("Box node requires config"),
    };
    let cfg = resolve_config_strings(raw_cfg, context);

    let token = match cfg.get("token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Box node missing 'token' (OAuth2 access token)"),
    };
    let endpoint = match cfg.get("endpoint").and_then(|v| v.as_str()) {
        Some(e) if !e.is_empty() => e.to_string(),
        _ => {
            return NodeExecutionResult::failed(
                "Box node missing 'endpoint' (e.g. /folders/0/items)",
            )
        }
    };
    let method = cfg
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();

    let url = format!("https://api.box.com/2.0{endpoint}");

    let mut req = client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        )
        .header("Authorization", format!("Bearer {token}"))
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
        Err(e) => NodeExecutionResult::failed(format!("Box request error: {e}")),
    }
}

// ── Slice 279: Okta ───────────────────────────────────────────────────────────

// ── Slice 316: Google Drive ────────────────────────────────────────────────────
pub(super) async fn execute_googledrive(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let access_token = match cfg.get("access_token").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return NodeExecutionResult::failed("Google Drive requires 'access_token'"),
    };
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("list")
        .to_string();
    let auth = format!("Bearer {access_token}");

    match operation.as_str() {
        "list" => {
            let mut url = "https://www.googleapis.com/drive/v3/files?pageSize=100".to_string();
            if let Some(q) = cfg.get("query").and_then(|v| v.as_str()) {
                url.push_str(&format!("&q={}", urlencoding_simple(q)));
            }
            if let Some(fields) = cfg.get("fields").and_then(|v| v.as_str()) {
                url.push_str(&format!("&fields={}", urlencoding_simple(fields)));
            }
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
                Err(e) => NodeExecutionResult::failed(format!("Google Drive list error: {e}")),
            }
        }
        "get" => {
            let file_id = match cfg.get("file_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => return NodeExecutionResult::failed("Google Drive get requires 'file_id'"),
            };
            let url = format!("https://www.googleapis.com/drive/v3/files/{file_id}?fields=*");
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
                Err(e) => NodeExecutionResult::failed(format!("Google Drive get error: {e}")),
            }
        }
        "delete" => {
            let file_id = match cfg.get("file_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => return NodeExecutionResult::failed("Google Drive delete requires 'file_id'"),
            };
            let url = format!("https://www.googleapis.com/drive/v3/files/{file_id}");
            match http_client
                .delete(&url)
                .header("Authorization", &auth)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status }).to_string(),
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("Google Drive delete error: {e}")),
            }
        }
        "create_folder" => {
            let name = cfg
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("New Folder")
                .to_string();
            let parent_id = cfg.get("parent_id").and_then(|v| v.as_str());
            let mut metadata = serde_json::json!({
                "name": name,
                "mimeType": "application/vnd.google-apps.folder"
            });
            if let Some(pid) = parent_id {
                metadata["parents"] = serde_json::json!([pid]);
            }
            match http_client
                .post("https://www.googleapis.com/drive/v3/files")
                .header("Authorization", &auth)
                .header("Content-Type", "application/json")
                .json(&metadata)
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
                    NodeExecutionResult::failed(format!("Google Drive create_folder error: {e}"))
                }
            }
        }
        other => NodeExecutionResult::failed(format!("Google Drive unknown operation '{other}'")),
    }
}

// ── Slice 320: AWS S3 ──────────────────────────────────────────────────────────
pub(super) async fn execute_awss3(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let access_key_id = match cfg.get("access_key_id").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("AWS S3 requires 'access_key_id'"),
    };
    let secret_access_key = match cfg.get("secret_access_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("AWS S3 requires 'secret_access_key'"),
    };
    let bucket = match cfg.get("bucket").and_then(|v| v.as_str()) {
        Some(b) if !b.is_empty() => b.to_string(),
        _ => return NodeExecutionResult::failed("AWS S3 requires 'bucket'"),
    };
    let region = cfg
        .get("region")
        .and_then(|v| v.as_str())
        .unwrap_or("us-east-1")
        .to_string();
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("list")
        .to_string();

    let host = if region == "us-east-1" {
        format!("{}.s3.amazonaws.com", bucket)
    } else {
        format!("{}.s3.{}.amazonaws.com", bucket, region)
    };
    let base_url = format!("https://{}", host);

    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let timestamp = now.as_secs();
    let date_str = {
        let (y, m, d) = epoch_to_ymd(timestamp);
        format!("{:04}{:02}{:02}", y, m, d)
    };
    let datetime_str = {
        let h = (timestamp % 86400) / 3600;
        let min = (timestamp % 3600) / 60;
        let sec = timestamp % 60;
        format!("{}T{:02}{:02}{:02}Z", date_str, h, min, sec)
    };
    const EMPTY_HASH: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    match operation.as_str() {
        "list" => {
            let prefix = cfg
                .get("prefix")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let canonical_query = if prefix.is_empty() {
                "list-type=2".to_string()
            } else {
                format!("list-type=2&prefix={}", sigv4_uri_encode(&prefix))
            };
            let url = format!("{}/?{}", base_url, canonical_query);
            let auth = aws_sigv4_s3_auth(
                &access_key_id,
                &secret_access_key,
                &region,
                &date_str,
                &datetime_str,
                "GET",
                &host,
                "/",
                &canonical_query,
                EMPTY_HASH,
            );
            match http_client
                .get(&url)
                .header("Authorization", auth)
                .header("x-amz-date", &datetime_str)
                .header("x-amz-content-sha256", EMPTY_HASH)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body = resp.text().await.unwrap_or_default();
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string(),
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("S3 list error: {e}")),
            }
        }
        "get_object" => {
            let key = match cfg.get("key").and_then(|v| v.as_str()) {
                Some(k) if !k.is_empty() => k.to_string(),
                _ => return NodeExecutionResult::failed("S3 get_object requires 'key'"),
            };
            let key_path = format!("/{}", key.trim_start_matches('/'));
            let url = format!("{}{}", base_url, key_path);
            let auth = aws_sigv4_s3_auth(
                &access_key_id,
                &secret_access_key,
                &region,
                &date_str,
                &datetime_str,
                "GET",
                &host,
                &key_path,
                "",
                EMPTY_HASH,
            );
            match http_client
                .get(&url)
                .header("Authorization", auth)
                .header("x-amz-date", &datetime_str)
                .header("x-amz-content-sha256", EMPTY_HASH)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body = resp.text().await.unwrap_or_default();
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string(),
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("S3 get_object error: {e}")),
            }
        }
        "delete_object" => {
            let key = match cfg.get("key").and_then(|v| v.as_str()) {
                Some(k) if !k.is_empty() => k.to_string(),
                _ => return NodeExecutionResult::failed("S3 delete_object requires 'key'"),
            };
            let key_path = format!("/{}", key.trim_start_matches('/'));
            let url = format!("{}{}", base_url, key_path);
            let auth = aws_sigv4_s3_auth(
                &access_key_id,
                &secret_access_key,
                &region,
                &date_str,
                &datetime_str,
                "DELETE",
                &host,
                &key_path,
                "",
                EMPTY_HASH,
            );
            match http_client
                .delete(&url)
                .header("Authorization", auth)
                .header("x-amz-date", &datetime_str)
                .header("x-amz-content-sha256", EMPTY_HASH)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status }).to_string(),
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("S3 delete_object error: {e}")),
            }
        }
        "put_object" => {
            let key = match cfg.get("key").and_then(|v| v.as_str()) {
                Some(k) if !k.is_empty() => k.to_string(),
                _ => return NodeExecutionResult::failed("S3 put_object requires 'key'"),
            };
            let body_content = cfg
                .get("body")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let content_type = cfg
                .get("content_type")
                .and_then(|v| v.as_str())
                .unwrap_or("application/octet-stream")
                .to_string();
            let key_path = format!("/{}", key.trim_start_matches('/'));
            let url = format!("{}{}", base_url, key_path);
            let payload_hash = {
                use sha2::{Digest, Sha256};
                hex::encode(Sha256::digest(body_content.as_bytes()))
            };
            let auth = aws_sigv4_s3_auth(
                &access_key_id,
                &secret_access_key,
                &region,
                &date_str,
                &datetime_str,
                "PUT",
                &host,
                &key_path,
                "",
                &payload_hash,
            );
            match http_client
                .put(&url)
                .header("Authorization", auth)
                .header("x-amz-date", &datetime_str)
                .header("x-amz-content-sha256", &payload_hash)
                .header("Content-Type", &content_type)
                .body(body_content)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status }).to_string(),
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("S3 put_object error: {e}")),
            }
        }
        other => NodeExecutionResult::failed(format!("S3 unknown operation '{other}'")),
    }
}

fn sigv4_uri_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            b => format!("%{:02X}", b),
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn aws_sigv4_s3_auth(
    access_key_id: &str,
    secret_access_key: &str,
    region: &str,
    date_str: &str,
    datetime_str: &str,
    method: &str,
    host: &str,
    canonical_uri: &str,
    canonical_query: &str,
    payload_hash: &str,
) -> String {
    use hmac::{Hmac, Mac};
    use sha2::{Digest, Sha256};
    type HmacSha256 = Hmac<Sha256>;

    let canonical_headers = format!(
        "host:{}\nx-amz-content-sha256:{}\nx-amz-date:{}\n",
        host, payload_hash, datetime_str
    );
    let signed_headers = "host;x-amz-content-sha256;x-amz-date";

    let canonical_request = format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        method, canonical_uri, canonical_query, canonical_headers, signed_headers, payload_hash
    );

    let credential_scope = format!("{}/{}/s3/aws4_request", date_str, region);
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{}\n{}\n{}",
        datetime_str,
        credential_scope,
        hex::encode(Sha256::digest(canonical_request.as_bytes()))
    );

    let k_date = {
        let mut mac = HmacSha256::new_from_slice(format!("AWS4{}", secret_access_key).as_bytes())
            .expect("valid key");
        mac.update(date_str.as_bytes());
        mac.finalize().into_bytes()
    };
    let k_region = {
        let mut mac = HmacSha256::new_from_slice(&k_date).expect("valid key");
        mac.update(region.as_bytes());
        mac.finalize().into_bytes()
    };
    let k_service = {
        let mut mac = HmacSha256::new_from_slice(&k_region).expect("valid key");
        mac.update(b"s3");
        mac.finalize().into_bytes()
    };
    let k_signing = {
        let mut mac = HmacSha256::new_from_slice(&k_service).expect("valid key");
        mac.update(b"aws4_request");
        mac.finalize().into_bytes()
    };
    let signature = {
        let mut mac = HmacSha256::new_from_slice(&k_signing).expect("valid key");
        mac.update(string_to_sign.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    };

    format!(
        "AWS4-HMAC-SHA256 Credential={}/{},SignedHeaders={},Signature={}",
        access_key_id, credential_scope, signed_headers, signature
    )
}

fn epoch_to_ymd(secs: u64) -> (u32, u32, u32) {
    let days = secs / 86400;
    let mut y = 1970u32;
    let mut d = days as u32;
    loop {
        let dy = if (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400) {
            366
        } else {
            365
        };
        if d < dy {
            break;
        }
        d -= dy;
        y += 1;
    }
    let leap = (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400);
    let month_days = [
        31u32,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut m = 0u32;
    for &md in &month_days {
        if d < md {
            break;
        }
        d -= md;
        m += 1;
    }
    (y, m + 1, d + 1)
}

// ── Slice 325: Cloudinary ──────────────────────────────────────────────────────
pub(super) async fn execute_cloudinary(
    node: &Node,
    context: &ExecutionContext,
    http_client: &reqwest::Client,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let cloud_name = match cfg.get("cloud_name").and_then(|v| v.as_str()) {
        Some(n) if !n.is_empty() => n.to_string(),
        _ => return NodeExecutionResult::failed("Cloudinary requires 'cloud_name'"),
    };
    let api_key = match cfg.get("api_key").and_then(|v| v.as_str()) {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => return NodeExecutionResult::failed("Cloudinary requires 'api_key'"),
    };
    let api_secret = match cfg.get("api_secret").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return NodeExecutionResult::failed("Cloudinary requires 'api_secret'"),
    };
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("list")
        .to_string();

    use base64::Engine as _;
    let encoded = base64::engine::general_purpose::STANDARD
        .encode(format!("{api_key}:{api_secret}").as_bytes());
    let auth = format!("Basic {encoded}");

    match operation.as_str() {
        "list" => {
            let resource_type = cfg
                .get("resource_type")
                .and_then(|v| v.as_str())
                .unwrap_or("image");
            let url =
                format!("https://api.cloudinary.com/v1_1/{cloud_name}/resources/{resource_type}");
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
                Err(e) => NodeExecutionResult::failed(format!("Cloudinary list error: {e}")),
            }
        }
        "upload" => {
            let file = match cfg.get("file").and_then(|v| v.as_str()) {
                Some(f) if !f.is_empty() => f.to_string(),
                _ => {
                    return NodeExecutionResult::failed(
                        "Cloudinary upload requires 'file' (URL or base64 data URI)",
                    )
                }
            };
            let resource_type = cfg
                .get("resource_type")
                .and_then(|v| v.as_str())
                .unwrap_or("image");
            let url =
                format!("https://api.cloudinary.com/v1_1/{cloud_name}/{resource_type}/upload");
            let mut form_data = std::collections::HashMap::new();
            form_data.insert("file", file.clone());
            form_data.insert("api_key", api_key.clone());
            // Timestamp-based signature would be needed for authenticated uploads
            // Using unsigned upload preset if configured
            if let Some(preset) = cfg.get("upload_preset").and_then(|v| v.as_str()) {
                form_data.insert("upload_preset", preset.to_string());
            }
            if let Some(folder) = cfg.get("folder").and_then(|v| v.as_str()) {
                form_data.insert("folder", folder.to_string());
            }
            match http_client.post(&url).form(&form_data).send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let body: serde_json::Value =
                        resp.json().await.unwrap_or(serde_json::Value::Null);
                    NodeExecutionResult::succeeded(
                        serde_json::json!({ "status": status, "body": body }).to_string(),
                    )
                }
                Err(e) => NodeExecutionResult::failed(format!("Cloudinary upload error: {e}")),
            }
        }
        "destroy" => {
            let public_id = match cfg.get("public_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => return NodeExecutionResult::failed("Cloudinary destroy requires 'public_id'"),
            };
            let resource_type = cfg
                .get("resource_type")
                .and_then(|v| v.as_str())
                .unwrap_or("image");
            let url =
                format!("https://api.cloudinary.com/v1_1/{cloud_name}/{resource_type}/destroy");
            let body = serde_json::json!({ "public_id": public_id });
            match http_client
                .post(&url)
                .header("Authorization", &auth)
                .header("Content-Type", "application/json")
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
                Err(e) => NodeExecutionResult::failed(format!("Cloudinary destroy error: {e}")),
            }
        }
        "transform_url" => {
            let public_id = match cfg.get("public_id").and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => {
                    return NodeExecutionResult::failed(
                        "Cloudinary transform_url requires 'public_id'",
                    )
                }
            };
            let transformation = cfg
                .get("transformation")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let resource_type = cfg
                .get("resource_type")
                .and_then(|v| v.as_str())
                .unwrap_or("image");
            let format = cfg.get("format").and_then(|v| v.as_str()).unwrap_or("jpg");
            let url = if transformation.is_empty() {
                format!("https://res.cloudinary.com/{cloud_name}/{resource_type}/upload/{public_id}.{format}")
            } else {
                format!("https://res.cloudinary.com/{cloud_name}/{resource_type}/upload/{transformation}/{public_id}.{format}")
            };
            NodeExecutionResult::succeeded(
                serde_json::json!({ "url": url, "public_id": public_id }).to_string(),
            )
        }
        other => NodeExecutionResult::failed(format!("Cloudinary unknown operation '{other}'")),
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
    async fn dropbox_fails_without_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "db1".into(),
            node_type: NodeType::Dropbox,
            config: Some(serde_json::json!({ "operation": "list_folder" })),
        };
        let r = execute_dropbox(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("token"));
    }

    #[tokio::test]
    async fn dropbox_rejects_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "db2".into(),
            node_type: NodeType::Dropbox,
            config: Some(serde_json::json!({ "token": "sl.abc", "operation": "unknown_op" })),
        };
        let r = execute_dropbox(&n, &ctx(), &c).await;
        assert!(r.error.is_some());
    }

    // ── Cloudflare ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn box_fails_without_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "bx1".into(),
            node_type: NodeType::Box,
            config: Some(serde_json::json!({ "endpoint": "/folders/0/items" })),
        };
        let r = execute_box(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("token"));
    }

    #[tokio::test]
    async fn box_fails_without_endpoint() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "bx2".into(),
            node_type: NodeType::Box,
            config: Some(serde_json::json!({ "token": "abc" })),
        };
        let r = execute_box(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("endpoint"));
    }

    // ── Okta ──────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn googledrive_fails_without_access_token() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "g1".into(),
            node_type: NodeType::Googledrive,
            config: Some(serde_json::json!({"operation":"list"})),
        };
        let r = execute_googledrive(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("access_token"));
    }

    #[tokio::test]
    async fn googledrive_get_fails_without_file_id() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "g2".into(),
            node_type: NodeType::Googledrive,
            config: Some(serde_json::json!({"access_token":"test","operation":"get"})),
        };
        let r = execute_googledrive(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("file_id"));
    }

    #[tokio::test]
    async fn googledrive_delete_fails_without_file_id() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "g3".into(),
            node_type: NodeType::Googledrive,
            config: Some(serde_json::json!({"access_token":"test","operation":"delete"})),
        };
        let r = execute_googledrive(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("file_id"));
    }

    #[tokio::test]
    async fn googledrive_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "g4".into(),
            node_type: NodeType::Googledrive,
            config: Some(serde_json::json!({"access_token":"test","operation":"bad"})),
        };
        let r = execute_googledrive(&n, &ctx(), &c).await;
        assert!(r
            .error
            .as_deref()
            .unwrap_or("")
            .contains("unknown operation"));
    }

    // ── WooCommerce ───────────────────────────────────────────────────────────

    #[tokio::test]
    async fn awss3_fails_without_access_key_id() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "a1".into(),
            node_type: NodeType::Awss3,
            config: Some(serde_json::json!({"secret_access_key":"sec","bucket":"my-bucket"})),
        };
        let r = execute_awss3(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("access_key_id"));
    }

    #[tokio::test]
    async fn awss3_fails_without_bucket() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "a2".into(),
            node_type: NodeType::Awss3,
            config: Some(serde_json::json!({"access_key_id":"key","secret_access_key":"sec"})),
        };
        let r = execute_awss3(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("bucket"));
    }

    #[tokio::test]
    async fn awss3_get_object_fails_without_key() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "a3".into(),
            node_type: NodeType::Awss3,
            config: Some(
                serde_json::json!({"access_key_id":"k","secret_access_key":"s","bucket":"b","operation":"get_object"}),
            ),
        };
        let r = execute_awss3(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("key"));
    }

    #[tokio::test]
    async fn awss3_unknown_operation() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "a4".into(),
            node_type: NodeType::Awss3,
            config: Some(
                serde_json::json!({"access_key_id":"k","secret_access_key":"s","bucket":"b","operation":"bad"}),
            ),
        };
        let r = execute_awss3(&n, &ctx(), &c).await;
        assert!(r
            .error
            .as_deref()
            .unwrap_or("")
            .contains("unknown operation"));
    }

    // ── SigV4 signing ─────────────────────────────────────────────────────────

    #[test]
    fn sigv4_uri_encode_encodes_spaces_as_percent20() {
        assert_eq!(sigv4_uri_encode("hello world"), "hello%20world");
    }

    #[test]
    fn sigv4_uri_encode_encodes_slashes() {
        assert_eq!(sigv4_uri_encode("foo/bar"), "foo%2Fbar");
    }

    #[test]
    fn sigv4_uri_encode_passes_through_unreserved_chars() {
        assert_eq!(sigv4_uri_encode("abc-123.~_"), "abc-123.~_");
    }

    #[test]
    fn aws_sigv4_s3_auth_header_well_formed() {
        let auth = aws_sigv4_s3_auth(
            "AKIAIOSFODNN7EXAMPLE",
            "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
            "us-east-1",
            "20130524",
            "20130524T000000Z",
            "GET",
            "examplebucket.s3.amazonaws.com",
            "/",
            "list-type=2",
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        );
        let expected_prefix = "AWS4-HMAC-SHA256 Credential=AKIAIOSFODNN7EXAMPLE/20130524/us-east-1/s3/aws4_request,SignedHeaders=host;x-amz-content-sha256;x-amz-date,Signature=";
        assert!(
            auth.starts_with(expected_prefix),
            "bad header prefix: {auth}"
        );
        let sig = auth.split("Signature=").nth(1).unwrap_or("");
        assert_eq!(
            sig.len(),
            64,
            "signature must be 64 hex chars, got {}",
            sig.len()
        );
        assert!(
            sig.chars().all(|c| c.is_ascii_hexdigit()),
            "signature is not hex"
        );
        assert_ne!(sig, "placeholder");
    }

    #[test]
    fn aws_sigv4_s3_auth_is_deterministic() {
        let (aki, sak, reg, ds, dts, meth, host, uri, qry, ph) = (
            "AKID",
            "SECRET",
            "us-west-2",
            "20260101",
            "20260101T120000Z",
            "PUT",
            "mybucket.s3.us-west-2.amazonaws.com",
            "/mykey.txt",
            "",
            "abc123hash",
        );
        let a1 = aws_sigv4_s3_auth(aki, sak, reg, ds, dts, meth, host, uri, qry, ph);
        let a2 = aws_sigv4_s3_auth(aki, sak, reg, ds, dts, meth, host, uri, qry, ph);
        assert_eq!(a1, a2);
    }

    #[test]
    fn aws_sigv4_s3_auth_differs_by_secret() {
        let (aki, reg, ds, dts, meth, host, uri, qry, ph) = (
            "AKID",
            "us-east-1",
            "20260101",
            "20260101T000000Z",
            "GET",
            "b.s3.amazonaws.com",
            "/",
            "",
            "emptyhash",
        );
        let a1 = aws_sigv4_s3_auth(aki, "SECRET1", reg, ds, dts, meth, host, uri, qry, ph);
        let a2 = aws_sigv4_s3_auth(aki, "SECRET2", reg, ds, dts, meth, host, uri, qry, ph);
        assert_ne!(
            a1, a2,
            "different secrets must produce different signatures"
        );
    }

    // ── Hugging Face ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn cloudinary_fails_without_cloud_name() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "cl1".into(),
            node_type: NodeType::Cloudinary,
            config: Some(serde_json::json!({"api_key":"k","api_secret":"s"})),
        };
        let r = execute_cloudinary(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("cloud_name"));
    }

    #[tokio::test]
    async fn cloudinary_fails_without_api_key() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "cl2".into(),
            node_type: NodeType::Cloudinary,
            config: Some(serde_json::json!({"cloud_name":"mycloud","api_secret":"s"})),
        };
        let r = execute_cloudinary(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("api_key"));
    }

    #[tokio::test]
    async fn cloudinary_destroy_fails_without_public_id() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "cl3".into(),
            node_type: NodeType::Cloudinary,
            config: Some(
                serde_json::json!({"cloud_name":"c","api_key":"k","api_secret":"s","operation":"destroy"}),
            ),
        };
        let r = execute_cloudinary(&n, &ctx(), &c).await;
        assert!(r.error.as_deref().unwrap_or("").contains("public_id"));
    }

    #[tokio::test]
    async fn cloudinary_transform_url_succeeds_without_network() {
        let c = reqwest::Client::new();
        let n = Node {
            id: "cl4".into(),
            node_type: NodeType::Cloudinary,
            config: Some(
                serde_json::json!({"cloud_name":"mycloud","api_key":"k","api_secret":"s","operation":"transform_url","public_id":"sample","transformation":"w_300,h_200"}),
            ),
        };
        let r = execute_cloudinary(&n, &ctx(), &c).await;
        // transform_url is local — no network needed
        assert!(r
            .output_json
            .as_deref()
            .unwrap_or("")
            .contains("res.cloudinary.com"));
    }
}
