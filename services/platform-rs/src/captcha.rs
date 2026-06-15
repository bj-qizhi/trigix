// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! Pluggable bot/abuse captcha verification for Cloudflare Turnstile and
//! hCaptcha. Both expose the same `siteverify` contract (`secret` + `response`
//! form fields, a JSON `{ "success": bool }` reply), so one implementation
//! covers both — only the endpoint URL differs.
//!
//! Verification is **opt-in**: [`CaptchaVerifier::from_env`] returns `None`
//! unless both `CAPTCHA_PROVIDER` and `CAPTCHA_SECRET` are set, in which case
//! callers skip the check entirely (zero behavior change in dev/test).

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CaptchaProvider {
    Turnstile,
    Hcaptcha,
}

impl CaptchaProvider {
    fn siteverify_url(self) -> &'static str {
        match self {
            Self::Turnstile => "https://challenges.cloudflare.com/turnstile/v0/siteverify",
            Self::Hcaptcha => "https://api.hcaptcha.com/siteverify",
        }
    }

    /// Parses `CAPTCHA_PROVIDER`. Accepts `turnstile`/`cloudflare` and `hcaptcha`.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "turnstile" | "cloudflare" => Some(Self::Turnstile),
            "hcaptcha" => Some(Self::Hcaptcha),
            _ => None,
        }
    }
}

#[derive(Clone)]
pub struct CaptchaVerifier {
    provider: CaptchaProvider,
    secret: String,
    http: reqwest::Client,
}

impl CaptchaVerifier {
    /// Builds a verifier only when a provider and secret are both configured;
    /// otherwise `None`, meaning captcha enforcement is disabled.
    pub fn from_env() -> Option<Self> {
        let provider = CaptchaProvider::parse(&std::env::var("CAPTCHA_PROVIDER").ok()?)?;
        let secret = std::env::var("CAPTCHA_SECRET")
            .ok()
            .filter(|s| !s.is_empty())?;
        Some(Self {
            provider,
            secret,
            http: reqwest::Client::new(),
        })
    }

    pub fn provider(&self) -> CaptchaProvider {
        self.provider
    }

    /// Verifies a client-supplied captcha `token` against the provider.
    /// `remote_ip` is optional and forwarded as `remoteip` when present.
    pub async fn verify(&self, token: &str, remote_ip: Option<&str>) -> Result<(), String> {
        if token.trim().is_empty() {
            return Err("missing captcha token".to_string());
        }
        let mut params: Vec<(&str, &str)> =
            vec![("secret", self.secret.as_str()), ("response", token)];
        if let Some(ip) = remote_ip {
            if !ip.is_empty() {
                params.push(("remoteip", ip));
            }
        }
        let res = self
            .http
            .post(self.provider.siteverify_url())
            .form(&params)
            .send()
            .await
            .map_err(|e| format!("captcha request failed: {e}"))?;
        let body: serde_json::Value = res
            .json()
            .await
            .map_err(|e| format!("captcha response parse failed: {e}"))?;
        if captcha_succeeded(&body) {
            Ok(())
        } else {
            Err(format!(
                "captcha verification failed: {}",
                captcha_error_codes(&body)
            ))
        }
    }
}

/// Reads the `success` boolean common to Turnstile and hCaptcha siteverify replies.
pub fn captcha_succeeded(body: &serde_json::Value) -> bool {
    body.get("success")
        .and_then(|s| s.as_bool())
        .unwrap_or(false)
}

/// Renders the provider's `error-codes` array as a compact string for logging.
fn captcha_error_codes(body: &serde_json::Value) -> String {
    body.get("error-codes")
        .and_then(|c| c.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn provider_parse_accepts_known_aliases() {
        assert_eq!(
            CaptchaProvider::parse("turnstile"),
            Some(CaptchaProvider::Turnstile)
        );
        assert_eq!(
            CaptchaProvider::parse("Cloudflare"),
            Some(CaptchaProvider::Turnstile)
        );
        assert_eq!(
            CaptchaProvider::parse(" hCaptcha "),
            Some(CaptchaProvider::Hcaptcha)
        );
        assert_eq!(CaptchaProvider::parse("recaptcha"), None);
    }

    #[test]
    fn siteverify_urls_are_provider_specific() {
        assert!(CaptchaProvider::Turnstile
            .siteverify_url()
            .contains("challenges.cloudflare.com"));
        assert!(CaptchaProvider::Hcaptcha
            .siteverify_url()
            .contains("api.hcaptcha.com"));
    }

    #[test]
    fn success_and_error_codes_parse() {
        assert!(captcha_succeeded(&json!({"success": true})));
        assert!(!captcha_succeeded(&json!({"success": false})));
        assert!(!captcha_succeeded(&json!({})));
        let body = json!({"success": false, "error-codes": ["invalid-input-response", "timeout-or-duplicate"]});
        assert_eq!(
            captcha_error_codes(&body),
            "invalid-input-response,timeout-or-duplicate"
        );
    }
}
