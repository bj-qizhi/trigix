use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use base64::Engine as _;
use desktop_protocol::CommandOutcome;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};

const BASE64: base64::engine::general_purpose::GeneralPurpose =
    base64::engine::general_purpose::STANDARD;
const ABSOLUTE_MAX_EVIDENCE_BYTES: usize = 1024 * 1024;
const ABSOLUTE_MAX_RETENTION_DAYS: u16 = 365;
const MAX_ACTION_DURATION_MS: u64 = 5 * 60 * 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    AdapterAudit,
    Screenshot,
}

impl EvidenceKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::AdapterAudit => "adapter_audit",
            Self::Screenshot => "screenshot",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectorStrategy {
    AutomationId,
    ControlTypeAndName,
    NameAndSibling,
    WindowAutomationId,
    ExecutableAndTitle,
    Executable,
    Title,
    ControlType,
    ApplicationIdentity,
    NotApplicable,
}

impl SelectorStrategy {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::AutomationId => "automation_id",
            Self::ControlTypeAndName => "control_type_and_name",
            Self::NameAndSibling => "name_and_sibling",
            Self::WindowAutomationId => "window_automation_id",
            Self::ExecutableAndTitle => "executable_and_title",
            Self::Executable => "executable",
            Self::Title => "title",
            Self::ControlType => "control_type",
            Self::ApplicationIdentity => "application_identity",
            Self::NotApplicable => "not_applicable",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedactionAttestation {
    pub policy_version: String,
    pub succeeded: bool,
    pub sensitive_regions: u16,
    pub redacted_regions: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceUploadRequest {
    pub command_id: String,
    pub execution_id: String,
    pub project_id: String,
    pub kind: EvidenceKind,
    pub selector_strategy: SelectorStrategy,
    #[serde(default)]
    pub selector_fallback_depth: u8,
    #[serde(default)]
    pub selector_fallback_used: bool,
    pub application_id: String,
    pub started_at_unix_ms: u64,
    pub completed_at_unix_ms: u64,
    pub outcome: CommandOutcome,
    pub retention_days: u16,
    #[serde(default)]
    pub capture_opt_in: bool,
    pub redaction: RedactionAttestation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload_base64: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceRecord {
    pub evidence_id: String,
    pub tenant_id: String,
    pub project_id: String,
    pub execution_id: String,
    pub command_id: String,
    pub device_id: String,
    pub kind: EvidenceKind,
    pub selector_strategy: SelectorStrategy,
    pub selector_fallback_depth: u8,
    pub selector_fallback_used: bool,
    pub application_id: String,
    pub started_at_unix_ms: u64,
    pub completed_at_unix_ms: u64,
    pub outcome: CommandOutcome,
    pub policy_version: String,
    pub redacted_regions: u16,
    pub content_type: Option<String>,
    pub content_sha256: Option<String>,
    pub byte_size: u32,
    pub expires_at_unix_ms: u64,
    pub created_at_unix_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvidenceError {
    Invalid(&'static str),
    Disabled,
    EncryptionRequired,
    NotFound,
    Conflict,
    Store(String),
}

impl std::fmt::Display for EvidenceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(field) => write!(formatter, "invalid evidence field: {field}"),
            Self::Disabled => formatter.write_str("screenshot evidence is disabled by policy"),
            Self::EncryptionRequired => {
                formatter.write_str("screenshot evidence requires encryption at rest")
            }
            Self::NotFound => formatter.write_str("evidence was not found"),
            Self::Conflict => formatter.write_str("evidence already exists"),
            Self::Store(message) => write!(formatter, "evidence store error: {message}"),
        }
    }
}

impl std::error::Error for EvidenceError {}

#[derive(Debug, Clone)]
pub struct EvidencePolicy {
    pub screenshots_enabled: bool,
    pub max_payload_bytes: usize,
    pub max_retention_days: u16,
}

impl Default for EvidencePolicy {
    fn default() -> Self {
        Self {
            screenshots_enabled: false,
            max_payload_bytes: ABSOLUTE_MAX_EVIDENCE_BYTES,
            max_retention_days: 30,
        }
    }
}

impl EvidencePolicy {
    pub fn from_env() -> Self {
        let screenshots_enabled = std::env::var("DESKTOP_EVIDENCE_SCREENSHOTS_ENABLED")
            .ok()
            .is_some_and(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            });
        let max_payload_bytes = std::env::var("DESKTOP_EVIDENCE_MAX_BYTES")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(ABSOLUTE_MAX_EVIDENCE_BYTES)
            .min(ABSOLUTE_MAX_EVIDENCE_BYTES);
        let max_retention_days = std::env::var("DESKTOP_EVIDENCE_RETENTION_DAYS")
            .ok()
            .and_then(|value| value.parse::<u16>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(30)
            .min(ABSOLUTE_MAX_RETENTION_DAYS);
        Self {
            screenshots_enabled,
            max_payload_bytes,
            max_retention_days,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PreparedEvidence {
    pub record: EvidenceRecord,
    ciphertext: Option<String>,
}

pub fn prepare_evidence(
    tenant_id: &str,
    device_id: &str,
    request: EvidenceUploadRequest,
    policy: &EvidencePolicy,
    now_unix_ms: u64,
) -> Result<PreparedEvidence, EvidenceError> {
    if request.selector_fallback_depth > 4
        || request.selector_fallback_used != (request.selector_fallback_depth > 0)
        || (request.selector_strategy == SelectorStrategy::NotApplicable
            && request.selector_fallback_used)
    {
        return Err(EvidenceError::Invalid("selector_fallback"));
    }
    for (value, field) in [
        (tenant_id, "tenant_id"),
        (device_id, "device_id"),
        (&request.command_id, "command_id"),
        (&request.execution_id, "execution_id"),
        (&request.project_id, "project_id"),
        (&request.application_id, "application_id"),
        (
            &request.redaction.policy_version,
            "redaction.policy_version",
        ),
    ] {
        validate_identifier(value, field)?;
    }
    if request.completed_at_unix_ms < request.started_at_unix_ms
        || request
            .completed_at_unix_ms
            .saturating_sub(request.started_at_unix_ms)
            > MAX_ACTION_DURATION_MS
        || request.completed_at_unix_ms > now_unix_ms.saturating_add(60_000)
    {
        return Err(EvidenceError::Invalid("timing"));
    }
    if request.retention_days == 0 || request.retention_days > policy.max_retention_days {
        return Err(EvidenceError::Invalid("retention_days"));
    }
    if matches!(request.outcome, CommandOutcome::AwaitingApproval) {
        return Err(EvidenceError::Invalid("outcome"));
    }
    if !request.redaction.succeeded
        || request.redaction.redacted_regions != request.redaction.sensitive_regions
    {
        return Err(EvidenceError::Invalid("redaction"));
    }

    let (ciphertext, content_type, content_sha256, byte_size) = match request.kind {
        EvidenceKind::AdapterAudit => {
            if request.capture_opt_in
                || request.payload_base64.is_some()
                || request.content_type.is_some()
            {
                return Err(EvidenceError::Invalid("adapter_audit.payload"));
            }
            (None, None, None, 0)
        }
        EvidenceKind::Screenshot => {
            if !policy.screenshots_enabled {
                return Err(EvidenceError::Disabled);
            }
            if !request.capture_opt_in {
                return Err(EvidenceError::Invalid("capture_opt_in"));
            }
            let content_type = request
                .content_type
                .filter(|value| matches!(value.as_str(), "image/png" | "image/webp"))
                .ok_or(EvidenceError::Invalid("content_type"))?;
            let encoded = request
                .payload_base64
                .ok_or(EvidenceError::Invalid("payload_base64"))?;
            if encoded.len() > policy.max_payload_bytes.saturating_mul(4) / 3 + 8 {
                return Err(EvidenceError::Invalid("payload_size"));
            }
            let payload = BASE64
                .decode(encoded)
                .map_err(|_| EvidenceError::Invalid("payload_base64"))?;
            if payload.is_empty() || payload.len() > policy.max_payload_bytes {
                return Err(EvidenceError::Invalid("payload_size"));
            }
            validate_image_signature(&content_type, &payload)?;
            let digest = hex::encode(Sha256::digest(&payload));
            let byte_size =
                u32::try_from(payload.len()).map_err(|_| EvidenceError::Invalid("payload_size"))?;
            let ciphertext = crate::crypto::encrypt_bytes_required(&payload)
                .map_err(|_| EvidenceError::EncryptionRequired)?;
            (
                Some(ciphertext),
                Some(content_type),
                Some(digest),
                byte_size,
            )
        }
    };

    let retention_ms = u64::from(request.retention_days)
        .checked_mul(86_400_000)
        .ok_or(EvidenceError::Invalid("retention_days"))?;
    let expires_at_unix_ms = now_unix_ms
        .checked_add(retention_ms)
        .ok_or(EvidenceError::Invalid("retention_days"))?;
    Ok(PreparedEvidence {
        record: EvidenceRecord {
            evidence_id: format!("desktop-evidence-{}", uuid::Uuid::new_v4()),
            tenant_id: tenant_id.to_owned(),
            project_id: request.project_id,
            execution_id: request.execution_id,
            command_id: request.command_id,
            device_id: device_id.to_owned(),
            kind: request.kind,
            selector_strategy: request.selector_strategy,
            selector_fallback_depth: request.selector_fallback_depth,
            selector_fallback_used: request.selector_fallback_used,
            application_id: request.application_id,
            started_at_unix_ms: request.started_at_unix_ms,
            completed_at_unix_ms: request.completed_at_unix_ms,
            outcome: request.outcome,
            policy_version: request.redaction.policy_version,
            redacted_regions: request.redaction.redacted_regions,
            content_type,
            content_sha256,
            byte_size,
            expires_at_unix_ms,
            created_at_unix_ms: now_unix_ms,
        },
        ciphertext,
    })
}

fn validate_identifier(value: &str, field: &'static str) -> Result<(), EvidenceError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':'))
    {
        return Err(EvidenceError::Invalid(field));
    }
    Ok(())
}

fn validate_image_signature(content_type: &str, payload: &[u8]) -> Result<(), EvidenceError> {
    let valid = match content_type {
        "image/png" => payload.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/webp" => {
            payload.len() >= 12 && payload.starts_with(b"RIFF") && &payload[8..12] == b"WEBP"
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(EvidenceError::Invalid("image_signature"))
    }
}

#[derive(Clone, Default)]
pub struct MemoryDesktopEvidenceStore {
    records: Arc<Mutex<HashMap<String, PreparedEvidence>>>,
}

impl MemoryDesktopEvidenceStore {
    async fn create(&self, evidence: PreparedEvidence) -> Result<EvidenceRecord, EvidenceError> {
        let mut records = self
            .records
            .lock()
            .map_err(|_| EvidenceError::Store("evidence lock is poisoned".to_owned()))?;
        if records.contains_key(&evidence.record.evidence_id) {
            return Err(EvidenceError::Conflict);
        }
        let record = evidence.record.clone();
        records.insert(record.evidence_id.clone(), evidence);
        Ok(record)
    }

    async fn list(&self, tenant_id: &str, execution_id: &str) -> Vec<EvidenceRecord> {
        let Ok(records) = self.records.lock() else {
            return Vec::new();
        };
        let now_unix_ms = unix_millis();
        let mut output = records
            .values()
            .filter(|evidence| {
                evidence.record.tenant_id == tenant_id
                    && evidence.record.execution_id == execution_id
                    && evidence.record.expires_at_unix_ms > now_unix_ms
            })
            .map(|evidence| evidence.record.clone())
            .collect::<Vec<_>>();
        output.sort_by_key(|record| record.created_at_unix_ms);
        output
    }

    async fn delete(&self, tenant_id: &str, evidence_id: &str) -> Result<(), EvidenceError> {
        let mut records = self
            .records
            .lock()
            .map_err(|_| EvidenceError::Store("evidence lock is poisoned".to_owned()))?;
        if records
            .get(evidence_id)
            .is_none_or(|evidence| evidence.record.tenant_id != tenant_id)
        {
            return Err(EvidenceError::NotFound);
        }
        records.remove(evidence_id);
        Ok(())
    }

    async fn purge_expired(&self, now_unix_ms: u64) -> Result<u64, EvidenceError> {
        let mut records = self
            .records
            .lock()
            .map_err(|_| EvidenceError::Store("evidence lock is poisoned".to_owned()))?;
        let before = records.len();
        records.retain(|_, evidence| evidence.record.expires_at_unix_ms > now_unix_ms);
        Ok((before - records.len()) as u64)
    }
}

#[derive(Clone)]
pub struct PostgresDesktopEvidenceStore {
    pool: PgPool,
}

impl PostgresDesktopEvidenceStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn create(&self, evidence: PreparedEvidence) -> Result<EvidenceRecord, EvidenceError> {
        let record = &evidence.record;
        sqlx::query(
            r#"INSERT INTO af_desktop_evidence
               (id, tenant_id, project_id, execution_id, command_id, device_id, kind,
                selector_strategy, selector_fallback_depth, selector_fallback_used,
                application_id, started_at_unix_ms, completed_at_unix_ms,
                outcome, policy_version, redacted_regions, content_type, content_sha256,
                byte_size, payload_ciphertext, expires_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,
                       to_timestamp($21::double precision / 1000.0))"#,
        )
        .bind(&record.evidence_id)
        .bind(&record.tenant_id)
        .bind(&record.project_id)
        .bind(&record.execution_id)
        .bind(&record.command_id)
        .bind(&record.device_id)
        .bind(record.kind.as_str())
        .bind(record.selector_strategy.as_str())
        .bind(i16::from(record.selector_fallback_depth))
        .bind(record.selector_fallback_used)
        .bind(&record.application_id)
        .bind(record.started_at_unix_ms as i64)
        .bind(record.completed_at_unix_ms as i64)
        .bind(outcome_name(record.outcome))
        .bind(&record.policy_version)
        .bind(i32::from(record.redacted_regions))
        .bind(&record.content_type)
        .bind(&record.content_sha256)
        .bind(i64::from(record.byte_size))
        .bind(evidence.ciphertext)
        .bind(record.expires_at_unix_ms as i64)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            if error
                .as_database_error()
                .and_then(|error| error.code())
                .as_deref()
                == Some("23505")
            {
                EvidenceError::Conflict
            } else {
                EvidenceError::Store(error.to_string())
            }
        })?;
        Ok(record.clone())
    }

    async fn list(
        &self,
        tenant_id: &str,
        execution_id: &str,
    ) -> Result<Vec<EvidenceRecord>, EvidenceError> {
        let rows = sqlx::query(
            r#"SELECT id, tenant_id, project_id, execution_id, command_id, device_id, kind,
                      selector_strategy, selector_fallback_depth, selector_fallback_used,
                      application_id, started_at_unix_ms,
                      completed_at_unix_ms, outcome, policy_version, redacted_regions,
                      content_type, content_sha256, byte_size,
                      (EXTRACT(EPOCH FROM expires_at) * 1000)::bigint AS expires_at_unix_ms,
                      (EXTRACT(EPOCH FROM created_at) * 1000)::bigint AS created_at_unix_ms
               FROM af_desktop_evidence
               WHERE tenant_id = $1 AND execution_id = $2 AND expires_at > now()
               ORDER BY created_at ASC"#,
        )
        .bind(tenant_id)
        .bind(execution_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| EvidenceError::Store(error.to_string()))?;
        rows.iter().map(row_to_record).collect()
    }

    async fn delete(&self, tenant_id: &str, evidence_id: &str) -> Result<(), EvidenceError> {
        let result =
            sqlx::query("DELETE FROM af_desktop_evidence WHERE tenant_id = $1 AND id = $2")
                .bind(tenant_id)
                .bind(evidence_id)
                .execute(&self.pool)
                .await
                .map_err(|error| EvidenceError::Store(error.to_string()))?;
        if result.rows_affected() == 0 {
            return Err(EvidenceError::NotFound);
        }
        Ok(())
    }

    async fn purge_expired(&self, now_unix_ms: u64) -> Result<u64, EvidenceError> {
        sqlx::query(
            "DELETE FROM af_desktop_evidence WHERE expires_at <= to_timestamp($1::double precision / 1000.0)",
        )
        .bind(now_unix_ms as i64)
        .execute(&self.pool)
        .await
        .map(|result| result.rows_affected())
        .map_err(|error| EvidenceError::Store(error.to_string()))
    }
}

fn row_to_record(row: &sqlx::postgres::PgRow) -> Result<EvidenceRecord, EvidenceError> {
    Ok(EvidenceRecord {
        evidence_id: row.try_get("id").map_err(store_error)?,
        tenant_id: row.try_get("tenant_id").map_err(store_error)?,
        project_id: row.try_get("project_id").map_err(store_error)?,
        execution_id: row.try_get("execution_id").map_err(store_error)?,
        command_id: row.try_get("command_id").map_err(store_error)?,
        device_id: row.try_get("device_id").map_err(store_error)?,
        kind: parse_kind(
            row.try_get::<String, _>("kind")
                .map_err(store_error)?
                .as_str(),
        )?,
        selector_strategy: parse_selector(
            row.try_get::<String, _>("selector_strategy")
                .map_err(store_error)?
                .as_str(),
        )?,
        selector_fallback_depth: row
            .try_get::<i16, _>("selector_fallback_depth")
            .map_err(store_error)? as u8,
        selector_fallback_used: row.try_get("selector_fallback_used").map_err(store_error)?,
        application_id: row.try_get("application_id").map_err(store_error)?,
        started_at_unix_ms: row
            .try_get::<i64, _>("started_at_unix_ms")
            .map_err(store_error)? as u64,
        completed_at_unix_ms: row
            .try_get::<i64, _>("completed_at_unix_ms")
            .map_err(store_error)? as u64,
        outcome: parse_outcome(
            row.try_get::<String, _>("outcome")
                .map_err(store_error)?
                .as_str(),
        )?,
        policy_version: row.try_get("policy_version").map_err(store_error)?,
        redacted_regions: row
            .try_get::<i32, _>("redacted_regions")
            .map_err(store_error)? as u16,
        content_type: row.try_get("content_type").map_err(store_error)?,
        content_sha256: row.try_get("content_sha256").map_err(store_error)?,
        byte_size: row.try_get::<i64, _>("byte_size").map_err(store_error)? as u32,
        expires_at_unix_ms: row
            .try_get::<i64, _>("expires_at_unix_ms")
            .map_err(store_error)? as u64,
        created_at_unix_ms: row
            .try_get::<i64, _>("created_at_unix_ms")
            .map_err(store_error)? as u64,
    })
}

fn parse_kind(value: &str) -> Result<EvidenceKind, EvidenceError> {
    match value {
        "adapter_audit" => Ok(EvidenceKind::AdapterAudit),
        "screenshot" => Ok(EvidenceKind::Screenshot),
        _ => Err(EvidenceError::Store(
            "invalid persisted evidence kind".to_owned(),
        )),
    }
}

fn parse_selector(value: &str) -> Result<SelectorStrategy, EvidenceError> {
    match value {
        "automation_id" => Ok(SelectorStrategy::AutomationId),
        "control_type_and_name" => Ok(SelectorStrategy::ControlTypeAndName),
        "name_and_sibling" => Ok(SelectorStrategy::NameAndSibling),
        "window_automation_id" => Ok(SelectorStrategy::WindowAutomationId),
        "executable_and_title" => Ok(SelectorStrategy::ExecutableAndTitle),
        "executable" => Ok(SelectorStrategy::Executable),
        "title" => Ok(SelectorStrategy::Title),
        "control_type" => Ok(SelectorStrategy::ControlType),
        "application_identity" => Ok(SelectorStrategy::ApplicationIdentity),
        "not_applicable" => Ok(SelectorStrategy::NotApplicable),
        _ => Err(EvidenceError::Store(
            "invalid persisted selector strategy".to_owned(),
        )),
    }
}

fn outcome_name(outcome: CommandOutcome) -> &'static str {
    match outcome {
        CommandOutcome::Succeeded => "succeeded",
        CommandOutcome::Failed => "failed",
        CommandOutcome::Rejected => "rejected",
        CommandOutcome::AwaitingApproval => "awaiting_approval",
        CommandOutcome::Cancelled => "cancelled",
        CommandOutcome::TimedOut => "timed_out",
    }
}

fn parse_outcome(value: &str) -> Result<CommandOutcome, EvidenceError> {
    match value {
        "succeeded" => Ok(CommandOutcome::Succeeded),
        "failed" => Ok(CommandOutcome::Failed),
        "rejected" => Ok(CommandOutcome::Rejected),
        "cancelled" => Ok(CommandOutcome::Cancelled),
        "timed_out" => Ok(CommandOutcome::TimedOut),
        _ => Err(EvidenceError::Store(
            "invalid persisted evidence outcome".to_owned(),
        )),
    }
}

fn store_error(error: impl std::fmt::Display) -> EvidenceError {
    EvidenceError::Store(error.to_string())
}

fn unix_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[derive(Clone)]
pub enum PlatformDesktopEvidenceStore {
    Memory(MemoryDesktopEvidenceStore),
    Postgres(PostgresDesktopEvidenceStore),
}

impl Default for PlatformDesktopEvidenceStore {
    fn default() -> Self {
        Self::Memory(MemoryDesktopEvidenceStore::default())
    }
}

impl PlatformDesktopEvidenceStore {
    pub fn postgres(pool: PgPool) -> Self {
        Self::Postgres(PostgresDesktopEvidenceStore::new(pool))
    }

