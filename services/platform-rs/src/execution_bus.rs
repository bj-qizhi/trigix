// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! In-memory live event bus for executions, keyed by execution id.
//!
//! The `/v1/executions/{id}/events` SSE stream historically re-read the whole
//! execution record from Postgres every 400ms. That is fine for coarse status
//! but can't carry high-frequency, node-level (and eventually token-level)
//! updates without hammering the DB. This bus lets the executor's progress
//! callback *push* compact events straight to connected SSE subscribers,
//! bypassing Postgres; the SSE endpoint still polls (slowly) for a
//! reconnect-safe snapshot and terminal detection.
//!
//! Scope: this is a per-process fan-out and only wires up the inline executor
//! (same process). Separate executor deployments need a cross-process channel
//! (a later stage).

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex, OnceLock};

use tokio::sync::broadcast;

/// One SSE-shaped event: an event name and its already-serialized JSON data.
#[derive(Clone, Debug)]
pub struct ExecEvent {
    pub event: String,
    pub data: String,
}

static BUS: LazyLock<Mutex<HashMap<String, broadcast::Sender<ExecEvent>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Unique per-process id. Lets the Redis bridge ignore the echo of events this
/// instance itself published (which were already delivered locally).
pub static INSTANCE_ID: LazyLock<String> =
    LazyLock::new(|| format!("inst-{}", uuid::Uuid::new_v4()));

/// Fans a published event out to other instances (a Redis publish). Installed
/// once at boot when Redis is available; absent → single-instance, local only.
/// Signature: `(execution_id, event, data)`.
type RemotePublisher = Arc<dyn Fn(&str, &str, &str) + Send + Sync>;
static REMOTE: OnceLock<RemotePublisher> = OnceLock::new();

/// Install the cross-instance fan-out hook (idempotent — first wins).
pub fn set_remote(publisher: RemotePublisher) {
    let _ = REMOTE.set(publisher);
}

/// Subscribe to an execution's live events, creating the channel if needed.
/// The sender is retained until [`close`] is called (on terminal status).
pub fn subscribe(execution_id: &str) -> broadcast::Receiver<ExecEvent> {
    let mut map = BUS.lock().expect("execution bus poisoned");
    map.entry(execution_id.to_string())
        .or_insert_with(|| broadcast::channel(256).0)
        .subscribe()
}

/// Deliver an event to this process's live subscribers only. A no-op (dropped)
/// when nobody is subscribed — the SSE endpoint's snapshot poll covers that
/// case, so we never create a channel just to publish.
fn deliver_local(execution_id: &str, event: &str, data: String) {
    let map = BUS.lock().expect("execution bus poisoned");
    if let Some(tx) = map.get(execution_id) {
        let _ = tx.send(ExecEvent {
            event: event.to_string(),
            data,
        });
    }
}

/// Publish an event: deliver to local subscribers and fan out to other instances
/// (when a Redis bridge is installed) so an SSE client on any instance sees it.
pub fn publish(execution_id: &str, event: &str, data: String) {
    if let Some(remote) = REMOTE.get() {
        remote(execution_id, event, &data);
    }
    deliver_local(execution_id, event, data);
}

/// Deliver an event that arrived from another instance via the Redis bridge.
/// Local delivery only — never re-fans-out, so events can't loop between
/// instances. On a terminal `update`, the local channel is also dropped.
pub fn deliver_from_remote(execution_id: &str, event: &str, data: String) {
    deliver_local(execution_id, event, data);
    if event == "update" {
        close(execution_id);
    }
}

/// Drop an execution's channel once it reaches a terminal state so the map
/// doesn't grow unbounded.
pub fn close(execution_id: &str) {
    BUS.lock()
        .expect("execution bus poisoned")
        .remove(execution_id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn publish_reaches_subscribers_and_close_removes_channel() {
        let id = "exec-bus-test-1";
        let mut rx = subscribe(id);
        publish(id, "node", r#"{"node_id":"a"}"#.to_string());
        let ev = rx.recv().await.expect("event");
        assert_eq!(ev.event, "node");
        assert_eq!(ev.data, r#"{"node_id":"a"}"#);
        close(id);
        // After close, publishing with no channel is a silent no-op.
        publish(id, "node", "{}".to_string());
    }

    #[tokio::test]
    async fn publish_without_subscriber_is_a_noop() {
        // Must not create a channel or panic when nobody is listening.
        publish("exec-bus-nobody", "node", "{}".to_string());
        assert!(BUS.lock().unwrap().get("exec-bus-nobody").is_none());
    }

    #[tokio::test]
    async fn deliver_from_remote_reaches_local_subscribers() {
        // A cross-instance event (arriving via the Redis bridge) must land on
        // this process's subscribers exactly like a local publish.
        let id = "exec-bus-remote-1";
        let mut rx = subscribe(id);
        deliver_from_remote(id, "token", r#"{"delta":"hi"}"#.to_string());
        let ev = rx.recv().await.expect("event");
        assert_eq!(ev.event, "token");
        assert_eq!(ev.data, r#"{"delta":"hi"}"#);
        close(id);
    }

    #[tokio::test]
    async fn deliver_from_remote_terminal_update_closes_channel() {
        let id = "exec-bus-remote-2";
        let _rx = subscribe(id);
        assert!(BUS.lock().unwrap().contains_key(id));
        deliver_from_remote(id, "update", r#"{"status":"succeeded"}"#.to_string());
        // The terminal update drops the local channel so the map can't grow.
        assert!(!BUS.lock().unwrap().contains_key(id));
    }

    #[test]
    fn instance_id_is_stable_within_a_process() {
        assert_eq!(*INSTANCE_ID, *INSTANCE_ID);
        assert!(INSTANCE_ID.starts_with("inst-"));
    }
}
