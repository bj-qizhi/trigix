// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! Stripe Payments — Checkout Sessions, Customer Portal, webhook signature verification.

use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Thin Stripe REST API client built on reqwest.
/// Instantiate via [`StripeClient::from_env`] — returns `None` when
/// `STRIPE_SECRET_KEY` is absent or empty.
#[derive(Clone)]
pub struct StripeClient {
    secret_key: String,
    http: reqwest::Client,
}

impl StripeClient {
    pub fn from_env() -> Option<Self> {
        let key = std::env::var("STRIPE_SECRET_KEY").unwrap_or_default();
        if key.is_empty() {
            return None;
        }
        Some(Self {
            secret_key: key,
            http: reqwest::Client::new(),
        })
    }

    async fn stripe_post(
        &self,
        path: &str,
        params: &[(&str, &str)],
    ) -> Result<serde_json::Value, String> {
        let body = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, percent_encode(v)))
            .collect::<Vec<_>>()
            .join("&");

        let res = self
            .http
            .post(format!("https://api.stripe.com{path}"))
            .bearer_auth(&self.secret_key)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .map_err(|e| format!("Stripe request failed: {e}"))?;

        let status = res.status();
        let text = res.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(format!("Stripe {path} {status}: {text}"));
        }
        serde_json::from_str(&text).map_err(|e| format!("Stripe JSON: {e} — {text}"))
    }

    /// Creates a hosted Checkout Session and returns the redirect URL.
    pub async fn create_checkout_session(
        &self,
        price_id: &str,
        tier: &str,
        tenant_id: &str,
        customer_id: Option<&str>,
        customer_email: Option<&str>,
        success_url: &str,
        cancel_url: &str,
    ) -> Result<String, String> {
        let mut params: Vec<(&str, &str)> = vec![
            ("mode", "subscription"),
            ("line_items[0][price]", price_id),
            ("line_items[0][quantity]", "1"),
            ("success_url", success_url),
            ("cancel_url", cancel_url),
            ("metadata[tenant_id]", tenant_id),
            ("metadata[tier]", tier),
            ("allow_promotion_codes", "true"),
            ("billing_address_collection", "auto"),
        ];
        // Prefer existing Stripe customer; fall back to pre-filling email
        if let Some(cid) = customer_id {
            params.push(("customer", cid));
        } else if let Some(email) = customer_email {
            params.push(("customer_email", email));
        }

        let val = self.stripe_post("/v1/checkout/sessions", &params).await?;
        val["url"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "Stripe checkout session missing url field".to_string())
    }

    /// Creates a Customer Portal session and returns the redirect URL.
    pub async fn create_portal_session(
        &self,
        customer_id: &str,
        return_url: &str,
    ) -> Result<String, String> {
        let params = [("customer", customer_id), ("return_url", return_url)];
        let val = self
            .stripe_post("/v1/billing/portal/sessions", &params)
            .await?;
        val["url"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "Stripe portal session missing url field".to_string())
    }

    /// Reports a usage event to a Stripe Billing Meter via the Meter Events API
    /// (`POST /v1/billing/meter_events`).
    ///
    /// `event_name` must match the Stripe meter's configured event name. The
    /// default meter payload maps the customer through `stripe_customer_id` and
    /// reads the quantity from `value`. `identifier` deduplicates retried sends
    /// so an at-least-once caller cannot double-bill within the same meter window.
    pub async fn report_meter_event(
        &self,
        event_name: &str,
        customer_id: &str,
        value: i64,
        identifier: &str,
    ) -> Result<(), String> {
        let value = value.to_string();
        let params = meter_event_params(event_name, identifier, customer_id, value.as_str());
        self.stripe_post("/v1/billing/meter_events", &params)
            .await
            .map(|_| ())
    }

    /// Verifies a `Stripe-Signature` header using HMAC-SHA256.
    ///
    /// Header format: `t=<unix_ts>,v1=<hex_hmac>`
    pub fn verify_webhook_signature(payload: &[u8], sig_header: &str, secret: &str) -> bool {
        let mut ts: &str = "";
        let mut v1_sigs: Vec<&str> = vec![];
        for part in sig_header.split(',') {
            if let Some(v) = part.strip_prefix("t=") {
                ts = v;
            } else if let Some(v) = part.strip_prefix("v1=") {
                v1_sigs.push(v);
            }
        }
        if ts.is_empty() || v1_sigs.is_empty() {
            return false;
        }

        // signed_payload = "<timestamp>.<raw_body>"
        let mut signed: Vec<u8> = ts.as_bytes().to_vec();
        signed.push(b'.');
        signed.extend_from_slice(payload);

        let Ok(mut mac) = HmacSha256::new_from_slice(secret.as_bytes()) else {
            return false;
        };
        mac.update(&signed);
        let computed = hex::encode(mac.finalize().into_bytes());
        v1_sigs
            .iter()
            .any(|s| constant_time_eq(s.as_bytes(), computed.as_bytes()))
    }
}