    pub async fn create(
        &self,
        evidence: PreparedEvidence,
    ) -> Result<EvidenceRecord, EvidenceError> {
        match self {
            Self::Memory(store) => store.create(evidence).await,
            Self::Postgres(store) => store.create(evidence).await,
        }
    }

    pub async fn list(
        &self,
        tenant_id: &str,
        execution_id: &str,
    ) -> Result<Vec<EvidenceRecord>, EvidenceError> {
        match self {
            Self::Memory(store) => Ok(store.list(tenant_id, execution_id).await),
            Self::Postgres(store) => store.list(tenant_id, execution_id).await,
        }
    }

    pub async fn delete(&self, tenant_id: &str, evidence_id: &str) -> Result<(), EvidenceError> {
        match self {
            Self::Memory(store) => store.delete(tenant_id, evidence_id).await,
            Self::Postgres(store) => store.delete(tenant_id, evidence_id).await,
        }
    }

    pub async fn purge_expired(&self, now_unix_ms: u64) -> Result<u64, EvidenceError> {
        match self {
            Self::Memory(store) => store.purge_expired(now_unix_ms).await,
            Self::Postgres(store) => store.purge_expired(now_unix_ms).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn audit_request() -> EvidenceUploadRequest {
        EvidenceUploadRequest {
            command_id: "command-1".to_owned(),
            execution_id: "execution-1".to_owned(),
            project_id: "project-1".to_owned(),
            kind: EvidenceKind::AdapterAudit,
            selector_strategy: SelectorStrategy::AutomationId,
            selector_fallback_depth: 0,
            selector_fallback_used: false,
            application_id: "fixture".to_owned(),
            started_at_unix_ms: 1_000,
            completed_at_unix_ms: 1_100,
            outcome: CommandOutcome::Succeeded,
            retention_days: 7,
            capture_opt_in: false,
            redaction: RedactionAttestation {
                policy_version: "redaction-v1".to_owned(),
                succeeded: true,
                sensitive_regions: 0,
                redacted_regions: 0,
            },
            content_type: None,
            payload_base64: None,
        }
    }

    #[tokio::test]
    async fn audit_metadata_is_bounded_tenant_isolated_and_retention_bound() {
        let policy = EvidencePolicy::default();
        let now = unix_millis();
        let mut request = audit_request();
        request.started_at_unix_ms = now - 100;
        request.completed_at_unix_ms = now;
        let prepared = prepare_evidence("tenant-1", "device-1", request, &policy, now).unwrap();
        let expiry = prepared.record.expires_at_unix_ms;
        let store = PlatformDesktopEvidenceStore::default();
        let created = store.create(prepared).await.unwrap();
        assert_eq!(created.byte_size, 0);
        assert_eq!(store.list("tenant-2", "execution-1").await.unwrap(), vec![]);
        assert_eq!(
            store.list("tenant-1", "execution-1").await.unwrap().len(),
            1
        );
        assert_eq!(store.purge_expired(expiry).await.unwrap(), 1);
        assert!(store
            .list("tenant-1", "execution-1")
            .await
            .unwrap()
            .is_empty());
    }

    #[test]
    fn selector_fallback_telemetry_is_bounded_and_consistent() {
        let policy = EvidencePolicy::default();
        let mut request = audit_request();
        request.selector_strategy = SelectorStrategy::ControlTypeAndName;
        request.selector_fallback_depth = 1;
        request.selector_fallback_used = true;
        let prepared =
            prepare_evidence("tenant-1", "device-1", request.clone(), &policy, 2_000).unwrap();
        assert_eq!(prepared.record.selector_fallback_depth, 1);
        assert!(prepared.record.selector_fallback_used);

        request.selector_fallback_used = false;
        assert_eq!(
            prepare_evidence("tenant-1", "device-1", request, &policy, 2_000).unwrap_err(),
            EvidenceError::Invalid("selector_fallback")
        );
    }

    #[test]
    fn screenshot_capture_fails_closed_when_disabled_or_redaction_fails() {
        let mut request = audit_request();
        request.kind = EvidenceKind::Screenshot;
        request.capture_opt_in = true;
        request.content_type = Some("image/png".to_owned());
        request.payload_base64 = Some(BASE64.encode(b"\x89PNG\r\n\x1a\nredacted"));
        assert_eq!(
            prepare_evidence(
                "tenant-1",
                "device-1",
                request.clone(),
                &EvidencePolicy::default(),
                2_000
            )
            .unwrap_err(),
            EvidenceError::Disabled
        );

        request.redaction.succeeded = false;
        let policy = EvidencePolicy {
            screenshots_enabled: true,
            ..EvidencePolicy::default()
        };
        assert_eq!(
            prepare_evidence("tenant-1", "device-1", request, &policy, 2_000).unwrap_err(),
            EvidenceError::Invalid("redaction")
        );
    }

    #[test]
    fn screenshot_rejects_forged_content_and_unbounded_payloads_before_encryption() {
        let policy = EvidencePolicy {
            screenshots_enabled: true,
            max_payload_bytes: 16,
            max_retention_days: 7,
        };
        let mut request = audit_request();
        request.kind = EvidenceKind::Screenshot;
        request.capture_opt_in = true;
        request.content_type = Some("image/png".to_owned());
        request.payload_base64 = Some(BASE64.encode(b"not-an-image"));
        assert_eq!(
            prepare_evidence("tenant-1", "device-1", request.clone(), &policy, 2_000).unwrap_err(),
            EvidenceError::Invalid("image_signature")
        );
        request.payload_base64 = Some(BASE64.encode([0_u8; 17]));
        assert_eq!(
            prepare_evidence("tenant-1", "device-1", request, &policy, 2_000).unwrap_err(),
            EvidenceError::Invalid("payload_size")
        );
    }

    #[tokio::test]
    async fn deletion_cannot_cross_tenant_boundary() {
        let prepared = prepare_evidence(
            "tenant-1",
            "device-1",
            audit_request(),
            &EvidencePolicy::default(),
            2_000,
        )
        .unwrap();
        let evidence_id = prepared.record.evidence_id.clone();
        let store = PlatformDesktopEvidenceStore::default();
        store.create(prepared).await.unwrap();
        assert_eq!(
            store.delete("tenant-2", &evidence_id).await,
            Err(EvidenceError::NotFound)
        );
        store.delete("tenant-1", &evidence_id).await.unwrap();
    }
}
