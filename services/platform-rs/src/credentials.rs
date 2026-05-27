use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialRecord {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing)]
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialSummary {
    pub id: String,
    pub name: String,
}

impl From<&CredentialRecord> for CredentialSummary {
    fn from(r: &CredentialRecord) -> Self {
        Self { id: r.id.clone(), name: r.name.clone() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CredentialError {
    NotFound,
    NameTaken,
    StoreUnavailable,
}

pub trait CredentialStore: Clone + Send + Sync + 'static {
    fn create(
        &self,
        tenant_id: &str,
        name: &str,
        value: &str,
    ) -> impl std::future::Future<Output = Result<CredentialSummary, CredentialError>> + Send;

    fn list(
        &self,
        tenant_id: &str,
    ) -> impl std::future::Future<Output = Result<Vec<CredentialSummary>, CredentialError>> + Send;

    fn get_by_name(
        &self,
        tenant_id: &str,
        name: &str,
    ) -> impl std::future::Future<Output = Result<Option<String>, CredentialError>> + Send;

    fn delete(
        &self,
        tenant_id: &str,
        id: &str,
    ) -> impl std::future::Future<Output = Result<(), CredentialError>> + Send;
}

fn next_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    format!("cred-{:x}", nanos)
}

fn key(tenant_id: &str, id: &str) -> String {
    format!("{}/{}", tenant_id, id)
}

#[derive(Clone, Default)]
pub struct MemoryCredentialStore {
    records: Arc<RwLock<HashMap<String, CredentialRecord>>>,
}

impl CredentialStore for MemoryCredentialStore {
    async fn create(
        &self,
        tenant_id: &str,
        name: &str,
        value: &str,
    ) -> Result<CredentialSummary, CredentialError> {
        let mut records = self.records.write().map_err(|_| CredentialError::StoreUnavailable)?;
        let already_taken = records
            .values()
            .any(|r| r.name == name && records.contains_key(&key(tenant_id, &r.id)));
        if already_taken {
            return Err(CredentialError::NameTaken);
        }
        let id = next_id();
        let record = CredentialRecord {
            id: id.clone(),
            name: name.to_string(),
            value: value.to_string(),
        };
        records.insert(key(tenant_id, &id), record.clone());
        Ok(CredentialSummary::from(&record))
    }