/// Builds the form parameters for a Stripe Billing Meter Event. Extracted so the
/// Stripe wire contract (default payload keys `stripe_customer_id` / `value`) can
/// be unit-tested without hitting the network.
fn meter_event_params<'a>(
    event_name: &'a str,
    identifier: &'a str,
    customer_id: &'a str,
    value: &'a str,
) -> [(&'static str, &'a str); 4] {
    [
        ("event_name", event_name),
        ("identifier", identifier),
        ("payload[stripe_customer_id]", customer_id),
        ("payload[value]", value),
    ]
}

/// Maps a Stripe Price ID → tier name using env vars.
/// Reads `STRIPE_PRICE_PRO`, `STRIPE_PRICE_BUSINESS`, `STRIPE_PRICE_ENTERPRISE`.
pub fn price_id_to_tier(price_id: &str) -> Option<String> {
    for (env_var, tier) in [
        ("STRIPE_PRICE_PRO", "pro"),
        ("STRIPE_PRICE_BUSINESS", "business"),
        ("STRIPE_PRICE_ENTERPRISE", "enterprise"),
    ] {
        if let Ok(v) = std::env::var(env_var) {
            if !v.is_empty() && price_id == v {
                return Some(tier.to_string());
            }
        }
    }
    None
}

/// Maps a tier name → Stripe Price ID from env vars.
pub fn tier_to_price_id(tier: &str) -> Option<String> {
    let env_var = match tier {
        "pro" => "STRIPE_PRICE_PRO",
        "business" => "STRIPE_PRICE_BUSINESS",
        "enterprise" => "STRIPE_PRICE_ENTERPRISE",
        _ => return None,
    };
    let v = std::env::var(env_var).unwrap_or_default();
    if v.is_empty() {
        None
    } else {
        Some(v)
    }
}

/// Percent-encodes a string for use as an `application/x-www-form-urlencoded` value.
/// Leaves Stripe bracket-notation keys (`[`, `]`) unencoded when used as keys.
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            _ => {
                out.push('%');
                out.push_str(&format!("{:02X}", b));
            }
        }
    }
    out
}

/// Constant-time byte-slice comparison to prevent timing attacks on HMAC comparison.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_webhook_bad_secret_returns_false() {
        let payload = b"test payload";
        let sig = "t=1234,v1=badhash";
        assert!(!StripeClient::verify_webhook_signature(
            payload,
            sig,
            "wrong_secret"
        ));
    }

    #[test]
    fn verify_webhook_valid_signature() {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        type H = Hmac<Sha256>;

        let secret = "whsec_test_secret";
        let ts = "1700000000";
        let payload = b"{\"type\":\"checkout.session.completed\"}";
        let mut signed = ts.as_bytes().to_vec();
        signed.push(b'.');
        signed.extend_from_slice(payload);

        let mut mac = H::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(&signed);
        let sig_hex = hex::encode(mac.finalize().into_bytes());
        let header = format!("t={ts},v1={sig_hex}");

        assert!(StripeClient::verify_webhook_signature(
            payload, &header, secret
        ));
    }

    #[test]
    fn price_to_tier_unknown_returns_none() {
        assert!(price_id_to_tier("price_unknown").is_none());
    }

    #[test]
    fn meter_event_params_shape_matches_stripe_contract() {
        let params = meter_event_params("trigix_tokens", "id-1", "cus_42", "1500");
        assert_eq!(params[0], ("event_name", "trigix_tokens"));
        assert_eq!(params[1], ("identifier", "id-1"));
        assert_eq!(params[2], ("payload[stripe_customer_id]", "cus_42"));
        assert_eq!(params[3], ("payload[value]", "1500"));
    }
}
