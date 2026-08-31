use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

pub const MAX_VOICE_TRANSCRIPT_BYTES: usize = 4_096;
const MAX_RETENTION_DAYS: u16 = 30;
const DAY_MS: u64 = 86_400_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VoicePrivacyPolicy {
    pub policy_version: String,
    pub retain_redacted_transcripts: bool,
    pub transcript_retention_days: u16,
    pub metadata_retention_days: u16,
}

impl Default for VoicePrivacyPolicy {
    fn default() -> Self {
        Self {
            policy_version: "voice-privacy-v1".to_owned(),
            retain_redacted_transcripts: false,
            transcript_retention_days: 0,
            metadata_retention_days: 7,
        }
    }
}

impl VoicePrivacyPolicy {
    pub fn validate(&self) -> Result<(), VoiceConversationError> {
        validate_identifier(&self.policy_version)?;
        if self.metadata_retention_days == 0
            || self.metadata_retention_days > MAX_RETENTION_DAYS
            || (self.retain_redacted_transcripts
                && (self.transcript_retention_days == 0
                    || self.transcript_retention_days > MAX_RETENTION_DAYS))
            || (!self.retain_redacted_transcripts && self.transcript_retention_days != 0)
        {
            return Err(VoiceConversationError::InvalidPolicy);
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalVoiceTranscriptRequest {
    pub tenant_id: String,
    pub session_id: String,
    pub sequence: u32,
    pub occurred_at_unix_ms: u64,
    pub transcript: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VoiceConversationRecord {
    pub conversation_id: String,
    pub tenant_id: String,
    pub session_id: String,
    pub sequence: u32,
    pub occurred_at_unix_ms: u64,
    pub accepted_at_unix_ms: u64,
    pub policy_version: String,
    pub transcript_retained: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redacted_transcript: Option<String>,
    pub expires_at_unix_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceConversationError {
    InvalidPolicy,
    InvalidRequest,
    Duplicate,
    NotFound,
    StoreUnavailable,
}

#[derive(Clone, Default)]
pub struct VoiceConversationStore {
    inner: Arc<Mutex<VoiceConversationState>>,
}

#[derive(Default)]
struct VoiceConversationState {
    policies: HashMap<String, VoicePrivacyPolicy>,
    records: HashMap<String, VoiceConversationRecord>,
    sequence_index: HashMap<(String, String, u32), (String, String)>,
}

impl VoiceConversationStore {
    pub fn set_policy(
        &self,
        tenant_id: &str,
        policy: VoicePrivacyPolicy,
    ) -> Result<VoicePrivacyPolicy, VoiceConversationError> {
        validate_identifier(tenant_id)?;
        policy.validate()?;
        self.inner
            .lock()
            .map_err(|_| VoiceConversationError::StoreUnavailable)?
            .policies
            .insert(tenant_id.to_owned(), policy.clone());
        Ok(policy)
    }

    pub fn policy(&self, tenant_id: &str) -> Result<VoicePrivacyPolicy, VoiceConversationError> {
        validate_identifier(tenant_id)?;
        Ok(self
            .inner
            .lock()
            .map_err(|_| VoiceConversationError::StoreUnavailable)?
            .policies
            .get(tenant_id)
            .cloned()
            .unwrap_or_default())
    }

    pub fn accept_final_transcript(
        &self,
        tenant_id: &str,
        request: FinalVoiceTranscriptRequest,
        now_unix_ms: u64,
    ) -> Result<VoiceConversationRecord, VoiceConversationError> {
        validate_request(tenant_id, &request, now_unix_ms)?;

        let mut state = self
            .inner
            .lock()
            .map_err(|_| VoiceConversationError::StoreUnavailable)?;
        let key = (
            tenant_id.to_owned(),
            request.session_id.clone(),
            request.sequence,
        );
        let fingerprint = request_fingerprint(tenant_id, &request);
        if let Some((conversation_id, stored_fingerprint)) = state.sequence_index.get(&key) {
            if stored_fingerprint == &fingerprint {
                return state
                    .records
                    .get(conversation_id)
                    .cloned()
                    .ok_or(VoiceConversationError::StoreUnavailable);
            }
            return Err(VoiceConversationError::Duplicate);
        }
        let policy = state.policies.get(tenant_id).cloned().unwrap_or_default();
        let record = prepare_record(tenant_id, request, policy, now_unix_ms)?;
        state
            .sequence_index
            .insert(key, (record.conversation_id.clone(), fingerprint));
        state
            .records
            .insert(record.conversation_id.clone(), record.clone());
        Ok(record)
    }

    pub fn get(
        &self,
        tenant_id: &str,
        conversation_id: &str,
        now_unix_ms: u64,
    ) -> Result<VoiceConversationRecord, VoiceConversationError> {
        self.sweep(now_unix_ms)?;
        self.inner
            .lock()
            .map_err(|_| VoiceConversationError::StoreUnavailable)?
            .records
            .get(conversation_id)
            .filter(|record| record.tenant_id == tenant_id)
            .cloned()
            .ok_or(VoiceConversationError::NotFound)
    }

    pub fn delete(
        &self,
        tenant_id: &str,
        conversation_id: &str,
    ) -> Result<(), VoiceConversationError> {
        let mut state = self
            .inner
            .lock()
            .map_err(|_| VoiceConversationError::StoreUnavailable)?;
        let Some(record) = state
            .records
            .get(conversation_id)
            .filter(|record| record.tenant_id == tenant_id)
            .cloned()
        else {
            return Err(VoiceConversationError::NotFound);
        };
        state.records.remove(conversation_id);
        state
            .sequence_index
            .remove(&(record.tenant_id, record.session_id, record.sequence));
        Ok(())
    }

    pub fn sweep(&self, now_unix_ms: u64) -> Result<usize, VoiceConversationError> {
        let mut state = self
            .inner
            .lock()
            .map_err(|_| VoiceConversationError::StoreUnavailable)?;
        let expired: Vec<_> = state
            .records
            .values()
            .filter(|record| record.expires_at_unix_ms <= now_unix_ms)
            .map(|record| {
                (
                    record.conversation_id.clone(),
                    (
                        record.tenant_id.clone(),
                        record.session_id.clone(),
                        record.sequence,
                    ),
                )
            })
            .collect();
        for (id, key) in &expired {
            state.records.remove(id);
            state.sequence_index.remove(key);
        }
        Ok(expired.len())
    }
}

#[derive(Clone)]
pub struct PostgresVoiceConversationStore {
    pool: PgPool,
}

impl PostgresVoiceConversationStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn set_policy(
        &self,
        tenant_id: &str,
        policy: VoicePrivacyPolicy,
    ) -> Result<VoicePrivacyPolicy, VoiceConversationError> {
        validate_identifier(tenant_id)?;
        policy.validate()?;
        let mut transaction = self.pool.begin().await.map_err(store_error)?;
        set_tenant_context(&mut transaction, tenant_id).await?;
        sqlx::query(
            r#"INSERT INTO af_voice_privacy_policies
               (tenant_id, policy_version, retain_redacted_transcripts,
                transcript_retention_days, metadata_retention_days, updated_at)
               VALUES ($1,$2,$3,$4,$5,now())
               ON CONFLICT (tenant_id) DO UPDATE SET
                 policy_version=EXCLUDED.policy_version,
                 retain_redacted_transcripts=EXCLUDED.retain_redacted_transcripts,
                 transcript_retention_days=EXCLUDED.transcript_retention_days,
                 metadata_retention_days=EXCLUDED.metadata_retention_days,
                 updated_at=now()"#,
        )
        .bind(tenant_id)
        .bind(&policy.policy_version)
        .bind(policy.retain_redacted_transcripts)
        .bind(
            i16::try_from(policy.transcript_retention_days)
                .map_err(|_| VoiceConversationError::InvalidPolicy)?,
        )
        .bind(
            i16::try_from(policy.metadata_retention_days)
                .map_err(|_| VoiceConversationError::InvalidPolicy)?,
        )
        .execute(&mut *transaction)
        .await
        .map_err(store_error)?;
        transaction.commit().await.map_err(store_error)?;
        Ok(policy)
    }

    async fn policy(&self, tenant_id: &str) -> Result<VoicePrivacyPolicy, VoiceConversationError> {
        validate_identifier(tenant_id)?;
        let mut transaction = self.pool.begin().await.map_err(store_error)?;
        set_tenant_context(&mut transaction, tenant_id).await?;
        let policy = fetch_policy(&mut transaction, tenant_id).await?;
        transaction.commit().await.map_err(store_error)?;
        Ok(policy)
    }

    async fn accept_final_transcript(
        &self,
        tenant_id: &str,
        request: FinalVoiceTranscriptRequest,
        now_unix_ms: u64,
    ) -> Result<VoiceConversationRecord, VoiceConversationError> {
        validate_request(tenant_id, &request, now_unix_ms)?;
        let fingerprint = request_fingerprint(tenant_id, &request);
        let session_id = request.session_id.clone();
        let sequence = request.sequence;
        let mut transaction = self.pool.begin().await.map_err(store_error)?;
        set_tenant_context(&mut transaction, tenant_id).await?;
        sqlx::query(
            "DELETE FROM af_voice_conversations WHERE tenant_id=$1 AND session_id=$2 AND sequence=$3 AND expires_at <= now()",
        )
        .bind(tenant_id)
        .bind(&session_id)
        .bind(i64::from(sequence))
        .execute(&mut *transaction)
        .await
        .map_err(store_error)?;
        let policy = fetch_policy(&mut transaction, tenant_id).await?;
        let record = prepare_record(tenant_id, request, policy, now_unix_ms)?;
        let encrypted_fingerprint = crate::crypto::encrypt(&fingerprint);
        let inserted = sqlx::query(
            r#"INSERT INTO af_voice_conversations
               (id, tenant_id, session_id, sequence, occurred_at_unix_ms,
                accepted_at_unix_ms, policy_version, transcript_retained,
                redacted_transcript, request_fingerprint, expires_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,
                       to_timestamp($11::double precision / 1000.0))
               ON CONFLICT (tenant_id, session_id, sequence) DO NOTHING"#,
        )
        .bind(&record.conversation_id)
        .bind(&record.tenant_id)
        .bind(&record.session_id)
        .bind(i64::from(record.sequence))
        .bind(to_i64(record.occurred_at_unix_ms)?)
        .bind(to_i64(record.accepted_at_unix_ms)?)
        .bind(&record.policy_version)
        .bind(record.transcript_retained)
        .bind(&record.redacted_transcript)
        .bind(encrypted_fingerprint)
        .bind(to_i64(record.expires_at_unix_ms)?)
        .execute(&mut *transaction)
        .await
        .map_err(store_error)?;
        if inserted.rows_affected() == 1 {
            transaction.commit().await.map_err(store_error)?;
            return Ok(record);
        }

        let row = sqlx::query(
            r#"SELECT id, tenant_id, session_id, sequence, occurred_at_unix_ms,
                      accepted_at_unix_ms, policy_version, transcript_retained,
                      redacted_transcript, request_fingerprint,
                      (EXTRACT(EPOCH FROM expires_at) * 1000)::bigint AS expires_at_unix_ms
               FROM af_voice_conversations
               WHERE tenant_id=$1 AND session_id=$2 AND sequence=$3"#,
        )
        .bind(tenant_id)
        .bind(&session_id)
        .bind(i64::from(sequence))
        .fetch_optional(&mut *transaction)
        .await
        .map_err(store_error)?
        .ok_or(VoiceConversationError::StoreUnavailable)?;
        let stored_fingerprint: String = row.try_get("request_fingerprint").map_err(store_error)?;
        if crate::crypto::decrypt(&stored_fingerprint) != fingerprint {
            return Err(VoiceConversationError::Duplicate);
        }
        let existing = row_to_record(&row)?;
        transaction.commit().await.map_err(store_error)?;
        Ok(existing)
    }

    async fn get(
        &self,
        tenant_id: &str,
        conversation_id: &str,
        now_unix_ms: u64,
    ) -> Result<VoiceConversationRecord, VoiceConversationError> {
        validate_identifier(tenant_id)?;
        validate_identifier(conversation_id)?;
        let mut transaction = self.pool.begin().await.map_err(store_error)?;
        set_tenant_context(&mut transaction, tenant_id).await?;
        let row = sqlx::query(
            r#"SELECT id, tenant_id, session_id, sequence, occurred_at_unix_ms,
                      accepted_at_unix_ms, policy_version, transcript_retained,
                      redacted_transcript,
                      (EXTRACT(EPOCH FROM expires_at) * 1000)::bigint AS expires_at_unix_ms
               FROM af_voice_conversations
               WHERE tenant_id=$1 AND id=$2
                 AND expires_at > to_timestamp($3::double precision / 1000.0)"#,
        )
        .bind(tenant_id)
        .bind(conversation_id)
        .bind(to_i64(now_unix_ms)?)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(store_error)?;
        transaction.commit().await.map_err(store_error)?;
        row.as_ref()
            .map(row_to_record)
            .transpose()?
            .ok_or(VoiceConversationError::NotFound)
    }

    async fn delete(
        &self,
        tenant_id: &str,
        conversation_id: &str,
    ) -> Result<(), VoiceConversationError> {
        validate_identifier(tenant_id)?;
        validate_identifier(conversation_id)?;
        let mut transaction = self.pool.begin().await.map_err(store_error)?;
        set_tenant_context(&mut transaction, tenant_id).await?;
        let result = sqlx::query("DELETE FROM af_voice_conversations WHERE tenant_id=$1 AND id=$2")
            .bind(tenant_id)
            .bind(conversation_id)
            .execute(&mut *transaction)
            .await
            .map_err(store_error)?;
        transaction.commit().await.map_err(store_error)?;
        if result.rows_affected() == 0 {
            return Err(VoiceConversationError::NotFound);
        }
        Ok(())
    }

    async fn sweep(&self, now_unix_ms: u64) -> Result<usize, VoiceConversationError> {
        let removed = sqlx::query_scalar::<_, i64>(
            "SELECT af_sweep_expired_voice_conversations(to_timestamp($1::double precision / 1000.0))",
        )
        .bind(to_i64(now_unix_ms)?)
        .fetch_one(&self.pool)
        .await
        .map_err(store_error)?;
        usize::try_from(removed).map_err(|_| VoiceConversationError::StoreUnavailable)
    }
}

#[derive(Clone)]
pub enum PlatformVoiceConversationStore {
    Memory(VoiceConversationStore),
    Postgres(PostgresVoiceConversationStore),
}

impl Default for PlatformVoiceConversationStore {
    fn default() -> Self {
        Self::Memory(VoiceConversationStore::default())
    }
}

impl PlatformVoiceConversationStore {
    pub fn postgres(pool: PgPool) -> Self {
        Self::Postgres(PostgresVoiceConversationStore::new(pool))
    }

    pub async fn set_policy(
        &self,
        tenant_id: &str,
        policy: VoicePrivacyPolicy,
    ) -> Result<VoicePrivacyPolicy, VoiceConversationError> {
        match self {
            Self::Memory(store) => store.set_policy(tenant_id, policy),
            Self::Postgres(store) => store.set_policy(tenant_id, policy).await,
        }
    }

    pub async fn policy(
        &self,
        tenant_id: &str,
    ) -> Result<VoicePrivacyPolicy, VoiceConversationError> {
        match self {
            Self::Memory(store) => store.policy(tenant_id),
            Self::Postgres(store) => store.policy(tenant_id).await,
        }
    }

    pub async fn accept_final_transcript(
        &self,
        tenant_id: &str,
        request: FinalVoiceTranscriptRequest,
        now_unix_ms: u64,
    ) -> Result<VoiceConversationRecord, VoiceConversationError> {
        match self {
            Self::Memory(store) => store.accept_final_transcript(tenant_id, request, now_unix_ms),
            Self::Postgres(store) => {
                store
                    .accept_final_transcript(tenant_id, request, now_unix_ms)
                    .await
            }
        }
    }

    pub async fn get(
        &self,
        tenant_id: &str,
        conversation_id: &str,
        now_unix_ms: u64,
    ) -> Result<VoiceConversationRecord, VoiceConversationError> {
        match self {
            Self::Memory(store) => store.get(tenant_id, conversation_id, now_unix_ms),
            Self::Postgres(store) => store.get(tenant_id, conversation_id, now_unix_ms).await,
        }
    }

    pub async fn delete(
        &self,
        tenant_id: &str,
        conversation_id: &str,
    ) -> Result<(), VoiceConversationError> {
        match self {
            Self::Memory(store) => store.delete(tenant_id, conversation_id),
            Self::Postgres(store) => store.delete(tenant_id, conversation_id).await,
        }
    }

    pub async fn sweep(&self, now_unix_ms: u64) -> Result<usize, VoiceConversationError> {
        match self {
            Self::Memory(store) => store.sweep(now_unix_ms),
            Self::Postgres(store) => store.sweep(now_unix_ms).await,
        }
    }
}

fn validate_request(
    tenant_id: &str,
    request: &FinalVoiceTranscriptRequest,
    now_unix_ms: u64,
) -> Result<(), VoiceConversationError> {
    validate_identifier(tenant_id)?;
    validate_identifier(&request.session_id)?;
    if request.sequence == 0
        || request.occurred_at_unix_ms == 0
        || request.occurred_at_unix_ms > now_unix_ms.saturating_add(300_000)
        || request.transcript.trim().is_empty()
        || request.transcript.len() > MAX_VOICE_TRANSCRIPT_BYTES
        || request
            .transcript
            .chars()
            .any(|value| value.is_control() && !value.is_whitespace())
        || i64::try_from(now_unix_ms).is_err()
    {
        return Err(VoiceConversationError::InvalidRequest);
    }
    Ok(())
}

fn prepare_record(
    tenant_id: &str,
    request: FinalVoiceTranscriptRequest,
    policy: VoicePrivacyPolicy,
    now_unix_ms: u64,
) -> Result<VoiceConversationRecord, VoiceConversationError> {
    policy.validate()?;
    let redacted_transcript = policy
        .retain_redacted_transcripts
        .then(|| redact_transcript(&request.transcript));
    if redacted_transcript
        .as_ref()
        .is_some_and(|value| value.len() > MAX_VOICE_TRANSCRIPT_BYTES)
    {
        return Err(VoiceConversationError::InvalidRequest);
    }
    let retention_days = if policy.retain_redacted_transcripts {
        policy
            .metadata_retention_days
            .min(policy.transcript_retention_days)
    } else {
        policy.metadata_retention_days
    };
    let expires_at_unix_ms = now_unix_ms
        .checked_add(u64::from(retention_days).saturating_mul(DAY_MS))
        .filter(|value| i64::try_from(*value).is_ok())
        .ok_or(VoiceConversationError::InvalidRequest)?;
    Ok(VoiceConversationRecord {
        conversation_id: Uuid::new_v4().to_string(),
        tenant_id: tenant_id.to_owned(),
        session_id: request.session_id,
        sequence: request.sequence,
        occurred_at_unix_ms: request.occurred_at_unix_ms,
        accepted_at_unix_ms: now_unix_ms,
        policy_version: policy.policy_version,
        transcript_retained: redacted_transcript.is_some(),
        redacted_transcript,
        expires_at_unix_ms,
    })
}

fn request_fingerprint(tenant_id: &str, request: &FinalVoiceTranscriptRequest) -> String {
    let mut digest = Sha256::new();
    for value in [
        tenant_id.as_bytes(),
        request.session_id.as_bytes(),
        request.transcript.as_bytes(),
    ] {
        digest.update((value.len() as u64).to_be_bytes());
        digest.update(value);
    }
    digest.update(request.sequence.to_be_bytes());
    digest.update(request.occurred_at_unix_ms.to_be_bytes());
    hex::encode(digest.finalize())
}

async fn set_tenant_context(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: &str,
) -> Result<(), VoiceConversationError> {
    sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
        .bind(tenant_id)
        .execute(&mut **transaction)
        .await
        .map_err(store_error)?;
    Ok(())
}

async fn fetch_policy(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: &str,
) -> Result<VoicePrivacyPolicy, VoiceConversationError> {
    let row = sqlx::query("SELECT policy_version, retain_redacted_transcripts, transcript_retention_days, metadata_retention_days FROM af_voice_privacy_policies WHERE tenant_id=$1")
        .bind(tenant_id)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(store_error)?;
    let Some(row) = row else {
        return Ok(VoicePrivacyPolicy::default());
    };
    let policy = VoicePrivacyPolicy {
        policy_version: row.try_get("policy_version").map_err(store_error)?,
        retain_redacted_transcripts: row
            .try_get("retain_redacted_transcripts")
            .map_err(store_error)?,
        transcript_retention_days: u16::try_from(
            row.try_get::<i16, _>("transcript_retention_days")
                .map_err(store_error)?,
        )
        .map_err(|_| VoiceConversationError::StoreUnavailable)?,
        metadata_retention_days: u16::try_from(
            row.try_get::<i16, _>("metadata_retention_days")
                .map_err(store_error)?,
        )
        .map_err(|_| VoiceConversationError::StoreUnavailable)?,
    };
    policy.validate()?;
    Ok(policy)
}

fn row_to_record(
    row: &sqlx::postgres::PgRow,
) -> Result<VoiceConversationRecord, VoiceConversationError> {
    Ok(VoiceConversationRecord {
        conversation_id: row.try_get("id").map_err(store_error)?,
        tenant_id: row.try_get("tenant_id").map_err(store_error)?,
        session_id: row.try_get("session_id").map_err(store_error)?,
        sequence: u32::try_from(row.try_get::<i64, _>("sequence").map_err(store_error)?)
            .map_err(|_| VoiceConversationError::StoreUnavailable)?,
        occurred_at_unix_ms: u64::try_from(
            row.try_get::<i64, _>("occurred_at_unix_ms")
                .map_err(store_error)?,
        )
        .map_err(|_| VoiceConversationError::StoreUnavailable)?,
        accepted_at_unix_ms: u64::try_from(
            row.try_get::<i64, _>("accepted_at_unix_ms")
                .map_err(store_error)?,
        )
        .map_err(|_| VoiceConversationError::StoreUnavailable)?,
        policy_version: row.try_get("policy_version").map_err(store_error)?,
        transcript_retained: row.try_get("transcript_retained").map_err(store_error)?,
        redacted_transcript: row.try_get("redacted_transcript").map_err(store_error)?,
        expires_at_unix_ms: u64::try_from(
            row.try_get::<i64, _>("expires_at_unix_ms")
                .map_err(store_error)?,
        )
        .map_err(|_| VoiceConversationError::StoreUnavailable)?,
    })
}

fn to_i64(value: u64) -> Result<i64, VoiceConversationError> {
    i64::try_from(value).map_err(|_| VoiceConversationError::InvalidRequest)
}

fn store_error(_error: impl std::fmt::Display) -> VoiceConversationError {
    VoiceConversationError::StoreUnavailable
}

fn validate_identifier(value: &str) -> Result<(), VoiceConversationError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(VoiceConversationError::InvalidRequest);
    }
    Ok(())
}

fn redact_transcript(transcript: &str) -> String {
    let mut redact_next = false;
    transcript
        .split_whitespace()
        .map(|token| {
            let trimmed = token.trim_matches(|character: char| !character.is_alphanumeric());
            if redact_next {
                redact_next = false;
                "[sensitive]".to_owned()
            } else if trimmed.eq_ignore_ascii_case("bearer") {
                redact_next = true;
                "[sensitive]".to_owned()
            } else if trimmed.contains('@') && trimmed.contains('.') {
                "[email]".to_owned()
            } else if trimmed.starts_with("sk-")
                || trimmed
                    .chars()
                    .filter(|value| value.is_ascii_digit())
                    .count()
                    >= 6
            {
                "[sensitive]".to_owned()
            } else {
                token.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(tenant_id: &str) -> FinalVoiceTranscriptRequest {
        FinalVoiceTranscriptRequest {
            tenant_id: tenant_id.to_owned(),
            session_id: "voice-session-1".to_owned(),
            sequence: 1,
            occurred_at_unix_ms: 1_000,
            transcript: "email person@example.com card 4111111111111111".to_owned(),
        }
    }

    #[test]
    fn default_policy_retains_metadata_without_content() {
        let store = VoiceConversationStore::default();
        let record = store
            .accept_final_transcript("tenant-a", request("tenant-a"), 1_001)
            .unwrap();
        assert!(!record.transcript_retained);
        assert_eq!(record.redacted_transcript, None);
    }

    #[test]
    fn enabled_retention_redacts_before_storage() {
        let store = VoiceConversationStore::default();
        store
            .set_policy(
                "tenant-a",
                VoicePrivacyPolicy {
                    retain_redacted_transcripts: true,
                    transcript_retention_days: 3,
                    ..VoicePrivacyPolicy::default()
                },
            )
            .unwrap();
        let mut retained = request("tenant-a");
        retained.transcript.push_str(" Bearer secret-token");
        let record = store
            .accept_final_transcript("tenant-a", retained, 1_001)
            .unwrap();
        assert_eq!(
            record.redacted_transcript.as_deref(),
            Some("email [email] card [sensitive] [sensitive] [sensitive]")
        );
        assert!(!format!("{record:?}").contains("person@example.com"));
        assert!(!format!("{record:?}").contains("secret-token"));
    }

    #[test]
    fn tenant_isolation_and_expiry_fail_closed() {
        let store = VoiceConversationStore::default();
        let record = store
            .accept_final_transcript("tenant-a", request("tenant-a"), 1_001)
            .unwrap();
        assert_eq!(
            store.get("tenant-b", &record.conversation_id, 1_002),
            Err(VoiceConversationError::NotFound)
        );
        assert_eq!(
            store.delete("tenant-b", &record.conversation_id),
            Err(VoiceConversationError::NotFound)
        );
        assert_eq!(store.sweep(record.expires_at_unix_ms).unwrap(), 1);
        assert_eq!(
            store.get(
                "tenant-a",
                &record.conversation_id,
                record.expires_at_unix_ms
            ),
            Err(VoiceConversationError::NotFound)
        );
    }

    #[test]
    fn malformed_and_authority_shaped_payloads_are_rejected() {
        assert!(
            serde_json::from_value::<FinalVoiceTranscriptRequest>(serde_json::json!({
                "tenant_id": "tenant-a",
                "session_id": "voice-session-1",
                "sequence": 1,
                "occurred_at_unix_ms": 1_000,
                "transcript": "open settings",
                "raw_audio": "AAAA",
                "desktop_action": {"type": "launch_application"}
            }))
            .is_err()
        );
        let store = VoiceConversationStore::default();
        let mut invalid = request("tenant-a");
        invalid.transcript = "\u{0000}".to_owned();
        assert_eq!(
            store.accept_final_transcript("tenant-a", invalid, 1_001),
            Err(VoiceConversationError::InvalidRequest)
        );
    }

    #[test]
    fn replay_is_idempotent_but_conflicting_sequence_is_rejected() {
        let store = VoiceConversationStore::default();
        let first = store
            .accept_final_transcript("tenant-a", request("tenant-a"), 1_001)
            .unwrap();
        let replay = store
            .accept_final_transcript("tenant-a", request("tenant-a"), 1_001)
            .unwrap();
        assert_eq!(replay, first);

        let mut conflicting = request("tenant-a");
        conflicting.transcript = "different transcript".to_owned();
        assert_eq!(
            store.accept_final_transcript("tenant-a", conflicting, 1_001),
            Err(VoiceConversationError::Duplicate)
        );
    }
}
