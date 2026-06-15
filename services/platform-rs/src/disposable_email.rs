// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! Disposable / throwaway email-domain detection, used to keep free-tier signups
//! from being farmed with one-shot inboxes.
//!
//! Enabled by default; set `BLOCK_DISPOSABLE_EMAIL=false` to turn it off. The
//! built-in list covers the most common temp-mail providers; extend it without a
//! redeploy via `DISPOSABLE_EMAIL_EXTRA` (comma-separated domains).

/// Built-in set of well-known disposable email domains. Intentionally curated
/// (not exhaustive) — high-traffic temp-mail providers only, so legitimate
/// corporate/consumer domains are never caught.
const BUILTIN_DISPOSABLE: &[&str] = &[
    "mailinator.com",
    "guerrillamail.com",
    "guerrillamail.info",
    "sharklasers.com",
    "10minutemail.com",
    "10minutemail.net",
    "tempmail.com",
    "temp-mail.org",
    "tempmailo.com",
    "throwawaymail.com",
    "yopmail.com",
    "yopmail.fr",
    "getnada.com",
    "trashmail.com",
    "trashmail.de",
    "maildrop.cc",
    "mailnesia.com",
    "fakeinbox.com",
    "dispostable.com",
    "mintemail.com",
    "mohmal.com",
    "emailondeck.com",
    "spam4.me",
    "tempr.email",
    "moakt.com",
    "luxusmail.org",
    "discard.email",
    "mailcatch.com",
    "inboxkitten.com",
    "tmailto.plus",
];

/// Whether disposable-email blocking is active. Defaults to `true`; only an
/// explicit `BLOCK_DISPOSABLE_EMAIL=false` (case-insensitive) disables it.
pub fn blocking_enabled() -> bool {
    match std::env::var("BLOCK_DISPOSABLE_EMAIL") {
        Ok(v) => !matches!(v.trim().to_ascii_lowercase().as_str(), "false" | "0" | "no"),
        Err(_) => true,
    }
}

/// Extra disposable domains from `DISPOSABLE_EMAIL_EXTRA` (comma-separated).
fn extra_domains() -> Vec<String> {
    std::env::var("DISPOSABLE_EMAIL_EXTRA")
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Extracts the lowercased domain part of an email address.
fn domain_of(email: &str) -> Option<String> {
    email
        .rsplit_once('@')
        .map(|(_, d)| d.trim().to_ascii_lowercase())
        .filter(|d| !d.is_empty())
}

/// Returns true if `email`'s domain is a known disposable provider. Matches the
/// exact domain as well as any subdomain of a listed domain.
pub fn is_disposable(email: &str) -> bool {
    let Some(domain) = domain_of(email) else {
        return false;
    };
    let matches = |d: &str| domain == d || domain.ends_with(&format!(".{d}"));
    BUILTIN_DISPOSABLE.iter().any(|d| matches(d)) || extra_domains().iter().any(|d| matches(d))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_known_disposable_domains() {
        assert!(is_disposable("abuser@mailinator.com"));
        assert!(is_disposable("x@guerrillamail.com"));
        // subdomain of a listed domain
        assert!(is_disposable("x@inbox.mailinator.com"));
    }

    #[test]
    fn allows_legitimate_domains() {
        assert!(!is_disposable("alice@example.com"));
        assert!(!is_disposable("user@gmail.com"));
        assert!(!is_disposable("dev@trigix.local"));
        assert!(!is_disposable("not-an-email"));
    }
}
