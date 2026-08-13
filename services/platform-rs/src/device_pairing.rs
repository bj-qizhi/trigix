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
pub const DEVICE_STALE_AFTER_SECONDS: i64 = 90;

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
    pub updated_at: i64,
    pub last_seen_at: Option<i64>,
    pub stale: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeviceList {
    pub items: Vec<PairedDevice>,
    pub next_offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CredentialRotationStarted {
    pub device_id: String,
    pub rotation_id: String,
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
    InvalidState,
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
            Self::InvalidState => write!(f, "device state does not allow this operation"),
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
            Self::InvalidState => "invalid_state",
            Self::Store(_) => "store_error",
        }
    }
}

#[derive(Default)]
pub struct MemoryDevicePairingStore {
    sessions: Mutex<HashMap<String, PairingSession>>,
    devices: Mutex<HashMap<String, StoredDevice>>,
}

#[derive(Debug, Clone)]
struct StoredDevice {
    view: PairedDevice,
    credential_id: String,
    credential_hash: String,
    pending_rotation: Option<(String, String)>,
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
            updated_at: now,
            last_seen_at: None,
            stale: false,
        };
        let credential = session.pending_credential.as_deref().ok_or_else(|| {
            PairingError::Store("pending session is missing credential".to_string())
        })?;
        devices.insert(
            device.id.clone(),
            StoredDevice {
                view: device.clone(),
                credential_id: session.credential_id.clone(),
                credential_hash: hash_secret(credential),
                pending_rotation: None,
            },
        );
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

    async fn list_devices(
        &self,
        tenant_id: &str,
        state: Option<&str>,
        limit: u32,
        offset: u32,
    ) -> DeviceList {
        let mut devices: Vec<_> = self
            .devices
            .lock()
            .expect("paired devices lock")
            .values()
            .filter(|stored| {
                stored.view.tenant_id == tenant_id
                    && state.is_none_or(|expected| stored.view.state == expected)
            })
            .map(|stored| stored.view.clone())
            .collect();
        devices.sort_by(|left, right| {
            right
                .created_at
                .cmp(&left.created_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        let total = devices.len();
        let items = devices
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .collect::<Vec<_>>();
        let consumed = offset as usize + items.len();
        DeviceList {
            items,
            next_offset: (consumed < total).then_some(consumed.min(u32::MAX as usize) as u32),
        }
    }

    async fn get_device(&self, tenant_id: &str, device_id: &str) -> Option<PairedDevice> {
        self.devices
            .lock()
            .expect("paired devices lock")
            .get(device_id)
            .filter(|stored| stored.view.tenant_id == tenant_id)
            .map(|stored| stored.view.clone())
    }

    async fn rename_device(
        &self,
        tenant_id: &str,
        device_id: &str,
        display_name: &str,
    ) -> Result<PairedDevice, PairingError> {
        let mut devices = self.devices.lock().expect("paired devices lock");
        let stored = devices
            .get_mut(device_id)
            .filter(|stored| stored.view.tenant_id == tenant_id)
            .ok_or(PairingError::NotFound)?;
        stored.view.display_name = display_name.to_string();
        stored.view.updated_at = unix_now();
        Ok(stored.view.clone())
    }

    async fn set_device_state(
        &self,
        tenant_id: &str,
        device_id: &str,
        state: &str,
    ) -> Result<PairedDevice, PairingError> {
        let mut devices = self.devices.lock().expect("paired devices lock");
        let stored = devices
            .get_mut(device_id)
            .filter(|stored| stored.view.tenant_id == tenant_id)
            .ok_or(PairingError::NotFound)?;
        if stored.view.state == "revoked" || !matches!(state, "suspended" | "revoked") {
            return Err(PairingError::InvalidState);
        }
        stored.view.state = state.to_string();
        stored.view.updated_at = unix_now();
        if state == "revoked" {
            stored.credential_hash.clear();
            stored.pending_rotation = None;
        }
        Ok(stored.view.clone())
    }

    async fn start_rotation(
        &self,
        tenant_id: &str,
        device_id: &str,
    ) -> Result<CredentialRotationStarted, PairingError> {
        let mut devices = self.devices.lock().expect("paired devices lock");
        let stored = devices
            .get_mut(device_id)
            .filter(|stored| stored.view.tenant_id == tenant_id)
            .ok_or(PairingError::NotFound)?;
        if stored.view.state == "revoked" {
            return Err(PairingError::InvalidState);
        }
        let rotation_id = uuid::Uuid::new_v4().to_string();
        stored.pending_rotation = Some((rotation_id.clone(), random_secret("desktop_")));
        stored.view.updated_at = unix_now();
        Ok(CredentialRotationStarted {
            device_id: device_id.to_string(),
            rotation_id,
        })
    }

    async fn claim_rotation(
        &self,
        device_id: &str,
        current_credential: &str,
    ) -> Result<DeviceCredential, PairingError> {
        let mut devices = self.devices.lock().expect("paired devices lock");
        let stored = devices.get_mut(device_id).ok_or(PairingError::NotFound)?;
        if stored.view.state == "revoked"
            || stored.credential_hash != hash_secret(current_credential)
        {
            return Err(PairingError::InvalidClaim);
        }
        let (_, credential) = stored
            .pending_rotation
            .take()
            .ok_or(PairingError::InvalidState)?;
        let credential_id = uuid::Uuid::new_v4().to_string();
        stored.credential_hash = hash_secret(&credential);
        stored.credential_id = credential_id.clone();
        stored.view.updated_at = unix_now();
        Ok(DeviceCredential {
            device_id: device_id.to_string(),
            tenant_id: stored.view.tenant_id.clone(),
            credential_id,
            credential,
        })
    }

    async fn authenticate_device(
        &self,
        device_id: &str,
        credential: &str,
    ) -> Result<PairedDevice, PairingError> {
        let devices = self.devices.lock().expect("paired devices lock");
        let stored = devices.get(device_id).ok_or(PairingError::InvalidClaim)?;
        if matches!(stored.view.state.as_str(), "suspended" | "revoked") {
            return Err(PairingError::InvalidState);
        }
        if stored.credential_hash != hash_secret(credential) {
            return Err(PairingError::InvalidClaim);
        }
        Ok(stored.view.clone())
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
            updated_at: now,
            last_seen_at: None,
            stale: false,
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

    async fn list_devices(
        &self,
        tenant_id: &str,
        state: Option<&str>,
        limit: u32,
        offset: u32,
    ) -> Result<DeviceList, PairingError> {
        let rows: Vec<DeviceRow> = sqlx::query_as(
            r#"SELECT id, tenant_id, display_name, operating_system, agent_version,
                      capabilities_json, state, paired_by,
                      EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at,
                      EXTRACT(EPOCH FROM updated_at)::BIGINT AS updated_at,
                      EXTRACT(EPOCH FROM last_seen_at)::BIGINT AS last_seen_at
               FROM af_desktop_devices
               WHERE tenant_id = $1 AND ($2::TEXT IS NULL OR state = $2)
               ORDER BY created_at DESC, id ASC LIMIT $3 OFFSET $4"#,
        )
        .bind(tenant_id)
        .bind(state)
        .bind(i64::from(limit) + 1)
        .bind(i64::from(offset))
        .fetch_all(&self.pool)
        .await
        .map_err(|e| PairingError::Store(e.to_string()))?;
        let has_more = rows.len() > limit as usize;
        let items = rows
            .into_iter()
            .take(limit as usize)
            .map(TryInto::try_into)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(DeviceList {
            next_offset: has_more.then_some(offset.saturating_add(items.len() as u32)),
            items,
        })
    }

    async fn get_device(
        &self,
        tenant_id: &str,
        device_id: &str,
    ) -> Result<Option<PairedDevice>, PairingError> {
        let row: Option<DeviceRow> = sqlx::query_as(
            r#"SELECT id, tenant_id, display_name, operating_system, agent_version,
                      capabilities_json, state, paired_by,
                      EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at,
                      EXTRACT(EPOCH FROM updated_at)::BIGINT AS updated_at,
                      EXTRACT(EPOCH FROM last_seen_at)::BIGINT AS last_seen_at
               FROM af_desktop_devices WHERE tenant_id = $1 AND id = $2"#,
        )
        .bind(tenant_id)
        .bind(device_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PairingError::Store(e.to_string()))?;
        row.map(TryInto::try_into).transpose()
    }

    async fn rename_device(
        &self,
        tenant_id: &str,
        device_id: &str,
        display_name: &str,
    ) -> Result<PairedDevice, PairingError> {
        sqlx::query(
            r#"UPDATE af_desktop_devices SET display_name = $3, updated_at = now()
               WHERE tenant_id = $1 AND id = $2"#,
        )
        .bind(tenant_id)
        .bind(device_id)
        .bind(display_name)
        .execute(&self.pool)
        .await
        .map_err(|e| PairingError::Store(e.to_string()))?
        .rows_affected()
        .eq(&1)
        .then_some(())
        .ok_or(PairingError::NotFound)?;
        self.get_device(tenant_id, device_id)
            .await?
            .ok_or(PairingError::NotFound)
    }

    async fn set_device_state(
        &self,
        tenant_id: &str,
        device_id: &str,
        state: &str,
    ) -> Result<PairedDevice, PairingError> {
        if !matches!(state, "suspended" | "revoked") {
            return Err(PairingError::InvalidState);
        }
        let result = if state == "revoked" {
            sqlx::query(
                r#"UPDATE af_desktop_devices
                   SET state = 'revoked', credential_hash = '',
                       pending_rotation_id = NULL, pending_credential_ciphertext = NULL,
                       updated_at = now()
                   WHERE tenant_id = $1 AND id = $2 AND state <> 'revoked'"#,
            )
            .bind(tenant_id)
            .bind(device_id)
            .execute(&self.pool)
            .await
        } else {
            sqlx::query(
                r#"UPDATE af_desktop_devices SET state = 'suspended', updated_at = now()
                   WHERE tenant_id = $1 AND id = $2 AND state <> 'revoked'"#,
            )
            .bind(tenant_id)
            .bind(device_id)
            .execute(&self.pool)
            .await
        }
        .map_err(|e| PairingError::Store(e.to_string()))?;
        if result.rows_affected() != 1 {
            return if self.get_device(tenant_id, device_id).await?.is_some() {
                Err(PairingError::InvalidState)
            } else {
                Err(PairingError::NotFound)
            };
        }
        self.get_device(tenant_id, device_id)
            .await?
            .ok_or(PairingError::NotFound)
    }

    async fn start_rotation(
        &self,
        tenant_id: &str,
        device_id: &str,
    ) -> Result<CredentialRotationStarted, PairingError> {
        let rotation_id = uuid::Uuid::new_v4().to_string();
        let credential = random_secret("desktop_");
        let result = sqlx::query(
            r#"UPDATE af_desktop_devices
               SET pending_rotation_id = $3, pending_credential_ciphertext = $4,
                   updated_at = now()
               WHERE tenant_id = $1 AND id = $2 AND state <> 'revoked'"#,
        )
        .bind(tenant_id)
        .bind(device_id)
        .bind(&rotation_id)
        .bind(crate::crypto::encrypt(&credential))
        .execute(&self.pool)
        .await
        .map_err(|e| PairingError::Store(e.to_string()))?;
        if result.rows_affected() != 1 {
            return if self.get_device(tenant_id, device_id).await?.is_some() {
                Err(PairingError::InvalidState)
            } else {
                Err(PairingError::NotFound)
            };
        }
        Ok(CredentialRotationStarted {
            device_id: device_id.to_string(),
            rotation_id,
        })
    }

    async fn claim_rotation(
        &self,
        device_id: &str,
        current_credential: &str,
    ) -> Result<DeviceCredential, PairingError> {
        #[derive(sqlx::FromRow)]
        struct RotationRow {
            tenant_id: String,
            credential_hash: String,
            pending_rotation_id: Option<String>,
            pending_credential_ciphertext: Option<String>,
            state: String,
        }

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| PairingError::Store(e.to_string()))?;
        let row: RotationRow = sqlx::query_as(
            r#"SELECT tenant_id, credential_hash, pending_rotation_id,
                      pending_credential_ciphertext, state
               FROM af_desktop_devices WHERE id = $1 FOR UPDATE"#,
        )
        .bind(device_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| PairingError::Store(e.to_string()))?
        .ok_or(PairingError::NotFound)?;
        if row.state == "revoked" || row.credential_hash != hash_secret(current_credential) {
            return Err(PairingError::InvalidClaim);
        }
        let _rotation_id = row.pending_rotation_id.ok_or(PairingError::InvalidState)?;
        let ciphertext = row
            .pending_credential_ciphertext
            .ok_or(PairingError::InvalidState)?;
        let credential = crate::crypto::decrypt(&ciphertext);
        let credential_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            r#"UPDATE af_desktop_devices
               SET credential_id = $2, credential_hash = $3,
                   pending_rotation_id = NULL, pending_credential_ciphertext = NULL,
                   updated_at = now()
               WHERE id = $1"#,
        )
        .bind(device_id)
        .bind(&credential_id)
        .bind(hash_secret(&credential))
        .execute(&mut *tx)
        .await
        .map_err(|e| PairingError::Store(e.to_string()))?;
        tx.commit()
            .await
            .map_err(|e| PairingError::Store(e.to_string()))?;
        Ok(DeviceCredential {
            device_id: device_id.to_string(),
            tenant_id: row.tenant_id,
            credential_id,
            credential,
        })
    }

    async fn authenticate_device(
        &self,
        device_id: &str,
        credential: &str,
    ) -> Result<PairedDevice, PairingError> {
        let row: Option<DeviceRow> = sqlx::query_as(
            r#"SELECT id, tenant_id, display_name, operating_system, agent_version,
                      capabilities_json, state, paired_by,
                      EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at,
                      EXTRACT(EPOCH FROM updated_at)::BIGINT AS updated_at,
                      EXTRACT(EPOCH FROM last_seen_at)::BIGINT AS last_seen_at
               FROM af_desktop_devices
               WHERE id = $1 AND credential_hash = $2
                 AND state NOT IN ('suspended', 'revoked')"#,
        )
        .bind(device_id)
        .bind(hash_secret(credential))
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PairingError::Store(e.to_string()))?;
        row.map(TryInto::try_into)
            .transpose()?
            .ok_or(PairingError::InvalidClaim)
    }
}

