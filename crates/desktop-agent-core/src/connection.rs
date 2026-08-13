// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use std::time::Duration;

pub const DEFAULT_INITIAL_BACKOFF: Duration = Duration::from_secs(1);
pub const DEFAULT_MAX_BACKOFF: Duration = Duration::from_secs(60);

#[derive(Debug, Clone)]
pub struct ReconnectBackoff {
    attempt: u32,
    initial: Duration,
    maximum: Duration,
    jitter_state: u64,
}

impl ReconnectBackoff {
    pub fn new(initial: Duration, maximum: Duration, jitter_seed: u64) -> Self {
        assert!(
            !initial.is_zero(),
            "initial reconnect delay must be positive"
        );
        assert!(maximum >= initial, "maximum delay must cover initial delay");
        Self {
            attempt: 0,
            initial,
            maximum,
            jitter_state: jitter_seed.max(1),
        }
    }

    pub fn next_delay(&mut self) -> Duration {
        let multiplier = 1_u128 << self.attempt.min(20);
        let base_millis = self
            .initial
            .as_millis()
            .saturating_mul(multiplier)
            .min(self.maximum.as_millis());
        self.jitter_state ^= self.jitter_state << 13;
        self.jitter_state ^= self.jitter_state >> 7;
        self.jitter_state ^= self.jitter_state << 17;
        let jitter_percent = 80 + self.jitter_state % 41;
        let jittered = base_millis
            .saturating_mul(u128::from(jitter_percent))
            .saturating_div(100)
            .min(self.maximum.as_millis());
        self.attempt = self.attempt.saturating_add(1);
        Duration::from_millis(jittered.min(u64::MAX as u128) as u64)
    }

    pub fn reset(&mut self) {
        self.attempt = 0;
    }

    pub fn attempt(&self) -> u32 {
        self.attempt
    }
}

impl Default for ReconnectBackoff {
    fn default() -> Self {
        Self::new(DEFAULT_INITIAL_BACKOFF, DEFAULT_MAX_BACKOFF, 1)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceEndpoint {
    pub base_url: String,
    pub proxy_url: Option<String>,
}

impl DeviceEndpoint {
    pub fn validate(&self) -> Result<(), &'static str> {
        if !self.base_url.starts_with("https://") || self.base_url.len() <= "https://".len() {
            return Err("Device endpoint must use HTTPS");
        }
        if self.base_url.len() > 2048 || self.base_url.chars().any(char::is_whitespace) {
            return Err("Device endpoint is invalid");
        }
        if let Some(proxy) = self.proxy_url.as_deref() {
            if !(proxy.starts_with("http://") || proxy.starts_with("https://"))
                || proxy
                    .split_once("://")
                    .is_none_or(|(_, authority)| authority.is_empty())
                || proxy.len() > 2048
                || proxy.chars().any(char::is_whitespace)
            {
                return Err("Proxy endpoint is invalid");
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconnect_backoff_is_bounded_jittered_and_resets_after_long_session() {
        let mut backoff =
            ReconnectBackoff::new(Duration::from_secs(1), Duration::from_secs(30), 42);
        let delays = (0..50).map(|_| backoff.next_delay()).collect::<Vec<_>>();
        assert!(delays.iter().all(|delay| *delay <= Duration::from_secs(30)));
        assert!(delays
            .iter()
            .all(|delay| *delay >= Duration::from_millis(800)));
        assert!(delays.windows(2).any(|pair| pair[0] != pair[1]));
        assert_eq!(backoff.attempt(), 50);

        backoff.reset();
        assert_eq!(backoff.attempt(), 0);
        assert!(backoff.next_delay() <= Duration::from_millis(1_200));
    }

    #[test]
    fn endpoint_requires_tls_and_accepts_explicit_http_connect_proxy() {
        assert!(DeviceEndpoint {
            base_url: "http://platform.example".to_string(),
            proxy_url: None,
        }
        .validate()
        .is_err());
        assert!(DeviceEndpoint {
            base_url: "https://platform.example".to_string(),
            proxy_url: Some("http://proxy.corp:8080".to_string()),
        }
        .validate()
        .is_ok());
        assert!(DeviceEndpoint {
            base_url: "https://platform.example".to_string(),
            proxy_url: Some("socks5://proxy.corp".to_string()),
        }
        .validate()
        .is_err());
    }
}
