use crate::voice_conversation::VoiceConversationRecord;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

const MAX_TOOL_INPUT_BYTES: usize = 16_384;
const MAX_PROPOSAL_LIFETIME_SECONDS: u64 = 300;
pub const VOICE_TOOL_PROPOSAL_CONTRACT_VERSION: &str = "voice-tool-proposal-v1";

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "name",
    content = "arguments",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum VoiceToolRequest {
    ExecuteWorkflow {
        workflow_id: String,
        input: serde_json::Value,
    },
}

impl VoiceToolRequest {
    pub fn name(&self) -> &'static str {
        match self {
            Self::ExecuteWorkflow { .. } => "execute_workflow",
        }
    }

    pub fn validate(&self) -> Result<(), VoiceToolProposalError> {
        match self {
            Self::ExecuteWorkflow { workflow_id, input } => {
                validate_identifier(workflow_id)?;
                if !input.is_object()
                    || serde_json::to_vec(input)
                        .map_err(|_| VoiceToolProposalError::InvalidRequest)?
                        .len()
                        > MAX_TOOL_INPUT_BYTES
                {
                    return Err(VoiceToolProposalError::InvalidRequest);
                }
                Ok(())
            }
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateVoiceToolProposalRequest {
    pub contract_version: String,
    pub tenant_id: String,
    pub conversation_id: String,
    pub proposal_key: String,
    pub tool: VoiceToolRequest,
    #[serde(default = "default_proposal_lifetime_seconds")]
    pub expires_in_seconds: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VoiceToolProposalStatus {
    PendingConfirmation,
    Dispatching,
    Confirmed,
    Rejected,
    Expired,
}

#[derive(Clone, PartialEq, Serialize)]
pub struct VoiceToolProposalRecord {
    pub contract_version: String,
    pub proposal_id: String,
    pub tenant_id: String,
    pub actor_id: String,
    pub conversation_id: String,
    pub session_id: String,
    pub sequence: u32,
    pub policy_version: String,
    pub proposal_key: String,
    pub tool: VoiceToolRequest,
    pub status: VoiceToolProposalStatus,
    pub created_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceToolProposalError {
    InvalidRequest,
    ConflictingReplay,
    NotFound,
    InvalidState,
    StoreUnavailable,
}

#[derive(Clone, Default)]
pub struct VoiceToolProposalStore {
    inner: Arc<Mutex<VoiceToolProposalState>>,
}

#[derive(Default)]
struct VoiceToolProposalState {
    proposals: HashMap<String, VoiceToolProposalRecord>,
    replay_index: HashMap<(String, String, String), (String, String)>,
}

impl VoiceToolProposalStore {
    pub fn create(
        &self,
        tenant_id: &str,
        actor_id: &str,
        conversation: &VoiceConversationRecord,
        request: CreateVoiceToolProposalRequest,
        now_unix_ms: u64,
    ) -> Result<VoiceToolProposalRecord, VoiceToolProposalError> {
        validate_identifier(tenant_id)?;
        validate_actor(actor_id)?;
        validate_identifier(&request.proposal_key)?;
        request.tool.validate()?;
        if request.contract_version != VOICE_TOOL_PROPOSAL_CONTRACT_VERSION
            || conversation.tenant_id != tenant_id
            || conversation.conversation_id != request.conversation_id
            || request.expires_in_seconds == 0
            || request.expires_in_seconds > MAX_PROPOSAL_LIFETIME_SECONDS
        {
            return Err(VoiceToolProposalError::InvalidRequest);
        }
        let expires_at_unix_ms = now_unix_ms
            .checked_add(request.expires_in_seconds.saturating_mul(1_000))
            .ok_or(VoiceToolProposalError::InvalidRequest)?;
        let fingerprint = proposal_fingerprint(
            tenant_id,
            actor_id,
            conversation,
            &request.proposal_key,
            &request.tool,
            request.expires_in_seconds,
        )?;
        let replay_key = (
            tenant_id.to_owned(),
            conversation.conversation_id.clone(),
            request.proposal_key.clone(),
        );
        let mut state = self
            .inner
            .lock()
            .map_err(|_| VoiceToolProposalError::StoreUnavailable)?;
        if let Some((proposal_id, stored_fingerprint)) = state.replay_index.get(&replay_key) {
            if stored_fingerprint == &fingerprint {
                return state
                    .proposals
                    .get(proposal_id)
                    .cloned()
                    .ok_or(VoiceToolProposalError::StoreUnavailable);
            }
            return Err(VoiceToolProposalError::ConflictingReplay);
        }
        let record = VoiceToolProposalRecord {
            contract_version: VOICE_TOOL_PROPOSAL_CONTRACT_VERSION.to_owned(),
            proposal_id: format!("voice-tool-proposal-{}", Uuid::new_v4()),
            tenant_id: tenant_id.to_owned(),
            actor_id: actor_id.to_owned(),
            conversation_id: conversation.conversation_id.clone(),
            session_id: conversation.session_id.clone(),
            sequence: conversation.sequence,
            policy_version: conversation.policy_version.clone(),
            proposal_key: request.proposal_key,
            tool: request.tool,
            status: VoiceToolProposalStatus::PendingConfirmation,
            created_at_unix_ms: now_unix_ms,
            expires_at_unix_ms,
            execution_id: None,
        };
        state
            .replay_index
            .insert(replay_key, (record.proposal_id.clone(), fingerprint));
        state
            .proposals
            .insert(record.proposal_id.clone(), record.clone());
        Ok(record)
    }

    pub fn get(
        &self,
        tenant_id: &str,
        proposal_id: &str,
        now_unix_ms: u64,
    ) -> Result<VoiceToolProposalRecord, VoiceToolProposalError> {
        let mut state = self
            .inner
            .lock()
            .map_err(|_| VoiceToolProposalError::StoreUnavailable)?;
        let proposal = tenant_proposal_mut(&mut state, tenant_id, proposal_id)?;
        expire(proposal, now_unix_ms);
        Ok(proposal.clone())
    }

    pub fn claim_confirmation(
        &self,
        tenant_id: &str,
        proposal_id: &str,
        now_unix_ms: u64,
    ) -> Result<(VoiceToolProposalRecord, bool), VoiceToolProposalError> {
        let mut state = self
            .inner
            .lock()
            .map_err(|_| VoiceToolProposalError::StoreUnavailable)?;
        let proposal = tenant_proposal_mut(&mut state, tenant_id, proposal_id)?;
        expire(proposal, now_unix_ms);
        match proposal.status {
            VoiceToolProposalStatus::PendingConfirmation => {
                proposal.status = VoiceToolProposalStatus::Dispatching;
                Ok((proposal.clone(), true))
            }
            VoiceToolProposalStatus::Confirmed => Ok((proposal.clone(), false)),
            VoiceToolProposalStatus::Dispatching
            | VoiceToolProposalStatus::Rejected
            | VoiceToolProposalStatus::Expired => Err(VoiceToolProposalError::InvalidState),
        }
    }

    pub fn finalize_confirmation(
        &self,
        tenant_id: &str,
        proposal_id: &str,
        execution_id: &str,
    ) -> Result<VoiceToolProposalRecord, VoiceToolProposalError> {
        validate_identifier(execution_id)?;
        let mut state = self
            .inner
            .lock()
            .map_err(|_| VoiceToolProposalError::StoreUnavailable)?;
        let proposal = tenant_proposal_mut(&mut state, tenant_id, proposal_id)?;
        if proposal.status != VoiceToolProposalStatus::Dispatching {
            return Err(VoiceToolProposalError::InvalidState);
        }
        proposal.status = VoiceToolProposalStatus::Confirmed;
        proposal.execution_id = Some(execution_id.to_owned());
        Ok(proposal.clone())
    }

    pub fn release_confirmation(
        &self,
        tenant_id: &str,
        proposal_id: &str,
    ) -> Result<(), VoiceToolProposalError> {
        let mut state = self
            .inner
            .lock()
            .map_err(|_| VoiceToolProposalError::StoreUnavailable)?;
        let proposal = tenant_proposal_mut(&mut state, tenant_id, proposal_id)?;
        if proposal.status == VoiceToolProposalStatus::Dispatching {
            proposal.status = VoiceToolProposalStatus::PendingConfirmation;
        }
        Ok(())
    }

    pub fn reject(
        &self,
        tenant_id: &str,
        proposal_id: &str,
        now_unix_ms: u64,
    ) -> Result<VoiceToolProposalRecord, VoiceToolProposalError> {
        let mut state = self
            .inner
            .lock()
            .map_err(|_| VoiceToolProposalError::StoreUnavailable)?;
        let proposal = tenant_proposal_mut(&mut state, tenant_id, proposal_id)?;
        expire(proposal, now_unix_ms);
        match proposal.status {
            VoiceToolProposalStatus::PendingConfirmation => {
                proposal.status = VoiceToolProposalStatus::Rejected;
                Ok(proposal.clone())
            }
            VoiceToolProposalStatus::Rejected => Ok(proposal.clone()),
            _ => Err(VoiceToolProposalError::InvalidState),
        }
    }
}

fn tenant_proposal_mut<'a>(
    state: &'a mut VoiceToolProposalState,
    tenant_id: &str,
    proposal_id: &str,
) -> Result<&'a mut VoiceToolProposalRecord, VoiceToolProposalError> {
    state
        .proposals
        .get_mut(proposal_id)
        .filter(|proposal| proposal.tenant_id == tenant_id)
        .ok_or(VoiceToolProposalError::NotFound)
}

fn expire(proposal: &mut VoiceToolProposalRecord, now_unix_ms: u64) {
    if proposal.expires_at_unix_ms <= now_unix_ms
        && matches!(
            proposal.status,
            VoiceToolProposalStatus::PendingConfirmation | VoiceToolProposalStatus::Dispatching
        )
    {
        proposal.status = VoiceToolProposalStatus::Expired;
    }
}

fn proposal_fingerprint(
    tenant_id: &str,
    actor_id: &str,
    conversation: &VoiceConversationRecord,
    proposal_key: &str,
    tool: &VoiceToolRequest,
    expires_in_seconds: u64,
) -> Result<String, VoiceToolProposalError> {
    let tool = serde_json::to_vec(tool).map_err(|_| VoiceToolProposalError::InvalidRequest)?;
    let mut digest = Sha256::new();
    for value in [
        tenant_id.as_bytes(),
        actor_id.as_bytes(),
        conversation.conversation_id.as_bytes(),
        proposal_key.as_bytes(),
        tool.as_slice(),
    ] {
        digest.update((value.len() as u64).to_be_bytes());
        digest.update(value);
    }
    digest.update(expires_in_seconds.to_be_bytes());
    Ok(hex::encode(digest.finalize()))
}

fn validate_identifier(value: &str) -> Result<(), VoiceToolProposalError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(VoiceToolProposalError::InvalidRequest);
    }
    Ok(())
}

