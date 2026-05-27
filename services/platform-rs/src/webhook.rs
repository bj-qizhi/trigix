use std::{collections::HashMap, future::Future, sync::Arc};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookRecord {
    pub token: String,
    pub tenant_id: String,
    pub workflow_id: String,
    pub workflow_version_id: String,
}

#[derive(Debug)]
pub enum WebhookError {
    NotFound,
    StoreUnavailable,
}

pub trait WebhookStore: Clone + Send + Sync + 'static {
    fn upsert(
        &self,
        record: WebhookRecord,
    ) -> impl Future<Output = Result<WebhookRecord, WebhookError>> + Send;
    fn get_by_token(
        &self,
        token: &str,
    ) -> impl Future<Output = Result<WebhookRecord, WebhookError>> + Send;
    fn get_by_version(
        &self,
        workflow_version_id: &str,
    ) -> impl Future<Output = Result<Option<WebhookRecord>, WebhookError>> + Send;
}

#[derive(Clone, Default)]
pub struct MemoryWebhookStore {
    by_token: Arc<Mutex<HashMap<String, WebhookRecord>>>,
    by_version: Arc<Mutex<HashMap<String, String>>>,
}

impl WebhookStore for MemoryWebhookStore {
    fn upsert(
        &self,
        record: WebhookRecord,
    ) -> impl Future<Output = Result<WebhookRecord, WebhookError>> + Send {
        let by_token = self.by_token.clone();
        let by_version = self.by_version.clone();
        async move {
            by_token
                .lock()
                .await
                .insert(record.token.clone(), record.clone());
            by_version
                .lock()
                .await
                .insert(record.workflow_version_id.clone(), record.token.clone());
            Ok(record)
        }
    }

    fn get_by_token(
        &self,
        token: &str,
    ) -> impl Future<Output = Result<WebhookRecord, WebhookError>> + Send {
        let by_token = self.by_token.clone();
        let token = token.to_string();
        async move {
            by_token
                .lock()
                .await
                .get(&token)
                .cloned()
                .ok_or(WebhookError::NotFound)
        }
    }

    fn get_by_version(
        &self,
        workflow_version_id: &str,
    ) -> impl Future<Output = Result<Option<WebhookRecord>, WebhookError>> + Send {
        let by_token = self.by_token.clone();
        let by_version = self.by_version.clone();
        let version_id = workflow_version_id.to_string();
        async move {
            let versions = by_version.lock().await;
            let Some(token) = versions.get(&version_id).cloned() else {
                return Ok(None);
            };
            drop(versions);
            Ok(by_token.lock().await.get(&token).cloned())
        }
    }
}

pub type PlatformWebhookStore = MemoryWebhookStore;
