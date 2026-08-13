// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use desktop_protocol::DeviceDescriptor;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Transaction};

pub const PAIRING_TTL_SECONDS: i64 = 300;
pub const MAX_CLAIM_ATTEMPTS: i32 = 5;

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn hash_secret(secret: &str) -> String {
    hex::encode(Sha256::digest(secret.as_bytes()))
}

fn random_secret(prefix: &str) -> String {
    format!(
        "{prefix}{}_{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

fn random_pairing_code() -> String {
    uuid::Uuid::new_v4().simple().to_string()[..8].to_uppercase()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePairingSessionRequest {
    pub device: DeviceDescriptor,
    pub device_public_key: String,
}

impl CreatePairingSessionRequest {
    pub fn validate(&self) -> Result<(), PairingError> {
        self.device
            .validate()
            .map_err(|e| PairingError::InvalidRequest(e.to_string()))?;
        let key = self.device_public_key.trim();
        if !(32..=4096).contains(&key.len()) || key.contains('\0') {
            return Err(PairingError::InvalidRequest(
                "device_public_key must contain 32 to 4096 safe characters".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PairingSessionCreated {
    pub session_id: String,
    pub pairing_code: String,
    pub claim_secret: String,
    pub expires_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PairedDevice {
    pub id: String,
    pub tenant_id: String,
    pub display_name: String,
    pub operating_system: String,
    pub agent_version: String,
    pub capabilities: Vec<desktop_protocol::DeviceCapability>,
    pub state: String,
    pub paired_by: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeviceCredential {
    pub device_id: String,
    pub tenant_id: String,
    pub credential_id: String,
    pub credential: String,
}

#[derive(Debug, Clone)]
struct PairingSession {
    id: String,
    pairing_code: String,
    claim_secret_hash: String,
    pending_credential: Option<String>,
    credential_id: String,
    device: DeviceDescriptor,
    device_public_key: String,
    tenant_id: Option<String>,
    approved_by: Option<String>,
    status: String,
    attempt_count: i32,
    expires_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PairingError {
    InvalidRequest(String),
    NotFound,
    Expired,
    AlreadyUsed,
    InvalidClaim,
    AttemptsExceeded,
    DeviceConflict,
    Store(String),
}

impl std::fmt::Display for PairingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidRequest(message) => write!(f, "{message}"),
            Self::NotFound => write!(f, "pairing session not found"),
            Self::Expired => write!(f, "pairing session expired"),
            Self::AlreadyUsed => write!(f, "pairing session already used"),
            Self::InvalidClaim => write!(f, "invalid pairing claim"),
            Self::AttemptsExceeded => write!(f, "pairing claim attempts exceeded"),
            Self::DeviceConflict => write!(f, "device identity is already paired"),
            Self::Store(message) => write!(f, "pairing store error: {message}"),
        }
    }
}

impl PairingError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidRequest(_) => "invalid_request",
            Self::NotFound => "not_found",
            Self::Expired => "expired",
            Self::AlreadyUsed => "already_used",
            Self::InvalidClaim => "invalid_claim",
            Self::AttemptsExceeded => "attempts_exceeded",
            Self::DeviceConflict => "device_conflict",
            Self::Store(_) => "store_error",
        }
    }
}

#[derive(Default)]
pub struct MemoryDevicePairingStore {
    sessions: Mutex<HashMap<String, PairingSession>>,
    devices: Mutex<HashMap<String, PairedDevice>>,
}

impl MemoryDevicePairingStore {
    async fn create_session(
        &self,
        request: CreatePairingSessionRequest,
    ) -> Result<PairingSessionCreated, PairingError> {
        request.validate()?;
        let session_id = uuid::Uuid::new_v4().to_string();
        let claim_secret = random_secret("claim_");
        let credential = random_secret("desktop_");
        let credential_id = uuid::Uuid::new_v4().to_string();
        let expires_at = unix_now() + PAIRING_TTL_SECONDS;
        let mut sessions = self.sessions.lock().expect("pairing sessions lock");
        let pairing_code = loop {
            let candidate = random_pairing_code();
            if !sessions.values().any(|s| s.pairing_code == candidate) {
                break candidate;
            }
        };
        sessions.insert(
            session_id.clone(),
            PairingSession {
                id: session_id.clone(),
                pairing_code: pairing_code.clone(),
                claim_secret_hash: hash_secret(&claim_secret),
                pending_credential: Some(credential),
                credential_id,
                device: request.device,
                device_public_key: request.device_public_key,
                tenant_id: None,
                approved_by: None,
                status: "pending".to_string(),
                attempt_count: 0,
                expires_at,
            },
        );
        Ok(PairingSessionCreated {
            session_id,
            pairing_code,
            claim_secret,
            expires_at,
        })
    }

    async fn approve(
        &self,
        pairing_code: &str,
        tenant_id: &str,
        actor_id: &str,
    ) -> Result<PairedDevice, PairingError> {
        let now = unix_now();
        let mut sessions = self.sessions.lock().expect("pairing sessions lock");
        let session = sessions
            .values_mut()
            .find(|session| session.pairing_code == pairing_code)
            .ok_or(PairingError::NotFound)?;
        if session.expires_at <= now {
            session.status = "expired".to_string();
            return Err(PairingError::Expired);
        }
        if session.status != "pending" {
            return Err(PairingError::AlreadyUsed);
        }
        let mut devices = self.devices.lock().expect("paired devices lock");
        if devices.contains_key(&session.device.device_id) {
            return Err(PairingError::DeviceConflict);
        }
        let device = PairedDevice {
            id: session.device.device_id.clone(),
            tenant_id: tenant_id.to_string(),
            display_name: session.device.display_name.clone(),
            operating_system: session.device.operating_system.clone(),
            agent_version: session.device.agent_version.clone(),
            capabilities: session.device.capabilities.clone(),
            state: "paired".to_string(),
            paired_by: actor_id.to_string(),
            created_at: now,
        };
        devices.insert(device.id.clone(), device.clone());
        session.tenant_id = Some(tenant_id.to_string());
        session.approved_by = Some(actor_id.to_string());
        session.status = "approved".to_string();
        Ok(device)
    }

    async fn claim(
        &self,
        session_id: &str,
        claim_secret: &str,
    ) -> Result<DeviceCredential, PairingError> {
        let now = unix_now();
        let mut sessions = self.sessions.lock().expect("pairing sessions lock");
        let session = sessions.get_mut(session_id).ok_or(PairingError::NotFound)?;
        if session.expires_at <= now {
            session.status = "expired".to_string();
            return Err(PairingError::Expired);
        }
        if session.attempt_count >= MAX_CLAIM_ATTEMPTS {
            return Err(PairingError::AttemptsExceeded);
        }
        if session.claim_secret_hash != hash_secret(claim_secret) {
            session.attempt_count += 1;
            if session.attempt_count >= MAX_CLAIM_ATTEMPTS {
                session.status = "failed".to_string();
                return Err(PairingError::AttemptsExceeded);
            }
            return Err(PairingError::InvalidClaim);
        }
        if session.status != "approved" {
            return Err(if session.status == "claimed" {
                PairingError::AlreadyUsed
            } else {
                PairingError::InvalidClaim
            });
        }
        let credential = session
            .pending_credential
            .take()
            .ok_or(PairingError::AlreadyUsed)?;
        session.status = "claimed".to_string();
        Ok(DeviceCredential {
            device_id: session.device.device_id.clone(),
            tenant_id: session.tenant_id.clone().ok_or_else(|| {
                PairingError::Store("approved session is missing tenant".to_string())
            })?,
            credential_id: session.credential_id.clone(),
            credential,
        })
    }
}

#[derive(Clone)]
pub struct PostgresDevicePairingStore {
    pool: PgPool,
}

impl PostgresDevicePairingStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn create_session(
        &self,
        request: CreatePairingSessionRequest,
    ) -> Result<PairingSessionCreated, PairingError> {
        request.validate()?;
        let session_id = uuid::Uuid::new_v4().to_string();
        let claim_secret = random_secret("claim_");
        let credential = random_secret("desktop_");
        let credential_id = uuid::Uuid::new_v4().to_string();
        let expires_at = unix_now() + PAIRING_TTL_SECONDS;
        let device_json = serde_json::to_value(&request.device)
            .map_err(|e| PairingError::InvalidRequest(e.to_string()))?;

        for _ in 0..5 {
            let pairing_code = random_pairing_code();
            let result = sqlx::query(
                r#"INSERT INTO af_desktop_pairing_sessions
                   (id, pairing_code, claim_secret_hash, pending_credential_ciphertext,
                    credential_id, device_json, device_public_key, status, attempt_count,
                    expires_at)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, 'pending', 0,
                           to_timestamp($8))
                   ON CONFLICT (pairing_code) DO NOTHING"#,
            )
            .bind(&session_id)
            .bind(&pairing_code)
            .bind(hash_secret(&claim_secret))
            .bind(crate::crypto::encrypt(&credential))
            .bind(&credential_id)
            .bind(&device_json)
            .bind(request.device_public_key.trim())
            .bind(expires_at)
            .execute(&self.pool)
            .await
            .map_err(|e| PairingError::Store(e.to_string()))?;
            if result.rows_affected() == 1 {
                return Ok(PairingSessionCreated {
                    session_id,
                    pairing_code,
                    claim_secret,
                    expires_at,
                });
            }
        }
        Err(PairingError::Store(
            "could not allocate a unique pairing code".to_string(),
        ))
    }

    async fn load_session_for_update(
        tx: &mut Transaction<'_, Postgres>,
        column: &str,
        value: &str,
    ) -> Result<PairingSession, PairingError> {
        let sql = format!(
            r#"SELECT id, pairing_code, claim_secret_hash, pending_credential_ciphertext,
                      credential_id, device_json, device_public_key, tenant_id, approved_by,
                      status, attempt_count, EXTRACT(EPOCH FROM expires_at)::BIGINT AS expires_at
               FROM af_desktop_pairing_sessions WHERE {column} = $1 FOR UPDATE"#
        );
        let row: Option<PairingSessionRow> = sqlx::query_as(&sql)
            .bind(value)
            .fetch_optional(&mut **tx)
            .await
            .map_err(|e| PairingError::Store(e.to_string()))?;
        row.map(TryInto::try_into)
            .transpose()?
            .ok_or(PairingError::NotFound)
    }

    async fn approve(
        &self,
        pairing_code: &str,
        tenant_id: &str,
        actor_id: &str,
    ) -> Result<PairedDevice, PairingError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| PairingError::Store(e.to_string()))?;
        let mut session =
            Self::load_session_for_update(&mut tx, "pairing_code", pairing_code).await?;
        let now = unix_now();
        if session.expires_at <= now {
            sqlx::query("UPDATE af_desktop_pairing_sessions SET status = 'expired' WHERE id = $1")
                .bind(&session.id)
                .execute(&mut *tx)
                .await
                .map_err(|e| PairingError::Store(e.to_string()))?;
            tx.commit()
                .await
                .map_err(|e| PairingError::Store(e.to_string()))?;
            return Err(PairingError::Expired);
        }
        if session.status != "pending" {
            return Err(PairingError::AlreadyUsed);
        }
        let capabilities = serde_json::to_value(&session.device.capabilities)
            .map_err(|e| PairingError::Store(e.to_string()))?;
        let pending = session.pending_credential.as_deref().ok_or_else(|| {
            PairingError::Store("pending session is missing credential".to_string())
        })?;
        let credential = crate::crypto::decrypt(pending);
        let inserted = sqlx::query(
            r#"INSERT INTO af_desktop_devices
               (id, tenant_id, display_name, operating_system, agent_version,
                capabilities_json, public_key, credential_id, credential_hash,
                state, paired_by)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'paired', $10)
               ON CONFLICT DO NOTHING"#,
        )
        .bind(&session.device.device_id)
        .bind(tenant_id)
        .bind(&session.device.display_name)
        .bind(&session.device.operating_system)
        .bind(&session.device.agent_version)
        .bind(capabilities)
        .bind(session.device_public_key.trim())
        .bind(&session.credential_id)
        .bind(hash_secret(&credential))
        .bind(actor_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| PairingError::Store(e.to_string()))?;
        if inserted.rows_affected() != 1 {
            return Err(PairingError::DeviceConflict);
        }
        sqlx::query(
            r#"UPDATE af_desktop_pairing_sessions
               SET tenant_id = $2, approved_by = $3, approved_at = now(), status = 'approved'
               WHERE id = $1"#,
        )
        .bind(&session.id)
        .bind(tenant_id)
        .bind(actor_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| PairingError::Store(e.to_string()))?;
        tx.commit()
            .await
            .map_err(|e| PairingError::Store(e.to_string()))?;
        session.tenant_id = Some(tenant_id.to_string());
        Ok(PairedDevice {
            id: session.device.device_id,
            tenant_id: tenant_id.to_string(),
            display_name: session.device.display_name,
            operating_system: session.device.operating_system,
            agent_version: session.device.agent_version,
            capabilities: session.device.capabilities,
            state: "paired".to_string(),
            paired_by: actor_id.to_string(),
            created_at: now,
        })
    }

    async fn claim(
        &self,
        session_id: &str,
        claim_secret: &str,
    ) -> Result<DeviceCredential, PairingError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| PairingError::Store(e.to_string()))?;
        let session = Self::load_session_for_update(&mut tx, "id", session_id).await?;
        if session.expires_at <= unix_now() {
            sqlx::query("UPDATE af_desktop_pairing_sessions SET status = 'expired' WHERE id = $1")
                .bind(&session.id)
                .execute(&mut *tx)
                .await
                .map_err(|e| PairingError::Store(e.to_string()))?;
            tx.commit()
                .await
                .map_err(|e| PairingError::Store(e.to_string()))?;
            return Err(PairingError::Expired);
        }
        if session.attempt_count >= MAX_CLAIM_ATTEMPTS {
            return Err(PairingError::AttemptsExceeded);
        }
        if session.claim_secret_hash != hash_secret(claim_secret) {
            let attempts = session.attempt_count + 1;
            let status = if attempts >= MAX_CLAIM_ATTEMPTS {
                "failed"
            } else {
                session.status.as_str()
            };
            sqlx::query(
                "UPDATE af_desktop_pairing_sessions SET attempt_count = $2, status = $3 WHERE id = $1",
            )
            .bind(&session.id)
            .bind(attempts)
            .bind(status)
            .execute(&mut *tx)
            .await
            .map_err(|e| PairingError::Store(e.to_string()))?;
            tx.commit()
                .await
                .map_err(|e| PairingError::Store(e.to_string()))?;
            return Err(if attempts >= MAX_CLAIM_ATTEMPTS {
                PairingError::AttemptsExceeded
            } else {
                PairingError::InvalidClaim
            });
        }
        if session.status != "approved" {
            return Err(if session.status == "claimed" {
                PairingError::AlreadyUsed
            } else {
                PairingError::InvalidClaim
            });
        }
        let ciphertext = session
            .pending_credential
            .as_deref()
            .ok_or(PairingError::AlreadyUsed)?;
        let credential = crate::crypto::decrypt(ciphertext);
        sqlx::query(
            r#"UPDATE af_desktop_pairing_sessions
               SET status = 'claimed', claimed_at = now(), pending_credential_ciphertext = NULL
               WHERE id = $1"#,
        )
        .bind(&session.id)
        .execute(&mut *tx)
        .await
        .map_err(|e| PairingError::Store(e.to_string()))?;
        tx.commit()
            .await
            .map_err(|e| PairingError::Store(e.to_string()))?;
        Ok(DeviceCredential {
            device_id: session.device.device_id,
            tenant_id: session.tenant_id.ok_or_else(|| {
                PairingError::Store("approved session is missing tenant".to_string())
            })?,
            credential_id: session.credential_id,
            credential,
        })
    }
}