#[derive(sqlx::FromRow)]
struct DeviceRow {
    id: String,
    tenant_id: String,
    display_name: String,
    operating_system: String,
    agent_version: String,
    capabilities_json: serde_json::Value,
    state: String,
    paired_by: String,
    created_at: i64,
    updated_at: i64,
    last_seen_at: Option<i64>,
}

impl TryFrom<DeviceRow> for PairedDevice {
    type Error = PairingError;

    fn try_from(row: DeviceRow) -> Result<Self, Self::Error> {
        let stale = matches!(row.state.as_str(), "online" | "offline")
            && row
                .last_seen_at
                .is_some_and(|last_seen| unix_now() - last_seen > DEVICE_STALE_AFTER_SECONDS);
        Ok(Self {
            id: row.id,
            tenant_id: row.tenant_id,
            display_name: row.display_name,
            operating_system: row.operating_system,
            agent_version: row.agent_version,
            capabilities: serde_json::from_value(row.capabilities_json)
                .map_err(|e| PairingError::Store(e.to_string()))?,
            state: row.state,
            paired_by: row.paired_by,
            created_at: row.created_at,
            updated_at: row.updated_at,
            last_seen_at: row.last_seen_at,
            stale,
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

    pub async fn list_devices(
        &self,
        tenant_id: &str,
        state: Option<&str>,
        limit: u32,
        offset: u32,
    ) -> Result<DeviceList, PairingError> {
        match self {
            Self::Memory(store) => Ok(store.list_devices(tenant_id, state, limit, offset).await),
            Self::Postgres(store) => store.list_devices(tenant_id, state, limit, offset).await,
        }
    }

    pub async fn get_device(
        &self,
        tenant_id: &str,
        device_id: &str,
    ) -> Result<Option<PairedDevice>, PairingError> {
        match self {
            Self::Memory(store) => Ok(store.get_device(tenant_id, device_id).await),
            Self::Postgres(store) => store.get_device(tenant_id, device_id).await,
        }
    }

    pub async fn rename_device(
        &self,
        tenant_id: &str,
        device_id: &str,
        display_name: &str,
    ) -> Result<PairedDevice, PairingError> {
        match self {
            Self::Memory(store) => {
                store
                    .rename_device(tenant_id, device_id, display_name)
                    .await
            }
            Self::Postgres(store) => {
                store
                    .rename_device(tenant_id, device_id, display_name)
                    .await
            }
        }
    }

    pub async fn set_device_state(
        &self,
        tenant_id: &str,
        device_id: &str,
        state: &str,
    ) -> Result<PairedDevice, PairingError> {
        match self {
            Self::Memory(store) => store.set_device_state(tenant_id, device_id, state).await,
            Self::Postgres(store) => store.set_device_state(tenant_id, device_id, state).await,
        }
    }

    pub async fn start_rotation(
        &self,
        tenant_id: &str,
        device_id: &str,
    ) -> Result<CredentialRotationStarted, PairingError> {
        match self {
            Self::Memory(store) => store.start_rotation(tenant_id, device_id).await,
            Self::Postgres(store) => store.start_rotation(tenant_id, device_id).await,
        }
    }

    pub async fn claim_rotation(
        &self,
        device_id: &str,
        current_credential: &str,
    ) -> Result<DeviceCredential, PairingError> {
        match self {
            Self::Memory(store) => store.claim_rotation(device_id, current_credential).await,
            Self::Postgres(store) => store.claim_rotation(device_id, current_credential).await,
        }
    }

    pub async fn authenticate_device(
        &self,
        device_id: &str,
        credential: &str,
    ) -> Result<PairedDevice, PairingError> {
        match self {
            Self::Memory(store) => store.authenticate_device(device_id, credential).await,
            Self::Postgres(store) => store.authenticate_device(device_id, credential).await,
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

    async fn pair_device(
        store: &PlatformDevicePairingStore,
        device_id: &str,
        tenant_id: &str,
    ) -> DeviceCredential {
        let session = store.create_session(request(device_id)).await.unwrap();
        store
            .approve(&session.pairing_code, tenant_id, "admin-1")
            .await
            .unwrap();
        store
            .claim(&session.session_id, &session.claim_secret)
            .await
            .unwrap()
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

    #[tokio::test]
    async fn registry_is_tenant_scoped_filterable_and_paginated() {
        let store = PlatformDevicePairingStore::default();
        pair_device(&store, "device-a", "tenant-1").await;
        pair_device(&store, "device-b", "tenant-1").await;
        pair_device(&store, "device-c", "tenant-2").await;
        store
            .set_device_state("tenant-1", "device-b", "suspended")
            .await
            .unwrap();

        let first = store.list_devices("tenant-1", None, 1, 0).await.unwrap();
        assert_eq!(first.items.len(), 1);
        assert_eq!(first.next_offset, Some(1));
        let second = store
            .list_devices("tenant-1", None, 1, first.next_offset.unwrap())
            .await
            .unwrap();
        assert_eq!(second.items.len(), 1);
        assert_eq!(second.next_offset, None);
        assert!(first
            .items
            .iter()
            .chain(second.items.iter())
            .all(|device| device.tenant_id == "tenant-1"));
        let suspended = store
            .list_devices("tenant-1", Some("suspended"), 10, 0)
            .await
            .unwrap();
        assert_eq!(suspended.items.len(), 1);
        assert_eq!(suspended.items[0].id, "device-b");
        assert!(store
            .get_device("tenant-2", "device-a")
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn rotation_invalidates_old_credential_and_is_single_use() {
        let store = PlatformDevicePairingStore::default();
        let original = pair_device(&store, "device-rotation", "tenant-1").await;
        store
            .start_rotation("tenant-1", "device-rotation")
            .await
            .unwrap();
        let rotated = store
            .claim_rotation("device-rotation", &original.credential)
            .await
            .unwrap();
        assert_ne!(rotated.credential_id, original.credential_id);
        assert!(matches!(
            store
                .authenticate_device("device-rotation", &original.credential)
                .await,
            Err(PairingError::InvalidClaim)
        ));
        assert!(store
            .authenticate_device("device-rotation", &rotated.credential)
            .await
            .is_ok());
        assert!(matches!(
            store
                .claim_rotation("device-rotation", &rotated.credential)
                .await,
            Err(PairingError::InvalidState)
        ));
    }

    #[tokio::test]
    async fn concurrent_rotation_claim_has_one_winner() {
        let store = Arc::new(PlatformDevicePairingStore::default());
        let original = pair_device(&store, "device-concurrent-rotation", "tenant-1").await;
        store
            .start_rotation("tenant-1", "device-concurrent-rotation")
            .await
            .unwrap();
        let first = Arc::clone(&store);
        let second = Arc::clone(&store);
        let first_credential = original.credential.clone();
        let second_credential = original.credential;
        let (left, right) = tokio::join!(
            first.claim_rotation("device-concurrent-rotation", &first_credential),
            second.claim_rotation("device-concurrent-rotation", &second_credential)
        );
        assert_eq!(usize::from(left.is_ok()) + usize::from(right.is_ok()), 1);
    }

    #[tokio::test]
    async fn suspended_and_revoked_devices_cannot_authenticate() {
        let store = PlatformDevicePairingStore::default();
        let suspended = pair_device(&store, "device-suspended", "tenant-1").await;
        store
            .set_device_state("tenant-1", "device-suspended", "suspended")
            .await
            .unwrap();
        assert!(matches!(
            store
                .authenticate_device("device-suspended", &suspended.credential)
                .await,
            Err(PairingError::InvalidState)
        ));

        let revoked = pair_device(&store, "device-revoked", "tenant-1").await;
        store
            .set_device_state("tenant-1", "device-revoked", "revoked")
            .await
            .unwrap();
        assert!(matches!(
            store
                .authenticate_device("device-revoked", &revoked.credential)
                .await,
            Err(PairingError::InvalidState)
        ));
    }
}
