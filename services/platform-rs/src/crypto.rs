// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! Symmetric encryption-at-rest for stored secrets (credential values and SSO
//! client secrets). AES-256-GCM with a key derived from `CREDENTIAL_MASTER_KEY`.
//!
//! Design notes:
//! - When `CREDENTIAL_MASTER_KEY` is unset (dev), values are stored as-is.
//! - `decrypt` transparently passes through legacy plaintext (anything without
//!   the `enc:v1:` marker), so rows written before this feature keep working —
//!   no data migration required.
//! - Each ciphertext carries its own random 96-bit nonce.

use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce};
use base64::Engine as _;
use sha2::{Digest, Sha256};

const PREFIX: &str = "enc:v1:";
const B64: base64::engine::general_purpose::GeneralPurpose =
    base64::engine::general_purpose::STANDARD;

/// Derive a 32-byte key from whatever the operator put in CREDENTIAL_MASTER_KEY
/// (SHA-256, so any length/format works). `None` disables encryption.
fn master_key() -> Option<[u8; 32]> {
    let raw = std::env::var("CREDENTIAL_MASTER_KEY").ok()?;
    if raw.trim().is_empty() {
        return None;
    }
    Some(Sha256::digest(raw.as_bytes()).into())
}

fn encrypt_with_key(key: &[u8; 32], plaintext: &str) -> Option<String> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    // 96-bit random nonce from a v4 UUID (122 bits of entropy).
    let uuid = uuid::Uuid::new_v4();
    let nonce_bytes = &uuid.as_bytes()[..12];
    let nonce = Nonce::from_slice(nonce_bytes);
    let ct = cipher.encrypt(nonce, plaintext.as_bytes()).ok()?;
    let mut blob = nonce_bytes.to_vec();
    blob.extend_from_slice(&ct);
    Some(format!("{PREFIX}{}", B64.encode(blob)))
}

fn decrypt_with_key(key: &[u8; 32], stored: &str) -> String {
    let Some(rest) = stored.strip_prefix(PREFIX) else {
        return stored.to_string(); // legacy plaintext
    };
    let blob = match B64.decode(rest) {
        Ok(b) if b.len() > 12 => b,
        _ => return stored.to_string(),
    };
    let (nonce_bytes, ct) = blob.split_at(12);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    match cipher.decrypt(Nonce::from_slice(nonce_bytes), ct) {
        Ok(pt) => String::from_utf8_lossy(&pt).to_string(),
        Err(_) => stored.to_string(),
    }
}

/// Encrypt a secret for at-rest storage. Returns the plaintext unchanged when no
/// master key is configured.
pub fn encrypt(plaintext: &str) -> String {
    match master_key() {
        Some(key) => encrypt_with_key(&key, plaintext).unwrap_or_else(|| plaintext.to_string()),
        None => plaintext.to_string(),
    }
}

/// Decrypt a stored secret. Legacy plaintext (no `enc:v1:` marker) is returned
/// unchanged; an encrypted value with no key configured is returned as-is.
pub fn decrypt(stored: &str) -> String {
    if !stored.starts_with(PREFIX) {
        return stored.to_string();
    }
    match master_key() {
        Some(key) => decrypt_with_key(&key, stored),
        None => stored.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> [u8; 32] {
        Sha256::digest(b"unit-test-master-key").into()
    }

    #[test]
    fn round_trips() {
        let k = test_key();
        let secret = "sk-ant-super-secret-🔐";
        let enc = encrypt_with_key(&k, secret).unwrap();
        assert!(enc.starts_with(PREFIX));
        assert!(!enc.contains(secret));
        assert_eq!(decrypt_with_key(&k, &enc), secret);
    }

    #[test]
    fn distinct_nonces_per_encryption() {
        let k = test_key();
        let a = encrypt_with_key(&k, "same").unwrap();
        let b = encrypt_with_key(&k, "same").unwrap();
        assert_ne!(a, b, "ciphertexts must differ (random nonce)");
        assert_eq!(decrypt_with_key(&k, &a), "same");
        assert_eq!(decrypt_with_key(&k, &b), "same");
    }

    #[test]
    fn legacy_plaintext_passes_through() {
        let k = test_key();
        assert_eq!(decrypt_with_key(&k, "plain-old-secret"), "plain-old-secret");
    }

    #[test]
    fn wrong_key_does_not_yield_plaintext() {
        let k1 = test_key();
        let k2: [u8; 32] = Sha256::digest(b"different-key").into();
        let enc = encrypt_with_key(&k1, "topsecret").unwrap();
        // Decrypting with the wrong key fails closed (never returns the secret).
        assert_ne!(decrypt_with_key(&k2, &enc), "topsecret");
    }

    #[test]
    fn tampered_ciphertext_is_rejected() {
        let k = test_key();
        let enc = encrypt_with_key(&k, "secret").unwrap();
        let mut bad = enc.clone();
        bad.push('A'); // corrupt the base64 tail
        assert_ne!(decrypt_with_key(&k, &bad), "secret");
    }
}
