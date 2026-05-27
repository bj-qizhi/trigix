use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use tokio::sync::{oneshot, Mutex};

#[derive(Debug)]
pub enum ApprovalError {
    NotFound,
}

#[derive(Clone, Default)]
pub struct ApprovalGate {
    senders: Arc<Mutex<HashMap<String, oneshot::Sender<bool>>>>,
    waiting: Arc<Mutex<HashSet<String>>>,
}

impl ApprovalGate {
    pub async fn register(&self, execution_id: String) -> oneshot::Receiver<bool> {
        let (tx, rx) = oneshot::channel();
        self.senders.lock().await.insert(execution_id.clone(), tx);
        self.waiting.lock().await.insert(execution_id);
        rx
    }

    pub async fn resolve(&self, execution_id: &str, approved: bool) -> Result<(), ApprovalError> {
        let tx = self
            .senders
            .lock()
            .await
            .remove(execution_id)
            .ok_or(ApprovalError::NotFound)?;
        self.waiting.lock().await.remove(execution_id);
        // If receiver was dropped (execution already ended), treat as ok
        let _ = tx.send(approved);
        Ok(())
    }

    pub async fn is_waiting(&self, execution_id: &str) -> bool {
        self.waiting.lock().await.contains(execution_id)
    }
}
