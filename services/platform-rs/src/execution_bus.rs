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
use std::sync::{LazyLock, Mutex};

use tokio::sync::broadcast;

/// One SSE-shaped event: an event name and its already-serialized JSON data.
#[derive(Clone, Debug)]
pub struct ExecEvent {
    pub event: String,
    pub data: String,
}

static BUS: LazyLock<Mutex<HashMap<String, broadcast::Sender<ExecEvent>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Subscribe to an execution's live events, creating the channel if needed.
/// The sender is retained until [`close`] is called (on terminal status).
pub fn subscribe(execution_id: &str) -> broadcast::Receiver<ExecEvent> {
    let mut map = BUS.lock().expect("execution bus poisoned");
    map.entry(execution_id.to_string())
        .or_insert_with(|| broadcast::channel(256).0)
        .subscribe()
}

/// Push an event to any live subscribers. A no-op (dropped) when nobody is
/// subscribed — the SSE endpoint's snapshot poll covers that case, so we never
/// create a channel just to publish.
pub fn publish(execution_id: &str, event: &str, data: String) {
    let map = BUS.lock().expect("execution bus poisoned");
    if let Some(tx) = map.get(execution_id) {
        let _ = tx.send(ExecEvent {
            event: event.to_string(),
            data,
        });
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
}
