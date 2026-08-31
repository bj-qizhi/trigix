use serde::{Deserialize, Serialize};
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
    sequence_index: HashMap<(String, String, u32), String>,
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
        {
            return Err(VoiceConversationError::InvalidRequest);
        }

        let mut state = self
            .inner
            .lock()
            .map_err(|_| VoiceConversationError::StoreUnavailable)?;
        let key = (
            tenant_id.to_owned(),
            request.session_id.clone(),
            request.sequence,
        );
        if state.sequence_index.contains_key(&key) {
            return Err(VoiceConversationError::Duplicate);
        }
        let policy = state.policies.get(tenant_id).cloned().unwrap_or_default();
        policy.validate()?;
        let redacted_transcript = policy
            .retain_redacted_transcripts
            .then(|| redact_transcript(&request.transcript));
        let retention_days = if policy.retain_redacted_transcripts {
            policy
                .metadata_retention_days
                .min(policy.transcript_retention_days)
        } else {
            policy.metadata_retention_days
        };
        let expires_at_unix_ms = now_unix_ms
            .checked_add(u64::from(retention_days).saturating_mul(DAY_MS))
            .ok_or(VoiceConversationError::InvalidRequest)?;
        let record = VoiceConversationRecord {
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
        };
        state
            .sequence_index
            .insert(key, record.conversation_id.clone());
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
}
