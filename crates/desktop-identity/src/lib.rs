// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use ed25519_dalek::{Signer, SigningKey};
use rand_core::OsRng;

const SIGNING_KEY_BYTES: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityError {
    Store(String),
    CorruptIdentity,
}

impl std::fmt::Display for IdentityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Store(message) => write!(f, "device identity store error: {message}"),
            Self::CorruptIdentity => write!(f, "stored device identity is invalid"),
        }
    }
}

impl std::error::Error for IdentityError {}

/// Secure persistence boundary for the device private key.
///
/// Production Windows builds use [`WindowsCredentialStore`]. Tests can provide
/// an isolated implementation without exposing private key bytes to callers.
pub trait DeviceSecretStore {
    fn load(&self) -> Result<Option<String>, IdentityError>;
    fn store(&self, encoded_secret: &str) -> Result<(), IdentityError>;
}

/// A locally held Ed25519 identity. The signing key is deliberately private and
/// the type does not implement `Clone`, `Debug`, or serialization.
pub struct DeviceIdentity {
    signing_key: SigningKey,
}

impl DeviceIdentity {
    /// Loads the existing identity or generates and persists one before it is
    /// returned. Persistence failure fails closed instead of yielding an
    /// ephemeral identity that could be paired but never reused.
    pub fn load_or_create(store: &impl DeviceSecretStore) -> Result<Self, IdentityError> {
        match store.load()? {
            Some(encoded) => Self::from_encoded_secret(&encoded),
            None => {
                let signing_key = SigningKey::generate(&mut OsRng);
                let encoded = URL_SAFE_NO_PAD.encode(signing_key.to_bytes());
                store.store(&encoded)?;
                Ok(Self { signing_key })
            }
        }
    }

    fn from_encoded_secret(encoded: &str) -> Result<Self, IdentityError> {
        let bytes = URL_SAFE_NO_PAD
            .decode(encoded)
            .map_err(|_| IdentityError::CorruptIdentity)?;
        let bytes: [u8; SIGNING_KEY_BYTES] = bytes
            .try_into()
            .map_err(|_| IdentityError::CorruptIdentity)?;
        Ok(Self {
            signing_key: SigningKey::from_bytes(&bytes),
        })
    }

    pub fn public_key(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.signing_key.verifying_key().to_bytes())
    }

    pub fn sign(&self, challenge: &[u8]) -> String {
        URL_SAFE_NO_PAD.encode(self.signing_key.sign(challenge).to_bytes())
    }
}

#[cfg(windows)]
pub struct WindowsCredentialStore {
    service: String,
    account: String,
}

#[cfg(windows)]
impl WindowsCredentialStore {
    pub fn new(device_id: &str) -> Self {
        Self {
            service: "com.trigix.desktop.device-identity".to_string(),
            account: device_id.to_string(),
        }
    }

    fn entry(&self) -> Result<keyring::Entry, IdentityError> {
        keyring::Entry::new(&self.service, &self.account)
            .map_err(|error| IdentityError::Store(error.to_string()))
    }
}

#[cfg(windows)]
impl DeviceSecretStore for WindowsCredentialStore {
    fn load(&self) -> Result<Option<String>, IdentityError> {
        match self.entry()?.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(IdentityError::Store(error.to_string())),
        }
    }

    fn store(&self, encoded_secret: &str) -> Result<(), IdentityError> {
        self.entry()?
            .set_password(encoded_secret)
            .map_err(|error| IdentityError::Store(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    struct MemorySecretStore {
        secret: Mutex<Option<String>>,
        fail_write: bool,
    }

    impl DeviceSecretStore for MemorySecretStore {
        fn load(&self) -> Result<Option<String>, IdentityError> {
            Ok(self.secret.lock().unwrap().clone())
        }

        fn store(&self, encoded_secret: &str) -> Result<(), IdentityError> {
            if self.fail_write {
                return Err(IdentityError::Store("unavailable".to_string()));
            }
            *self.secret.lock().unwrap() = Some(encoded_secret.to_string());
            Ok(())
        }
    }

    #[test]
    fn identity_is_generated_once_and_reloaded() {
        let store = MemorySecretStore::default();
        let first = DeviceIdentity::load_or_create(&store).unwrap();
        let second = DeviceIdentity::load_or_create(&store).unwrap();
        assert_eq!(first.public_key(), second.public_key());
        assert_eq!(first.sign(b"challenge"), second.sign(b"challenge"));
        let persisted = store.secret.lock().unwrap().clone().unwrap();
        assert!(!persisted.contains(&first.public_key()));
    }

    #[test]
    fn persistence_failure_does_not_return_ephemeral_identity() {
        let store = MemorySecretStore {
            fail_write: true,
            ..Default::default()
        };
        assert!(matches!(
            DeviceIdentity::load_or_create(&store),
            Err(IdentityError::Store(_))
        ));
    }

    #[test]
    fn corrupt_persisted_identity_fails_closed() {
        let store = MemorySecretStore::default();
        *store.secret.lock().unwrap() = Some("not-a-key".to_string());
        assert!(matches!(
            DeviceIdentity::load_or_create(&store),
            Err(IdentityError::CorruptIdentity)
        ));
    }
}
