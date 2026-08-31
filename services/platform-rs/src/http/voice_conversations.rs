use super::*;
use crate::voice_conversation::{
    FinalVoiceTranscriptRequest, VoiceConversationError, VoicePrivacyPolicy,
};
use crate::voice_tool_proposal::{
    CreateVoiceToolProposalRequest, VoiceToolProposalError, VoiceToolProposalRecord,
    VoiceToolRequest,
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VoiceConversationQuery {
    tenant_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VoicePolicyRequest {
    tenant_id: String,
    policy: VoicePrivacyPolicy,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VoiceToolProposalQuery {
    tenant_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum VoiceToolProposalDecision {
    Confirm,
    Reject,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VoiceToolProposalDecisionRequest {
    tenant_id: String,
    decision: VoiceToolProposalDecision,
}

async fn accept_final_transcript(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(request): Json<FinalVoiceTranscriptRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    require_write(&claims)?;
    let tenant_id = effective_tenant_id(&claims, &request.tenant_id);
    let record = state
        .voice_conversation_store
        .accept_final_transcript(&tenant_id, request, now_unix_ms())
        .await
        .map_err(map_voice_error)?;
    Ok((
        StatusCode::ACCEPTED,
        Json(serde_json::to_value(record).unwrap_or_default()),
    ))
}

async fn get_conversation(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(conversation_id): Path<String>,
    Query(query): Query<VoiceConversationQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    let record = state
        .voice_conversation_store
        .get(&tenant_id, &conversation_id, now_unix_ms())
        .await
        .map_err(map_voice_error)?;
    Ok(Json(serde_json::to_value(record).unwrap_or_default()))
}

async fn delete_conversation(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(conversation_id): Path<String>,
    Query(query): Query<VoiceConversationQuery>,
) -> Result<StatusCode, ApiError> {
    require_admin(&claims)?;
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    state
        .voice_conversation_store
        .delete(&tenant_id, &conversation_id)
        .await
        .map_err(map_voice_error)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn set_voice_policy(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(request): Json<VoicePolicyRequest>,
) -> Result<Json<VoicePrivacyPolicy>, ApiError> {
    require_admin(&claims)?;
    let tenant_id = effective_tenant_id(&claims, &request.tenant_id);
    state
        .voice_conversation_store
        .set_policy(&tenant_id, request.policy)
        .await
        .map(Json)
        .map_err(map_voice_error)
}

async fn create_tool_proposal(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Json(request): Json<CreateVoiceToolProposalRequest>,
) -> Result<(StatusCode, Json<VoiceToolProposalRecord>), ApiError> {
    require_write(&claims)?;
    let tenant_id = effective_tenant_id(&claims, &request.tenant_id);
    let claims = claims.ok_or_else(|| ApiError::forbidden("Authentication required"))?;
    let actor_id = claims
        .user_id
        .clone()
        .filter(|value| !value.is_empty())
        .unwrap_or(claims.sub);
    let now = now_unix_ms();
    let conversation = state
        .voice_conversation_store
        .get(&tenant_id, &request.conversation_id, now)
        .await
        .map_err(map_voice_error)?;
    match &request.tool {
        VoiceToolRequest::ExecuteWorkflow { workflow_id, .. } => {
            let workflow = state
                .workflow_service
                .get_workflow(&tenant_id, workflow_id)
                .await
                .map_err(|_| ApiError::not_found("workflow"))?;
            if workflow.latest_version_id.is_none() {
                return Err(ApiError::bad_request("workflow has no published version"));
            }
        }
    }
    let proposal = state
        .voice_tool_proposal_store
        .create(&tenant_id, &actor_id, &conversation, request, now)
        .map_err(map_tool_proposal_error)?;
    state.audit_store.record(
        &tenant_id,
        "voice.tool_proposal.created",
        "voice_tool_proposal",
        &proposal.proposal_id,
        Some(serde_json::json!({
            "conversation_id": proposal.conversation_id,
            "tool": proposal.tool.name(),
            "expires_at_unix_ms": proposal.expires_at_unix_ms
        })),
    );
    Ok((StatusCode::CREATED, Json(proposal)))
}

async fn get_tool_proposal(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(proposal_id): Path<String>,
    Query(query): Query<VoiceToolProposalQuery>,
) -> Result<Json<VoiceToolProposalRecord>, ApiError> {
    require_write(&claims)?;
    let tenant_id = effective_tenant_id(&claims, &query.tenant_id);
    state
        .voice_tool_proposal_store
        .get(&tenant_id, &proposal_id, now_unix_ms())
        .map(Json)
        .map_err(map_tool_proposal_error)
}

async fn decide_tool_proposal(
    State(state): State<AppState>,
    Extension(claims): Extension<Option<Claims>>,
    Path(proposal_id): Path<String>,
    Json(request): Json<VoiceToolProposalDecisionRequest>,
) -> Result<Json<VoiceToolProposalRecord>, ApiError> {
    require_admin(&claims)?;
    let tenant_id = effective_tenant_id(&claims, &request.tenant_id);
    match request.decision {
        VoiceToolProposalDecision::Reject => {
            let proposal = state
                .voice_tool_proposal_store
                .reject(&tenant_id, &proposal_id, now_unix_ms())
                .map_err(map_tool_proposal_error)?;
            state.audit_store.record(
                &tenant_id,
                "voice.tool_proposal.rejected",
                "voice_tool_proposal",
                &proposal.proposal_id,
                Some(serde_json::json!({"tool": proposal.tool.name()})),
            );
            Ok(Json(proposal))
        }
        VoiceToolProposalDecision::Confirm => {
            let (proposal, claimed) = state
                .voice_tool_proposal_store
                .claim_confirmation(&tenant_id, &proposal_id, now_unix_ms())
                .map_err(map_tool_proposal_error)?;
            if !claimed {
                return Ok(Json(proposal));
            }
            let execution = match &proposal.tool {
                VoiceToolRequest::ExecuteWorkflow { workflow_id, input } => {
                    super::system::execute_workflow_tool(
                        &state,
                        &tenant_id,
                        workflow_id,
                        input,
                        "voice",
                    )
                    .await
                }
            };
            let execution = match execution {
                Ok(execution) => execution,
                Err(error) => {
                    let _ = state
                        .voice_tool_proposal_store
                        .release_confirmation(&tenant_id, &proposal_id);
                    return Err(error);
                }
            };
            let confirmed = state
                .voice_tool_proposal_store
                .finalize_confirmation(&tenant_id, &proposal_id, &execution.id)
                .map_err(map_tool_proposal_error)?;
            state.audit_store.record(
                &tenant_id,
                "voice.tool_proposal.confirmed",
                "voice_tool_proposal",
                &confirmed.proposal_id,
                Some(serde_json::json!({
                    "tool": confirmed.tool.name(),
                    "execution_id": execution.id
                })),
            );
            Ok(Json(confirmed))
        }
    }
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn map_voice_error(error: VoiceConversationError) -> ApiError {
    match error {
        VoiceConversationError::InvalidPolicy | VoiceConversationError::InvalidRequest => {
            ApiError::bad_request("invalid voice conversation request")
        }
        VoiceConversationError::Duplicate => ApiError {
            status: StatusCode::CONFLICT,
            message: "voice transcript already accepted".to_owned(),
        },
        VoiceConversationError::NotFound => ApiError::not_found("voice conversation"),
        VoiceConversationError::StoreUnavailable => ApiError::internal("voice_conversation_store"),
    }
}

fn map_tool_proposal_error(error: VoiceToolProposalError) -> ApiError {
    match error {
        VoiceToolProposalError::InvalidRequest => {
            ApiError::bad_request("invalid voice Tool proposal")
        }
        VoiceToolProposalError::ConflictingReplay => ApiError {
            status: StatusCode::CONFLICT,
            message: "voice Tool proposal key was reused with different input".to_owned(),
        },
        VoiceToolProposalError::NotFound => ApiError::not_found("voice Tool proposal"),
        VoiceToolProposalError::InvalidState => ApiError {
            status: StatusCode::CONFLICT,
            message: "voice Tool proposal cannot transition from its current state".to_owned(),
        },
        VoiceToolProposalError::StoreUnavailable => ApiError::internal("voice_tool_proposal_store"),
    }
}

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/voice/conversations/final-transcripts",
            post(accept_final_transcript),
        )
        .route(
            "/v1/voice/conversations/:conversation_id",
            get(get_conversation).delete(delete_conversation),
        )
        .route("/v1/voice/privacy-policy", put(set_voice_policy))
        .route("/v1/voice/tool-proposals", post(create_tool_proposal))
        .route(
            "/v1/voice/tool-proposals/:proposal_id",
            get(get_tool_proposal),
        )
        .route(
            "/v1/voice/tool-proposals/:proposal_id/decision",
            post(decide_tool_proposal),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use tower::ServiceExt;

    fn token(tenant_id: &str, role: crate::auth::Role) -> String {
        sign_token(&Claims {
            sub: "voice-user".to_owned(),
            tenant_id: tenant_id.to_owned(),
            workspace_id: "workspace-1".to_owned(),
            project_id: "project-1".to_owned(),
            exp: u64::MAX,
            role,
            ..Claims::default()
        })
        .unwrap()
    }

    #[tokio::test]
    async fn jwt_tenant_wins_and_default_response_contains_no_transcript() {
        let app = router();
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/voice/conversations/final-transcripts")
                    .header("content-type", "application/json")
                    .header(
                        "authorization",
                        format!("Bearer {}", token("tenant-a", crate::auth::Role::Editor)),
                    )
                    .body(Body::from(
                        serde_json::json!({
                            "tenant_id": "tenant-b",
                            "session_id": "voice-session-1",
                            "sequence": 1,
                            "occurred_at_unix_ms": 1_000,
                            "transcript": "private final transcript"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let record: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(record["tenant_id"], "tenant-a");
        assert_eq!(record["transcript_retained"], false);
        assert!(record.get("redacted_transcript").is_none());
        assert!(!String::from_utf8_lossy(&bytes).contains("private final transcript"));
    }

    #[tokio::test]
    async fn authority_fields_are_rejected_by_the_http_boundary() {
        let app = router();
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/voice/conversations/final-transcripts")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "tenant_id": "tenant-a",
                            "session_id": "voice-session-1",
                            "sequence": 1,
                            "occurred_at_unix_ms": 1_000,
                            "transcript": "open settings",
                            "desktop_action": {"type": "launch_application"}
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn tool_proposal_requires_confirmation_before_workflow_execution() {
        let app = router();
        let authorization = format!("Bearer {}", token("tenant-1", crate::auth::Role::Admin));
        let transcript_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/voice/conversations/final-transcripts")
                    .header("content-type", "application/json")
                    .header("authorization", &authorization)
                    .body(Body::from(
                        serde_json::json!({
                            "tenant_id": "tenant-1",
                            "session_id": "voice-tool-session-1",
                            "sequence": 1,
                            "occurred_at_unix_ms": 1_000,
                            "transcript": "run the published workflow"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(transcript_response.status(), StatusCode::ACCEPTED);
        let transcript: serde_json::Value = serde_json::from_slice(
            &to_bytes(transcript_response.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        let conversation_id = transcript["conversation_id"].as_str().unwrap();

        let proposal_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/voice/tool-proposals")
                    .header("content-type", "application/json")
                    .header("authorization", &authorization)
                    .body(Body::from(
                        serde_json::json!({
                            "contract_version": "voice-tool-proposal-v1",
                            "tenant_id": "tenant-1",
                            "conversation_id": conversation_id,
                            "proposal_key": "voice-tool-turn-1",
                            "tool": {
                                "name": "execute_workflow",
                                "arguments": {
                                    "workflow_id": "workflow-1",
                                    "input": {}
                                }
                            }
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(proposal_response.status(), StatusCode::CREATED);
        let proposal: serde_json::Value = serde_json::from_slice(
            &to_bytes(proposal_response.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(proposal["status"], "pending_confirmation");
        assert!(proposal.get("execution_id").is_none());
        let proposal_id = proposal["proposal_id"].as_str().unwrap();

        let confirmation = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/voice/tool-proposals/{proposal_id}/decision"))
                    .header("content-type", "application/json")
                    .header("authorization", authorization)
                    .body(Body::from(
                        serde_json::json!({
                            "tenant_id": "tenant-1",
                            "decision": "confirm"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(confirmation.status(), StatusCode::OK);
        let confirmed: serde_json::Value = serde_json::from_slice(
            &to_bytes(confirmation.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(confirmed["status"], "confirmed");
        assert!(confirmed["execution_id"].as_str().is_some());
    }
}