    async fn list(&self, tenant_id: &str) -> Result<Vec<CredentialSummary>, CredentialError> {
        let prefix = format!("{}/", tenant_id);
        let records = self.records.read().map_err(|_| CredentialError::StoreUnavailable)?;
        let mut out: Vec<CredentialSummary> = records
            .iter()
            .filter(|(k, _)| k.starts_with(&prefix))
            .map(|(_, r)| CredentialSummary::from(r))
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    async fn get_by_name(
        &self,
        tenant_id: &str,
        name: &str,
    ) -> Result<Option<String>, CredentialError> {
        let prefix = format!("{}/", tenant_id);
        let records = self.records.read().map_err(|_| CredentialError::StoreUnavailable)?;
        let value = records
            .iter()
            .filter(|(k, _)| k.starts_with(&prefix))
            .find(|(_, r)| r.name == name)
            .map(|(_, r)| r.value.clone());
        Ok(value)
    }

    async fn delete(&self, tenant_id: &str, id: &str) -> Result<(), CredentialError> {
        let mut records = self.records.write().map_err(|_| CredentialError::StoreUnavailable)?;
        records.remove(&key(tenant_id, id)).ok_or(CredentialError::NotFound)?;
        Ok(())
    }
}

pub type PlatformCredentialStore = MemoryCredentialStore;

/// Replace `{{credential.name}}` patterns in a JSON value.
/// Returns the resolved value and a list of any unresolved names.
pub async fn resolve_credentials_in_json(
    value: &serde_json::Value,
    store: &impl CredentialStore,
    tenant_id: &str,
) -> (serde_json::Value, Vec<String>) {
    let mut unresolved = Vec::new();
    let resolved = resolve_value(value, store, tenant_id, &mut unresolved).await;
    (resolved, unresolved)
}

async fn resolve_value(
    value: &serde_json::Value,
    store: &impl CredentialStore,
    tenant_id: &str,
    unresolved: &mut Vec<String>,
) -> serde_json::Value {
    match value {
        serde_json::Value::String(s) => {
            serde_json::Value::String(resolve_string(s, store, tenant_id, unresolved).await)
        }
        serde_json::Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (k, v) in map {
                out.insert(k.clone(), Box::pin(resolve_value(v, store, tenant_id, unresolved)).await);
            }
            serde_json::Value::Object(out)
        }
        serde_json::Value::Array(arr) => {
            let mut out = Vec::with_capacity(arr.len());
            for v in arr {
                out.push(Box::pin(resolve_value(v, store, tenant_id, unresolved)).await);
            }
            serde_json::Value::Array(out)
        }
        other => other.clone(),
    }
}

async fn resolve_string(
    s: &str,
    store: &impl CredentialStore,
    tenant_id: &str,
    unresolved: &mut Vec<String>,
) -> String {
    let mut result = s.to_string();
    let mut search = result.as_str();
    let mut output = String::new();

    while let Some(start) = search.find("{{credential.") {
        output.push_str(&search[..start]);
        let rest = &search[start + "{{credential.".len()..];
        if let Some(end) = rest.find("}}") {
            let name = &rest[..end];
            match store.get_by_name(tenant_id, name).await {
                Ok(Some(secret)) => output.push_str(&secret),
                _ => {
                    unresolved.push(name.to_string());
                    output.push_str(&search[start..start + "{{credential.".len() + end + "}}".len()]);
                }
            }
            search = &rest[end + "}}".len()..];
        } else {
            output.push_str(&search[start..]);
            search = "";
            break;
        }
    }
    output.push_str(search);
    result = output;
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn create_and_list_credentials() {
        let store = MemoryCredentialStore::default();
        let s = store.create("t1", "my-key", "secret123").await.unwrap();
        assert_eq!(s.name, "my-key");

        let list = store.list("t1").await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "my-key");
    }

    #[tokio::test]
    async fn get_by_name_returns_value() {
        let store = MemoryCredentialStore::default();
        store.create("t1", "api-key", "tok-abc").await.unwrap();
        let val = store.get_by_name("t1", "api-key").await.unwrap();
        assert_eq!(val, Some("tok-abc".to_string()));
    }

    #[tokio::test]
    async fn delete_credential() {
        let store = MemoryCredentialStore::default();
        let s = store.create("t1", "to-delete", "x").await.unwrap();
        store.delete("t1", &s.id).await.unwrap();
        let list = store.list("t1").await.unwrap();
        assert!(list.is_empty());
    }

    #[tokio::test]
    async fn resolve_credentials_in_json_replaces_patterns() {
        let store = MemoryCredentialStore::default();
        store.create("t1", "token", "Bearer sk-test").await.unwrap();

        let input = serde_json::json!({
            "url": "https://api.example.com",
            "headers": { "Authorization": "{{credential.token}}" }
        });
        let (resolved, unresolved) = resolve_credentials_in_json(&input, &store, "t1").await;
        assert!(unresolved.is_empty());
        assert_eq!(resolved["headers"]["Authorization"], "Bearer sk-test");
        assert_eq!(resolved["url"], "https://api.example.com");
    }

    #[tokio::test]
    async fn resolve_leaves_unknown_patterns_intact() {
        let store = MemoryCredentialStore::default();
        let input = serde_json::json!({ "h": "{{credential.missing}}" });
        let (resolved, unresolved) = resolve_credentials_in_json(&input, &store, "t1").await;
        assert_eq!(unresolved, vec!["missing".to_string()]);
        assert_eq!(resolved["h"], "{{credential.missing}}");
    }
}
