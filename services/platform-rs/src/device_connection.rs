// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::{watch, OwnedMutexGuard};

#[derive(Debug)]
struct ActiveConnection {
    session_id: String,
    disconnect: watch::Sender<Option<String>>,
}

pub struct ConnectionLease {
    pub session_id: String,
    pub replaced_session_id: Option<String>,
    pub cancellation: watch::Receiver<Option<String>>,
}

#[derive(Clone, Default)]
pub struct DeviceConnectionManager {
    active: Arc<Mutex<HashMap<String, ActiveConnection>>>,
    establishment: Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>>,
}

impl DeviceConnectionManager {
    pub async fn establishment_guard(&self, device_id: &str) -> OwnedMutexGuard<()> {
        let lock = self
            .establishment
            .lock()
            .expect("device establishment lock")
            .entry(device_id.to_string())
            .or_default()
            .clone();
        lock.lock_owned().await
    }

    pub fn replace(&self, device_id: &str, session_id: String) -> ConnectionLease {
        let (disconnect, cancellation) = watch::channel(None);
        let replaced = self.active.lock().expect("device connections lock").insert(
            device_id.to_string(),
            ActiveConnection {
                session_id: session_id.clone(),
                disconnect,
            },
        );
        let replaced_session_id = replaced.map(|connection| {
            let _ = connection
                .disconnect
                .send(Some("replaced_by_newer_session".to_string()));
            connection.session_id
        });
        ConnectionLease {
            session_id,
            replaced_session_id,
            cancellation,
        }
    }

    pub fn owns(&self, device_id: &str, session_id: &str) -> bool {
        self.active
            .lock()
            .expect("device connections lock")
            .get(device_id)
            .is_some_and(|connection| connection.session_id == session_id)
    }

    pub fn release(&self, device_id: &str, session_id: &str) -> bool {
        let mut active = self.active.lock().expect("device connections lock");
        if active
            .get(device_id)
            .is_some_and(|connection| connection.session_id == session_id)
        {
            active.remove(device_id);
            true
        } else {
            false
        }
    }

    pub fn disconnect(&self, device_id: &str, reason: &str) -> bool {
        let connection = self
            .active
            .lock()
            .expect("device connections lock")
            .remove(device_id);
        connection
            .is_some_and(|connection| connection.disconnect.send(Some(reason.to_string())).is_ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn newest_session_deterministically_replaces_previous_owner() {
        let manager = DeviceConnectionManager::default();
        let first = manager.replace("device-1", "session-1".to_string());
        let mut first_cancellation = first.cancellation;
        let second = manager.replace("device-1", "session-2".to_string());

        first_cancellation.changed().await.unwrap();
        assert_eq!(
            first_cancellation.borrow().as_deref(),
            Some("replaced_by_newer_session")
        );
        assert_eq!(second.replaced_session_id.as_deref(), Some("session-1"));
        assert!(!manager.owns("device-1", "session-1"));
        assert!(manager.owns("device-1", "session-2"));
        assert!(!manager.release("device-1", "session-1"));
        assert!(manager.release("device-1", "session-2"));
    }

    #[tokio::test]
    async fn administrative_disconnect_notifies_long_lived_session() {
        let manager = DeviceConnectionManager::default();
        let lease = manager.replace("device-1", "session-1".to_string());
        let mut cancellation = lease.cancellation;
        assert!(manager.disconnect("device-1", "device_suspended"));
        cancellation.changed().await.unwrap();
        assert_eq!(cancellation.borrow().as_deref(), Some("device_suspended"));
        assert!(!manager.owns("device-1", "session-1"));
    }

    #[tokio::test]
    async fn connection_establishment_is_serialized_per_device_only() {
        let manager = DeviceConnectionManager::default();
        let first = manager.establishment_guard("device-1").await;
        assert!(tokio::time::timeout(
            std::time::Duration::from_millis(50),
            manager.establishment_guard("device-2"),
        )
        .await
        .is_ok());
        assert!(tokio::time::timeout(
            std::time::Duration::from_millis(25),
            manager.establishment_guard("device-1"),
        )
        .await
        .is_err());
        drop(first);
        assert!(tokio::time::timeout(
            std::time::Duration::from_millis(50),
            manager.establishment_guard("device-1"),
        )
        .await
        .is_ok());
    }
}