fn validate_actor(value: &str) -> Result<(), VoiceToolProposalError> {
    if value.trim().is_empty()
        || value.len() > 128
        || value
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        return Err(VoiceToolProposalError::InvalidRequest);
    }
    Ok(())
}

fn default_proposal_lifetime_seconds() -> u64 {
    120
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conversation() -> VoiceConversationRecord {
        VoiceConversationRecord {
            conversation_id: "conversation-1".to_owned(),
            tenant_id: "tenant-a".to_owned(),
            session_id: "session-1".to_owned(),
            sequence: 1,
            occurred_at_unix_ms: 900,
            accepted_at_unix_ms: 1_000,
            policy_version: "voice-privacy-v1".to_owned(),
            transcript_retained: false,
            redacted_transcript: None,
            expires_at_unix_ms: 10_000,
        }
    }

    fn request(workflow_id: &str) -> CreateVoiceToolProposalRequest {
        CreateVoiceToolProposalRequest {
            contract_version: VOICE_TOOL_PROPOSAL_CONTRACT_VERSION.to_owned(),
            tenant_id: "tenant-a".to_owned(),
            conversation_id: "conversation-1".to_owned(),
            proposal_key: "proposal-turn-1".to_owned(),
            tool: VoiceToolRequest::ExecuteWorkflow {
                workflow_id: workflow_id.to_owned(),
                input: serde_json::json!({"symbol": "TEST"}),
            },
            expires_in_seconds: 120,
        }
    }

    #[test]
    fn creation_is_idempotent_and_has_no_execution_authority() {
        let store = VoiceToolProposalStore::default();
        let first = store
            .create(
                "tenant-a",
                "actor-1",
                &conversation(),
                request("workflow-1"),
                1_000,
            )
            .unwrap();
        let replay = store
            .create(
                "tenant-a",
                "actor-1",
                &conversation(),
                request("workflow-1"),
                1_000,
            )
            .unwrap();
        assert_eq!(first.proposal_id, replay.proposal_id);
        assert_eq!(first.status, VoiceToolProposalStatus::PendingConfirmation);
        assert!(first.execution_id.is_none());
        assert_eq!(
            store
                .create(
                    "tenant-a",
                    "actor-1",
                    &conversation(),
                    request("workflow-2"),
                    1_000
                )
                .err(),
            Some(VoiceToolProposalError::ConflictingReplay)
        );
    }

    #[test]
    fn confirmation_is_single_claim_tenant_scoped_and_expiring() {
        let store = VoiceToolProposalStore::default();
        let proposal = store
            .create(
                "tenant-a",
                "actor-1",
                &conversation(),
                request("workflow-1"),
                1_000,
            )
            .unwrap();
        assert_eq!(
            store.get("tenant-b", &proposal.proposal_id, 1_001).err(),
            Some(VoiceToolProposalError::NotFound)
        );
        let (_, claimed) = store
            .claim_confirmation("tenant-a", &proposal.proposal_id, 1_001)
            .unwrap();
        assert!(claimed);
        assert_eq!(
            store
                .claim_confirmation("tenant-a", &proposal.proposal_id, 1_002)
                .err(),
            Some(VoiceToolProposalError::InvalidState)
        );
        store
            .release_confirmation("tenant-a", &proposal.proposal_id)
            .unwrap();
        assert_eq!(
            store
                .get("tenant-a", &proposal.proposal_id, 121_000)
                .unwrap()
                .status,
            VoiceToolProposalStatus::Expired
        );
    }

    #[test]
    fn unknown_tools_and_desktop_authority_fields_are_rejected() {
        for payload in [
            serde_json::json!({
                "contract_version": "voice-tool-proposal-v1",
                "tenant_id": "tenant-a",
                "conversation_id": "conversation-1",
                "proposal_key": "proposal-turn-1",
                "tool": {"name": "launch_application", "arguments": {}}
            }),
            serde_json::json!({
                "contract_version": "voice-tool-proposal-v1",
                "tenant_id": "tenant-a",
                "conversation_id": "conversation-1",
                "proposal_key": "proposal-turn-1",
                "tool": {
                    "name": "execute_workflow",
                    "arguments": {"workflow_id": "workflow-1", "input": {}}
                },
                "desktop_action": {"kind": "launch_application"}
            }),
            serde_json::json!({
                "contract_version": "voice-tool-proposal-v2",
                "tenant_id": "tenant-a",
                "conversation_id": "conversation-1",
                "proposal_key": "proposal-turn-1",
                "tool": {
                    "name": "execute_workflow",
                    "arguments": {"workflow_id": "workflow-1", "input": {}}
                }
            }),
        ] {
            match serde_json::from_value::<CreateVoiceToolProposalRequest>(payload) {
                Ok(request) => assert_eq!(
                    VoiceToolProposalStore::default()
                        .create("tenant-a", "actor-1", &conversation(), request, 1_000)
                        .err(),
                    Some(VoiceToolProposalError::InvalidRequest)
                ),
                Err(_) => continue,
            }
        }
    }
}