#[derive(sqlx::FromRow)]
struct PairingSessionRow {
    id: String,
    pairing_code: String,
    claim_secret_hash: String,
    pending_credential_ciphertext: Option<String>,
    credential_id: String,
    device_json: serde_json::Value,
    device_public_key: String,
    tenant_id: Option<String>,
    approved_by: Option<String>,
    status: String,
    attempt_count: i32,
    expires_at: i64,
}

impl TryFrom<PairingSessionRow> for PairingSession {
    type Error = PairingError;

    fn try_from(row: PairingSessionRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            pairing_code: row.pairing_code,
            claim_secret_hash: row.claim_secret_hash,
            pending_credential: row.pending_credential_ciphertext,
            credential_id: row.credential_id,
            device: serde_json::from_value(row.device_json)
                .map_err(|e| PairingError::Store(e.to_string()))?,
            device_public_key: row.device_public_key,
            tenant_id: row.tenant_id,
            approved_by: row.approved_by,
            status: row.status,
            attempt_count: row.attempt_count,
            expires_at: row.expires_at,
        })
    }
}

#[derive(Clone)]
pub enum PlatformDevicePairingStore {
    Memory(Arc<MemoryDevicePairingStore>),
    Postgres(PostgresDevicePairingStore),
}

impl Default for PlatformDevicePairingStore {
    fn default() -> Self {
        Self::Memory(Arc::new(MemoryDevicePairingStore::default()))
    }
}

