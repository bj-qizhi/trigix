// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! Minimal server-side PostHog client for emitting conversion events (e.g. a
//! paid subscription) credited to the acquisition channel captured at signup.
//!
//! Opt-in: [`PostHogClient::from_env`] returns `None` unless `POSTHOG_API_KEY`
//! is set. Host defaults to PostHog Cloud US and is overridable via
//! `POSTHOG_HOST`.

#[derive(Clone)]
pub struct PostHogClient {
    api_key: String,
    host: String,
    http: reqwest::Client,
}

impl PostHogClient {
    /// Builds a client only when `POSTHOG_API_KEY` is configured.
    pub fn from_env() -> Option<Self> {
        let api_key = std::env::var("POSTHOG_API_KEY")
            .ok()
            .filter(|s| !s.is_empty())?;
        let host = std::env::var("POSTHOG_HOST")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "https://us.i.posthog.com".to_string());
        Some(Self {
            api_key,
            host: host.trim_end_matches('/').to_string(),
            http: reqwest::Client::new(),
        })
    }

    /// Captures a server-side event. Failures are logged, never propagated.
    pub async fn capture(&self, distinct_id: &str, event: &str, properties: serde_json::Value) {
        let body = capture_body(&self.api_key, event, distinct_id, properties);
        let url = format!("{}/capture/", self.host);
        if let Err(e) = self.http.post(&url).json(&body).send().await {
            tracing::warn!(event, error = %e, "PostHog capture failed");
        }
    }
}

/// Builds the PostHog `/capture/` request body. Extracted for unit testing.
pub fn capture_body(
    api_key: &str,
    event: &str,
    distinct_id: &str,
    properties: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "api_key": api_key,
        "event": event,
        "distinct_id": distinct_id,
        "properties": properties,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn capture_body_shape() {
        let body = capture_body(
            "phc_123",
            "subscription_started",
            "user-1",
            json!({"tier": "pro", "utm_source": "google"}),
        );
        assert_eq!(body["api_key"], "phc_123");
        assert_eq!(body["event"], "subscription_started");
        assert_eq!(body["distinct_id"], "user-1");
        assert_eq!(body["properties"]["tier"], "pro");
        assert_eq!(body["properties"]["utm_source"], "google");
    }
}
