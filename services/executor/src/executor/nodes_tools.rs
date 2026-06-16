// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! Pure-compute utility nodes that need no network: cryptographic hashing /
//! HMAC and HMAC-signed JWT (HS256/384/512) sign & verify.

use super::*;
use crate::runtime::{ExecutionContext, NodeExecutionResult};
use base64::Engine as _;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256, Sha384, Sha512};
use workflow_core::Node;

fn b64url(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn b64url_decode(s: &str) -> Option<Vec<u8>> {
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(s)
        .ok()
}

// HMAC over `data` with `key`, selected by JWT-style algorithm name.
fn hmac_sign(alg: &str, key: &[u8], data: &[u8]) -> Option<Vec<u8>> {
    match alg {
        "HS256" => {
            let mut m = Hmac::<Sha256>::new_from_slice(key).ok()?;
            m.update(data);
            Some(m.finalize().into_bytes().to_vec())
        }
        "HS384" => {
            let mut m = Hmac::<Sha384>::new_from_slice(key).ok()?;
            m.update(data);
            Some(m.finalize().into_bytes().to_vec())
        }
        "HS512" => {
            let mut m = Hmac::<Sha512>::new_from_slice(key).ok()?;
            m.update(data);
            Some(m.finalize().into_bytes().to_vec())
        }
        _ => None,
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ── Hash / HMAC ───────────────────────────────────────────────────────────────
pub(super) async fn execute_hash(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);

    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("sha256")
        .to_string();
    let input = cfg
        .get("input")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let encoding = cfg
        .get("encoding")
        .and_then(|v| v.as_str())
        .unwrap_or("hex")
        .to_string();

    let digest: Vec<u8> = match operation.as_str() {
        "sha256" => Sha256::digest(input.as_bytes()).to_vec(),
        "sha384" => Sha384::digest(input.as_bytes()).to_vec(),
        "sha512" => Sha512::digest(input.as_bytes()).to_vec(),
        "hmac_sha256" | "hmac_sha384" | "hmac_sha512" => {
            let key = match cfg.get("key").and_then(|v| v.as_str()) {
                Some(k) if !k.is_empty() => k.to_string(),
                _ => {
                    return NodeExecutionResult::failed(format!("Hash {operation} requires 'key'"))
                }
            };
            let alg = match operation.as_str() {
                "hmac_sha256" => "HS256",
                "hmac_sha384" => "HS384",
                _ => "HS512",
            };
            match hmac_sign(alg, key.as_bytes(), input.as_bytes()) {
                Some(b) => b,
                None => return NodeExecutionResult::failed("Hash HMAC computation failed"),
            }
        }
        other => {
            return NodeExecutionResult::failed(format!(
                "Hash unknown operation '{other}' (expected sha256/sha384/sha512/hmac_sha256/hmac_sha384/hmac_sha512)"
            ))
        }
    };

    let encoded = match encoding.as_str() {
        "hex" => hex::encode(&digest),
        "base64" => base64::engine::general_purpose::STANDARD.encode(&digest),
        "base64url" => b64url(&digest),
        other => {
            return NodeExecutionResult::failed(format!(
                "Hash unknown encoding '{other}' (expected hex/base64/base64url)"
            ))
        }
    };

    NodeExecutionResult::succeeded(
        serde_json::json!({ "hash": encoded, "algorithm": operation, "encoding": encoding })
            .to_string(),
    )
}

// ── JWT (HMAC algorithms) ─────────────────────────────────────────────────────
pub(super) async fn execute_jwt(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);

    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("sign")
        .to_string();
    let algorithm = cfg
        .get("algorithm")
        .and_then(|v| v.as_str())
        .unwrap_or("HS256")
        .to_string();
    if !matches!(algorithm.as_str(), "HS256" | "HS384" | "HS512") {
        return NodeExecutionResult::failed(format!(
            "JWT unsupported algorithm '{algorithm}' (expected HS256/HS384/HS512)"
        ));
    }
    let secret = match cfg.get("secret").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return NodeExecutionResult::failed("JWT requires 'secret'"),
    };

    match operation.as_str() {
        "sign" => {
            let mut payload = match cfg.get("payload") {
                Some(p) => json_array_or_parse(p),
                None => serde_json::json!({}),
            };
            if !payload.is_object() {
                return NodeExecutionResult::failed("JWT sign 'payload' must be a JSON object");
            }
            if let Some(secs) = cfg.get("expires_in_secs").and_then(|v| v.as_u64()) {
                let obj = payload.as_object_mut().unwrap();
                let now = now_secs();
                obj.entry("iat").or_insert(serde_json::json!(now));
                obj.insert("exp".to_string(), serde_json::json!(now + secs));
            }
            let header = serde_json::json!({ "alg": algorithm, "typ": "JWT" });
            let header_b64 = b64url(header.to_string().as_bytes());
            let payload_b64 = b64url(payload.to_string().as_bytes());
            let signing_input = format!("{header_b64}.{payload_b64}");
            let sig = match hmac_sign(&algorithm, secret.as_bytes(), signing_input.as_bytes()) {
                Some(s) => b64url(&s),
                None => return NodeExecutionResult::failed("JWT signing failed"),
            };
            NodeExecutionResult::succeeded(
                serde_json::json!({ "token": format!("{signing_input}.{sig}") }).to_string(),
            )
        }
        "verify" => {
            let token = match cfg.get("token").and_then(|v| v.as_str()) {
                Some(t) if !t.is_empty() => t.to_string(),
                _ => return NodeExecutionResult::failed("JWT verify requires 'token'"),
            };
            let parts: Vec<&str> = token.split('.').collect();
            if parts.len() != 3 {
                return NodeExecutionResult::succeeded(
                    serde_json::json!({ "valid": false, "error": "malformed token" }).to_string(),
                );
            }
            let signing_input = format!("{}.{}", parts[0], parts[1]);
            let expected = match hmac_sign(&algorithm, secret.as_bytes(), signing_input.as_bytes())
            {
                Some(s) => s,
                None => return NodeExecutionResult::failed("JWT verify HMAC failed"),
            };
            let provided =
                match b64url_decode(parts[2]) {
                    Some(s) => s,
                    None => return NodeExecutionResult::succeeded(
                        serde_json::json!({ "valid": false, "error": "bad signature encoding" })
                            .to_string(),
                    ),
                };
            // Constant-time comparison.
            let sig_ok = expected.len() == provided.len()
                && expected
                    .iter()
                    .zip(provided.iter())
                    .fold(0u8, |acc, (a, b)| acc | (a ^ b))
                    == 0;
            if !sig_ok {
                return NodeExecutionResult::succeeded(
                    serde_json::json!({ "valid": false, "error": "signature mismatch" })
                        .to_string(),
                );
            }
            let payload: serde_json::Value = match b64url_decode(parts[1])
                .and_then(|b| serde_json::from_slice(&b).ok())
            {
                Some(p) => p,
                None => {
                    return NodeExecutionResult::succeeded(
                        serde_json::json!({ "valid": false, "error": "bad payload" }).to_string(),
                    )
                }
            };
            // Reject expired tokens when an exp claim is present.
            if let Some(exp) = payload.get("exp").and_then(|v| v.as_u64()) {
                if now_secs() >= exp {
                    return NodeExecutionResult::succeeded(
                        serde_json::json!({ "valid": false, "error": "expired", "payload": payload })
                            .to_string(),
                    );
                }
            }
            NodeExecutionResult::succeeded(
                serde_json::json!({ "valid": true, "payload": payload }).to_string(),
            )
        }
        other => NodeExecutionResult::failed(format!("JWT unknown operation '{other}'")),
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
    async fn hash_sha256_known_vector() {
        // SHA-256("abc") = ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
        let n = Node {
            id: "h1".into(),
            node_type: NodeType::Hash,
            config: Some(serde_json::json!({"operation":"sha256","input":"abc"})),
        };
        let r = execute_hash(&n, &ctx()).await;
        let out: serde_json::Value =
            serde_json::from_str(r.output_json.as_deref().unwrap_or("{}")).unwrap();
        assert_eq!(
            out["hash"],
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[tokio::test]
    async fn hash_hmac_requires_key() {
        let n = Node {
            id: "h2".into(),
            node_type: NodeType::Hash,
            config: Some(serde_json::json!({"operation":"hmac_sha256","input":"abc"})),
        };
        let r = execute_hash(&n, &ctx()).await;
        assert!(r.error.as_deref().unwrap_or("").contains("key"));
    }

    #[tokio::test]
    async fn jwt_sign_then_verify_roundtrips() {
        let sign = Node {
            id: "j1".into(),
            node_type: NodeType::Jwt,
            config: Some(serde_json::json!({
                "operation":"sign","secret":"s3cret",
                "payload":{"sub":"123","name":"Ada"}
            })),
        };
        let signed = execute_jwt(&sign, &ctx()).await;
        let token =
            serde_json::from_str::<serde_json::Value>(signed.output_json.as_deref().unwrap())
                .unwrap()["token"]
                .as_str()
                .unwrap()
                .to_string();

        let verify = Node {
            id: "j2".into(),
            node_type: NodeType::Jwt,
            config: Some(serde_json::json!({
                "operation":"verify","secret":"s3cret","token":token
            })),
        };
        let v = execute_jwt(&verify, &ctx()).await;
        let out: serde_json::Value =
            serde_json::from_str(v.output_json.as_deref().unwrap()).unwrap();
        assert_eq!(out["valid"], true);
        assert_eq!(out["payload"]["name"], "Ada");
    }

    #[tokio::test]
    async fn jwt_verify_rejects_wrong_secret() {
        let sign = Node {
            id: "j3".into(),
            node_type: NodeType::Jwt,
            config: Some(serde_json::json!({
                "operation":"sign","secret":"right","payload":{"x":1}
            })),
        };
        let token = serde_json::from_str::<serde_json::Value>(
            execute_jwt(&sign, &ctx())
                .await
                .output_json
                .as_deref()
                .unwrap(),
        )
        .unwrap()["token"]
            .as_str()
            .unwrap()
            .to_string();

        let verify = Node {
            id: "j4".into(),
            node_type: NodeType::Jwt,
            config: Some(serde_json::json!({
                "operation":"verify","secret":"wrong","token":token
            })),
        };
        let out: serde_json::Value = serde_json::from_str(
            execute_jwt(&verify, &ctx())
                .await
                .output_json
                .as_deref()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(out["valid"], false);
    }

    #[tokio::test]
    async fn jwt_requires_secret() {
        let n = Node {
            id: "j5".into(),
            node_type: NodeType::Jwt,
            config: Some(serde_json::json!({"operation":"sign","payload":{}})),
        };
        let r = execute_jwt(&n, &ctx()).await;
        assert!(r.error.as_deref().unwrap_or("").contains("secret"));
    }
}