impl PlatformDevicePairingStore {
    pub fn postgres(pool: PgPool) -> Self {
        Self::Postgres(PostgresDevicePairingStore::new(pool))
    }

    pub async fn create_session(
        &self,
        request: CreatePairingSessionRequest,
    ) -> Result<PairingSessionCreated, PairingError> {
        match self {
            Self::Memory(store) => store.create_session(request).await,
            Self::Postgres(store) => store.create_session(request).await,
        }
    }

    pub async fn approve(
        &self,
        pairing_code: &str,
        tenant_id: &str,
        actor_id: &str,
    ) -> Result<PairedDevice, PairingError> {
        match self {
            Self::Memory(store) => store.approve(pairing_code, tenant_id, actor_id).await,
            Self::Postgres(store) => store.approve(pairing_code, tenant_id, actor_id).await,
        }
    }

    pub async fn claim(
        &self,
        session_id: &str,
        claim_secret: &str,
    ) -> Result<DeviceCredential, PairingError> {
        match self {
            Self::Memory(store) => store.claim(session_id, claim_secret).await,
            Self::Postgres(store) => store.claim(session_id, claim_secret).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use desktop_protocol::DeviceCapability;

    fn request(device_id: &str) -> CreatePairingSessionRequest {
        CreatePairingSessionRequest {
            device: DeviceDescriptor {
                device_id: device_id.to_string(),
                display_name: "Test Device".to_string(),
                operating_system: "windows".to_string(),
                agent_version: "1.0.0".to_string(),
                capabilities: vec![DeviceCapability::SystemInformation],
            },
            device_public_key: "A".repeat(64),
        }
    }

    #[tokio::test]
    async fn pairing_is_single_use_and_credential_is_device_only() {
        let store = PlatformDevicePairingStore::default();
        let session = store.create_session(request("device-1")).await.unwrap();
        let device = store
            .approve(&session.pairing_code, "tenant-1", "admin-1")
            .await
            .unwrap();
        assert_eq!(device.tenant_id, "tenant-1");
        assert!(matches!(
            store
                .approve(&session.pairing_code, "tenant-2", "admin-2")
                .await,
            Err(PairingError::AlreadyUsed)
        ));
        let credential = store
            .claim(&session.session_id, &session.claim_secret)
            .await
            .unwrap();
        assert!(credential.credential.starts_with("desktop_"));
        assert!(matches!(
            store
                .claim(&session.session_id, &session.claim_secret)
                .await,
            Err(PairingError::AlreadyUsed)
        ));
    }

    #[tokio::test]
    async fn invalid_claims_are_bounded() {
        let store = PlatformDevicePairingStore::default();
        let session = store.create_session(request("device-2")).await.unwrap();
        store
            .approve(&session.pairing_code, "tenant-1", "admin-1")
            .await
            .unwrap();
        for _ in 0..MAX_CLAIM_ATTEMPTS - 1 {
            assert!(matches!(
                store.claim(&session.session_id, "wrong").await,
                Err(PairingError::InvalidClaim)
            ));
        }
        assert!(matches!(
            store.claim(&session.session_id, "wrong").await,
            Err(PairingError::AttemptsExceeded)
        ));
        assert!(matches!(
            store
                .claim(&session.session_id, &session.claim_secret)
                .await,
            Err(PairingError::AttemptsExceeded)
        ));
    }

    #[tokio::test]
    async fn concurrent_approval_has_one_winner() {
        let store = Arc::new(PlatformDevicePairingStore::default());
        let session = store.create_session(request("device-3")).await.unwrap();
        let first_store = Arc::clone(&store);
        let second_store = Arc::clone(&store);
        let first_code = session.pairing_code.clone();
        let second_code = session.pairing_code;
        let (first, second) = tokio::join!(
            first_store.approve(&first_code, "tenant-1", "admin-1"),
            second_store.approve(&second_code, "tenant-2", "admin-2")
        );
        assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
    }

    #[tokio::test]
    async fn expired_session_fails_closed() {
        let store = PlatformDevicePairingStore::default();
        let session = store.create_session(request("device-4")).await.unwrap();
        let PlatformDevicePairingStore::Memory(memory) = &store else {
            unreachable!();
        };
        memory
            .sessions
            .lock()
            .unwrap()
            .get_mut(&session.session_id)
            .unwrap()
            .expires_at = unix_now() - 1;
        assert!(matches!(
            store
                .approve(&session.pairing_code, "tenant-1", "admin-1")
                .await,
            Err(PairingError::Expired)
        ));
        assert!(matches!(
            store
                .claim(&session.session_id, &session.claim_secret)
                .await,
            Err(PairingError::Expired)
        ));
    }
}
